use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use axum::{
    Extension, Json, Router,
    extract::{Path as AxumPath, Query, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use mysql::{Opts, Pool, PooledConn, Transaction, TxOpts, prelude::Queryable};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};
use uuid::Uuid;

pub mod auth;
pub mod headless;
mod package;
pub mod serialization;
pub mod workflow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub kind: FieldKind,
    pub required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Textarea,
    Url,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub description: String,
    pub renderer: Renderer,
    pub fields: Vec<Field>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Renderer {
    Hero,
    Text,
    Callout,
    Cards,
    External,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Region {
    pub name: String,
    pub allowed_components: Vec<String>,
    pub max_components: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub regions: Vec<Region>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub component_id: String,
    pub region: String,
    pub fields: HashMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Page {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub template_id: String,
    pub blocks: Vec<Block>,
    pub revision: i64,
    pub published_revision: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bootstrap {
    pub pages: Vec<Page>,
    pub templates: Vec<Template>,
    pub components: Vec<Component>,
    pub access: workflow::Access,
    pub reviews: Vec<workflow::Review>,
}

struct StoredData {
    pages: Vec<Page>,
    templates: Vec<Template>,
    components: Vec<Component>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub page: Page,
    pub template: Template,
    pub components: Vec<Component>,
}

#[derive(Clone)]
pub struct AppState {
    db: Pool,
    auth: Arc<dyn auth::AuthProvider>,
    database_slots: Arc<Semaphore>,
    origin: auth::OriginPolicy,
    headless: headless::ApiKeys,
}

#[derive(Debug)]
struct ApiError(StatusCode, String);
impl ApiError {
    fn bad(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }
    fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "not found".into())
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self(StatusCode::CONFLICT, message.into())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"error": self.1}))).into_response()
    }
}
type Result<T> = std::result::Result<T, ApiError>;

impl AppState {
    pub fn open(
        database_url: &str,
        password: String,
        secure_cookie: bool,
    ) -> std::result::Result<Self, String> {
        Self::open_with_auth(
            database_url,
            Arc::new(auth::Auth::password(password, secure_cookie)?),
        )
    }

    pub fn open_with_auth(
        database_url: &str,
        auth: Arc<dyn auth::AuthProvider>,
    ) -> std::result::Result<Self, String> {
        let origin = auth.origin_policy()?;
        let opts = Opts::from_url(database_url).map_err(|_| "DATABASE_URL must be a MySQL URL")?;
        let db = Pool::new(opts).map_err(|e| {
            db_error(e);
            "could not connect to MySQL"
        })?;
        let mut conn = db.get_conn().map_err(|e| {
            db_error(e);
            "could not connect to MySQL"
        })?;
        for statement in [
            "CREATE TABLE IF NOT EXISTS cms_lock(id INT PRIMARY KEY) ENGINE=InnoDB",
            "INSERT IGNORE INTO cms_lock(id) VALUES(1)",
            "CREATE TABLE IF NOT EXISTS components(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS templates(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS pages(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, slug VARBINARY(2048) NOT NULL UNIQUE, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS snapshots(page_id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, slug VARBINARY(2048) NOT NULL UNIQUE, data LONGTEXT NOT NULL, FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS route_aliases(slug VARBINARY(2048) PRIMARY KEY, page_id VARCHAR(255) COLLATE utf8mb4_bin NOT NULL, FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS organization(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            // What each Git export path held at this database's last pull or push. See serialization.
            "CREATE TABLE IF NOT EXISTS sync_base(path VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, hash CHAR(64) NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS reviews(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL, FOREIGN KEY(id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ] {
            conn.query_drop(statement).map_err(|e| {
                db_error(e);
                "could not initialize MySQL schema"
            })?;
        }
        // Bound running and queued blocking jobs independently of Tokio's thread pool.
        let state = Self {
            db,
            auth,
            database_slots: Arc::new(Semaphore::new(16)),
            origin,
            headless: headless::ApiKeys::default(),
        };
        state
            .transaction(|db| workflow::initialize(db))
            .map_err(|e| e.1)?;
        state.seed().map_err(|e| e.1)?;
        Ok(state)
    }

    fn seed(&self) -> Result<()> {
        self.transaction(|tx| {
            let count: i64 = tx
                .query_first("SELECT count(*) FROM components")
                .map_err(db_error)?
                .unwrap();
            if count != 0 {
                return Ok(());
            }
            let specs = [
                (
                    "hero",
                    "Hero",
                    Renderer::Hero,
                    vec![
                        ("eyebrow", false, FieldKind::Text),
                        ("title", true, FieldKind::Text),
                        ("body", false, FieldKind::Textarea),
                        ("button_label", false, FieldKind::Text),
                        ("button_url", false, FieldKind::Url),
                    ],
                ),
                (
                    "text",
                    "Text",
                    Renderer::Text,
                    vec![
                        ("title", true, FieldKind::Text),
                        ("body", false, FieldKind::Textarea),
                    ],
                ),
                (
                    "callout",
                    "Callout",
                    Renderer::Callout,
                    vec![
                        ("title", true, FieldKind::Text),
                        ("body", false, FieldKind::Textarea),
                        ("button_label", false, FieldKind::Text),
                        ("button_url", false, FieldKind::Url),
                    ],
                ),
                (
                    "cards",
                    "Cards",
                    Renderer::Cards,
                    vec![
                        ("title", true, FieldKind::Text),
                        ("body", false, FieldKind::Textarea),
                    ],
                ),
            ];
            let mut components = Vec::new();
            for (id, name, renderer, fields) in specs {
                let component = Component {
                    id: id.into(),
                    name: name.into(),
                    description: format!("{name} content block"),
                    renderer,
                    fields: fields
                        .into_iter()
                        .map(|(name, required, kind)| Field {
                            name: name.into(),
                            label: title_case(name),
                            kind,
                            required,
                        })
                        .collect(),
                };
                tx.exec_drop(
                    "INSERT INTO components VALUES(?,?)",
                    (&component.id, json(&component)?),
                )
                .map_err(db_error)?;
                components.push(component);
            }
            let template = Template {
                id: "homepage".into(),
                name: "Homepage".into(),
                description: "A flexible homepage".into(),
                regions: vec![Region {
                    name: "main".into(),
                    allowed_components: components.iter().map(|c| c.id.clone()).collect(),
                    max_components: 20,
                }],
            };
            tx.exec_drop(
                "INSERT INTO templates VALUES(?,?)",
                (&template.id, json(&template)?),
            )
            .map_err(db_error)?;
            let blocks = vec![
                Block {
                    id: "welcome-hero".into(),
                    component_id: "hero".into(),
                    region: "main".into(),
                    fields: HashMap::from([
                        ("eyebrow".into(), "Welcome".into()),
                        ("title".into(), "Build your site".into()),
                        (
                            "body".into(),
                            "Edit this starter page in the admin area.".into(),
                        ),
                    ]),
                },
                Block {
                    id: "welcome-text".into(),
                    component_id: "text".into(),
                    region: "main".into(),
                    fields: HashMap::from([
                        ("title".into(), "Start publishing".into()),
                        (
                            "body".into(),
                            "Add content, preview your draft, and publish when it is ready.".into(),
                        ),
                    ]),
                },
            ];
            let page = Page {
                id: "home".into(),
                title: "Home".into(),
                slug: "/".into(),
                aliases: vec![],
                template_id: template.id.clone(),
                blocks,
                revision: 1,
                published_revision: None,
            };
            tx.exec_drop(
                "INSERT INTO pages VALUES(?,?,?)",
                (&page.id, &page.slug, json(&page)?),
            )
            .map_err(db_error)?;
            serialization::record_base(tx, &components, &[template])
        })
    }

    fn transaction<T>(&self, f: impl FnOnce(&mut Transaction<'_>) -> Result<T>) -> Result<T> {
        database_transaction(&self.db, TransactionMode::Commit, f)
    }

    async fn run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Transaction<'_>) -> Result<T> + Send + 'static,
    {
        self.blocking(move |db| database_transaction(db, TransactionMode::Commit, f))
            .await
    }

    async fn read<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut PooledConn) -> Result<T> + Send + 'static,
    {
        self.blocking(move |db| f(&mut db.get_conn().map_err(db_error)?))
            .await
    }

    async fn blocking<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Pool) -> Result<T> + Send + 'static,
    {
        let permit = self
            .database_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                ApiError(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "database is busy; retry later".into(),
                )
            })?;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            // Keep the permit until the job ends, even if its HTTP request is cancelled.
            let _permit = permit;
            f(&db)
        })
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database task failed".into(),
            )
        })?
    }
}

#[derive(Clone, Copy)]
enum TransactionMode {
    Commit,
    Rollback,
}

fn database_transaction<T>(
    db: &Pool,
    mode: TransactionMode,
    f: impl FnOnce(&mut Transaction<'_>) -> Result<T>,
) -> Result<T> {
    let mut conn = db.get_conn().map_err(db_error)?;
    let mut tx = conn
        .start_transaction(TxOpts::default())
        .map_err(db_error)?;
    // All server and CLI writes share the same lock. Errors roll back the batch.
    tx.query_drop("SELECT id FROM cms_lock WHERE id=1 FOR UPDATE")
        .map_err(db_error)?;
    let result = f(&mut tx)?;
    match mode {
        TransactionMode::Commit => tx.commit(),
        TransactionMode::Rollback => tx.rollback(),
    }
    .map_err(db_error)?;
    Ok(result)
}

pub fn router(state: AppState, static_dir: impl AsRef<Path>) -> Router {
    let admin = Router::new()
        .route("/bootstrap", get(bootstrap))
        .route("/pages", post(create_page))
        .route("/pages/{id}", put(update_page).delete(delete_page))
        .route("/pages/{id}/publish", post(publish_page))
        .route("/pages/{id}/submit", post(workflow::submit))
        .route("/reviews/{id}", post(workflow::decide))
        .route(
            "/organization",
            get(workflow::get_organization).put(workflow::save_organization),
        )
        .route("/templates", post(create_template))
        .route(
            "/templates/{id}",
            put(update_template).delete(delete_template),
        )
        .route("/components", post(create_component))
        .route(
            "/components/{id}",
            put(update_component).delete(delete_component),
        )
        .route("/package", get(package::export).post(package::install))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let index = static_dir.as_ref().join("index.html");
    Router::new()
        .route("/health", get(health))
        .route("/api/content", get(content))
        .nest("/api/headless", headless::routes(state.clone()))
        .nest("/api/admin", admin)
        .fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(index)))
        .with_state(state.clone())
        .merge(state.auth.routes())
        .layer(middleware::from_fn_with_state(state, redirect_alias))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
}

async fn redirect_alias(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    if matches!(*request.method(), Method::GET | Method::HEAD) && validate_slug(&path).is_ok() {
        let target = state
            .read(move |db| {
                db.exec_first::<String, _, _>(
                    "SELECT s.slug FROM route_aliases a JOIN snapshots s ON s.page_id=a.page_id WHERE a.slug=?",
                    (path,),
                )
                .map_err(db_error)
            })
            .await;
        match target {
            Ok(Some(mut target)) => {
                if let Some(query) = request.uri().query() {
                    target.push('?');
                    target.push_str(query);
                }
                return (StatusCode::MOVED_PERMANENTLY, [(header::LOCATION, target)])
                    .into_response();
            }
            Err(error) => return error.into_response(),
            Ok(None) => {}
        }
    }
    next.run(request).await
}

async fn health(State(state): State<AppState>) -> Result<StatusCode> {
    state
        .read(|db| {
            db.query_drop("SELECT 1").map_err(db_error)?;
            Ok(StatusCode::OK)
        })
        .await
}

async fn require_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    if request.method() != axum::http::Method::GET
        && let Err(error) = state.origin.check(&headers)
    {
        return error.into_response();
    }
    match state.auth.authorize(&headers).await {
        Ok(editor) => {
            request.extensions_mut().insert(editor);
            next.run(request).await
        }
        Err(status) => ApiError(status, "authentication required".into()).into_response(),
    }
}

async fn bootstrap(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
) -> Result<Json<Bootstrap>> {
    state
        .run_as(editor, |db, access| {
            let mut data = load_data(db, true)?;
            data.pages.retain(|page| access.allows(&page.slug));
            let visible: HashSet<&str> = data.pages.iter().map(|page| page.id.as_str()).collect();
            let reviews: Vec<workflow::Review> = load_all(db, "reviews")?;
            let reviews = reviews
                .into_iter()
                .filter(|review| {
                    access.allows(&review.content.page.slug) && visible.contains(review.id.as_str())
                })
                .collect();
            Ok(Json(Bootstrap {
                pages: data.pages,
                templates: data.templates,
                components: data.components,
                access,
                reviews,
            }))
        })
        .await
}
fn load_data(db: &mut impl Queryable, pages: bool) -> Result<StoredData> {
    Ok(StoredData {
        pages: if pages {
            load_all(db, "pages")?
        } else {
            Vec::new()
        },
        templates: load_all(db, "templates")?,
        components: load_all(db, "components")?,
    })
}
fn load_all<T: serde::de::DeserializeOwned>(
    db: &mut impl Queryable,
    table: &str,
) -> Result<Vec<T>> {
    let rows: Vec<String> = db
        .query(format!("SELECT data FROM {table} ORDER BY id"))
        .map_err(db_error)?;
    rows.into_iter().map(|v| parse(&v)).collect()
}

#[derive(Deserialize)]
struct NewPage {
    title: String,
    slug: String,
    template_id: String,
}
async fn create_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(input): Json<NewPage>,
) -> Result<(StatusCode, Json<Page>)> {
    state
        .run_as(editor, move |db, access| {
            validate_slug(&input.slug)?;
            access.page(&input.slug)?;
            require_template(db, &input.template_id)?;
            if input.title.trim().is_empty() {
                return Err(ApiError::bad("title is required"));
            }
            let page = Page {
                id: Uuid::new_v4().to_string(),
                title: input.title,
                slug: input.slug,
                aliases: vec![],
                template_id: input.template_id,
                blocks: vec![],
                revision: 1,
                published_revision: None,
            };
            insert_page(db, &page)?;
            let pages = load_all(db, "pages")?;
            validate_route_paths(db, &pages)?;
            Ok((StatusCode::CREATED, Json(page)))
        })
        .await
}
async fn update_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(mut page): Json<Page>,
) -> Result<Json<Page>> {
    state
        .run_as(editor, move |db, access| {
            if page.id != id {
                return Err(ApiError::bad("page id does not match path"));
            }
            let old = require_page(db, &id)?;
            access.page(&old.slug)?;
            access.page(&page.slug)?;
            let old_aliases: HashSet<_> = old.aliases.iter().collect();
            let new_aliases: HashSet<_> = page.aliases.iter().collect();
            for path in old_aliases.symmetric_difference(&new_aliases) {
                access.page(path)?;
            }
            if page.revision != old.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            page.published_revision = old.published_revision;
            validate_page(db, &page)?;
            page.revision += 1;
            if page.slug != old.slug {
                if old.slug == "/" {
                    return Err(ApiError::bad("the root page cannot be moved"));
                }
                if is_descendant(&page.slug, &old.slug) {
                    return Err(ApiError::bad("a page cannot move into its own subtree"));
                }
                let mut pages: Vec<Page> = load_all(db, "pages")?;
                let mut changed = Vec::new();
                for child in &mut pages {
                    if child.id == page.id {
                        *child = page.clone();
                    } else if is_descendant(&child.slug, &old.slug) {
                        access.page(&child.slug)?;
                        child.slug = format!(
                            "{}{}",
                            page.slug.trim_end_matches('/'),
                            &child.slug[old.slug.len()..]
                        );
                        child.revision += 1;
                        validate_slug(&child.slug)?;
                        access.page(&child.slug)?;
                        changed.push(child.clone());
                    }
                }
                let mut paths = HashSet::new();
                if pages.iter().any(|p| !paths.insert(&p.slug)) {
                    return Err(ApiError::conflict("a destination path already exists"));
                }
                validate_route_paths(db, &pages)?;
                // Temporary non-public paths permit moves to an ancestor without
                // transient unique-key collisions. Snapshots remain untouched.
                changed.push(page.clone());
                for item in &changed {
                    db.exec_drop(
                        "UPDATE pages SET slug=? WHERE id=?",
                        (format!("__move_{}", item.id), &item.id),
                    )
                    .map_err(db_error)?;
                }
                for item in &changed {
                    db.exec_drop(
                        "UPDATE pages SET slug=?,data=? WHERE id=?",
                        (&item.slug, json(item)?, &item.id),
                    )
                    .map_err(constraint_error)?;
                }
                return Ok(Json(page));
            }
            db.exec_drop(
                "UPDATE pages SET slug=?,data=? WHERE id=?",
                (&page.slug, json(&page)?, id),
            )
            .map_err(constraint_error)?;
            let pages = load_all(db, "pages")?;
            validate_route_paths(db, &pages)?;
            Ok(Json(page))
        })
        .await
}
#[derive(Deserialize)]
struct Revision {
    revision: i64,
}
async fn publish_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<Revision>,
) -> Result<Json<Page>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let page = require_page(db, &id)?;
            if page.revision != input.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            let snapshot = workflow::current_content(db, page)?;
            Ok(Json(publish(db, snapshot)?))
        })
        .await
}
// Callers build and validate the snapshot within this same locked transaction.
fn publish(db: &mut impl Queryable, mut snapshot: Content) -> Result<Page> {
    snapshot.page.published_revision = Some(snapshot.page.revision);
    let page = &snapshot.page;
    let occupied: Vec<String> = db
        .exec(
            "SELECT slug FROM snapshots WHERE page_id<>? UNION ALL SELECT slug FROM route_aliases WHERE page_id<>?",
            (&page.id, &page.id),
        )
        .map_err(db_error)?;
    if occupied
        .iter()
        .any(|path| path == &page.slug || page.aliases.contains(path))
    {
        return Err(ApiError::conflict("a published path already exists"));
    }
    db.exec_drop(
        "UPDATE pages SET data=? WHERE id=?",
        (json(page)?, &page.id),
    )
    .map_err(db_error)?;
    // Never use an upsert here: a conflicting slug belongs to another page.
    db.exec_drop("DELETE FROM snapshots WHERE page_id=?", (&page.id,))
        .map_err(db_error)?;
    db.exec_drop(
        "INSERT INTO snapshots(page_id,slug,data) VALUES(?,?,?)",
        (&page.id, &page.slug, json(&snapshot)?),
    )
    .map_err(constraint_error)?;
    db.exec_drop("DELETE FROM route_aliases WHERE page_id=?", (&page.id,))
        .map_err(db_error)?;
    for alias in &page.aliases {
        db.exec_drop(
            "INSERT INTO route_aliases(slug,page_id) VALUES(?,?)",
            (alias, &page.id),
        )
        .map_err(constraint_error)?;
    }
    Ok(snapshot.page)
}
async fn delete_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let page = require_page(db, &id)?;
            let pages: Vec<Page> = load_all(db, "pages")?;
            if pages.iter().any(|p| is_descendant(&p.slug, &page.slug)) {
                return Err(ApiError::conflict("move or delete child pages first"));
            }
            db.exec_drop("DELETE FROM pages WHERE id=?", (id,))
                .map_err(db_error)?;
            if db.affected_rows() == 0 {
                Err(ApiError::not_found())
            } else {
                Ok(StatusCode::NO_CONTENT)
            }
        })
        .await
}

#[derive(Deserialize)]
struct NewTemplate {
    name: String,
    description: String,
    regions: Vec<Region>,
}
async fn create_template(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(input): Json<NewTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let item = Template {
                id: Uuid::new_v4().to_string(),
                name: input.name,
                description: input.description,
                regions: input.regions,
            };
            validate_template(db, &item)?;
            db.exec_drop(
                "INSERT INTO templates VALUES(?,?)",
                (&item.id, json(&item)?),
            )
            .map_err(db_error)?;
            Ok((StatusCode::CREATED, Json(item)))
        })
        .await
}
async fn update_template(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(item): Json<Template>,
) -> Result<Json<Template>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            if item.id != id {
                return Err(ApiError::bad("template id does not match path"));
            }
            require_template(db, &id)?;
            validate_template(db, &item)?;
            validate_schema_change(db, Some(&item), None)?;
            db.exec_drop("UPDATE templates SET data=? WHERE id=?", (json(&item)?, id))
                .map_err(db_error)?;
            Ok(Json(item))
        })
        .await
}

async fn delete_template(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            delete_definition(db, "templates", &id)?;
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}

/// Delete an unused template or component. Published snapshots keep their own copies.
fn delete_definition(db: &mut Transaction<'_>, table: &str, id: &str) -> Result<()> {
    let pages: Vec<Page> = load_all(db, "pages")?;
    let mut users = Vec::new();
    if table == "templates" {
        users.extend(
            pages
                .iter()
                .filter(|p| p.template_id == id)
                .map(|p| format!("page {}", p.slug)),
        );
    } else {
        let templates: Vec<Template> = load_all(db, "templates")?;
        users.extend(
            templates
                .iter()
                .filter(|t| {
                    t.regions
                        .iter()
                        .any(|r| r.allowed_components.iter().any(|c| c == id))
                })
                .map(|t| format!("template {}", t.name)),
        );
        users.extend(
            pages
                .iter()
                .filter(|p| p.blocks.iter().any(|b| b.component_id == id))
                .map(|p| format!("page {}", p.slug)),
        );
    }
    if !users.is_empty() {
        let kind = table.trim_end_matches('s');
        return Err(ApiError::conflict(format!(
            "{kind} {id} is still used by {}",
            users.join(", ")
        )));
    }
    db.exec_drop(format!("DELETE FROM {table} WHERE id=?"), (id,))
        .map_err(db_error)?;
    if db.affected_rows() == 0 {
        return Err(ApiError::not_found());
    }
    Ok(())
}

#[derive(Deserialize)]
struct NewComponent {
    name: String,
    description: String,
    renderer: Renderer,
    fields: Vec<Field>,
}
async fn create_component(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(input): Json<NewComponent>,
) -> Result<(StatusCode, Json<Component>)> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let item = Component {
                id: Uuid::new_v4().to_string(),
                name: input.name,
                description: input.description,
                renderer: input.renderer,
                fields: input.fields,
            };
            validate_component(&item)?;
            db.exec_drop(
                "INSERT INTO components VALUES(?,?)",
                (&item.id, json(&item)?),
            )
            .map_err(db_error)?;
            Ok((StatusCode::CREATED, Json(item)))
        })
        .await
}
async fn update_component(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(item): Json<Component>,
) -> Result<Json<Component>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            if item.id != id {
                return Err(ApiError::bad("component id does not match path"));
            }
            require_component(db, &id)?;
            validate_component(&item)?;
            validate_schema_change(db, None, Some(&item))?;
            db.exec_drop(
                "UPDATE components SET data=? WHERE id=?",
                (json(&item)?, id),
            )
            .map_err(db_error)?;
            Ok(Json(item))
        })
        .await
}
async fn delete_component(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            delete_definition(db, "components", &id)?;
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}

#[derive(Deserialize)]
struct ContentQuery {
    slug: String,
}
async fn content(
    State(state): State<AppState>,
    Query(query): Query<ContentQuery>,
) -> Result<Json<Content>> {
    validate_slug(&query.slug)?;
    state
        .read(move |db| {
            let raw: Option<String> = db
                .exec_first("SELECT data FROM snapshots WHERE slug=?", (query.slug,))
                .map_err(db_error)?;
            Ok(Json(parse(&raw.ok_or_else(ApiError::not_found)?)?))
        })
        .await
}

fn validate_schema_change(
    db: &mut impl Queryable,
    template: Option<&Template>,
    component: Option<&Component>,
) -> Result<()> {
    let mut pages: Vec<Page> = load_all(db, "pages")?;
    pages.retain(|page| {
        template.is_some_and(|t| t.id == page.template_id)
            || component.is_some_and(|c| page.blocks.iter().any(|b| b.component_id == c.id))
    });
    let mut definitions = Definitions::load(db, &pages)?;
    if let Some(template) = template {
        definitions
            .templates
            .insert(template.id.clone(), template.clone());
    }
    if let Some(component) = component {
        definitions
            .components
            .insert(component.id.clone(), component.clone());
    }
    for page in &pages {
        definitions.validate(page)?;
    }
    Ok(())
}
fn validate_page(db: &mut impl Queryable, page: &Page) -> Result<()> {
    Definitions::load(db, std::slice::from_ref(page))?.validate(page)
}

// One operation's definitions, shared by validation and snapshot construction.
// Never retained across transactions, so schema changes need no cache invalidation.
struct Definitions {
    templates: HashMap<String, Template>,
    components: HashMap<String, Component>,
}

impl Definitions {
    fn load(db: &mut impl Queryable, pages: &[Page]) -> Result<Self> {
        let mut templates = HashMap::new();
        let mut components = HashMap::new();
        for page in pages {
            if !templates.contains_key(&page.template_id) {
                templates.insert(
                    page.template_id.clone(),
                    require_template(db, &page.template_id)?,
                );
            }
            for block in &page.blocks {
                if !components.contains_key(&block.component_id) {
                    components.insert(
                        block.component_id.clone(),
                        require_component(db, &block.component_id)?,
                    );
                }
            }
        }
        Ok(Self {
            templates,
            components,
        })
    }

    fn validate(&self, page: &Page) -> Result<()> {
        if page.title.trim().is_empty() {
            return Err(ApiError::bad("title is required"));
        }
        validate_slug(&page.slug)?;
        let mut aliases = HashSet::new();
        for alias in &page.aliases {
            validate_slug(alias)?;
            if alias == &page.slug || !aliases.insert(alias) {
                return Err(ApiError::bad(
                    "aliases must be unique and different from the page path",
                ));
            }
        }
        let template = self
            .templates
            .get(&page.template_id)
            .ok_or_else(ApiError::not_found)?;
        let mut block_ids = HashSet::new();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for block in &page.blocks {
            if block.id.trim().is_empty() || !block_ids.insert(&block.id) {
                return Err(ApiError::bad("block ids must be non-empty and unique"));
            }
            let region = template
                .regions
                .iter()
                .find(|r| r.name == block.region)
                .ok_or_else(|| ApiError::bad(format!("unknown region: {}", block.region)))?;
            if !region.allowed_components.contains(&block.component_id) {
                return Err(ApiError::bad(format!(
                    "component {} is not allowed in region {}",
                    block.component_id, block.region
                )));
            }
            *counts.entry(&block.region).or_default() += 1;
            let component = self
                .components
                .get(&block.component_id)
                .ok_or_else(ApiError::not_found)?;
            validate_block(block, component)?;
        }
        for region in &template.regions {
            if counts.get(region.name.as_str()).copied().unwrap_or(0) > region.max_components {
                return Err(ApiError::bad(format!(
                    "region {} exceeds maximum",
                    region.name
                )));
            }
        }
        Ok(())
    }
}
fn validate_block(block: &Block, component: &Component) -> Result<()> {
    for key in block.fields.keys() {
        if !component.fields.iter().any(|f| &f.name == key) {
            return Err(ApiError::bad(format!("unknown field: {key}")));
        }
    }
    for field in &component.fields {
        let value = block
            .fields
            .get(&field.name)
            .map(|s| s.trim())
            .unwrap_or("");
        if field.required && value.is_empty() {
            return Err(ApiError::bad(format!("field {} is required", field.name)));
        }
        if matches!(field.kind, FieldKind::Url) && !value.is_empty() {
            validate_url(value)?;
        }
    }
    Ok(())
}
fn validate_component(item: &Component) -> Result<()> {
    if item.name.trim().is_empty() {
        return Err(ApiError::bad("component name is required"));
    }
    let mut names = HashSet::new();
    for field in &item.fields {
        if field.name.trim().is_empty()
            || field.label.trim().is_empty()
            || !names.insert(&field.name)
        {
            return Err(ApiError::bad("field names must be non-empty and unique"));
        }
    }
    Ok(())
}
fn validate_template(db: &mut impl Queryable, item: &Template) -> Result<()> {
    if item.name.trim().is_empty() {
        return Err(ApiError::bad("template name is required"));
    }
    let mut names = HashSet::new();
    for region in &item.regions {
        if region.name.trim().is_empty() || !names.insert(&region.name) {
            return Err(ApiError::bad("region names must be non-empty and unique"));
        }
        if region.max_components == 0 {
            return Err(ApiError::bad("region maximum must be positive"));
        }
        for id in &region.allowed_components {
            require_component(db, id)?;
        }
    }
    Ok(())
}
fn validate_route_paths(db: &mut impl Queryable, pages: &[Page]) -> Result<()> {
    let mut paths = HashMap::new();
    for page in pages {
        for path in std::iter::once(&page.slug).chain(&page.aliases) {
            if paths.insert(path.as_str(), page.id.as_str()).is_some() {
                return Err(ApiError::conflict("a page or alias already uses this path"));
            }
        }
    }
    let published_aliases: Vec<(String, String)> = db
        .query("SELECT slug,page_id FROM route_aliases")
        .map_err(db_error)?;
    for (path, owner) in published_aliases {
        if paths.get(path.as_str()).is_some_and(|id| *id != owner) {
            return Err(ApiError::conflict(
                "a published alias already uses this path",
            ));
        }
    }
    let snapshots: Vec<(String, String)> = db
        .query("SELECT slug,page_id FROM snapshots")
        .map_err(db_error)?;
    for page in pages {
        if snapshots
            .iter()
            .any(|(path, owner)| owner != &page.id && page.aliases.contains(path))
        {
            return Err(ApiError::conflict(
                "a published page already uses this alias path",
            ));
        }
    }
    Ok(())
}

fn is_descendant(path: &str, parent: &str) -> bool {
    path != parent
        && (parent == "/"
            || path
                .strip_prefix(parent)
                .is_some_and(|rest| rest.starts_with('/')))
}

fn validate_slug(value: &str) -> Result<()> {
    if value.len() > 2048 {
        return Err(ApiError::bad("slug must be at most 2048 bytes"));
    }
    if value == "/" {
        Ok(())
    } else {
        let segments: Vec<_> = value.strip_prefix('/').unwrap_or("").split('/').collect();
        let valid = !segments.is_empty()
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            })
            && !matches!(
                segments[0].to_ascii_lowercase().as_str(),
                "admin" | "api" | "health" | "assets"
            );
        if valid {
            Ok(())
        } else {
            Err(ApiError::bad("slug must be a safe canonical path"))
        }
    }
}
fn validate_url(value: &str) -> Result<()> {
    if value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace() || character == '\\')
    {
        return Err(ApiError::bad(
            "URL must be a relative path or an http/https URL",
        ));
    }
    if value.starts_with('/') && !value.starts_with("//") {
        return Ok(());
    }
    let parsed = value.parse::<axum::http::Uri>().ok();
    if parsed.as_ref().is_some_and(|uri| {
        matches!(uri.scheme_str(), Some("http" | "https"))
            && uri
                .authority()
                .is_some_and(|authority| !authority.host().is_empty())
    }) {
        Ok(())
    } else {
        Err(ApiError::bad(
            "URL must be a relative path or an http/https URL",
        ))
    }
}
fn require_page(db: &mut impl Queryable, id: &str) -> Result<Page> {
    load_one(db, "pages", id)
}
fn require_template(db: &mut impl Queryable, id: &str) -> Result<Template> {
    load_one(db, "templates", id)
}
fn require_component(db: &mut impl Queryable, id: &str) -> Result<Component> {
    load_one(db, "components", id)
}
fn load_one<T: serde::de::DeserializeOwned>(
    db: &mut impl Queryable,
    table: &str,
    id: &str,
) -> Result<T> {
    let raw: Option<String> = db
        .exec_first(format!("SELECT data FROM {table} WHERE id=?"), (id,))
        .map_err(db_error)?;
    parse(&raw.ok_or_else(ApiError::not_found)?)
}
fn insert_page(db: &mut impl Queryable, page: &Page) -> Result<()> {
    db.exec_drop(
        "INSERT INTO pages VALUES(?,?,?)",
        (&page.id, &page.slug, json(page)?),
    )
    .map_err(constraint_error)?;
    Ok(())
}
fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "serialization failed".into(),
        )
    })
}
fn parse<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored data is invalid".into(),
        )
    })
}
fn db_error(error: mysql::Error) -> ApiError {
    // Server messages can contain submitted content. Log codes, never SQL or values.
    match error {
        mysql::Error::MySqlError(error) => eprintln!(
            "MySQL operation failed: code={}, state={}",
            error.code, error.state
        ),
        mysql::Error::IoError(error) => eprintln!(
            "MySQL I/O failed: kind={:?}, os_code={:?}",
            error.kind(),
            error.raw_os_error()
        ),
        _ => eprintln!("MySQL operation failed: driver or protocol error"),
    }
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "database error".into())
}
fn constraint_error(error: mysql::Error) -> ApiError {
    if matches!(&error, mysql::Error::MySqlError(error) if error.code == 1062) {
        ApiError::conflict("id or slug already exists")
    } else {
        db_error(error)
    }
}
fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
        .replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn cancelled_requests_keep_their_database_slot_until_work_finishes() {
        // An empty pool is sufficient: this test never connects to a database.
        let state = AppState {
            db: Pool::new(Opts::from_url("mysql://127.0.0.1/test?pool_min=0&pool_max=1").unwrap())
                .unwrap(),
            auth: Arc::new(auth::Auth::password("test".into(), false).unwrap()),
            database_slots: Arc::new(Semaphore::new(1)),
            origin: auth::OriginPolicy::from_env(false).unwrap(),
            headless: headless::ApiKeys::default(),
        };
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, wait) = std::sync::mpsc::channel();
        let running = {
            let state = state.clone();
            tokio::spawn(async move {
                state
                    .blocking(move |_| {
                        started.send(()).unwrap();
                        wait.recv_timeout(Duration::from_secs(5)).unwrap();
                        Ok(())
                    })
                    .await
            })
        };
        ready.await.unwrap();
        assert_eq!(
            state.blocking(|_| Ok(())).await.unwrap_err().0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        assert_eq!(
            state.blocking(|_| Ok(())).await.unwrap_err().0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        finish.send(()).unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(2), state.database_slots.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(state.blocking(|_| Ok(42)).await.unwrap(), 42);
    }
}
