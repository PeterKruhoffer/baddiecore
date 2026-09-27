use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use mysql::{Opts, Pool, Transaction, TxOpts, prelude::Queryable};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};
use uuid::Uuid;

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
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub page: Page,
    pub template: Template,
    pub components: Vec<Component>,
}

#[derive(Clone)]
pub struct AppState {
    db: Pool,
    password: Arc<String>,
    sessions: Arc<Mutex<SessionStore>>,
    secure_cookie: bool,
}

const SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_SESSIONS: usize = 128;

#[derive(Default)]
struct SessionStore {
    tokens: HashMap<String, Instant>,
}

impl SessionStore {
    fn insert(&mut self, token: String, now: Instant) {
        self.prune(now);
        if self.tokens.len() >= MAX_SESSIONS
            && let Some(oldest) = self
                .tokens
                .iter()
                .min_by_key(|(_, expires)| *expires)
                .map(|(token, _)| token.clone())
        {
            self.tokens.remove(&oldest);
        }
        self.tokens.insert(token, now + SESSION_LIFETIME);
    }

    fn contains(&mut self, token: &str, now: Instant) -> bool {
        self.prune(now);
        self.tokens.contains_key(token)
    }

    fn remove(&mut self, token: &str) {
        self.tokens.remove(token);
    }

    fn prune(&mut self, now: Instant) {
        self.tokens.retain(|_, expires| *expires > now);
    }
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
        if password.is_empty() {
            return Err("BADDIE_ADMIN_PASSWORD must not be empty".into());
        }
        let opts = Opts::from_url(database_url).map_err(|_| "DATABASE_URL must be a MySQL URL")?;
        let db = Pool::new(opts).map_err(|_| "could not connect to MySQL")?;
        let mut conn = db.get_conn().map_err(|_| "could not connect to MySQL")?;
        for statement in [
            "CREATE TABLE IF NOT EXISTS cms_lock(id INT PRIMARY KEY) ENGINE=InnoDB",
            "INSERT IGNORE INTO cms_lock(id) VALUES(1)",
            "CREATE TABLE IF NOT EXISTS components(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS templates(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS pages(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, slug VARBINARY(2048) NOT NULL UNIQUE, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS snapshots(page_id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, slug VARBINARY(2048) NOT NULL UNIQUE, data LONGTEXT NOT NULL, FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ] {
            conn.query_drop(statement)
                .map_err(|_| "could not initialize MySQL schema")?;
        }
        let state = Self {
            db,
            password: Arc::new(password),
            sessions: Default::default(),
            secure_cookie,
        };
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
                template_id: template.id,
                blocks,
                revision: 1,
                published_revision: None,
            };
            tx.exec_drop(
                "INSERT INTO pages VALUES(?,?,?)",
                (&page.id, &page.slug, json(&page)?),
            )
            .map_err(db_error)?;
            Ok(())
        })
    }

    fn transaction<T>(&self, f: impl FnOnce(&mut Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut conn = self.db.get_conn().map_err(db_error)?;
        let mut tx = conn
            .start_transaction(TxOpts::default())
            .map_err(db_error)?;
        // Serialize validation and writes, including overlapping deployments, just as
        // the original single-connection store did. Errors roll back the whole operation.
        tx.query_drop("SELECT id FROM cms_lock WHERE id=1 FOR UPDATE")
            .map_err(db_error)?;
        let result = f(&mut tx)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }

    async fn run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Transaction<'_>) -> Result<T> + Send + 'static,
    {
        let state = self.clone();
        tokio::task::spawn_blocking(move || state.transaction(f))
            .await
            .map_err(|_| {
                ApiError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database task failed".into(),
                )
            })?
    }
}

pub fn router(state: AppState, static_dir: impl AsRef<Path>) -> Router {
    let admin = Router::new()
        .route("/bootstrap", get(bootstrap))
        .route("/pages", post(create_page))
        .route("/pages/{id}", put(update_page).delete(delete_page))
        .route("/pages/{id}/publish", post(publish_page))
        .route("/templates", post(create_template))
        .route("/templates/{id}", put(update_template))
        .route("/components", post(create_component))
        .route("/components/{id}", put(update_component))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));
    let index = static_dir.as_ref().join("index.html");
    Router::new()
        .route("/health", get(health))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/content", get(content))
        .nest("/api/admin", admin)
        .fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(index)))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<StatusCode> {
    state
        .run(|db| {
            db.query_drop("SELECT 1").map_err(db_error)?;
            Ok(StatusCode::OK)
        })
        .await
}

async fn require_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if request.method() != axum::http::Method::GET && !same_origin(&headers) {
        return ApiError(StatusCode::FORBIDDEN, "origin does not match host".into())
            .into_response();
    }
    let token = cookie_value(&headers, "baddie_session");
    if token
        .as_ref()
        .is_some_and(|t| state.sessions.lock().unwrap().contains(t, Instant::now()))
    {
        next.run(request).await
    } else {
        ApiError(StatusCode::UNAUTHORIZED, "authentication required".into()).into_response()
    }
}
fn same_origin(headers: &HeaderMap) -> bool {
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

#[derive(Deserialize)]
struct Login {
    password: String,
}
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response> {
    if !same_origin(&headers) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "origin does not match host".into(),
        ));
    }
    if input
        .password
        .as_bytes()
        .ct_eq(state.password.as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid password".into(),
        ));
    }
    let token = Uuid::new_v4().to_string();
    state
        .sessions
        .lock()
        .unwrap()
        .insert(token.clone(), Instant::now());
    let cookie = Cookie::build(("baddie_session", token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(state.secure_cookie)
        .build();
    let value = HeaderValue::from_str(&cookie.to_string()).unwrap();
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(header::SET_COOKIE, value);
    Ok(response)
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    if !same_origin(&headers) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "origin does not match host".into(),
        ));
    }
    if let Some(token) = cookie_value(&headers, "baddie_session") {
        state.sessions.lock().unwrap().remove(&token);
    }
    let mut cookie = Cookie::build(("baddie_session", ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(state.secure_cookie)
        .build();
    cookie.make_removal();
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).unwrap(),
    );
    Ok(response)
}

async fn bootstrap(State(state): State<AppState>) -> Result<Json<Bootstrap>> {
    state.run(|db| Ok(Json(load_bootstrap(db)?))).await
}
fn load_bootstrap(db: &mut impl Queryable) -> Result<Bootstrap> {
    Ok(Bootstrap {
        pages: load_all(db, "pages")?,
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
    rows.iter().map(|v| parse(v)).collect()
}

#[derive(Deserialize)]
struct NewPage {
    title: String,
    slug: String,
    template_id: String,
}
async fn create_page(
    State(state): State<AppState>,
    Json(input): Json<NewPage>,
) -> Result<(StatusCode, Json<Page>)> {
    state
        .run(move |db| {
            validate_slug(&input.slug)?;
            require_template(db, &input.template_id)?;
            if input.title.trim().is_empty() {
                return Err(ApiError::bad("title is required"));
            }
            let page = Page {
                id: Uuid::new_v4().to_string(),
                title: input.title,
                slug: input.slug,
                template_id: input.template_id,
                blocks: vec![],
                revision: 1,
                published_revision: None,
            };
            insert_page(db, &page)?;
            Ok((StatusCode::CREATED, Json(page)))
        })
        .await
}
async fn update_page(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(mut page): Json<Page>,
) -> Result<Json<Page>> {
    state
        .run(move |db| {
            if page.id != id {
                return Err(ApiError::bad("page id does not match path"));
            }
            let old = require_page(db, &id)?;
            if page.revision != old.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            page.published_revision = old.published_revision;
            validate_page(db, &page)?;
            page.revision += 1;
            db.exec_drop(
                "UPDATE pages SET slug=?,data=? WHERE id=?",
                (&page.slug, json(&page)?, id),
            )
            .map_err(constraint_error)?;
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
    AxumPath(id): AxumPath<String>,
    Json(input): Json<Revision>,
) -> Result<Json<Page>> {
    state
        .run(move |db| {
            let mut page = require_page(db, &id)?;
            if page.revision != input.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            validate_page(db, &page)?;
            page.published_revision = Some(page.revision);
            let template = require_template(db, &page.template_id)?;
            let components = components_for_page(db, &page)?;
            let snapshot = Content {
                page: page.clone(),
                template,
                components,
            };
            db.exec_drop("UPDATE pages SET data=? WHERE id=?", (json(&page)?, &id))
                .map_err(db_error)?;
            // Delete then insert in the same transaction: MySQL's ON DUPLICATE KEY
            // would also match another page's slug and overwrite its snapshot.
            db.exec_drop("DELETE FROM snapshots WHERE page_id=?", (&id,))
                .map_err(db_error)?;
            db.exec_drop(
                "INSERT INTO snapshots(page_id,slug,data) VALUES(?,?,?)",
                (&id, &page.slug, json(&snapshot)?),
            )
            .map_err(constraint_error)?;
            Ok(Json(page))
        })
        .await
}
async fn delete_page(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run(move |db| {
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
    Json(input): Json<NewTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    state
        .run(move |db| {
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
    AxumPath(id): AxumPath<String>,
    Json(item): Json<Template>,
) -> Result<Json<Template>> {
    state
        .run(move |db| {
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

#[derive(Deserialize)]
struct NewComponent {
    name: String,
    description: String,
    renderer: Renderer,
    fields: Vec<Field>,
}
async fn create_component(
    State(state): State<AppState>,
    Json(input): Json<NewComponent>,
) -> Result<(StatusCode, Json<Component>)> {
    state
        .run(move |db| {
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
    AxumPath(id): AxumPath<String>,
    Json(item): Json<Component>,
) -> Result<Json<Component>> {
    state
        .run(move |db| {
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
        .run(move |db| {
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
    let pages: Vec<Page> = load_all(db, "pages")?;
    for page in pages {
        if template.is_some_and(|t| t.id == page.template_id)
            || component.is_some_and(|c| page.blocks.iter().any(|b| b.component_id == c.id))
        {
            validate_page_with(db, &page, template, component)?;
        }
    }
    Ok(())
}
fn validate_page(db: &mut impl Queryable, page: &Page) -> Result<()> {
    validate_page_with(db, page, None, None)
}
fn validate_page_with(
    db: &mut impl Queryable,
    page: &Page,
    replacement_template: Option<&Template>,
    replacement_component: Option<&Component>,
) -> Result<()> {
    if page.title.trim().is_empty() {
        return Err(ApiError::bad("title is required"));
    }
    validate_slug(&page.slug)?;
    let template = if replacement_template.is_some_and(|t| t.id == page.template_id) {
        replacement_template.unwrap().clone()
    } else {
        require_template(db, &page.template_id)?
    };
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
        let component = if replacement_component.is_some_and(|c| c.id == block.component_id) {
            replacement_component.unwrap().clone()
        } else {
            require_component(db, &block.component_id)?
        };
        validate_block(block, &component)?;
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
fn components_for_page(db: &mut impl Queryable, page: &Page) -> Result<Vec<Component>> {
    let ids: HashSet<&str> = page
        .blocks
        .iter()
        .map(|b| b.component_id.as_str())
        .collect();
    ids.into_iter()
        .map(|id| require_component(db, id))
        .collect()
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
fn db_error(_: mysql::Error) -> ApiError {
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

    #[test]
    fn sessions_expire_and_are_bounded() {
        let start = Instant::now();
        let mut sessions = SessionStore::default();
        sessions.insert("expired".into(), start);
        assert!(sessions.contains("expired", start + SESSION_LIFETIME - Duration::from_secs(1)));
        assert!(!sessions.contains("expired", start + SESSION_LIFETIME));

        for index in 0..=MAX_SESSIONS {
            sessions.insert(index.to_string(), start + Duration::from_secs(index as u64));
        }
        assert_eq!(sessions.tokens.len(), MAX_SESSIONS);
        assert!(!sessions.tokens.contains_key("0"));
    }
}
