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
        let admin = Pool::new(opts).unwrap();
        let name = format!("baddie_test_{}", uuid::Uuid::new_v4().simple());
        admin
            .get_conn()
            .unwrap()
            .query_drop(format!("CREATE DATABASE `{name}`"))
            .unwrap();
        let url = format!("{}/{name}", base.trim_end_matches('/'));
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
    let mut home = bootstrap(&app).await.pages.remove(0);
    let response = call(
        &app.app,
        "POST",
        "/api/admin/pages/home/publish",
        Some(&app.cookie),
        Some(&json!({"revision":1})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    home.slug = "/moved".into();
    let response = call(
        &app.app,
        "PUT",
        "/api/admin/pages/home",
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
    replacement.slug = "/".into();
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
    for (slug, id) in [("/", "home"), ("/replacement", replacement.id.as_str())] {
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
            "/api/admin/pages/home/publish",
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
        call(&app.app, "GET", "/api/content?slug=/", None, None::<&Value>)
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
async fn health_checks_database_access() {
    let app = setup().await;
    assert_eq!(
        call(&app.app, "GET", "/health", None, None::<&Value>)
            .await
            .status(),
        StatusCode::OK
    );
    let pool = Pool::new(Opts::from_url(&app.db.url).unwrap()).unwrap();
    pool.get_conn()
        .unwrap()
        .query_drop("DROP TABLE cms_lock")
        .unwrap();
    assert_eq!(
        call(&app.app, "GET", "/health", None, None::<&Value>)
            .await
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
