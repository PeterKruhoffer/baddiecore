use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use baddiecore::{AppState, Bootstrap, Component, Content, Field, FieldKind, Page, router};
use http_body_util::BodyExt;
use mysql::{Opts, Pool, prelude::Queryable};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct TestApp {
    _dir: TempDir,
    app: Router,
    cookie: String,
    db: TestDatabase,
}

struct TestDatabase {
    admin: Pool,
    name: String,
    url: String,
}

impl TestDatabase {
    fn new() -> Self {
        let base = std::env::var("TEST_DATABASE_URL")
            .expect("Set TEST_DATABASE_URL to a disposable MySQL server URL without a database or query string");
        let opts = Opts::from_url(&base).expect("invalid TEST_DATABASE_URL");
        assert!(opts.get_db_name().is_none() && !base.contains('?'));
        // Tests create several independent app/CLI pools in parallel. Keep idle
        // connections at zero rather than exhausting MySQL's default limit.
        let pool_query = "pool_min=0&pool_max=4";
        let admin = Pool::new(Opts::from_url(&format!("{base}?{pool_query}")).unwrap()).unwrap();
        let name = format!("baddie_test_{}", uuid::Uuid::new_v4().simple());
        admin
            .get_conn()
            .unwrap()
            .query_drop(format!("CREATE DATABASE `{name}`"))
            .unwrap();
        let url = format!("{}/{name}?{pool_query}", base.trim_end_matches('/'));
        Self { admin, name, url }
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        self.admin
            .get_conn()
            .unwrap()
            .query_drop(format!("DROP DATABASE `{}`", self.name))
            .unwrap();
    }
}

#[tokio::test]
async fn spa_routes_serve_html_with_success_status() {
    let db = TestDatabase::new();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.html"), "<html>CMS</html>").unwrap();
    let app = router(
        AppState::open(&db.url, Some(PASSWORD.into()), false).unwrap(),
        dir.path(),
    );
    for path in ["/admin", "/about/team"] {
        let response = call(&app, "GET", path, None, None::<&Value>).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "text/html");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"<html>CMS</html>");
    }
}

const PASSWORD: &str = "test-password";

async fn setup() -> TestApp {
    setup_with_keys(baddiecore::headless::ApiKeys::default()).await
}

async fn setup_with_keys(keys: baddiecore::headless::ApiKeys) -> TestApp {
    let dir = tempfile::tempdir().unwrap();
    let db = TestDatabase::new();
    let app = router(
        AppState::open(&db.url, Some(PASSWORD.into()), false)
            .unwrap()
            .with_headless_keys(keys),
        dir.path(),
    );
    let response = call(
        &app,
        "POST",
        "/api/login",
        None,
        Some(&json!({"username":"admin","password":PASSWORD})),
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
        app,
        cookie,
        db,
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

const CONTENT_KEY: &str = "test-content-key-0123456789abcdef";
const COMPONENT_KEY: &str = "test-component-key-9876543210abcd";

async fn headless_setup() -> TestApp {
    setup_with_keys(
        baddiecore::headless::ApiKeys::new(Some(CONTENT_KEY.into()), Some(COMPONENT_KEY.into()))
            .unwrap(),
    )
    .await
}

async fn key_call(
    app: &Router,
    method: &str,
    uri: &str,
    key: &str,
    body: Option<&Value>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {key}"))
        .header(header::CONTENT_TYPE, "application/json");
    if uri.starts_with("/api/headless/") {
        // Cookie CSRF policy must not block server-to-server key authentication.
        request = request.header(header::ORIGIN, "https://consumer.example");
    }
    app.clone()
        .oneshot(
            request
                .body(
                    body.map(|v| Body::from(serde_json::to_vec(v).unwrap()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap(),
        )
        .await
        .unwrap()
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
    // Sharing a cached definition must not skip validation of later instances.
    let mut repeated = page.blocks[0].clone();
    repeated.id = "second-hero".into();
    repeated.fields.remove("title");
    page.blocks.push(repeated);
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&page)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    page.blocks
        .last_mut()
        .unwrap()
        .fields
        .insert("title".into(), "Different second title".into());
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
    assert_eq!(
        saved.blocks.last().unwrap().fields["title"],
        "Different second title"
    );
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
        richtext: None,
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

    // An administrator can already sign in, so a changed bootstrap password is ignored.
    let reopened = router(
        AppState::open(&app.db.url, Some("changed-password".into()), false).unwrap(),
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
        Some(&json!({"username":"admin","password":"changed-password"})),
    )
    .await;
    assert_eq!(login.status(), StatusCode::UNAUTHORIZED);
    // Sessions live in the database and survive restarts.
    assert_eq!(
        call(
            &reopened,
            "GET",
            "/api/admin/bootstrap",
            Some(&app.cookie),
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::OK
    );
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

#[tokio::test]
async fn mysql_paths_are_case_sensitive_and_bounded() {
    let app = setup().await;
    let longest = format!("/{}", "x".repeat(2047));
    for slug in ["/About", "/about", longest.as_str()] {
        let response = call(
            &app.app,
            "POST",
            "/api/admin/pages",
            Some(&app.cookie),
            Some(&json!({"title":"Æøå 日本語 🦀", "slug":slug, "template_id":"homepage"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let page: Page = read(response).await;
        let response = call(
            &app.app,
            "POST",
            &format!("/api/admin/pages/{}/publish", page.id),
            Some(&app.cookie),
            Some(&json!({"revision":1})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let public: Content = read(
            call(
                &app.app,
                "GET",
                &format!("/api/content?slug={slug}"),
                None,
                None::<&Value>,
            )
            .await,
        )
        .await;
        assert_eq!(public.page.id, page.id);
        assert_eq!(public.page.title, "Æøå 日本語 🦀");
    }
    for (slug, expected) in [
        ("/About".to_owned(), StatusCode::CONFLICT),
        (format!("{longest}x"), StatusCode::BAD_REQUEST),
    ] {
        let response = call(
            &app.app,
            "POST",
            "/api/admin/pages",
            Some(&app.cookie),
            Some(&json!({"title":"Duplicate", "slug":slug, "template_id":"homepage"})),
        )
        .await;
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn publish_conflict_rolls_back_and_delete_cascades() {
    let app = setup().await;
    let mut home = create_test_page(&app, "/original").await;
    let response = call(
        &app.app,
        "POST",
        &format!("/api/admin/pages/{}/publish", home.id),
        Some(&app.cookie),
        Some(&json!({"revision":1})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    home.slug = "/moved".into();
    let response = call(
        &app.app,
        "PUT",
        &format!("/api/admin/pages/{}", home.id),
        Some(&app.cookie),
        Some(&home),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = call(
        &app.app,
        "POST",
        "/api/admin/pages",
        Some(&app.cookie),
        Some(&json!({"title":"Replacement", "slug":"/replacement", "template_id":"homepage"})),
    )
    .await;
    let mut replacement: Page = read(response).await;
    let publish = format!("/api/admin/pages/{}/publish", replacement.id);
    let edit = format!("/api/admin/pages/{}", replacement.id);
    assert_eq!(
        call(
            &app.app,
            "POST",
            &publish,
            Some(&app.cookie),
            Some(&json!({"revision":1}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    replacement.slug = "/original".into();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &edit,
            Some(&app.cookie),
            Some(&replacement)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app.app,
            "POST",
            &publish,
            Some(&app.cookie),
            Some(&json!({"revision":2}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let draft = bootstrap(&app)
        .await
        .pages
        .into_iter()
        .find(|p| p.id == replacement.id)
        .unwrap();
    assert_eq!(draft.published_revision, Some(1));
    for (slug, id) in [
        ("/original", home.id.as_str()),
        ("/replacement", replacement.id.as_str()),
    ] {
        let public: Content = read(
            call(
                &app.app,
                "GET",
                &format!("/api/content?slug={slug}"),
                None,
                None::<&Value>,
            )
            .await,
        )
        .await;
        assert_eq!(public.page.id, id);
    }
    // Moving the old publication releases the slug; the next publish succeeds.
    assert_eq!(
        call(
            &app.app,
            "POST",
            &format!("/api/admin/pages/{}/publish", home.id),
            Some(&app.cookie),
            Some(&json!({"revision":2}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app.app,
            "POST",
            &publish,
            Some(&app.cookie),
            Some(&json!({"revision":2}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/replacement",
            None,
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app.app, "DELETE", &edit, Some(&app.cookie), None::<&Value>)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/original",
            None,
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app.app, "DELETE", &edit, Some(&app.cookie), None::<&Value>)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn concurrent_instances_cannot_overwrite_a_revision() {
    let app = setup().await;
    let other = router(
        AppState::open(&app.db.url, Some(PASSWORD.into()), true).unwrap(),
        app._dir.path(),
    );
    let login = call(
        &other,
        "POST",
        "/api/login",
        None,
        Some(&json!({"username":"admin","password":PASSWORD})),
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.contains("Secure"));
    let other_cookie = cookie.split(';').next().unwrap().to_owned();
    let mut first = bootstrap(&app).await.pages.remove(0);
    first.title = "First edit".into();
    let mut second = first.clone();
    second.title = "Second edit".into();
    let (a, b) = tokio::join!(
        call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&first)
        ),
        call(
            &other,
            "PUT",
            "/api/admin/pages/home",
            Some(&other_cookie),
            Some(&second)
        ),
    );
    let winner = match (a.status(), b.status()) {
        (StatusCode::OK, StatusCode::CONFLICT) => first.title,
        (StatusCode::CONFLICT, StatusCode::OK) => second.title,
        statuses => panic!("unexpected save results: {statuses:?}"),
    };
    let saved = bootstrap(&app).await.pages.remove(0);
    assert_eq!(saved.revision, 2);
    assert_eq!(saved.title, winner);
}

#[test]
fn failed_seed_rolls_back_all_inserts() {
    let db = TestDatabase::new();
    drop(AppState::open(&db.url, Some(PASSWORD.into()), false).unwrap());
    let pool = Pool::new(Opts::from_url(&db.url).unwrap()).unwrap();
    let mut conn = pool.get_conn().unwrap();
    for statement in [
        "DELETE FROM pages",
        "DELETE FROM templates",
        "DELETE FROM components",
        "INSERT INTO templates(id,data) VALUES('homepage','{}')",
    ] {
        conn.query_drop(statement).unwrap();
    }
    assert!(AppState::open(&db.url, Some(PASSWORD.into()), false).is_err());
    assert_eq!(
        conn.query_first::<i64, _>("SELECT COUNT(*) FROM components")
            .unwrap(),
        Some(0)
    );
}

#[tokio::test]
async fn public_reads_and_health_do_not_wait_for_editorial_transactions() {
    let app = setup().await;
    let page = bootstrap(&app).await.pages.remove(0);
    assert_eq!(
        call(
            &app.app,
            "POST",
            &format!("/api/admin/pages/{}/publish", page.id),
            Some(&app.cookie),
            Some(&json!({"revision":page.revision}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let mut conn = pool.get_conn().unwrap();
    let mut tx = conn.start_transaction(mysql::TxOpts::default()).unwrap();
    tx.query_drop("SELECT id FROM cms_lock WHERE id=1 FOR UPDATE")
        .unwrap();
    // An uncommitted publication must not replace the public snapshot.
    tx.query_drop("DELETE FROM snapshots").unwrap();
    for path in ["/health", "/api/content?slug=/"] {
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            call(&app.app, "GET", path, None, None::<&Value>),
        )
        .await
        .expect("public read waited on the CMS lock");
        assert_eq!(response.status(), StatusCode::OK);
        if path.starts_with("/api/content") {
            let content: Content = read(response).await;
            assert_eq!(content.page.id, page.id);
            assert_eq!(content.page.published_revision, Some(page.revision));
        }
    }
    let editorial = call(
        &app.app,
        "GET",
        "/api/admin/bootstrap",
        Some(&app.cookie),
        None::<&Value>,
    );
    tokio::pin!(editorial);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut editorial)
            .await
            .is_err()
    );
    tx.rollback().unwrap();
    assert_eq!(editorial.await.status(), StatusCode::OK);
}

#[tokio::test]
async fn custom_auth_guards_cms_without_affecting_public_content() {
    use baddiecore::auth::{AuthProvider, Authorization, Editor};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    struct CustomAuth {
        allow: AtomicBool,
        calls: AtomicUsize,
    }
    impl AuthProvider for CustomAuth {
        fn routes(&self) -> Router {
            Router::new().route(
                "/api/auth/config",
                axum::routing::get(|| async {
                    axum::Json(json!({"method":"redirect", "label":"Company login"}))
                }),
            )
        }
        fn authorize<'a>(&'a self, _: &'a axum::http::HeaderMap) -> Authorization<'a> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if self.allow.load(Ordering::SeqCst) {
                    Ok(Editor {
                        id: "custom-editor".into(),
                    })
                } else {
                    Err(StatusCode::UNAUTHORIZED)
                }
            })
        }
    }
    let db = TestDatabase::new();
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(CustomAuth {
        allow: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
    });
    let app = router(
        AppState::open_with_auth(&db.url, provider.clone()).unwrap(),
        dir.path(),
    );
    assert_eq!(
        call(&app, "GET", "/api/auth/config", None, None::<&Value>)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", "/api/content?slug=/", None, None::<&Value>)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        call(&app, "GET", "/api/admin/bootstrap", None, None::<&Value>)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    provider.allow.store(true, Ordering::SeqCst);
    assert_eq!(
        call(&app, "GET", "/api/admin/bootstrap", None, None::<&Value>)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/pages")
                .header(header::HOST, "cms.example")
                .header(header::ORIGIN, "https://evil.example")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn aliases_redirect_only_after_publication_and_follow_the_published_path() {
    let app = setup().await;
    let mut page = create_test_page(&app, "/shop/campaign-2026").await;
    page.aliases = vec!["/summer-sale".into(), "/Sale".into()];
    let url = format!("/api/admin/pages/{}", page.id);
    page = read(call(&app.app, "PUT", &url, Some(&app.cookie), Some(&page)).await).await;
    assert_ne!(
        call(&app.app, "GET", "/summer-sale", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
    let publish_url = format!("{url}/publish");
    page = read(
        call(
            &app.app,
            "POST",
            &publish_url,
            Some(&app.cookie),
            Some(&json!({"revision":page.revision})),
        )
        .await,
    )
    .await;
    for method in ["GET", "HEAD"] {
        let response = call(
            &app.app,
            method,
            "/summer-sale?utm_source=email&x=%2F",
            None,
            None::<&Value>,
        )
        .await;
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(
            response.headers()[header::LOCATION],
            "/shop/campaign-2026?utm_source=email&x=%2F"
        );
        assert!(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty()
        );
    }
    for path in [
        "/sale",
        "/summer-sale/child",
        "/shop/campaign-2026",
        "/health",
        "/admin",
    ] {
        assert_ne!(
            call(&app.app, "GET", path, None, None::<&Value>)
                .await
                .status(),
            StatusCode::MOVED_PERMANENTLY
        );
    }
    assert_ne!(
        call(&app.app, "POST", "/summer-sale", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
    page.slug = "/shop/autumn".into();
    page.aliases = vec!["/summer-sale".into(), "/shop/campaign-2026".into()];
    page = read(call(&app.app, "PUT", &url, Some(&app.cookie), Some(&page)).await).await;
    assert_eq!(
        call(&app.app, "GET", "/summer-sale", None, None::<&Value>)
            .await
            .headers()[header::LOCATION],
        "/shop/campaign-2026"
    );
    page = read(
        call(
            &app.app,
            "POST",
            &publish_url,
            Some(&app.cookie),
            Some(&json!({"revision":page.revision})),
        )
        .await,
    )
    .await;
    let reopened = router(
        AppState::open(&app.db.url, Some(PASSWORD.into()), false).unwrap(),
        app._dir.path(),
    );
    for path in ["/summer-sale", "/shop/campaign-2026"] {
        let response = call(&reopened, "GET", path, None, None::<&Value>).await;
        assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
        assert_eq!(response.headers()[header::LOCATION], "/shop/autumn");
    }
    assert_ne!(
        call(&app.app, "GET", "/Sale", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
    page.aliases.clear();
    page = read(call(&app.app, "PUT", &url, Some(&app.cookie), Some(&page)).await).await;
    assert_eq!(
        call(&app.app, "GET", "/summer-sale", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
    page = read(
        call(
            &app.app,
            "POST",
            &publish_url,
            Some(&app.cookie),
            Some(&json!({"revision":page.revision})),
        )
        .await,
    )
    .await;
    assert_ne!(
        call(&app.app, "GET", "/summer-sale", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
    page.aliases.push("/autumn-sale".into());
    page = read(call(&app.app, "PUT", &url, Some(&app.cookie), Some(&page)).await).await;
    assert_eq!(
        call(
            &app.app,
            "POST",
            &publish_url,
            Some(&app.cookie),
            Some(&json!({"revision":page.revision}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(&app.app, "DELETE", &url, Some(&app.cookie), None::<&Value>)
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_ne!(
        call(&app.app, "GET", "/autumn-sale", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
}

#[tokio::test]
async fn aliases_reject_unsafe_paths_collisions_and_imports_atomically() {
    let app = setup().await;
    let mut first = create_test_page(&app, "/first").await;
    let mut second = create_test_page(&app, "/second").await;
    let first_url = format!("/api/admin/pages/{}", first.id);
    let second_url = format!("/api/admin/pages/{}", second.id);
    for aliases in [
        vec!["/first"],
        vec!["/dup", "/dup"],
        vec!["//evil.test"],
        vec!["/Admin/x"],
        vec!["/api/x"],
        vec!["/assets/x"],
        vec!["/health/x"],
        vec!["/bad?x=1"],
        vec!["/bad\r\nLocation: evil"],
        vec!["relative"],
        vec!["/trailing/"],
        vec!["/ümlaut"],
        vec!["/bad//path"],
    ] {
        let mut invalid = first.clone();
        invalid.aliases = aliases.into_iter().map(String::from).collect();
        assert_eq!(
            call(
                &app.app,
                "PUT",
                &first_url,
                Some(&app.cookie),
                Some(&invalid)
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let mut invalid = first.clone();
    invalid.aliases.push(format!("/{}", "a".repeat(2048)));
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &first_url,
            Some(&app.cookie),
            Some(&invalid)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    invalid.aliases = vec!["/second".into()];
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &first_url,
            Some(&app.cookie),
            Some(&invalid)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    first.aliases = vec!["/offer".into()];
    first = read(call(&app.app, "PUT", &first_url, Some(&app.cookie), Some(&first)).await).await;
    second.aliases = vec!["/offer".into()];
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &second_url,
            Some(&app.cookie),
            Some(&second)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    second.aliases.clear();
    second.slug = "/offer".into();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &second_url,
            Some(&app.cookie),
            Some(&second)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.app,
            "POST",
            "/api/admin/pages",
            Some(&app.cookie),
            Some(&json!({"title":"Collision","slug":"/offer","template_id":"homepage"}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.app,
            "POST",
            &format!("{first_url}/publish"),
            Some(&app.cookie),
            Some(&json!({"revision":first.revision}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    first.slug = "/renamed".into();
    first.aliases.clear();
    let _: Page =
        read(call(&app.app, "PUT", &first_url, Some(&app.cookie), Some(&first)).await).await;
    second.slug = "/second".into();
    // Neither a live alias nor a live page path can be claimed by another draft alias.
    for path in ["/offer", "/first"] {
        second.aliases = vec![path.into()];
        assert_eq!(
            call(
                &app.app,
                "PUT",
                &second_url,
                Some(&app.cookie),
                Some(&second)
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
    }
    let package = export_package(&app, "/second").await;
    let entry = format!("pages/{}.yaml", second.id);
    let with_aliases = |aliases: Value| {
        repack(&package, |files| {
            files.get_mut(&entry).unwrap()["data"]["aliases"] = aliases
        })
    };
    let before = serde_json::to_value(bootstrap(&app).await).unwrap();
    let install = |body| raw_call(&app.app, "POST", "/api/admin/package", &app.cookie, body);
    let response = install(with_aliases(json!(["/offer"]))).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    assert_eq!(
        call(&app.app, "GET", "/offer", None, None::<&Value>)
            .await
            .headers()[header::LOCATION],
        "/first"
    );
    let response = install(with_aliases(json!(["/valid-import"]))).await;
    assert_eq!(response.status(), StatusCode::OK);
    let exported = &package_entries(&export_package(&app, "/second").await)[&entry];
    assert_eq!(exported["data"]["aliases"], json!(["/valid-import"]));
    assert_ne!(
        call(&app.app, "GET", "/valid-import", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
}

#[tokio::test]
async fn editors_need_alias_path_grants_and_review_approval_publishes_aliases() {
    let app = setup().await;
    members(&app).await;
    let scoped = member_app(&app);
    let mut page = create_test_page(&app, "/news/story").await;
    let url = format!("/api/admin/pages/{}", page.id);
    page.aliases = vec!["/newspaper/story".into()];
    assert_eq!(
        call(&scoped, "PUT", &url, Some("editor"), Some(&page))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    page.aliases = vec!["/news/offer".into()];
    page = read(call(&scoped, "PUT", &url, Some("editor"), Some(&page)).await).await;
    let review: Value = read(
        call(
            &scoped,
            "POST",
            &format!("{url}/submit"),
            Some("editor"),
            Some(&json!({"revision":page.revision})),
        )
        .await,
    )
    .await;
    assert_ne!(
        call(&app.app, "GET", "/news/offer", None, None::<&Value>)
            .await
            .status(),
        StatusCode::MOVED_PERMANENTLY
    );
    assert_eq!(call(&scoped, "POST", &format!("/api/admin/reviews/{}", page.id), Some("reviewer"), Some(&json!({"revision":page.revision,"submission_id":review["submission_id"],"approve":true,"feedback":""}))).await.status(), StatusCode::OK);
    assert_eq!(
        call(&app.app, "GET", "/news/offer", None, None::<&Value>)
            .await
            .headers()[header::LOCATION],
        "/news/story"
    );
    // An admin-owned alias outside the editor's grants may remain during content edits,
    // but that editor cannot remove or change it.
    page.aliases.push("/outside".into());
    page = read(call(&scoped, "PUT", &url, Some("admin"), Some(&page)).await).await;
    page.title = "Edited copy".into();
    page = read(call(&scoped, "PUT", &url, Some("editor"), Some(&page)).await).await;
    page.aliases.retain(|path| path != "/outside");
    assert_eq!(
        call(&scoped, "PUT", &url, Some("editor"), Some(&page))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

async fn create_test_page(app: &TestApp, slug: &str) -> Page {
    let response = call(
        &app.app,
        "POST",
        "/api/admin/pages",
        Some(&app.cookie),
        Some(&json!({"title": slug, "slug": slug, "template_id": "homepage"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    read(response).await
}

#[tokio::test]
async fn moving_a_branch_updates_only_descendant_drafts_and_invalidates_stale_saves() {
    let app = setup().await;
    let mut parent = create_test_page(&app, "/about").await;
    let child = create_test_page(&app, "/about/team").await;
    let grandchild = create_test_page(&app, "/about/team/history").await;
    let sibling = create_test_page(&app, "/about-us").await;
    let published = call(
        &app.app,
        "POST",
        &format!("/api/admin/pages/{}/publish", child.id),
        Some(&app.cookie),
        Some(&json!({"revision": child.revision})),
    )
    .await;
    assert_eq!(published.status(), StatusCode::OK);
    parent.slug = "/company".into();
    let response = call(
        &app.app,
        "PUT",
        &format!("/api/admin/pages/{}", parent.id),
        Some(&app.cookie),
        Some(&parent),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let pages = bootstrap(&app).await.pages;
    for (id, path, revision) in [
        (&parent.id, "/company", 2),
        (&child.id, "/company/team", 2),
        (&grandchild.id, "/company/team/history", 2),
        (&sibling.id, "/about-us", 1),
    ] {
        let page = pages.iter().find(|p| &p.id == id).unwrap();
        assert_eq!(page.slug, path);
        assert_eq!(page.revision, revision);
    }
    assert_eq!(
        pages
            .iter()
            .find(|p| p.id == child.id)
            .unwrap()
            .published_revision,
        Some(1)
    );
    let live: Content = read(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/about/team",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(live.page.slug, "/about/team");
    assert_eq!(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/company/team",
            None,
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &format!("/api/admin/pages/{}", child.id),
            Some(&app.cookie),
            Some(&child)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.app,
            "DELETE",
            &format!("/api/admin/pages/{}", parent.id),
            Some(&app.cookie),
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.app,
            "DELETE",
            &format!("/api/admin/pages/{}", sibling.id),
            Some(&app.cookie),
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn branch_moves_reject_cycles_collisions_root_moves_and_long_descendants_atomically() {
    let app = setup().await;
    let parent = create_test_page(&app, "/a").await;
    create_test_page(&app, "/a/team").await;
    create_test_page(&app, "/destination/team").await;
    let before = serde_json::to_value(bootstrap(&app).await).unwrap();
    for (path, status) in [
        ("/a/nested".to_string(), StatusCode::BAD_REQUEST),
        ("/destination".to_string(), StatusCode::CONFLICT),
        (format!("/{}", "x".repeat(2045)), StatusCode::BAD_REQUEST),
    ] {
        let mut moved = parent.clone();
        moved.slug = path;
        assert_eq!(
            call(
                &app.app,
                "PUT",
                &format!("/api/admin/pages/{}", parent.id),
                Some(&app.cookie),
                Some(&moved)
            )
            .await
            .status(),
            status
        );
        assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    }
    let mut home = bootstrap(&app)
        .await
        .pages
        .into_iter()
        .find(|p| p.slug == "/")
        .unwrap();
    home.slug = "/moved-home".into();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &format!("/api/admin/pages/{}", home.id),
            Some(&app.cookie),
            Some(&home)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    // Moving to a missing ancestor can overlap old subtree paths without a real collision.
    let mut nested = create_test_page(&app, "/folder/branch").await;
    let nested_child = create_test_page(&app, "/folder/branch/branch").await;
    nested.slug = "/folder".into();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            &format!("/api/admin/pages/{}", nested.id),
            Some(&app.cookie),
            Some(&nested)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        bootstrap(&app)
            .await
            .pages
            .iter()
            .find(|p| p.id == nested_child.id)
            .unwrap()
            .slug,
        "/folder/branch"
    );
}

/// Package entries parsed as YAML, by name.
fn package_entries(package: &[u8]) -> std::collections::BTreeMap<String, Value> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(package)).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().unwrap().into_owned();
            let mut text = String::new();
            std::io::Read::read_to_string(&mut file, &mut text).unwrap();
            (name, serde_yaml_ng::from_str(&text).unwrap())
        })
        .collect()
}

/// Edit a package's entries and zip them again.
fn repack(
    package: &[u8],
    edit: impl FnOnce(&mut std::collections::BTreeMap<String, Value>),
) -> Vec<u8> {
    let mut files = package_entries(package);
    edit(&mut files);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, value) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(
            &mut zip,
            serde_yaml_ng::to_string(&value).unwrap().as_bytes(),
        )
        .unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn edit_yaml(path: &std::path::Path, edit: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_yaml_ng::from_slice(&std::fs::read(path).unwrap()).unwrap();
    edit(&mut value["data"]);
    std::fs::write(path, serde_yaml_ng::to_string(&value).unwrap()).unwrap();
}

#[tokio::test]
async fn cli_pull_push_round_trip_definitions_and_dry_run_rolls_back() {
    use std::process::Command;
    let app = setup().await;
    let dir = tempfile::tempdir().unwrap();
    let run = |command: &str, flags: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_baddiecore"))
            .arg(command)
            .arg(dir.path())
            .args(flags)
            .env("DATABASE_URL", &app.db.url)
            .output()
            .unwrap()
    };
    let ok = |command: &str, flags: &[&str]| {
        let result = run(command, flags);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    ok("pull", &[]);
    assert!(!dir.path().join("pages").exists());
    let template_path = dir.path().join("templates/homepage.yaml");
    let original = std::fs::read(&template_path).unwrap();
    ok("pull", &[]);
    assert_eq!(std::fs::read(&template_path).unwrap(), original);
    edit_yaml(&template_path, |v| {
        v["description"] = json!("Reviewed in git")
    });
    ok("push", &["--dry-run"]);
    assert_ne!(
        bootstrap(&app).await.templates[0].description,
        "Reviewed in git"
    );
    ok("push", &[]);
    assert_eq!(
        bootstrap(&app).await.templates[0].description,
        "Reviewed in git"
    );
    // Content pages never travel through Git; they use packages.
    assert!(!run("pull", &["--pages"]).status.success());
    assert!(!dir.path().join("pages").exists());
}

#[tokio::test]
async fn imports_validate_existing_drafts_and_preserve_live_snapshots() {
    use baddiecore::serialization::{pull, push};
    let app = setup().await;
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        call(
            &app.app,
            "POST",
            "/api/admin/pages/home/publish",
            Some(&app.cookie),
            Some(&json!({"revision": 1}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    pull(&pool, dir.path(), false).unwrap();
    let before = serde_json::to_value(bootstrap(&app).await).unwrap();
    let component_path = dir.path().join("components/hero.yaml");
    let field = |required| json!({"name":"new_field","label":"New field","kind":"text","required":required});
    edit_yaml(&component_path, |v| {
        v["fields"].as_array_mut().unwrap().push(field(true))
    });
    // A required field would break the home draft, so nothing changes.
    assert!(push(&pool, dir.path(), false, false).is_err());
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    edit_yaml(&component_path, |v| {
        *v["fields"].as_array_mut().unwrap().last_mut().unwrap() = field(false)
    });
    push(&pool, dir.path(), false, true).unwrap();
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    push(&pool, dir.path(), false, false).unwrap();
    let data = bootstrap(&app).await;
    let hero = data.components.iter().find(|c| c.id == "hero").unwrap();
    assert!(hero.fields.iter().any(|f| f.name == "new_field"));
    assert_eq!(
        (data.pages[0].revision, data.pages[0].published_revision),
        (1, Some(1))
    );
    let live: Content =
        read(call(&app.app, "GET", "/api/content?slug=/", None, None::<&Value>).await).await;
    assert!(
        !live
            .components
            .iter()
            .find(|c| c.id == "hero")
            .unwrap()
            .fields
            .iter()
            .any(|f| f.name == "new_field")
    );
    // Definitions reach another initialized instance; its pages are untouched.
    let other = setup().await;
    let other_pool = Pool::new(Opts::from_url(&other.db.url).unwrap()).unwrap();
    let pages = bootstrap(&other).await.pages;
    push(&other_pool, dir.path(), false, false).unwrap();
    let imported = bootstrap(&other).await;
    assert_eq!(imported.pages, pages);
    let hero = imported.components.iter().find(|c| c.id == "hero").unwrap();
    assert!(hero.fields.iter().any(|f| f.name == "new_field"));
}

#[tokio::test]
async fn package_path_swaps_are_atomic_and_bring_missing_definitions() {
    let app = setup().await;
    let first = create_test_page(&app, "/first").await;
    let second = create_test_page(&app, "/second").await;
    let package = export_package(&app, "/").await;
    let first_entry = format!("pages/{}.yaml", first.id);
    let second_entry = format!("pages/{}.yaml", second.id);
    let install = |app: &TestApp, body| {
        let (router, cookie) = (app.app.clone(), app.cookie.clone());
        async move { raw_call(&router, "POST", "/api/admin/package", &cookie, body).await }
    };
    let half = repack(&package, |files| {
        files.get_mut(&first_entry).unwrap()["data"]["slug"] = json!("/second")
    });
    let before = serde_json::to_value(bootstrap(&app).await).unwrap();
    assert_eq!(install(&app, half).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    let swapped = repack(&package, |files| {
        files.get_mut(&first_entry).unwrap()["data"]["slug"] = json!("/second");
        files.get_mut(&second_entry).unwrap()["data"]["slug"] = json!("/first");
    });
    assert_eq!(install(&app, swapped).await.status(), StatusCode::OK);
    let pages = bootstrap(&app).await.pages;
    let slug = |id: &str| pages.iter().find(|p| p.id == id).unwrap().slug.clone();
    assert_eq!(
        (slug(&first.id), slug(&second.id)),
        ("/second".into(), "/first".into())
    );

    // New definitions arrive with the pages that use them in another installation.
    let bundled = repack(&package, |files| {
        let mut component = files["components/text.yaml"].clone();
        component["data"]["id"] = json!("package-component");
        files.insert("components/package-component.yaml".into(), component);
        let mut template = files["templates/homepage.yaml"].clone();
        template["data"]["id"] = json!("package-template");
        template["data"]["regions"][0]["allowed_components"] = json!(["package-component"]);
        files.insert("templates/package-template.yaml".into(), template);
        files.get_mut(&second_entry).unwrap()["data"]["template_id"] = json!("package-template");
    });
    let other = setup().await;
    let response = install(&other, bundled).await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: Value = read(response).await;
    assert_eq!(
        (
            result["components_added"].clone(),
            result["templates_added"].clone()
        ),
        (json!(1), json!(1))
    );
    let imported = bootstrap(&other).await;
    let page = imported.pages.iter().find(|p| p.id == second.id).unwrap();
    assert_eq!(page.template_id, "package-template");
    assert_eq!((page.revision, page.published_revision), (1, None));
}

#[tokio::test]
async fn forced_pull_repairs_malformed_exports_and_never_reads_pages() {
    use baddiecore::serialization::{pull, push};
    let app = setup().await;
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    pull(&pool, dir.path(), false).unwrap();
    let hero = dir.path().join("components/hero.yaml");
    let original = std::fs::read(&hero).unwrap();
    let obsolete = dir.path().join("components/deleted.yaml");
    let notes = dir.path().join("components/notes.txt");
    std::fs::write(&hero, "invalid: [").unwrap();
    std::fs::write(&obsolete, "invalid too: [").unwrap();
    std::fs::write(&notes, "keep this").unwrap();
    // Both are file-side changes that pull leaves for push, which rejects them.
    let sync = pull(&pool, dir.path(), false).unwrap();
    assert_eq!(sync.pending.len(), 2);
    assert_eq!(std::fs::read_to_string(&hero).unwrap(), "invalid: [");
    assert!(push(&pool, dir.path(), false, false).is_err());
    pull(&pool, dir.path(), true).unwrap();
    assert_eq!(std::fs::read(&hero).unwrap(), original);
    assert!(!obsolete.exists());
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "keep this");
    assert!(!dir.path().join("pages").exists());
    // This would fail if pull still deserialized pages.
    pool.get_conn()
        .unwrap()
        .query_drop("UPDATE pages SET data='not json'")
        .unwrap();
    pull(&pool, dir.path(), true).unwrap();
}

#[tokio::test]
async fn cms_origin_policy_uses_the_builtin_providers_transport_configuration() {
    let db = TestDatabase::new();
    let dir = tempfile::tempdir().unwrap();
    for secure in [false, true] {
        let app = router(
            AppState::open(&db.url, Some(PASSWORD.into()), secure).unwrap(),
            dir.path(),
        );
        let login = call(
            &app,
            "POST",
            "/api/login",
            None,
            Some(&json!({"username":"admin","password":PASSWORD})),
        )
        .await;
        let cookie = login.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        for (origin, allowed) in [
            ("http://cms.example", !secure),
            ("https://cms.example", secure),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/admin/pages/home/publish")
                        .header(header::HOST, "cms.example")
                        .header(header::ORIGIN, origin)
                        .header(header::COOKIE, cookie)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"revision":1}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if allowed {
                    StatusCode::OK
                } else {
                    StatusCode::FORBIDDEN
                },
                "{origin}, secure={secure}"
            );
        }
    }
}

// Test-only provider: cookies contain stable IDs, never enabled by the server.
fn member_app(app: &TestApp) -> Router {
    struct Identity;
    impl baddiecore::auth::AuthProvider for Identity {
        fn routes(&self) -> Router {
            Router::new()
        }
        fn authorize<'a>(
            &'a self,
            headers: &'a axum::http::HeaderMap,
        ) -> baddiecore::auth::Authorization<'a> {
            Box::pin(async move {
                Ok(baddiecore::auth::Editor {
                    id: headers[header::COOKIE].to_str().unwrap().into(),
                })
            })
        }
    }
    router(
        AppState::open_with_auth(&app.db.url, std::sync::Arc::new(Identity)).unwrap(),
        app._dir.path(),
    )
}

async fn members(app: &TestApp) -> Value {
    let current: Value = read(
        call(
            &app.app,
            "GET",
            "/api/admin/organization",
            Some(&app.cookie),
            None::<&Value>,
        )
        .await,
    )
    .await;
    let org = json!({"revision":current["revision"],"members":[
        {"id":"admin","name":"Admin","role":"admin","paths":[],"groups":[]},
        {"id":"editor","name":"Editor","role":"editor","paths":["/other"],"groups":["news"]},
        {"id":"reviewer","name":"Reviewer","role":"reviewer","paths":[],"groups":[]}
    ],"groups":[{"id":"news","name":"News team","paths":["/news"]}]});
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/organization",
        Some(&app.cookie),
        Some(&org),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    read(response).await
}

#[tokio::test]
async fn membership_scopes_and_admin_routes_cannot_be_bypassed() {
    let app = setup().await;
    let mut org = members(&app).await;
    let scoped = member_app(&app);
    let news = create_test_page(&app, "/news").await;
    let child = create_test_page(&app, "/news/child").await;
    create_test_page(&app, "/newspaper").await;
    create_test_page(&app, "/other").await;
    let home = bootstrap(&app)
        .await
        .pages
        .into_iter()
        .find(|p| p.id == "home")
        .unwrap();
    for id in ["unknown", "Admin"] {
        assert_eq!(
            call(
                &scoped,
                "GET",
                "/api/admin/bootstrap",
                Some(id),
                None::<&Value>
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    let visible: Bootstrap = read(
        call(
            &scoped,
            "GET",
            "/api/admin/bootstrap",
            Some("editor"),
            None::<&Value>,
        )
        .await,
    )
    .await;
    let slugs: Vec<_> = visible.pages.iter().map(|p| p.slug.as_str()).collect();
    assert_eq!(slugs.len(), 3);
    assert!(
        slugs.contains(&"/news") && slugs.contains(&"/news/child") && slugs.contains(&"/other")
    );
    for role in ["editor", "reviewer"] {
        for (method, url, body) in [
            (
                "POST",
                "/api/admin/pages/home/publish",
                json!({"revision":home.revision}),
            ),
            ("DELETE", "/api/admin/pages/home", json!({})),
            (
                "POST",
                "/api/admin/components",
                json!({"name":"X","description":"","renderer":"text","fields":[]}),
            ),
            (
                "PUT",
                "/api/admin/components/text",
                json!({"id":"text","name":"X","description":"","renderer":"text","fields":[]}),
            ),
            (
                "POST",
                "/api/admin/templates",
                json!({"name":"X","description":"","regions":[]}),
            ),
            (
                "PUT",
                "/api/admin/templates/homepage",
                json!({"id":"homepage","name":"X","description":"","regions":[]}),
            ),
            ("PUT", "/api/admin/organization", org.clone()),
        ] {
            assert_eq!(
                call(&scoped, method, url, Some(role), Some(&body))
                    .await
                    .status(),
                StatusCode::FORBIDDEN,
                "{role} {url}"
            );
        }
        assert_eq!(
            call(
                &scoped,
                "GET",
                "/api/admin/organization",
                Some(role),
                None::<&Value>
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        call(
            &scoped,
            "PUT",
            "/api/admin/pages/home",
            Some("editor"),
            Some(&home)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &scoped,
            "POST",
            "/api/admin/pages/home/submit",
            Some("editor"),
            Some(&json!({"revision":1}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &scoped,
            "POST",
            "/api/admin/pages",
            Some("editor"),
            Some(&json!({"title":"escape","slug":"/newspaper/x","template_id":"homepage"}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let mut moved = news.clone();
    moved.slug = "/outside".into();
    assert_eq!(
        call(
            &scoped,
            "PUT",
            &format!("/api/admin/pages/{}", news.id),
            Some("editor"),
            Some(&moved)
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let after = bootstrap(&app).await;
    assert_eq!(
        after.pages.iter().find(|p| p.id == child.id).unwrap(),
        &child
    );
    // Valid move affects descendants and increments their revisions.
    moved.slug = "/other/news".into();
    assert_eq!(
        call(
            &scoped,
            "PUT",
            &format!("/api/admin/pages/{}", news.id),
            Some("editor"),
            Some(&moved)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let after = bootstrap(&app).await;
    let moved_child = after.pages.iter().find(|p| p.id == child.id).unwrap();
    assert_eq!(moved_child.slug, "/other/news/child");
    assert_eq!(moved_child.revision, child.revision + 1);
    // Revoking group access takes effect without reauthentication.
    org["groups"][0]["paths"] = json!([]);
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/organization",
            Some(&app.cookie),
            Some(&org)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(
            &scoped,
            "POST",
            "/api/admin/pages",
            Some("editor"),
            Some(&json!({"title":"denied","slug":"/news/new","template_id":"homepage"}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    // Stale membership writes cannot restore revoked grants.
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/organization",
            Some(&app.cookie),
            Some(&org)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn reviews_bind_submission_content_schema_and_transitions() {
    let app = setup().await;
    members(&app).await;
    let scoped = member_app(&app);
    let mut page = create_test_page(&app, "/news/story").await;
    let submit_url = format!("/api/admin/pages/{}/submit", page.id);
    let review_url = format!("/api/admin/reviews/{}", page.id);
    let page_url = format!("/api/admin/pages/{}", page.id);
    let submit = |revision| json!({"revision":revision});
    let decision = |review: &Value, approve, feedback| json!({"revision":review["content"]["page"]["revision"],"submission_id":review["submission_id"],"approve":approve,"feedback":feedback});
    let review: Value = read(
        call(
            &scoped,
            "POST",
            &submit_url,
            Some("editor"),
            Some(&submit(page.revision)),
        )
        .await,
    )
    .await;
    assert_eq!(review["status"], "submitted");
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("editor"),
            Some(&decision(&review, true, ""))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, false, "  "))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let rejected: Value = read(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, false, "Please add detail")),
        )
        .await,
    )
    .await;
    assert_eq!(rejected["status"], "changes_requested");
    assert_eq!(rejected["feedback"], "Please add detail");
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, true, ""))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let review: Value = read(
        call(
            &scoped,
            "POST",
            &submit_url,
            Some("editor"),
            Some(&submit(page.revision)),
        )
        .await,
    )
    .await;
    page.title = "Changed after submission".into();
    page = read(call(&scoped, "PUT", &page_url, Some("editor"), Some(&page)).await).await;
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, true, ""))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/news/story",
            None,
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let review: Value = read(
        call(
            &scoped,
            "POST",
            &submit_url,
            Some("editor"),
            Some(&submit(page.revision)),
        )
        .await,
    )
    .await;
    let mut template = bootstrap(&app).await.templates[0].clone();
    template.name = "Changed schema".into();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/templates/homepage",
            Some(&app.cookie),
            Some(&template)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, true, ""))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let replacement: Value = read(
        call(
            &scoped,
            "POST",
            &submit_url,
            Some("editor"),
            Some(&submit(page.revision)),
        )
        .await,
    )
    .await;
    // Same page revision, different schema/submission. An old tab must not approve it.
    assert_ne!(review["submission_id"], replacement["submission_id"]);
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, true, ""))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let approved: Value = read(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&replacement, true, "Ready")),
        )
        .await,
    )
    .await;
    assert_eq!(approved["status"], "approved");
    let public: Content = read(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/news/story",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(public.page.title, "Changed after submission");
    assert_eq!(public.template.name, "Changed schema");
    assert_eq!(public.page.published_revision, Some(page.revision));
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&replacement, true, ""))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    // Existing published pages follow the same review process; edits stay private.
    page.published_revision = public.page.published_revision;
    page.title = "Second edition".into();
    let page: Page = read(call(&scoped, "PUT", &page_url, Some("editor"), Some(&page)).await).await;
    let review: Value = read(
        call(
            &scoped,
            "POST",
            &submit_url,
            Some("editor"),
            Some(&submit(page.revision)),
        )
        .await,
    )
    .await;
    let before: Content = read(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/news/story",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(before, public);
    assert_eq!(
        call(
            &scoped,
            "POST",
            &review_url,
            Some("reviewer"),
            Some(&decision(&review, true, ""))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let after: Content = read(
        call(
            &app.app,
            "GET",
            "/api/content?slug=/news/story",
            None,
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(after.page.title, "Second edition");
}

#[tokio::test]
async fn membership_rejects_orphaned_groups_and_last_admin_removal_atomically() {
    let app = setup().await;
    let org = members(&app).await;
    let mut orphaned_group = org.clone();
    orphaned_group["groups"] = json!([]);
    let mut no_admin = org.clone();
    no_admin["members"][0]["role"] = json!("reviewer");
    for invalid in [orphaned_group, no_admin] {
        assert_eq!(
            call(
                &app.app,
                "PUT",
                "/api/admin/organization",
                Some(&app.cookie),
                Some(&invalid),
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        let current: Value = read(
            call(
                &app.app,
                "GET",
                "/api/admin/organization",
                Some(&app.cookie),
                None::<&Value>,
            )
            .await,
        )
        .await;
        assert_eq!(current, org);
    }
}

#[tokio::test]
async fn headless_keys_are_disabled_by_default_and_cannot_cross_capabilities() {
    let disabled = setup().await;
    assert_eq!(
        key_call(
            &disabled.app,
            "GET",
            "/api/headless/pages",
            CONTENT_KEY,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let app = headless_setup().await;
    for uri in ["/api/headless/pages", "/api/headless/content?slug=/"] {
        for cookie in [None, Some(app.cookie.as_str())] {
            let response = call(&app.app, "GET", uri, cookie, None::<&Value>).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[header::WWW_AUTHENTICATE], "Bearer");
        }
        for key in [COMPONENT_KEY, "wrong", "test-content-key-0123456789abcdee"] {
            assert_eq!(
                key_call(&app.app, "GET", uri, key, None).await.status(),
                StatusCode::UNAUTHORIZED
            );
        }
    }
    assert_eq!(
        key_call(&app.app, "HEAD", "/api/headless/pages", CONTENT_KEY, None)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        key_call(
            &app.app,
            "PUT",
            "/api/headless/components/app-promo",
            CONTENT_KEY,
            Some(&json!({}))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    for key in [CONTENT_KEY, COMPONENT_KEY] {
        assert_eq!(
            key_call(&app.app, "GET", "/api/admin/bootstrap", key, None)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            key_call(
                &app.app,
                "POST",
                "/api/admin/pages/home/publish",
                key,
                Some(&json!({"revision":1}))
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/headless/pages")
                .header(header::AUTHORIZATION, format!("Bearer {CONTENT_KEY}"))
                .header(header::AUTHORIZATION, format!("Bearer {CONTENT_KEY}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        call(
            &app.app,
            "GET",
            &format!("/api/headless/pages?api_key={CONTENT_KEY}"),
            None,
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn headless_external_components_are_editor_definitions_and_delivery_is_snapshot_only() {
    let app = headless_setup().await;
    let uri = "/api/headless/components/app-promo";
    let mut component = json!({"id":"app-promo", "name":"Product promotion", "description":"Rendered by the shop",
    "renderer":"external", "fields":[
        {"name":"headline", "label":"Headline", "kind":"text", "required":true},
        {"name":"destination", "label":"Destination", "kind":"url", "required":false}
    ]});
    let response = key_call(&app.app, "PUT", uri, COMPONENT_KEY, Some(&component)).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(read::<Value>(response).await, component);
    assert_eq!(
        key_call(&app.app, "PUT", uri, COMPONENT_KEY, Some(&component))
            .await
            .status(),
        StatusCode::OK
    );
    let data = bootstrap(&app).await;
    assert!(
        data.components
            .iter()
            .any(|c| c.id == "app-promo" && c.renderer == baddiecore::Renderer::External)
    );
    let mut template = serde_json::to_value(&data.templates[0]).unwrap();
    template["regions"][0]["allowed_components"]
        .as_array_mut()
        .unwrap()
        .push(json!("app-promo"));
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/templates/homepage",
            Some(&app.cookie),
            Some(&template)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let mut page =
        serde_json::to_value(data.pages.iter().find(|p| p.id == "home").unwrap()).unwrap();
    page["blocks"].as_array_mut().unwrap().push(
        json!({"id":"promo-instance", "component_id":"app-promo", "region":"main",
        "fields":{"headline":"Summer offer", "destination":"/shop/summer"}}),
    );
    let page: Value = read(
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
    let empty: Value =
        read(key_call(&app.app, "GET", "/api/headless/pages", CONTENT_KEY, None).await).await;
    assert_eq!(empty, json!([]));
    assert_eq!(
        key_call(
            &app.app,
            "GET",
            "/api/headless/content?slug=/",
            CONTENT_KEY,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &app.app,
            "POST",
            "/api/admin/pages/home/publish",
            Some(&app.cookie),
            Some(&json!({"revision":page["revision"]}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let response = key_call(
        &app.app,
        "GET",
        "/api/headless/content?slug=/",
        CONTENT_KEY,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let snapshot: Value = read(response).await;
    assert_eq!(
        snapshot["page"]["blocks"][2]["fields"]["headline"],
        "Summer offer"
    );
    assert_eq!(
        snapshot["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "app-promo")
            .unwrap(),
        &component
    );
    assert_eq!(snapshot["template"]["regions"][0]["name"], "main");

    // Draft and schema updates must not leak private titles or field values.
    let mut draft = page;
    draft["title"] = json!("Private title");
    draft["blocks"][2]["fields"]["headline"] = json!("Private offer");
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/pages/home",
            Some(&app.cookie),
            Some(&draft)
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app.app,
            "POST",
            "/api/admin/pages",
            Some(&app.cookie),
            Some(&json!({"title":"Hidden page", "slug":"/hidden", "template_id":"homepage"}))
        )
        .await
        .status(),
        StatusCode::CREATED
    );
    component["name"] = json!("Draft schema name");
    assert_eq!(
        key_call(&app.app, "PUT", uri, COMPONENT_KEY, Some(&component))
            .await
            .status(),
        StatusCode::OK
    );
    let unchanged: Value = read(
        key_call(
            &app.app,
            "GET",
            "/api/headless/content?slug=/",
            CONTENT_KEY,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(unchanged, snapshot);
    let listing: Value =
        read(key_call(&app.app, "GET", "/api/headless/pages", CONTENT_KEY, None).await).await;
    assert_eq!(
        listing,
        json!([{"id":"home", "title":"Home", "slug":"/", "template_id":"homepage", "revision":2}])
    );
    assert_eq!(
        key_call(
            &app.app,
            "GET",
            "/api/headless/content?slug=/hidden",
            CONTENT_KEY,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    let mut invalid = component.clone();
    invalid["fields"] = json!([]);
    assert_eq!(
        key_call(&app.app, "PUT", uri, COMPONENT_KEY, Some(&invalid))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let current = bootstrap(&app)
        .await
        .components
        .into_iter()
        .find(|c| c.id == "app-promo")
        .unwrap();
    assert_eq!(serde_json::to_value(current).unwrap(), component);
}

#[tokio::test]
async fn headless_registration_validates_ids_schemas_and_protects_builtin_components() {
    let app = headless_setup().await;
    let valid = json!({"id":"app-card", "name":"Card", "description":"", "renderer":"external",
        "fields":[{"name":"title", "label":"Title", "kind":"text", "required":true}]});
    for (uri, input) in [
        ("/api/headless/components/different", valid.clone()),
        ("/api/headless/components/bad.id", {
            let mut v = valid.clone();
            v["id"] = json!("bad.id");
            v
        }),
        ("/api/headless/components/app-card", {
            let mut v = valid.clone();
            v["name"] = json!("");
            v
        }),
        ("/api/headless/components/app-card", {
            let mut v = valid.clone();
            v["renderer"] = json!("hero");
            v
        }),
        ("/api/headless/components/app-card", {
            let mut v = valid.clone();
            v["fields"]
                .as_array_mut()
                .unwrap()
                .push(valid["fields"][0].clone());
            v
        }),
    ] {
        assert_eq!(
            key_call(&app.app, "PUT", uri, COMPONENT_KEY, Some(&input))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let mut collision = valid.clone();
    collision["id"] = json!("hero");
    assert_eq!(
        key_call(
            &app.app,
            "PUT",
            "/api/headless/components/hero",
            COMPONENT_KEY,
            Some(&collision)
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(bootstrap(&app).await.components.len(), 4);
    assert_eq!(
        key_call(
            &app.app,
            "PUT",
            "/api/headless/components/app-card",
            COMPONENT_KEY,
            Some(&valid)
        )
        .await
        .status(),
        StatusCode::CREATED
    );
}

async fn raw_call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: &str,
    body: Vec<u8>,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::HOST, "example.test")
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/zip")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn package_names(package: &[u8]) -> Vec<String> {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(package)).unwrap();
    let mut names: Vec<String> = archive
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect();
    names.sort();
    names
}

async fn export_package(app: &TestApp, path: &str) -> Vec<u8> {
    let response = call(
        &app.app,
        "GET",
        &format!("/api/admin/package?path={path}"),
        Some(&app.cookie),
        None::<&Value>,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/zip");
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn packages_move_page_branches_between_installations_as_drafts() {
    let source = setup().await;
    let template: Value = read(
        call(
            &source.app,
            "POST",
            "/api/admin/templates",
            Some(&source.cookie),
            Some(&json!({"name":"Article","description":"","regions":[
                {"name":"main","allowed_components":["text"],"max_components":5}]})),
        )
        .await,
    )
    .await;
    let template_id = template["id"].as_str().unwrap().to_owned();
    let response = call(
        &source.app,
        "POST",
        "/api/admin/pages",
        Some(&source.cookie),
        Some(&json!({"title":"News","slug":"/news","template_id":template_id})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let news: Page = read(response).await;
    let story = create_test_page(&source, "/news/story").await;
    create_test_page(&source, "/newspaper").await;
    // Definitions already on the target belong to Git and must not be overwritten.
    let mut homepage = bootstrap(&source)
        .await
        .templates
        .into_iter()
        .find(|t| t.id == "homepage")
        .unwrap();
    homepage.description = "Changed only on the source".into();
    let response = call(
        &source.app,
        "PUT",
        "/api/admin/templates/homepage",
        Some(&source.cookie),
        Some(&homepage),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let package = export_package(&source, "/news").await;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(package.clone())).unwrap();
    let names = package_names(&package);
    assert!(names.contains(&format!("pages/{}.yaml", news.id)));
    assert!(names.contains(&format!("pages/{}.yaml", story.id)));
    assert!(names.contains(&format!("templates/{template_id}.yaml")));
    assert!(names.contains(&"components/text.yaml".to_owned()));
    assert_eq!(names.iter().filter(|n| n.starts_with("pages/")).count(), 2);
    let mut text = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name(&format!("pages/{}.yaml", news.id)).unwrap(),
        &mut text,
    )
    .unwrap();
    assert!(text.contains("slug: /news") && !text.contains("revision"));

    let target = setup().await;
    let original = bootstrap(&target).await;
    // A different page already owns one of the paths: nothing is installed.
    let blocker = create_test_page(&target, "/news/story").await;
    let before = serde_json::to_value(bootstrap(&target).await).unwrap();
    let response = raw_call(
        &target.app,
        "POST",
        "/api/admin/package",
        &target.cookie,
        package.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        serde_json::to_value(bootstrap(&target).await).unwrap(),
        before
    );
    let response = call(
        &target.app,
        "DELETE",
        &format!("/api/admin/pages/{}", blocker.id),
        Some(&target.cookie),
        None::<&Value>,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = raw_call(
        &target.app,
        "POST",
        "/api/admin/package",
        &target.cookie,
        package.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: Value = read(response).await;
    assert_eq!(result["created"], json!(["/news", "/news/story"]));
    assert_eq!(result["templates_added"], 1);
    assert_eq!(result["components_added"], 0);
    let installed = bootstrap(&target).await;
    let page = installed.pages.iter().find(|p| p.id == news.id).unwrap();
    assert_eq!((page.revision, page.published_revision), (1, None));
    assert_eq!(page.template_id, template_id);
    assert_eq!(installed.templates.len(), original.templates.len() + 1);
    assert_eq!(
        installed
            .templates
            .iter()
            .find(|t| t.id == "homepage")
            .unwrap()
            .description,
        original
            .templates
            .iter()
            .find(|t| t.id == "homepage")
            .unwrap()
            .description
    );

    // Reinstalling is idempotent; changed pages get the target's next revision.
    let result: Value = read(
        raw_call(
            &target.app,
            "POST",
            "/api/admin/package",
            &target.cookie,
            package,
        )
        .await,
    )
    .await;
    assert_eq!(result["unchanged"], json!(["/news", "/news/story"]));
    let mut renamed = news.clone();
    renamed.title = "Latest news".into();
    let response = call(
        &source.app,
        "PUT",
        &format!("/api/admin/pages/{}", news.id),
        Some(&source.cookie),
        Some(&renamed),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: Value = read(
        raw_call(
            &target.app,
            "POST",
            "/api/admin/package",
            &target.cookie,
            export_package(&source, "/news").await,
        )
        .await,
    )
    .await;
    assert_eq!(result["updated"], json!(["/news"]));
    let page = bootstrap(&target)
        .await
        .pages
        .into_iter()
        .find(|p| p.id == news.id)
        .unwrap();
    assert_eq!((page.title.as_str(), page.revision), ("Latest news", 2));

    // The whole site exports from "/"; empty paths and junk uploads are rejected.
    let site = export_package(&source, "/").await;
    assert_eq!(
        package_names(&site)
            .iter()
            .filter(|n| n.starts_with("pages/"))
            .count(),
        4
    );
    for (method, uri, body) in [
        ("GET", "/api/admin/package?path=/missing", Vec::new()),
        ("POST", "/api/admin/package", b"not a zip".to_vec()),
    ] {
        let response = raw_call(&target.app, method, uri, &target.cookie, body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }
}

#[tokio::test]
async fn packages_are_admin_only() {
    let app = setup().await;
    members(&app).await;
    let package = export_package(&app, "/").await;
    let members = member_app(&app);
    for cookie in ["editor", "reviewer"] {
        let response = raw_call(
            &members,
            "GET",
            "/api/admin/package?path=/",
            cookie,
            Vec::new(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = raw_call(
            &members,
            "POST",
            "/api/admin/package",
            cookie,
            package.clone(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = raw_call(&members, "POST", "/api/admin/package", "admin", package).await;
    assert_eq!(response.status(), StatusCode::OK);
}

async fn send(app: &TestApp, method: &str, uri: &str, body: Option<&Value>) -> StatusCode {
    call(&app.app, method, uri, Some(&app.cookie), body)
        .await
        .status()
}

async fn template(app: &TestApp, id: &str) -> Option<baddiecore::Template> {
    bootstrap(app)
        .await
        .templates
        .into_iter()
        .find(|t| t.id == id)
}

#[tokio::test]
async fn deletions_travel_through_git_without_resurrection_or_lost_work() {
    use baddiecore::serialization::{Side, pull, push, status};
    let repo = tempfile::tempdir().unwrap();
    let dir = repo.path();
    let pool = |app: &TestApp| Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let (alice, bob, prod) = (setup().await, setup().await, setup().await);
    let (alice_db, bob_db, prod_db) = (pool(&alice), pool(&bob), pool(&prod));
    // Pushing requires an export directory that tracks the group.
    assert!(push(&bob_db, dir, false, false).is_err());

    let article: Value = read(
        call(
            &alice.app,
            "POST",
            "/api/admin/templates",
            Some(&alice.cookie),
            Some(&json!({"name":"Article","description":"","regions":[
                {"name":"main","allowed_components":["text"],"max_components":5}]})),
        )
        .await,
    )
    .await;
    let article = article["id"].as_str().unwrap().to_owned();
    let article_file = dir.join(format!("templates/{article}.yaml"));
    pull(&alice_db, dir, false).unwrap();
    assert!(article_file.exists() && dir.join("baddiecore.yaml").exists());
    for db in [&bob_db, &prod_db] {
        let sync = push(db, dir, false, false).unwrap();
        assert_eq!(sync.applied.len(), 1, "{:?}", sync.applied);
        assert_eq!(sync.applied[0].action, "added");
    }

    // Alice deletes the template in her CMS; pull deletes its file.
    let uri = format!("/api/admin/templates/{article}");
    assert_eq!(
        send(&alice, "DELETE", &uri, None).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(&alice, "DELETE", &uri, None).await,
        StatusCode::NOT_FOUND
    );
    let sync = pull(&alice_db, dir, false).unwrap();
    assert_eq!(sync.applied[0].action, "deleted");
    assert!(!article_file.exists());

    // Bob sees an incoming deletion. Pulling first must not export the template back.
    let changes = status(&bob_db, dir).unwrap();
    assert_eq!(
        (changes[0].side, changes[0].action),
        (Side::Files, "deleted")
    );
    let sync = pull(&bob_db, dir, false).unwrap();
    assert!(sync.applied.is_empty() && !article_file.exists());
    push(&bob_db, dir, false, false).unwrap();
    assert!(template(&bob, &article).await.is_none());
    assert!(status(&bob_db, dir).unwrap().is_empty());

    // Work that exists only in Bob's database survives his push, waiting for his pull.
    let landing: Value = read(
        call(
            &bob.app,
            "POST",
            "/api/admin/templates",
            Some(&bob.cookie),
            Some(&json!({"name":"Landing","description":"","regions":[
                {"name":"main","allowed_components":["hero"],"max_components":5}]})),
        )
        .await,
    )
    .await;
    let landing = landing["id"].as_str().unwrap().to_owned();
    let sync = push(&bob_db, dir, false, false).unwrap();
    assert_eq!(sync.pending.len(), 1);
    assert_eq!(sync.pending[0].side, Side::Database);
    assert!(template(&bob, &landing).await.is_some());

    // A template still used by pages is not deleted; the push changes nothing.
    let response = call(
        &prod.app,
        "POST",
        "/api/admin/pages",
        Some(&prod.cookie),
        Some(&json!({"title":"News","slug":"/news","template_id":article})),
    )
    .await;
    let news: Page = read(response).await;
    let error = push(&prod_db, dir, false, false).unwrap_err();
    assert!(error.0.contains("still used by page /news"), "{error}");
    assert!(template(&prod, &article).await.is_some());
    let uri = format!("/api/admin/pages/{}", news.id);
    assert_eq!(
        send(&prod, "DELETE", &uri, None).await,
        StatusCode::NO_CONTENT
    );
    push(&prod_db, dir, false, false).unwrap();
    assert!(template(&prod, &article).await.is_none());

    // Components in use by templates or pages cannot be deleted through the API either.
    assert_eq!(
        send(&alice, "DELETE", "/api/admin/components/cards", None).await,
        StatusCode::CONFLICT
    );
    let mut homepage = template(&alice, "homepage").await.unwrap();
    homepage.regions[0]
        .allowed_components
        .retain(|c| c != "cards");
    homepage.description = "Without cards".into();
    let body = serde_json::to_value(&homepage).unwrap();
    assert_eq!(
        send(&alice, "PUT", "/api/admin/templates/homepage", Some(&body)).await,
        StatusCode::OK
    );
    assert_eq!(
        send(&alice, "DELETE", "/api/admin/components/cards", None).await,
        StatusCode::NO_CONTENT
    );
    pull(&alice_db, dir, false).unwrap();
    assert!(!dir.join("components/cards.yaml").exists());

    // Bob also edited the homepage, so it conflicts; neither command guesses.
    let mut mine = template(&bob, "homepage").await.unwrap();
    mine.description = "Bob's version".into();
    let body = serde_json::to_value(&mine).unwrap();
    send(&bob, "PUT", "/api/admin/templates/homepage", Some(&body)).await;
    let before = std::fs::read(dir.join("templates/homepage.yaml")).unwrap();
    assert!(push(&bob_db, dir, false, false).is_err());
    assert!(pull(&bob_db, dir, false).is_err());
    assert_eq!(
        std::fs::read(dir.join("templates/homepage.yaml")).unwrap(),
        before
    );
    assert!(
        bootstrap(&bob)
            .await
            .components
            .iter()
            .any(|c| c.id == "cards")
    );
    // Forcing makes Bob's database mirror the files, discarding his homepage edit and his
    // unpulled template. A dry run shows that first.
    let sync = push(&bob_db, dir, true, true).unwrap();
    assert!(
        sync.applied
            .iter()
            .any(|c| c.action == "deleted" && c.path.contains(&landing))
    );
    let sync = push(&bob_db, dir, true, false).unwrap();
    assert!(sync.pending.is_empty());
    assert!(template(&bob, &landing).await.is_none());
    assert_eq!(
        template(&bob, "homepage").await.unwrap().description,
        "Without cards"
    );
    assert!(
        !bootstrap(&bob)
            .await
            .components
            .iter()
            .any(|c| c.id == "cards")
    );

    // A new developer's seeded starter items follow the repository instead of returning.
    let carol = setup().await;
    let carol_db = pool(&carol);
    pull(&carol_db, dir, false).unwrap();
    assert!(!dir.join("components/cards.yaml").exists());
    push(&carol_db, dir, false, false).unwrap();
    assert!(
        !bootstrap(&carol)
            .await
            .components
            .iter()
            .any(|c| c.id == "cards")
    );
    assert!(status(&carol_db, dir).unwrap().is_empty());
}

async fn login(app: &Router, username: &str, password: &str) -> Result<String, StatusCode> {
    let response = call(
        app,
        "POST",
        "/api/login",
        None,
        Some(&json!({"username":username,"password":password})),
    )
    .await;
    if response.status() != StatusCode::NO_CONTENT {
        return Err(response.status());
    }
    Ok(session_cookie(&response))
}

fn session_cookie(response: &axum::response::Response) -> String {
    response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

async fn status_as(app: &Router, cookie: &str) -> StatusCode {
    call(
        app,
        "GET",
        "/api/admin/bootstrap",
        Some(cookie),
        None::<&Value>,
    )
    .await
    .status()
}

#[tokio::test]
async fn local_accounts_sign_in_change_passwords_and_end_sessions() {
    let app = setup().await;
    let org = members(&app).await;
    for (username, password) in [("admin", "wrong-password"), ("nobody", PASSWORD)] {
        assert_eq!(
            login(&app.app, username, password).await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }
    // Members without a password cannot sign in until an administrator sets one.
    assert_eq!(
        login(&app.app, "editor", "editor-password").await,
        Err(StatusCode::UNAUTHORIZED)
    );
    let set = |id: &str, password: &str| {
        let uri = format!("/api/admin/members/{id}/password");
        let body = json!({ "password": password });
        let app = &app;
        async move { send(app, "PUT", &uri, Some(&body)).await }
    };
    assert_eq!(set("editor", "short").await, StatusCode::BAD_REQUEST);
    assert_eq!(
        set("missing", "editor-password").await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        set("editor", "editor-password").await,
        StatusCode::NO_CONTENT
    );
    let editor = login(&app.app, "editor", "editor-password").await.unwrap();
    let access: Value = read(
        call(
            &app.app,
            "GET",
            "/api/admin/bootstrap",
            Some(&editor),
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(access["access"]["id"], "editor");
    assert_eq!(access["access"]["role"], "editor");
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/members/admin/password",
            Some(&editor),
            Some(&json!({"password":"taken-over"})),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );

    // Changing your own password needs the current one and ends other sessions.
    let other_editor = login(&app.app, "editor", "editor-password").await.unwrap();
    let change = |current: &str| {
        let body = json!({"current_password": current, "password": "editor-password-2"});
        let app = &app.app;
        let editor = editor.clone();
        async move {
            call(
                app,
                "POST",
                "/api/account/password",
                Some(&editor),
                Some(&body),
            )
            .await
        }
    };
    assert_eq!(
        change("wrong-password").await.status(),
        StatusCode::FORBIDDEN
    );
    let changed = change("editor-password").await;
    assert_eq!(changed.status(), StatusCode::NO_CONTENT);
    let editor = session_cookie(&changed);
    assert_eq!(
        status_as(&app.app, &other_editor).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(status_as(&app.app, &editor).await, StatusCode::OK);
    assert!(login(&app.app, "editor", "editor-password").await.is_err());

    // An administrator reset ends the member's sessions.
    assert_eq!(
        set("editor", "editor-password-3").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(status_as(&app.app, &editor).await, StatusCode::UNAUTHORIZED);
    let editor = login(&app.app, "editor", "editor-password-3")
        .await
        .unwrap();

    // Expired sessions are rejected.
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    pool.get_conn()
        .unwrap()
        .exec_drop(
            "UPDATE sessions SET expires_at=0 WHERE account_id=?",
            ("editor",),
        )
        .unwrap();
    assert_eq!(status_as(&app.app, &editor).await, StatusCode::UNAUTHORIZED);
    let editor = login(&app.app, "editor", "editor-password-3")
        .await
        .unwrap();

    // Removing a member deletes their password and sessions.
    let mut without_editor = org.clone();
    without_editor["members"] = json!(
        org["members"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["id"] != "editor")
            .collect::<Vec<_>>()
    );
    assert_eq!(
        send(
            &app,
            "PUT",
            "/api/admin/organization",
            Some(&without_editor)
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(status_as(&app.app, &editor).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        login(&app.app, "editor", "editor-password-3").await,
        Err(StatusCode::UNAUTHORIZED)
    );

    let logout = call(
        &app.app,
        "POST",
        "/api/logout",
        Some(&app.cookie),
        None::<&Value>,
    )
    .await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        status_as(&app.app, &app.cookie).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn failed_sign_ins_are_throttled_per_account() {
    let app = setup().await;
    for _ in 0..5 {
        assert_eq!(
            login(&app.app, "admin", "wrong-password").await,
            Err(StatusCode::UNAUTHORIZED)
        );
    }
    assert_eq!(
        login(&app.app, "admin", PASSWORD).await,
        Err(StatusCode::TOO_MANY_REQUESTS)
    );
    // Other accounts and existing sessions are unaffected.
    assert_eq!(
        login(&app.app, "nobody", "wrong-password").await,
        Err(StatusCode::UNAUTHORIZED)
    );
    assert_eq!(status_as(&app.app, &app.cookie).await, StatusCode::OK);
}

#[tokio::test]
async fn startup_needs_an_administrator_and_reset_admin_recovers_access() {
    let db = TestDatabase::new();
    let dir = tempfile::tempdir().unwrap();
    for password in [None, Some(String::new())] {
        assert!(AppState::open(&db.url, password, false).is_err());
    }
    assert!(baddiecore::reset_admin(&db.url, "alice", "short").is_err());
    baddiecore::reset_admin(&db.url, "alice", "alice-password").unwrap();
    let app = router(AppState::open(&db.url, None, false).unwrap(), dir.path());
    let alice = login(&app, "alice", "alice-password").await.unwrap();
    let org: Value = read(
        call(
            &app,
            "GET",
            "/api/admin/organization",
            Some(&alice),
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(org["members"][0]["id"], "alice");
    assert_eq!(org["members"][0]["role"], "admin");

    // Recovery promotes an existing member and ends their sessions.
    let mut demoted = org.clone();
    demoted["members"] = json!([
        org["members"][0],
        {"id":"bob","name":"Bob","role":"editor","paths":[],"groups":[]}
    ]);
    let response = call(
        &app,
        "PUT",
        "/api/admin/organization",
        Some(&alice),
        Some(&demoted),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    baddiecore::reset_admin(&db.url, "bob", "bob-password").unwrap();
    baddiecore::reset_admin(&db.url, "alice", "alice-password-2").unwrap();
    assert_eq!(status_as(&app, &alice).await, StatusCode::UNAUTHORIZED);
    let bob = login(&app, "bob", "bob-password").await.unwrap();
    let access: Value = read(
        call(
            &app,
            "GET",
            "/api/admin/bootstrap",
            Some(&bob),
            None::<&Value>,
        )
        .await,
    )
    .await;
    assert_eq!(access["access"]["role"], "admin");
}

#[tokio::test]
async fn richtext_validates_settings_required_content_and_preserves_published_icons_and_yaml() {
    use baddiecore::serialization::{pull, push};
    let app = setup().await;
    let data = bootstrap(&app).await;
    let mut component = data
        .components
        .into_iter()
        .find(|c| c.id == "hero")
        .unwrap();
    let body = component
        .fields
        .iter_mut()
        .find(|f| f.name == "body")
        .unwrap();
    body.kind = FieldKind::Richtext;
    body.required = true;
    body.richtext = Some(
        serde_json::from_value(json!({
            "features": ["bold", "link", "ordered_list"],
            "icons": [{"id":"star", "label":"Star", "src":"/assets/star.svg"}]
        }))
        .unwrap(),
    );
    // Existing plain text survives opting in without a data migration.
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/components/hero",
            Some(&app.cookie),
            Some(&component)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let mut page = data.pages.into_iter().find(|p| p.id == "home").unwrap();
    for invalid in [
        json!({"type":"doc", "content":[{"type":"paragraph"}]}),
        json!({"type":"doc", "content":[{"type":"paragraph", "content":[{"type":"text", "text":"Copy", "marks":[{"type":"italic"}]}]}]}),
        json!({"type":"doc", "content":[{"type":"paragraph", "content":[{"type":"icon", "attrs":{"id":"unknown"}}]}]}),
    ] {
        page.blocks[0]
            .fields
            .insert("body".into(), invalid.to_string());
        assert_eq!(
            call(
                &app.app,
                "PUT",
                "/api/admin/pages/home",
                Some(&app.cookie),
                Some(&page)
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let document = json!({"type":"doc", "content":[{"type":"orderedList", "attrs":{"start":4,"type":null}, "content":[{"type":"listItem", "content":[{"type":"paragraph", "content":[
        {"type":"text", "text":"Read more", "marks":[{"type":"bold"},{"type":"link", "attrs":{"href":"/about","target":"_blank","rel":"noopener noreferrer nofollow","class":null,"title":null}}]},
        {"type":"icon", "attrs":{"id":"star"}}
    ]}]}]}]}).to_string();
    page.blocks[0]
        .fields
        .insert("body".into(), document.clone());
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
    assert_eq!(saved.blocks[0].fields["body"], document);
    assert_eq!(
        call(
            &app.app,
            "POST",
            "/api/admin/pages/home/publish",
            Some(&app.cookie),
            Some(&json!({"revision":saved.revision}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let mut disabled = component.clone();
    disabled
        .fields
        .iter_mut()
        .find(|f| f.name == "body")
        .unwrap()
        .richtext
        .as_mut()
        .unwrap()
        .features
        .clear();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/components/hero",
            Some(&app.cookie),
            Some(&disabled)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let mut missing_icon = component.clone();
    missing_icon
        .fields
        .iter_mut()
        .find(|f| f.name == "body")
        .unwrap()
        .richtext
        .as_mut()
        .unwrap()
        .icons
        .clear();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/components/hero",
            Some(&app.cookie),
            Some(&missing_icon)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    component
        .fields
        .iter_mut()
        .find(|f| f.name == "body")
        .unwrap()
        .richtext
        .as_mut()
        .unwrap()
        .icons[0]
        .src = "/assets/new-star.svg".into();
    assert_eq!(
        call(
            &app.app,
            "PUT",
            "/api/admin/components/hero",
            Some(&app.cookie),
            Some(&component)
        )
        .await
        .status(),
        StatusCode::OK
    );
    let live: Content =
        read(call(&app.app, "GET", "/api/content?slug=/", None, None::<&Value>).await).await;
    assert_eq!(live.page.blocks[0].fields["body"], document);
    assert_eq!(
        live.components
            .iter()
            .find(|c| c.id == "hero")
            .unwrap()
            .fields
            .iter()
            .find(|f| f.name == "body")
            .unwrap()
            .richtext
            .as_ref()
            .unwrap()
            .icons[0]
            .src,
        "/assets/star.svg"
    );
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    pull(&pool, dir.path(), false).unwrap();
    push(&pool, dir.path(), false, false).unwrap();
    // Definitions travel through Git; editorial documents travel in packages.
    let target = setup().await;
    let target_pool = Pool::new(Opts::from_url(&target.db.url).unwrap()).unwrap();
    push(&target_pool, dir.path(), false, false).unwrap();
    let after = bootstrap(&target).await;
    assert_eq!(
        after
            .components
            .into_iter()
            .find(|c| c.id == "hero")
            .unwrap(),
        component
    );
    let response = raw_call(
        &target.app,
        "POST",
        "/api/admin/package",
        &target.cookie,
        export_package(&app, "/").await,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let after = bootstrap(&target).await;
    assert_eq!(
        after
            .pages
            .into_iter()
            .find(|p| p.id == "home")
            .unwrap()
            .blocks[0]
            .fields["body"],
        document
    );
}
