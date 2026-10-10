//! Authentication owns login routes and editor authorization, not CMS content.
//! The built-in provider signs members in with local passwords. Custom
//! implementations supply `AuthProvider` to `AppState::open_with_auth`.
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use mysql::{Pool, PooledConn, prelude::Queryable};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use url::Url;
use uuid::Uuid;

use crate::{ApiError, Result, db_error};

/// Authenticated identity. Local organization membership grants CMS access.
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
    /// CMS mutation origin policy. Custom providers default to the server environment.
    fn origin_policy(&self) -> std::result::Result<OriginPolicy, String> {
        let secure = std::env::var("BADDIE_SECURE_COOKIE")
            .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        OriginPolicy::from_env(secure)
    }
}

const SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_FAILURES: u32 = 5;
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_TRACKED_FAILURES: usize = 10_000;
const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_BYTES: usize = 1024;

// Argon2id uses 19 MiB per hash; bound concurrent hashing memory.
static HASHING: Semaphore = Semaphore::const_new(4);
// Unknown usernames still pay for a verification so timing does not reveal them.
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| hash_now("unknown account").unwrap());

/// Local accounts: Argon2id password hashes and sessions stored in MySQL.
/// Account IDs are organization member IDs.
#[derive(Clone)]
pub struct Auth {
    db: Pool,
    secure_cookie: bool,
    origin: OriginPolicy,
    failures: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
}

impl Auth {
    pub fn new(db: Pool, secure_cookie: bool) -> std::result::Result<Self, String> {
        Ok(Self {
            db,
            secure_cookie,
            origin: OriginPolicy::from_env(secure_cookie)?,
            failures: Default::default(),
        })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut PooledConn) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || f(&mut db.get_conn().map_err(db_error)?))
            .await
            .map_err(|_| {
                ApiError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database task failed".into(),
                )
            })?
    }

    /// Checks a password, counting failures per account to slow guessing.
    async fn check_password(&self, id: &str, password: String) -> Result<()> {
        {
            let mut failures = self.failures.lock().unwrap();
            let now = Instant::now();
            failures.retain(|_, (since, _)| now.duration_since(*since) < FAILURE_WINDOW);
            if failures.len() >= MAX_TRACKED_FAILURES
                || failures.get(id).is_some_and(|(_, n)| *n >= MAX_FAILURES)
            {
                return Err(ApiError(
                    StatusCode::TOO_MANY_REQUESTS,
                    "too many failed sign-ins; try again later".into(),
                ));
            }
        }
        let account = id.to_owned();
        let hash: Option<String> = self
            .blocking(move |db| {
                db.exec_first("SELECT password_hash FROM accounts WHERE id=?", (account,))
                    .map_err(db_error)
            })
            .await?;
        let known = hash.is_some();
        let valid = verify(hash.unwrap_or_else(|| DUMMY_HASH.clone()), password).await?;
        let mut failures = self.failures.lock().unwrap();
        if known && valid {
            failures.remove(id);
            return Ok(());
        }
        let entry = failures.entry(id.to_owned()).or_insert((Instant::now(), 0));
        entry.1 += 1;
        Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid username or password".into(),
        ))
    }

    async fn issue(&self, id: String, headers: &HeaderMap, response: &mut Response) -> Result<()> {
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let old = cookie_value(headers, "baddie_session").map(|old| digest(&old));
        let hashed = digest(&token);
        self.blocking(move |db| {
            let now = unix_now();
            db.exec_drop("DELETE FROM sessions WHERE expires_at<=?", (now,))
                .map_err(db_error)?;
            if let Some(old) = old {
                db.exec_drop("DELETE FROM sessions WHERE token_hash=?", (old,))
                    .map_err(db_error)?;
            }
            db.exec_drop(
                "INSERT INTO sessions(token_hash,account_id,expires_at) VALUES(?,?,?)",
                (hashed, id, now + SESSION_LIFETIME.as_secs() as i64),
            )
            .map_err(db_error)
        })
        .await?;
        set_cookie(response, self.cookie(token));
        Ok(())
    }

    fn cookie(&self, value: String) -> Cookie<'static> {
        Cookie::build(("baddie_session", value))
            .http_only(true)
            .secure(self.secure_cookie)
            .same_site(SameSite::Strict)
            .path("/")
            .build()
    }
}

impl AuthProvider for Auth {
    fn origin_policy(&self) -> std::result::Result<OriginPolicy, String> {
        Ok(self.origin.clone())
    }

    fn routes(&self) -> Router {
        Router::new()
            .route("/api/auth/config", get(config))
            .route("/api/login", post(login))
            .route("/api/logout", post(logout))
            .route("/api/account/password", post(change_password))
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
            let token = cookie_value(headers, "baddie_session").ok_or(StatusCode::UNAUTHORIZED)?;
            let hashed = digest(&token);
            let id: Option<String> = self
                .blocking(move |db| {
                    db.exec_first(
                        "SELECT account_id FROM sessions WHERE token_hash=? AND expires_at>?",
                        (hashed, unix_now()),
                    )
                    .map_err(db_error)
                })
                .await
                .map_err(|e| e.0)?;
            id.map(|id| Editor { id }).ok_or(StatusCode::UNAUTHORIZED)
        })
    }
}

async fn config() -> Json<serde_json::Value> {
    Json(json!({"method": "password"}))
}

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}

async fn login(
    State(auth): State<Auth>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response> {
    auth.origin.check(&headers)?;
    auth.check_password(&input.username, input.password).await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    auth.issue(input.username, &headers, &mut response).await?;
    Ok(response)
}

#[derive(Deserialize)]
struct PasswordChange {
    current_password: String,
    password: String,
}

/// Members change their own password. This ends their other sessions.
async fn change_password(
    State(auth): State<Auth>,
    headers: HeaderMap,
    Json(input): Json<PasswordChange>,
) -> Result<Response> {
    auth.origin.check(&headers)?;
    let editor = auth
        .authorize(&headers)
        .await
        .map_err(|status| ApiError(status, "authentication required".into()))?;
    let hash = hash_password(input.password).await?;
    auth.check_password(&editor.id, input.current_password)
        .await
        .map_err(|e| match e.0 {
            StatusCode::UNAUTHORIZED => ApiError(
                StatusCode::FORBIDDEN,
                "current password is incorrect".into(),
            ),
            _ => e,
        })?;
    let id = editor.id.clone();
    auth.blocking(move |db| store_password(db, &id, &hash))
        .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    auth.issue(editor.id, &headers, &mut response).await?;
    Ok(response)
}

async fn logout(State(auth): State<Auth>, headers: HeaderMap) -> Result<Response> {
    auth.origin.check(&headers)?;
    if let Some(token) = cookie_value(&headers, "baddie_session") {
        let hashed = digest(&token);
        auth.blocking(move |db| {
            db.exec_drop("DELETE FROM sessions WHERE token_hash=?", (hashed,))
                .map_err(db_error)
        })
        .await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    let mut cookie = auth.cookie(String::new());
    cookie.make_removal();
    set_cookie(&mut response, cookie);
    Ok(response)
}

fn validate_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_CHARS || password.len() > MAX_PASSWORD_BYTES {
        return Err(ApiError::bad(format!(
            "passwords need {MIN_PASSWORD_CHARS} to {MAX_PASSWORD_BYTES} characters"
        )));
    }
    Ok(())
}

/// Validates and hashes a new password on a blocking thread.
pub(crate) async fn hash_password(password: String) -> Result<String> {
    validate_password(&password)?;
    let _permit = HASHING.acquire().await.unwrap();
    tokio::task::spawn_blocking(move || hash_now(&password))
        .await
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "hashing failed".into()))?
}

/// Synchronous variant for startup and the CLI.
pub(crate) fn hash_password_now(password: &str) -> Result<String> {
    validate_password(password)?;
    hash_now(password)
}

fn hash_now(password: &str) -> Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "hashing failed".into()))
}

async fn verify(hash: String, password: String) -> Result<bool> {
    let _permit = HASHING.acquire().await.unwrap();
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "hashing failed".into()))
}

/// Sets an account's password hash and ends all of its sessions.
pub(crate) fn store_password(db: &mut impl Queryable, id: &str, hash: &str) -> Result<()> {
    if id.trim().is_empty() || id.len() > 255 {
        return Err(ApiError::bad("usernames need 1 to 255 bytes"));
    }
    db.exec_drop(
        "INSERT INTO accounts(id,password_hash) VALUES(?,?) ON DUPLICATE KEY UPDATE password_hash=VALUES(password_hash)",
        (id, hash),
    )
    .map_err(db_error)?;
    db.exec_drop("DELETE FROM sessions WHERE account_id=?", (id,))
        .map_err(db_error)
}

/// Deletes an account's password and sessions.
pub(crate) fn remove_account(db: &mut impl Queryable, id: &str) -> Result<()> {
    db.exec_drop("DELETE FROM accounts WHERE id=?", (id,))
        .map_err(db_error)
}

pub(crate) fn has_account(db: &mut impl Queryable, id: &str) -> Result<bool> {
    Ok(db
        .exec_first::<i64, _, _>("SELECT 1 FROM accounts WHERE id=?", (id,))
        .map_err(db_error)?
        .is_some())
}

/// Session tokens are stored only as digests, so a database copy cannot sign in.
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn set_cookie(response: &mut Response, cookie: Cookie<'_>) {
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.to_string().parse().unwrap());
}

/// Trusted browser-facing origin, or preserved Host plus the configured transport.
#[derive(Clone)]
pub struct OriginPolicy {
    external: Option<Url>,
    secure: bool,
}

impl OriginPolicy {
    pub(crate) fn from_env(secure: bool) -> std::result::Result<Self, String> {
        Self::new(
            std::env::var("BADDIE_ORIGIN")
                .ok()
                .filter(|v| !v.is_empty())
                .as_deref(),
            secure,
        )
    }

    pub fn new(external: Option<&str>, secure: bool) -> std::result::Result<Self, String> {
        let external = external
            .map(|value| {
                parse_origin(value)
                    .ok_or("BADDIE_ORIGIN must be an HTTP(S) origin without a path".to_owned())
            })
            .transpose()?;
        if external
            .as_ref()
            .is_some_and(|url| secure && url.scheme() != "https")
        {
            return Err("secure cookies require an HTTPS BADDIE_ORIGIN".into());
        }
        Ok(Self { external, secure })
    }

    pub(crate) fn check(&self, headers: &HeaderMap) -> Result<()> {
        if self.allows(headers) {
            Ok(())
        } else {
            Err(ApiError(
                StatusCode::FORBIDDEN,
                "origin does not match host".into(),
            ))
        }
    }

    fn allows(&self, headers: &HeaderMap) -> bool {
        let Some(origin) = headers.get(header::ORIGIN) else {
            return true;
        };
        let Some(origin) = origin.to_str().ok().and_then(parse_origin) else {
            return false;
        };
        if let Some(expected) = &self.external {
            return origin.origin() == expected.origin();
        }
        let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
            return false;
        };
        let scheme = if self.secure { "https" } else { "http" };
        parse_origin(&format!("{scheme}://{host}"))
            .is_some_and(|expected| origin.origin() == expected.origin())
    }
}

fn parse_origin(value: &str) -> Option<Url> {
    let (_, authority) = value.split_once("://")?;
    if authority
        .strip_suffix('/')
        .unwrap_or(authority)
        .contains('/')
    {
        return None;
    }
    let url = Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\\'))
    .then_some(url)
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

#[cfg(test)]
mod tests;
