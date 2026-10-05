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
        AppState::open(&db.url, "secret".into(), false).unwrap(),
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

async fn setup() -> TestApp {
    let dir = tempfile::tempdir().unwrap();
    let db = TestDatabase::new();
    let app = router(
        AppState::open(&db.url, "secret".into(), false).unwrap(),
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
        AppState::open(&app.db.url, "new-secret".into(), false).unwrap(),
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
        AppState::open(&app.db.url, "secret".into(), true).unwrap(),
        app._dir.path(),
    );
    let login = call(
        &other,
        "POST",
        "/api/login",
        None,
        Some(&json!({"password":"secret"})),
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
    drop(AppState::open(&db.url, "secret".into(), false).unwrap());
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
    assert!(AppState::open(&db.url, "secret".into(), false).is_err());
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

fn edit_yaml(path: &std::path::Path, edit: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_yaml_ng::from_slice(&std::fs::read(path).unwrap()).unwrap();
    edit(&mut value["data"]);
    std::fs::write(path, serde_yaml_ng::to_string(&value).unwrap()).unwrap();
}

#[tokio::test]
async fn cli_pull_push_round_trip_pages_are_opt_in_and_dry_run_rolls_back() {
    use std::process::Command;
    let app = setup().await;
    let dir = tempfile::tempdir().unwrap();
    let run = |command: &str, flags: &[&str]| {
        let result = Command::new(env!("CARGO_BIN_EXE_baddiecore"))
            .arg(command)
            .arg(dir.path())
            .args(flags)
            .env("DATABASE_URL", &app.db.url)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    run("pull", &[]);
    assert!(!dir.path().join("pages").exists());
    let template_path = dir.path().join("templates/homepage.yaml");
    let original = std::fs::read(&template_path).unwrap();
    run("pull", &[]);
    assert_eq!(std::fs::read(&template_path).unwrap(), original);
    edit_yaml(&template_path, |v| {
        v["description"] = json!("Reviewed in git")
    });
    run("push", &["--dry-run"]);
    assert_ne!(
        bootstrap(&app).await.templates[0].description,
        "Reviewed in git"
    );
    run("push", &[]);
    assert_eq!(
        bootstrap(&app).await.templates[0].description,
        "Reviewed in git"
    );
    run("pull", &["--pages", "--force"]);
    let page_path = dir.path().join("pages/home.yaml");
    let text = std::fs::read_to_string(&page_path).unwrap();
    assert!(!text.contains("revision"));
    let before = bootstrap(&app).await.pages[0].clone();
    run("push", &["--pages"]);
    assert_eq!(bootstrap(&app).await.pages[0], before);
    edit_yaml(&page_path, |v| v["title"] = json!("Imported home"));
    run("push", &[]);
    assert_eq!(bootstrap(&app).await.pages[0], before);
    run("push", &["--pages", "--dry-run"]);
    assert_eq!(bootstrap(&app).await.pages[0], before);
    run("push", &["--pages"]);
    let after = bootstrap(&app).await.pages[0].clone();
    assert_eq!(after.title, "Imported home");
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.published_revision, None);
}

#[tokio::test]
async fn imports_validate_the_final_batch_and_preserve_live_snapshots() {
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
    pull(&pool, dir.path(), true, false).unwrap();
    let before = serde_json::to_value(bootstrap(&app).await).unwrap();
    let component_path = dir.path().join("components/hero.yaml");
    let page_path = dir.path().join("pages/home.yaml");
    edit_yaml(&component_path, |v| {
        v["fields"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"new_field","label":"New field","kind":"text","required":true}))
    });
    // Schema-only push must not break a draft, even though the component write comes first.
    assert!(push(&pool, dir.path(), false, false).is_err());
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    edit_yaml(&page_path, |v| {
        v["blocks"][0]["fields"]["new_field"] = json!("Ready")
    });
    push(&pool, dir.path(), true, true).unwrap();
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    push(&pool, dir.path(), true, false).unwrap();
    let updated = bootstrap(&app).await.pages[0].clone();
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.published_revision, Some(1));
    let live: Content =
        read(call(&app.app, "GET", "/api/content?slug=/", None, None::<&Value>).await).await;
    assert_eq!(live.page.revision, 1);
    assert!(!live.page.blocks[0].fields.contains_key("new_field"));
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
    // Import definitions and a page into another initialized instance, retaining IDs.
    let other = setup().await;
    let other_pool = Pool::new(Opts::from_url(&other.db.url).unwrap()).unwrap();
    push(&other_pool, dir.path(), true, false).unwrap();
    let imported = bootstrap(&other).await.pages[0].clone();
    assert_eq!(imported.id, "home");
    assert_eq!(imported.blocks, updated.blocks);
    assert_eq!(imported.published_revision, None);
}

#[tokio::test]
async fn import_path_swaps_are_atomic_and_pull_requires_explicit_overwrites() {
    use baddiecore::serialization::{pull, push};
    let app = setup().await;
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let first = create_test_page(&app, "/first").await;
    let second = create_test_page(&app, "/second").await;
    let dir = tempfile::tempdir().unwrap();
    pull(&pool, dir.path(), true, false).unwrap();
    let first_file = dir.path().join(format!("pages/{}.yaml", first.id));
    let second_file = dir.path().join(format!("pages/{}.yaml", second.id));
    edit_yaml(&first_file, |v| v["slug"] = json!("/second"));
    let edited = std::fs::read(&first_file).unwrap();
    assert!(pull(&pool, dir.path(), true, false).is_err());
    assert_eq!(std::fs::read(&first_file).unwrap(), edited);
    let before = serde_json::to_value(bootstrap(&app).await).unwrap();
    assert!(push(&pool, dir.path(), true, false).is_err());
    assert_eq!(serde_json::to_value(bootstrap(&app).await).unwrap(), before);
    edit_yaml(&second_file, |v| v["slug"] = json!("/first"));
    push(&pool, dir.path(), true, false).unwrap();
    let pages = bootstrap(&app).await.pages;
    assert_eq!(
        pages.iter().find(|p| p.id == first.id).unwrap().slug,
        "/second"
    );
    assert_eq!(
        pages.iter().find(|p| p.id == second.id).unwrap().slug,
        "/first"
    );
    // Missing files never delete database items.
    std::fs::remove_file(&second_file).unwrap();
    push(&pool, dir.path(), true, false).unwrap();
    assert_eq!(bootstrap(&app).await.pages.len(), 3);
    pull(&pool, dir.path(), true, true).unwrap();
    assert_eq!(
        call(
            &app.app,
            "DELETE",
            &format!("/api/admin/pages/{}", first.id),
            Some(&app.cookie),
            None::<&Value>
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert!(pull(&pool, dir.path(), true, false).is_err());
    pull(&pool, dir.path(), true, true).unwrap();
    assert!(!first_file.exists());
    assert!(second_file.exists());

    // New definitions and pages can arrive together in an initialized instance.
    let component_file = dir.path().join("components/git-component.yaml");
    std::fs::copy(dir.path().join("components/text.yaml"), &component_file).unwrap();
    edit_yaml(&component_file, |v| v["id"] = json!("git-component"));
    let template_file = dir.path().join("templates/git-template.yaml");
    std::fs::copy(dir.path().join("templates/homepage.yaml"), &template_file).unwrap();
    edit_yaml(&template_file, |v| {
        v["id"] = json!("git-template");
        v["regions"][0]["allowed_components"] = json!(["git-component"]);
    });
    edit_yaml(&second_file, |v| v["template_id"] = json!("git-template"));
    let other = setup().await;
    let other_pool = Pool::new(Opts::from_url(&other.db.url).unwrap()).unwrap();
    push(&other_pool, dir.path(), true, false).unwrap();
    let imported = bootstrap(&other).await;
    let imported_page = imported.pages.iter().find(|p| p.id == second.id).unwrap();
    assert_eq!(imported_page.template_id, "git-template");
    assert_eq!(imported_page.slug, "/first");
    assert_eq!(imported_page.revision, 1);
    assert_eq!(imported_page.published_revision, None);
    assert!(imported.components.iter().any(|c| c.id == "git-component"));
    assert!(imported.templates.iter().any(|t| t.id == "git-template"));
}

#[tokio::test]
async fn forced_pull_repairs_malformed_exports_and_definition_only_pull_ignores_pages() {
    use baddiecore::serialization::pull;
    let app = setup().await;
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    pull(&pool, dir.path(), false, false).unwrap();
    let hero = dir.path().join("components/hero.yaml");
    let original = std::fs::read(&hero).unwrap();
    let obsolete = dir.path().join("components/deleted.yaml");
    let notes = dir.path().join("components/notes.txt");
    std::fs::write(&hero, "invalid: [").unwrap();
    std::fs::write(&obsolete, "invalid too: [").unwrap();
    std::fs::write(&notes, "keep this").unwrap();
    assert!(pull(&pool, dir.path(), false, false).is_err());
    assert_eq!(std::fs::read_to_string(&hero).unwrap(), "invalid: [");
    pull(&pool, dir.path(), false, true).unwrap();
    assert_eq!(std::fs::read(&hero).unwrap(), original);
    assert!(!obsolete.exists());
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "keep this");
    assert!(!dir.path().join("pages").exists());
    // This would fail if the definition-only command still deserialized pages.
    pool.get_conn()
        .unwrap()
        .query_drop("UPDATE pages SET data='not json'")
        .unwrap();
    pull(&pool, dir.path(), false, true).unwrap();
    assert!(pull(&pool, dir.path(), true, true).is_err());
}

#[tokio::test]
async fn cms_origin_policy_uses_the_builtin_providers_transport_configuration() {
    let db = TestDatabase::new();
    let dir = tempfile::tempdir().unwrap();
    for secure in [false, true] {
        let provider = baddiecore::auth::Auth::password("secret".into(), secure).unwrap();
        let app = router(
            AppState::open_with_auth(&db.url, std::sync::Arc::new(provider)).unwrap(),
            dir.path(),
        );
        let login = call(
            &app,
            "POST",
            "/api/login",
            None,
            Some(&json!({"password":"secret"})),
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
    let org = json!({"revision":0,"members":[
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
    for id in ["unknown", "shared-admin"] {
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
