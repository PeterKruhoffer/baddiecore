//! Authentication owns login routes and editor authorization, not CMS content.
//! Custom implementations supply `AuthProvider` to `AppState::open_with_auth`.
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{ApiError, Result};
mod workos;
pub use workos::WorkOs;

/// Returning an Editor grants full CMS access. Authentication alone is not enough.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    pub id: String,
}

pub type Authorization<'a> =
    Pin<Box<dyn Future<Output = std::result::Result<Editor, StatusCode>> + Send + 'a>>;

pub trait AuthProvider: Send + Sync {
    /// Supply /api/auth/config, /api/login and /api/logout, plus any callbacks.
    fn routes(&self) -> Router;
    fn authorize<'a>(&'a self, headers: &'a HeaderMap) -> Authorization<'a>;
}

const SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const LOGIN_LIFETIME: Duration = Duration::from_secs(10 * 60);
const MAX_SESSIONS: usize = 128;

#[derive(Clone)]
pub struct Auth {
    method: Arc<Method>,
    sessions: Arc<Mutex<Store<Arc<tokio::sync::Mutex<Session>>>>>,
    pending: Arc<Mutex<Store<String>>>,
    secure_cookie: bool,
}

enum Method {
    Password(String),
    WorkOs(Box<WorkOs>),
}

struct Session {
    editor: Editor,
    workos: Option<workos::Session>,
}

struct Store<T> {
    entries: HashMap<String, (Instant, T)>,
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<T> Store<T> {
    fn prune(&mut self, now: Instant) {
        self.entries.retain(|_, (expires, _)| *expires > now);
    }
    fn insert(&mut self, key: String, value: T, now: Instant, lifetime: Duration) {
        self.prune(now);
        if self.entries.len() >= MAX_SESSIONS
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (expires, _))| *expires)
                .map(|(key, _)| key.clone())
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, (now + lifetime, value));
    }
    fn remove(&mut self, key: &str) -> Option<T> {
        self.prune(Instant::now());
        self.entries.remove(key).map(|(_, value)| value)
    }
}

impl Auth {
    pub fn password(password: String, secure_cookie: bool) -> std::result::Result<Self, String> {
        if password.trim().is_empty() {
            return Err("BADDIE_ADMIN_PASSWORD must not be empty".into());
        }
        Ok(Self::new(Method::Password(password), secure_cookie))
    }

    pub fn workos(config: WorkOs, secure_cookie: bool) -> std::result::Result<Self, String> {
        if config.redirect_uri.scheme() == "https" && !secure_cookie {
            return Err("WorkOS HTTPS requires BADDIE_SECURE_COOKIE=true".into());
        }
        Ok(Self::new(Method::WorkOs(Box::new(config)), secure_cookie))
    }

    fn new(method: Method, secure_cookie: bool) -> Self {
        Self {
            method: Arc::new(method),
            sessions: Default::default(),
            pending: Default::default(),
            secure_cookie,
        }
    }

    pub fn from_env(secure_cookie: bool) -> std::result::Result<Self, String> {
        Self::from_config(|key| std::env::var(key).ok(), secure_cookie)
    }

    fn from_config(
        get: impl Fn(&str) -> Option<String>,
        secure_cookie: bool,
    ) -> std::result::Result<Self, String> {
        let required = |key: &str| {
            get(key)
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| format!("{key} is required"))
        };
        match get("BADDIE_AUTH").as_deref().unwrap_or("password") {
            "password" => Self::password(required("BADDIE_ADMIN_PASSWORD")?, secure_cookie),
            "workos" => Self::workos(
                WorkOs::new(
                    required("WORKOS_API_KEY")?,
                    required("WORKOS_CLIENT_ID")?,
                    required("WORKOS_REDIRECT_URI")?,
                    required("WORKOS_ORGANIZATION_ID")?,
                )?,
                secure_cookie,
            ),
            _ => Err("BADDIE_AUTH must be password or workos".into()),
        }
    }

    fn session(&self, headers: &HeaderMap) -> Option<Arc<tokio::sync::Mutex<Session>>> {
        let token = cookie_value(headers, "baddie_session")?;
        let mut sessions = self.sessions.lock().unwrap();
        sessions.prune(Instant::now());
        sessions.entries.get(&token).map(|(_, s)| s.clone())
    }

    fn issue(&self, session: Session, headers: &HeaderMap, response: &mut Response) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(old) = cookie_value(headers, "baddie_session") {
            sessions.remove(&old);
        }
        let token = Uuid::new_v4().to_string();
        sessions.insert(
            token.clone(),
            Arc::new(tokio::sync::Mutex::new(session)),
            Instant::now(),
            SESSION_LIFETIME,
        );
        set_cookie(response, self.cookie("baddie_session", token));
    }

    fn cookie(&self, name: &'static str, value: String) -> Cookie<'static> {
        Cookie::build((name, value))
            .http_only(true)
            .secure(self.secure_cookie)
            .same_site(if name == "baddie_login" {
                SameSite::Lax
            } else {
                SameSite::Strict
            })
            .path("/")
            .build()
    }
}

impl AuthProvider for Auth {
    fn routes(&self) -> Router {
        Router::new()
            .route("/api/auth/config", get(config))
            .route("/api/login", get(start_login).post(password_login))
            .route("/api/auth/callback", get(callback))
            .route("/api/logout", post(logout))
            .layer(middleware::map_response(|mut response: Response| async {
                response
                    .headers_mut()
                    .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
                response
                    .headers_mut()
                    .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
                response
            }))
            .with_state(self.clone())
    }

    fn authorize<'a>(&'a self, headers: &'a HeaderMap) -> Authorization<'a> {
        Box::pin(async move {
            let session = self.session(headers).ok_or(StatusCode::UNAUTHORIZED)?;
            let mut session = session.lock().await;
            if let (Method::WorkOs(config), Some(tokens)) =
                (self.method.as_ref(), session.workos.as_mut())
                && let Err(error) = config.validate(tokens).await
            {
                if let Some(token) = cookie_value(headers, "baddie_session") {
                    self.sessions.lock().unwrap().remove(&token);
                }
                return Err(error.0);
            }
            Ok(session.editor.clone())
        })
    }
}

async fn config(State(auth): State<Auth>) -> Json<serde_json::Value> {
    Json(match auth.method.as_ref() {
        Method::Password(_) => json!({"method": "password"}),
        Method::WorkOs(_) => json!({"method": "redirect", "label": "Sign in with WorkOS"}),
    })
}

#[derive(Deserialize)]
struct Login {
    password: String,
}

async fn password_login(
    State(auth): State<Auth>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response> {
    check_origin(&headers)?;
    let Method::Password(password) = auth.method.as_ref() else {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "password login is disabled".into(),
        ));
    };
    if input
        .password
        .as_bytes()
        .ct_eq(password.as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid password".into(),
        ));
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    auth.issue(
        Session {
            editor: Editor {
                id: "shared-admin".into(),
            },
            workos: None,
        },
        &headers,
        &mut response,
    );
    Ok(response)
}

async fn start_login(State(auth): State<Auth>, headers: HeaderMap) -> Result<Response> {
    check_origin(&headers)?;
    let Method::WorkOs(config) = auth.method.as_ref() else {
        return Err(ApiError::not_found());
    };
    let state = Uuid::new_v4().to_string();
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut pending = auth.pending.lock().unwrap();
    if let Some(old) = cookie_value(&headers, "baddie_login") {
        pending.remove(&old);
    }
    pending.insert(state.clone(), verifier, Instant::now(), LOGIN_LIFETIME);
    let url = config.authorization_url(&state, &challenge);
    let mut response = Redirect::to(url.as_str()).into_response();
    let mut cookie = auth.cookie("baddie_login", state);
    cookie.set_max_age(Some(LOGIN_LIFETIME.try_into().unwrap()));
    set_cookie(&mut response, cookie);
    Ok(response)
}

#[derive(Deserialize)]
struct Callback {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(auth): State<Auth>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Response {
    let result = complete_login(&auth, &headers, query).await;
    let mut response = match result {
        Ok(session) => {
            let mut response = Redirect::to("/admin").into_response();
            auth.issue(session, &headers, &mut response);
            response
        }
        Err(_) => Redirect::to("/admin?auth_error=1").into_response(),
    };
    let mut cookie = auth.cookie("baddie_login", String::new());
    cookie.make_removal();
    set_cookie(&mut response, cookie);
    response
}

async fn complete_login(auth: &Auth, headers: &HeaderMap, query: Callback) -> Result<Session> {
    let Method::WorkOs(config) = auth.method.as_ref() else {
        return Err(ApiError::not_found());
    };
    let invalid = || ApiError(StatusCode::UNAUTHORIZED, "sign-in failed".into());
    let state = cookie_value(headers, "baddie_login").ok_or_else(invalid)?;
    if query.state.as_deref() != Some(&state) {
        return Err(invalid());
    }
    let verifier = auth
        .pending
        .lock()
        .unwrap()
        .remove(&state)
        .ok_or_else(invalid)?;
    if query.error.is_some() {
        return Err(invalid());
    }
    let code = query.code.filter(|v| !v.is_empty()).ok_or_else(invalid)?;
    let tokens = config.exchange(&code, &verifier).await?;
    Ok(Session {
        editor: Editor {
            id: tokens.user_id.clone(),
        },
        workos: Some(tokens),
    })
}

async fn logout(State(auth): State<Auth>, headers: HeaderMap) -> Result<Response> {
    check_origin(&headers)?;
    let session = cookie_value(&headers, "baddie_session")
        .and_then(|token| auth.sessions.lock().unwrap().remove(&token));
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let (Method::WorkOs(config), Some(session)) = (auth.method.as_ref(), session) {
        let session = session.lock().await;
        if let Some(tokens) = &session.workos {
            response = Json(json!({"redirect_url": config.logout_url(&tokens.session_id)}))
                .into_response();
        }
    }
    let mut cookie = auth.cookie("baddie_session", String::new());
    cookie.make_removal();
    set_cookie(&mut response, cookie);
    Ok(response)
}

fn set_cookie(response: &mut Response, cookie: Cookie<'_>) {
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.to_string().parse().unwrap());
}

fn check_origin(headers: &HeaderMap) -> Result<()> {
    if same_origin(headers) {
        Ok(())
    } else {
        Err(ApiError(
            StatusCode::FORBIDDEN,
            "origin does not match host".into(),
        ))
    }
}

pub(crate) fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .is_some_and(|v| v == host)
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_owned())
        })
}
