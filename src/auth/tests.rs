use crate::auth::{Auth, AuthProvider, hash_password_now, validate_password};
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use mysql::{Opts, Pool};
use tower::ServiceExt;

// Origin checks run before any database access, so this pool never connects.
fn offline_auth() -> Auth {
    let pool =
        Pool::new(Opts::from_url("mysql://127.0.0.1/test?pool_min=0&pool_max=1").unwrap()).unwrap();
    Auth::new(pool, false).unwrap()
}

#[test]
fn passwords_need_a_minimum_length_and_hash_with_random_salts() {
    for short in ["", "1234567", "ñññññññ"] {
        assert!(validate_password(short).is_err());
        assert!(hash_password_now(short).is_err());
    }
    assert!(validate_password(&"x".repeat(1025)).is_err());
    let first = hash_password_now("long enough").unwrap();
    let second = hash_password_now("long enough").unwrap();
    assert!(first.starts_with("$argon2id$"));
    assert_ne!(first, second);
}

#[tokio::test]
async fn auth_routes_reject_cross_origin_mutations() {
    let auth = offline_auth();
    for uri in ["/api/login", "/api/logout", "/api/account/password"] {
        let response = auth
            .routes()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::HOST, "cms.example")
                    .header(header::ORIGIN, "https://evil.example")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"admin","password":"long enough","current_password":"long enough"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
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
