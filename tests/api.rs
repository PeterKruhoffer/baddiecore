use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use baddiecore::{AppState, Bootstrap, Component, Content, Field, FieldKind, Page, router};
use http_body_util::BodyExt;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct TestApp {
    _dir: TempDir,
    path: std::path::PathBuf,
    app: Router,
    cookie: String,
}

async fn setup() -> TestApp {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cms.db");
    let app = router(
        AppState::open(&path, "secret".into(), false).unwrap(),
        dir.path(),
    );
    let response = call(
        &app,
        "POST",
        "/api/login",
        None,
        Some(&json!({"password":"secret"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    TestApp {
        _dir: dir,
        path,
        app,
        cookie,
    }
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<&impl Serialize>,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "example.test");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    let has_body = body.is_some();
    let body = body
        .map(|v| Body::from(serde_json::to_vec(v).unwrap()))
        .unwrap_or_else(Body::empty);
    if has_body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    app.clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap()
}

async fn read<T: DeserializeOwned>(response: axum::response::Response) -> T {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn bootstrap(app: &TestApp) -> Bootstrap {
    read(
        call(
            &app.app,
            "GET",
            "/api/admin/bootstrap",
            Some(&app.cookie),
            None::<&Value>,
        )
        .await,
    )
    .await
}

#[tokio::test]
async fn admin_api_rejects_unauthorized_requests() {
    let app = setup().await;
    let response = call(
        &app.app,
        "GET",
        "/api/admin/bootstrap",
        None,
        None::<&Value>,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = call(
        &app.app,
        "POST",
        "/api/admin/pages",
        Some(&app.cookie),
        Some(&json!({"title":"X","slug":"/x","template_id":"homepage"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn rejects_invalid_blocks_urls_and_stale_revisions() {
    let app = setup().await;
    let mut page = bootstrap(&app)
        .await
        .pages
        .into_iter()
        .find(|p| p.id == "home")
        .unwrap();
    page.blocks[0].fields.remove("title");
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/pages/home",
        Some(&app.cookie),
        Some(&page),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    page.blocks[0].fields.insert("title".into(), "Hello".into());
    page.blocks[0]
        .fields
        .insert("button_url".into(), "//evil.example/path".into());
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/pages/home",
        Some(&app.cookie),
        Some(&page),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    page.blocks[0]
        .fields
        .insert("button_url".into(), "https://example.com/path".into());
    let saved: Page = read(
        call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&page),
        )
        .await,
    )
    .await;
    assert_eq!(saved.revision, 2);
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/pages/home",
        Some(&app.cookie),
        Some(&page),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn published_snapshot_isolated_from_drafts_and_schema_changes() {
    let app = setup().await;
    let mut page = bootstrap(&app)
        .await
        .pages
        .into_iter()
        .find(|p| p.id == "home")
        .unwrap();
    let published: Page = read(
        call(
            &app.app,
            "POST",
            "/api/admin/pages/home/publish",
            Some(&app.cookie),
            Some(&json!({"revision":1})),
        )
        .await,
    )
    .await;
    assert_eq!(published.published_revision, Some(1));

    page.title = "Draft title".into();
    let saved: Page = read(
        call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&page),
        )
        .await,
    )
    .await;
    assert_eq!(saved.revision, 2);
    let public: Content = read(
        call(
            &app.app,
            "GET",
            "/api/content?slug=%2F",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(public.page.title, "Home");

    let mut hero = bootstrap(&app)
        .await
        .components
        .into_iter()
        .find(|c| c.id == "hero")
        .unwrap();
    hero.name = "Published hero changed".into();
    let _: Component = read(
        call(
            &app.app,
            "PUT",
            "/api/admin/components/hero",
            Some(&app.cookie),
            Some(&hero),
        )
        .await,
    )
    .await;
    let public: Content = read(
        call(
            &app.app,
            "GET",
            "/api/content?slug=%2F",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(
        public
            .components
            .iter()
            .find(|c| c.id == "hero")
            .unwrap()
            .name,
        "Hero"
    );
}

#[tokio::test]
async fn rejects_schema_changes_that_break_drafts() {
    let app = setup().await;
    let mut hero = bootstrap(&app)
        .await
        .components
        .into_iter()
        .find(|c| c.id == "hero")
        .unwrap();
    hero.fields.push(Field {
        name: "new_required".into(),
        label: "New required".into(),
        kind: FieldKind::Text,
        required: true,
    });
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/components/hero",
        Some(&app.cookie),
        Some(&hero),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let mut template = bootstrap(&app)
        .await
        .templates
        .into_iter()
        .find(|t| t.id == "homepage")
        .unwrap();
    template.regions[0].max_components = 1;
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/templates/homepage",
        Some(&app.cookie),
        Some(&template),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn data_and_published_content_persist_after_reopen() {
    let app = setup().await;
    let response = call(
        &app.app,
        "POST",
        "/api/admin/pages",
        Some(&app.cookie),
        Some(&json!({"title":"About","slug":"/about","template_id":"homepage"})),
    )
    .await;
    let page: Page = read(response).await;
    let _: Page = read(
        call(
            &app.app,
            "POST",
            &format!("/api/admin/pages/{}/publish", page.id),
            Some(&app.cookie),
            Some(&json!({"revision":1})),
        )
        .await,
    )
    .await;

    let reopened = router(
        AppState::open(&app.path, "new-secret".into(), false).unwrap(),
        app._dir.path(),
    );
    let public: Content = read(
        call(
            &reopened,
            "GET",
            "/api/content?slug=%2Fabout",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(public.page.title, "About");
    let login = call(
        &reopened,
        "POST",
        "/api/login",
        None,
        Some(&json!({"password":"new-secret"})),
    )
    .await;
    assert_eq!(login.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn slug_and_origin_safety_are_enforced() {
    let app = setup().await;
    for slug in [
        "relative",
        "//host/path",
        "/a/../secret",
        "/x?query",
        "/admin",
        "/API/x",
        "/health/check",
        "/assets/x",
        "/two//parts",
        "/trailing/",
        "/white space",
        "/%2e%2e/secret",
        "/nonascii-é",
    ] {
        let response = call(
            &app.app,
            "POST",
            "/api/admin/pages",
            Some(&app.cookie),
            Some(&json!({"title":"Unsafe","slug":slug,"template_id":"homepage"})),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "accepted {slug}"
        );
    }
    let request = Request::builder()
        .method("POST")
        .uri("/api/admin/pages")
        .header(header::HOST, "example.test")
        .header(header::ORIGIN, "https://evil.test")
        .header(header::COOKIE, &app.cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"title":"X","slug":"/x","template_id":"homepage"}"#,
        ))
        .unwrap();
    let response = app.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn url_safety_is_enforced() {
    let app = setup().await;
    let mut page = bootstrap(&app)
        .await
        .pages
        .into_iter()
        .find(|p| p.id == "home")
        .unwrap();
    for url in [
        "//evil.example/x",
        "https:///missing-host",
        "ftp://example.test/x",
        "https://example.test\\evil",
        "https://example.test/a b",
        "https://example.test/\nnext",
    ] {
        page.blocks[0]
            .fields
            .insert("button_url".into(), url.into());
        let response = call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&page),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "accepted {url:?}"
        );
    }
    for url in [
        "/local/path?x=1",
        "http://example.test",
        "https://example.test/path",
    ] {
        page.blocks[0]
            .fields
            .insert("button_url".into(), url.into());
        let response = call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&page),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "rejected {url:?}");
        page = read(response).await;
    }
}
