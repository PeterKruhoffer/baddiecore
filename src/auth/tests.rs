use super::*;
use crate::auth::{Auth, AuthProvider, MAX_SESSIONS, SESSION_LIFETIME, Store};
use axum::{
    Json, Router,
    body::Body,
    http::{HeaderMap, Request, header},
    response::Response,
    routing::post,
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

struct Mock {
    auth: Auth,
    requests: Arc<Mutex<Vec<Value>>>,
    response: Arc<Mutex<(StatusCode, Value)>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn fixture(org: &str, verified: bool, expires: u64) -> Value {
    let claims =
        json!({"sid": "session_test", "sub": "user_editor", "org_id": org, "exp": expires});
    json!({"user": {"id": "user_editor", "email_verified": verified}, "organization_id": org,
        "access_token": format!("header.{}.signature", URL_SAFE_NO_PAD.encode(claims.to_string())),
        "refresh_token": "refresh_initial"})
}

fn expiry() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 1200
}

fn config(uri: &str) -> WorkOs {
    WorkOs::new(
        "test-key".into(),
        "client_test".into(),
        uri.into(),
        "org_allowed".into(),
    )
    .unwrap()
}

async fn mock() -> Mock {
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let response = Arc::new(Mutex::new((
        StatusCode::OK,
        fixture("org_allowed", true, expiry()),
    )));
    let app = Router::new().route(
        "/user_management/authenticate",
        post({
            let requests = requests.clone();
            let response = response.clone();
            move |Json(body): Json<Value>| {
                let requests = requests.clone();
                let response = response.clone();
                async move {
                    requests.lock().unwrap().push(body);
                    let (status, body) = response.lock().unwrap().clone();
                    (status, Json(body))
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut workos = config("https://cms.example/api/auth/callback");
    workos.api_url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Mock {
        auth: Auth::workos(workos, true).unwrap(),
        requests,
        response,
        task,
    }
}

async fn call(auth: &Auth, method: &str, uri: &str, cookie: Option<&str>, body: Value) -> Response {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    auth.routes()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

fn cookie(response: &Response, name: &str) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .find(|v| v.starts_with(&format!("{name}=")))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn headers(cookie: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie.parse().unwrap());
    headers
}

async fn begin(auth: &Auth) -> (String, HashMap<String, String>) {
    let response = call(auth, "GET", "/api/login", None, Value::Null).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let url = Url::parse(response.headers()[header::LOCATION].to_str().unwrap()).unwrap();
    let cookie = cookie(&response, "baddie_login");
    let raw = response.headers()[header::SET_COOKIE].to_str().unwrap();
    for attr in ["HttpOnly", "Secure", "SameSite=Lax", "Max-Age=600"] {
        assert!(raw.contains(attr));
    }
    (cookie, url.query_pairs().into_owned().collect())
}

async fn finish(auth: &Auth) -> (Response, HashMap<String, String>, String) {
    let (cookie, params) = begin(auth).await;
    let uri = format!(
        "/api/auth/callback?state={}&code=valid-code",
        params["state"]
    );
    (
        call(auth, "GET", &uri, Some(&cookie), Value::Null).await,
        params,
        cookie,
    )
}

#[test]
fn configuration_fails_closed_and_password_needs_no_workos() {
    assert!(Auth::from_config(|_| None, false).is_err());
    assert!(
        Auth::from_config(
            |k| (k == "BADDIE_ADMIN_PASSWORD").then(|| "secret".into()),
            false
        )
        .is_ok()
    );
    for method in ["workos", "none", "typo"] {
        assert!(
            Auth::from_config(
                |k| match k {
                    "BADDIE_AUTH" => Some(method.into()),
                    "BADDIE_ADMIN_PASSWORD" => Some("secret".into()),
                    _ => None,
                },
                false
            )
            .is_err()
        );
    }
    assert!(Auth::password(" ".into(), false).is_err());
    assert!(Auth::workos(config("https://cms.example/api/auth/callback"), false).is_err());
    for uri in [
        "http://cms.example/api/auth/callback",
        "https://cms.example/wrong",
        "https://cms.example/api/auth/callback?x=1",
        "https://cms.example/api/auth/callback#x",
        "https://user@cms.example/api/auth/callback",
    ] {
        assert!(WorkOs::new("key".into(), "client".into(), uri.into(), "org".into()).is_err());
    }
    assert!(Auth::workos(config("http://localhost:3000/api/auth/callback"), false).is_ok());
}

#[test]
fn sessions_expire_and_are_bounded() {
    let now = Instant::now();
    let mut store = Store::default();
    store
        .insert("expired".into(), (), now, SESSION_LIFETIME, MAX_SESSIONS)
        .unwrap();
    store.prune(now + SESSION_LIFETIME - Duration::from_secs(1));
    assert!(store.entries.contains_key("expired"));
    store.prune(now + SESSION_LIFETIME);
    assert!(store.entries.is_empty());
    for index in 0..MAX_SESSIONS {
        store
            .insert(
                index.to_string(),
                (),
                now + Duration::from_secs(index as u64),
                SESSION_LIFETIME,
                MAX_SESSIONS,
            )
            .unwrap();
    }
    assert!(
        store
            .insert("overflow".into(), (), now, SESSION_LIFETIME, MAX_SESSIONS)
            .is_err()
    );
    assert_eq!(store.entries.len(), MAX_SESSIONS);
    assert!(store.entries.contains_key("0"));
    store
        .insert(
            "replacement".into(),
            (),
            now + SESSION_LIFETIME,
            SESSION_LIFETIME,
            MAX_SESSIONS,
        )
        .unwrap();
    assert!(!store.entries.contains_key("0"));
    assert!(store.entries.contains_key("replacement"));
}

#[tokio::test]
async fn password_login_rotation_logout_and_expiry() {
    let auth = Auth::password("secret".into(), true).unwrap();
    assert_eq!(
        body(call(&auth, "GET", "/api/auth/config", None, Value::Null).await).await,
        json!({"method":"password"})
    );
    assert_eq!(
        call(
            &auth,
            "POST",
            "/api/login",
            None,
            json!({"password":"wrong"})
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let first = call(
        &auth,
        "POST",
        "/api/login",
        None,
        json!({"password":"secret"}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::NO_CONTENT);
    let old = cookie(&first, "baddie_session");
    assert_eq!(
        auth.authorize(&headers(&old)).await.unwrap().id,
        "shared-admin"
    );
    let second = call(
        &auth,
        "POST",
        "/api/login",
        Some(&old),
        json!({"password":"secret"}),
    )
    .await;
    let current = cookie(&second, "baddie_session");
    assert_ne!(old, current);
    assert_eq!(
        auth.authorize(&headers(&old)).await.unwrap_err(),
        StatusCode::UNAUTHORIZED
    );
    let response = call(&auth, "POST", "/api/logout", Some(&current), Value::Null).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert!(auth.authorize(&headers(&current)).await.is_err());
    let third = call(
        &auth,
        "POST",
        "/api/login",
        None,
        json!({"password":"secret"}),
    )
    .await;
    auth.sessions
        .lock()
        .unwrap()
        .prune(Instant::now() + SESSION_LIFETIME);
    assert!(
        auth.authorize(&headers(&cookie(&third, "baddie_session")))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn workos_login_pkce_replay_and_logout() {
    let mock = mock().await;
    assert_eq!(
        call(
            &mock.auth,
            "POST",
            "/api/login",
            None,
            json!({"password":"secret"})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let (response, params, login_cookie) = finish(&mock.auth).await;
    assert_eq!(response.headers()[header::LOCATION], "/admin");
    assert_eq!(params["provider"], "authkit");
    assert_eq!(params["organization_id"], "org_allowed");
    assert_eq!(
        params["redirect_uri"],
        "https://cms.example/api/auth/callback"
    );
    assert_eq!(params["code_challenge_method"], "S256");
    let requests = mock.requests.lock().unwrap().clone();
    assert_eq!(requests[0]["grant_type"], "authorization_code");
    assert_eq!(requests[0]["client_id"], "client_test");
    assert_eq!(requests[0]["client_secret"], "test-key");
    assert_eq!(requests[0]["code"], "valid-code");
    assert_eq!(
        params["code_challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(
            requests[0]["code_verifier"].as_str().unwrap().as_bytes()
        ))
    );
    let session_cookie = cookie(&response, "baddie_session");
    let h = headers(&session_cookie);
    assert_eq!(mock.auth.authorize(&h).await.unwrap().id, "user_editor");
    let replay = call(
        &mock.auth,
        "GET",
        &format!(
            "/api/auth/callback?state={}&code=valid-code",
            params["state"]
        ),
        Some(&login_cookie),
        Value::Null,
    )
    .await;
    assert_eq!(replay.headers()[header::LOCATION], "/admin?auth_error=1");
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
    let response = call(
        &mock.auth,
        "POST",
        "/api/logout",
        Some(&session_cookie),
        Value::Null,
    )
    .await;
    assert!(mock.auth.authorize(&h).await.is_err());
    let result = body(response).await;
    let url = Url::parse(result["redirect_url"].as_str().unwrap()).unwrap();
    assert_eq!(url.path(), "/user_management/sessions/logout");
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["session_id"], "session_test");
    assert_eq!(query["return_to"], "https://cms.example/admin");
}

#[tokio::test]
async fn callbacks_require_matching_live_browser_state() {
    let mock = mock().await;
    for case in [
        "no-cookie",
        "wrong-state",
        "expired",
        "cancelled",
        "no-code",
    ] {
        let (cookie, params) = begin(&mock.auth).await;
        if case == "expired" {
            mock.auth
                .pending
                .lock()
                .unwrap()
                .prune(Instant::now() + Duration::from_secs(601));
        }
        let state = if case == "wrong-state" {
            "wrong"
        } else {
            &params["state"]
        };
        let suffix = match case {
            "cancelled" => "error=access_denied",
            "no-code" => "",
            _ => "code=valid",
        };
        let response = call(
            &mock.auth,
            "GET",
            &format!("/api/auth/callback?state={state}&{suffix}"),
            if case == "no-cookie" {
                None
            } else {
                Some(&cookie)
            },
            Value::Null,
        )
        .await;
        assert_eq!(
            response.headers()[header::LOCATION],
            "/admin?auth_error=1",
            "{case}"
        );
        assert!(mock.auth.sessions.lock().unwrap().entries.is_empty());
    }
    assert!(mock.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unauthorized_or_failed_exchanges_never_create_sessions() {
    let mock = mock().await;
    for value in [
        fixture("org_other", true, expiry()),
        fixture("org_allowed", false, expiry()),
        fixture("org_allowed", true, 0),
        json!({"error":"secret provider detail"}),
    ] {
        *mock.response.lock().unwrap() = (StatusCode::OK, value);
        let (response, _, _) = finish(&mock.auth).await;
        assert_eq!(response.headers()[header::LOCATION], "/admin?auth_error=1");
        assert!(mock.auth.sessions.lock().unwrap().entries.is_empty());
    }
    for status in [StatusCode::BAD_REQUEST, StatusCode::INTERNAL_SERVER_ERROR] {
        *mock.response.lock().unwrap() = (status, json!({"error":"do not expose"}));
        assert_eq!(
            finish(&mock.auth).await.0.headers()[header::LOCATION],
            "/admin?auth_error=1"
        );
    }
}

#[tokio::test]
async fn refresh_is_serialized_rotates_tokens_and_denies_revocation() {
    let mock = mock().await;
    let (response, _, _) = finish(&mock.auth).await;
    let h = headers(&cookie(&response, "baddie_session"));
    let session = mock.auth.session(&h).unwrap();
    session.lock().await.workos.as_mut().unwrap().valid_until = Instant::now();
    mock.response.lock().unwrap().1["refresh_token"] = json!("refresh_rotated");
    let (a, b) = tokio::join!(mock.auth.authorize(&h), mock.auth.authorize(&h));
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(mock.requests.lock().unwrap().len(), 2);
    assert_eq!(
        mock.requests.lock().unwrap()[1]["refresh_token"],
        "refresh_initial"
    );
    session.lock().await.workos.as_mut().unwrap().valid_until = Instant::now();
    *mock.response.lock().unwrap() = (StatusCode::UNAUTHORIZED, json!({"error":"revoked"}));
    assert_eq!(
        mock.auth.authorize(&h).await.unwrap_err(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        mock.requests.lock().unwrap()[2]["refresh_token"],
        "refresh_rotated"
    );
    assert_eq!(
        mock.requests.lock().unwrap()[2]["organization_id"],
        "org_allowed"
    );
    assert!(mock.auth.session(&h).is_none());
}

#[tokio::test]
async fn auth_routes_reject_cross_origin_mutations() {
    let mock = mock().await;
    for auth in [&mock.auth, &Auth::password("secret".into(), false).unwrap()] {
        for (method, uri) in [
            ("POST", "/api/login"),
            ("POST", "/api/logout"),
            ("GET", "/api/login"),
        ] {
            let response = auth
                .routes()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header(header::HOST, "cms.example")
                        .header(header::ORIGIN, "https://evil.example")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"password":"secret"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
}

#[test]
fn origins_compare_scheme_host_and_effective_port_and_reject_malformed_headers() {
    use crate::auth::OriginPolicy;
    for secure in [false, true] {
        let policy = OriginPolicy::new(None, secure).unwrap();
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "cms.example".parse().unwrap());
        assert!(policy.check(&h).is_ok());
        for (origin, allowed) in [
            ("http://cms.example", !secure),
            ("https://cms.example", secure),
            ("https://CMS.EXAMPLE:443", secure),
            ("http://cms.example:80", !secure),
            ("https://cms.example:8443", false),
            ("https://evil.example", false),
            ("https://cms.example/path", false),
            ("https://cms.example/path/..", false),
            ("https://user@cms.example", false),
            ("https://cms.example?query", false),
            ("null", false),
        ] {
            h.insert(header::ORIGIN, origin.parse().unwrap());
            assert_eq!(
                policy.check(&h).is_ok(),
                allowed,
                "{origin}, secure={secure}"
            );
        }
        h.insert(
            header::ORIGIN,
            axum::http::HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        assert!(policy.check(&h).is_err());
    }
    let policy = OriginPolicy::new(Some("https://cms.example:8443"), true).unwrap();
    let mut h = HeaderMap::new();
    h.insert(header::HOST, "internal-proxy:3000".parse().unwrap());
    h.insert(header::ORIGIN, "https://cms.example:8443".parse().unwrap());
    assert!(policy.check(&h).is_ok());
    h.insert(header::ORIGIN, "https://cms.example".parse().unwrap());
    assert!(policy.check(&h).is_err());
    assert!(OriginPolicy::new(Some("http://cms.example"), true).is_err());
    assert!(OriginPolicy::new(Some("https://cms.example/path"), true).is_err());
}

#[tokio::test]
async fn full_pending_store_preserves_a_login_that_can_still_complete() {
    use crate::auth::{LOGIN_LIFETIME, MAX_PENDING_LOGINS};
    let mock = mock().await;
    let (login_cookie, params) = begin(&mock.auth).await;
    for index in 1..MAX_PENDING_LOGINS {
        mock.auth
            .pending
            .lock()
            .unwrap()
            .insert(
                format!("other-{index}"),
                "verifier".into(),
                Instant::now(),
                LOGIN_LIFETIME,
                MAX_PENDING_LOGINS,
            )
            .unwrap();
    }
    let rejected = call(&mock.auth, "GET", "/api/login", None, Value::Null).await;
    assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(!rejected.headers().contains_key(header::SET_COOKIE));
    let response = call(
        &mock.auth,
        "GET",
        &format!("/api/auth/callback?state={}&code=valid", params["state"]),
        Some(&login_cookie),
        Value::Null,
    )
    .await;
    assert_eq!(response.headers()[header::LOCATION], "/admin");
    assert!(
        mock.auth
            .authorize(&headers(&cookie(&response, "baddie_session")))
            .await
            .is_ok()
    );
    assert_eq!(
        call(&mock.auth, "GET", "/api/login", None, Value::Null)
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn authorization_waiting_on_a_session_cannot_outlive_its_expiry() {
    let auth = Auth::password("secret".into(), false).unwrap();
    let response = call(
        &auth,
        "POST",
        "/api/login",
        None,
        json!({"password":"secret"}),
    )
    .await;
    let h = headers(&cookie(&response, "baddie_session"));
    let session = auth.session(&h).unwrap();
    let guard = session.lock().await;
    let authorization = auth.authorize(&h);
    tokio::pin!(authorization);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut authorization)
            .await
            .is_err()
    );
    auth.sessions
        .lock()
        .unwrap()
        .prune(Instant::now() + SESSION_LIFETIME);
    drop(guard);
    assert_eq!(authorization.await.unwrap_err(), StatusCode::UNAUTHORIZED);
}
