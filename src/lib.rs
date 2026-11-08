use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use axum::{
    Extension, Router,
    extract::{DefaultBodyLimit, FromRequest, Request, State, rejection::JsonRejection},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use mysql::{Pool, prelude::Queryable};
use serde::{Deserialize, Serialize};
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};

use db::{Database, TransactionMode, db_error, load_all, load_data, migrate};
use definitions::{
    create_component, create_template, delete_component, delete_template, update_component,
    update_template,
};
pub use ids::{ComponentId, PageId, SitePath, TemplateId};
use pages::{
    content, create_page, delete_page, publish_page, unpublish_block, unpublish_page, update_page,
};
use seed::seed;

pub mod auth;
mod db;
mod definitions;
pub mod headless;
pub mod ids;
pub mod media;
mod package;
mod pages;
pub mod richtext;
pub mod schedule;
mod seed;
pub mod serialization;
mod store;
pub mod workflow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub kind: FieldKind,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub richtext: Option<richtext::Config>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Textarea,
    Url,
    Richtext,
    /// A `MediaId`, or empty.
    Media,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub id: ComponentId,
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
    pub allowed_components: Vec<ComponentId>,
    pub max_components: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub id: TemplateId,
    pub name: String,
    pub description: String,
    pub regions: Vec<Region>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: String,
    pub component_id: ComponentId,
    pub region: String,
    pub fields: HashMap<String, String>,
    /// Kept in the draft but left out of every publication. See workflow::current_content.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Page {
    pub id: PageId,
    pub title: String,
    pub slug: SitePath,
    #[serde(default)]
    pub aliases: Vec<SitePath>,
    pub template_id: TemplateId,
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
    pub schedules: Vec<schedule::Schedule>,
    pub media: Vec<media::Media>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub page: Page,
    pub template: Template,
    pub components: Vec<Component>,
    /// Metadata of the media the page's blocks reference, sorted by ID.
    #[serde(default)]
    pub media: Vec<media::Media>,
}

#[derive(Clone)]
pub struct AppState {
    db: Database,
    auth: Arc<dyn auth::AuthProvider>,
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

/// JSON body extractor and response. Use it instead of `axum::Json`: a body that
/// fails to parse, including an ID or path that fails its type's `parse`, becomes
/// a 400 with the contract's `{error}` shape rather than axum's plain-text 422.
#[derive(Debug)]
pub(crate) struct Json<T>(pub T);
impl<T: serde::de::DeserializeOwned, S: Send + Sync> FromRequest<S> for Json<T> {
    type Rejection = ApiError;
    async fn from_request(request: Request, state: &S) -> Result<Self> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                let status = match rejection {
                    JsonRejection::JsonDataError(_) | JsonRejection::JsonSyntaxError(_) => {
                        StatusCode::BAD_REQUEST
                    }
                    _ => rejection.status(),
                };
                Err(ApiError(status, rejection.body_text()))
            }
        }
    }
}
impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl AppState {
    /// Opens the CMS with built-in local accounts. When no administrator can
    /// sign in, `admin_password` creates or repairs the `admin` account.
    pub fn open(
        database_url: &str,
        admin_password: Option<String>,
        secure_cookie: bool,
    ) -> std::result::Result<Self, String> {
        let pool = db::connect(database_url, db::CONNECTIONS)?;
        prepare(&pool)?;
        db::transaction(&pool, TransactionMode::Commit, |tx| {
            workflow::bootstrap_admin(tx, admin_password.as_deref())
        })
        .map_err(|e| e.1)?;
        let db = Database::new(pool);
        let auth = auth::Auth::new(db.clone(), secure_cookie)?;
        Self::new(db, Arc::new(auth))
    }

    pub fn open_with_auth(
        database_url: &str,
        auth: Arc<dyn auth::AuthProvider>,
    ) -> std::result::Result<Self, String> {
        let pool = db::connect(database_url, db::CONNECTIONS)?;
        prepare(&pool)?;
        Self::new(Database::new(pool), auth)
    }

    fn new(db: Database, auth: Arc<dyn auth::AuthProvider>) -> std::result::Result<Self, String> {
        Ok(Self {
            db,
            origin: auth.origin_policy()?,
            auth,
            headless: headless::ApiKeys::default(),
        })
    }
}

/// Applies pending migrations, then creates the organization record and seed content when missing.
fn prepare(db: &Pool) -> std::result::Result<(), String> {
    migrate(db).map_err(|_| "could not migrate MySQL schema")?;
    db::transaction(db, TransactionMode::Commit, |tx| {
        // One-time backfill for databases created before page_paths existed.
        if tx
            .query_first::<i64, _>("SELECT 1 FROM page_paths LIMIT 1")
            .map_err(db_error)?
            .is_none()
        {
            store::backfill_paths(tx)?;
        }
        workflow::initialize(tx)
    })
    .map_err(|e| e.1)?;
    seed(db).map_err(|e| e.1)
}

/// Makes `id` an administrator and sets its password, for operator recovery.
pub fn reset_admin(
    database_url: &str,
    id: &str,
    password: &str,
) -> std::result::Result<(), String> {
    let db = db::connect(database_url, 1)?;
    prepare(&db)?;
    let hash = auth::hash_password_now(password).map_err(|e| e.1)?;
    db::transaction(&db, TransactionMode::Commit, |tx| {
        workflow::reset_admin(tx, id, &hash)
    })
    .map_err(|e| e.1)
}

pub fn router(state: AppState, static_dir: impl AsRef<Path>) -> Router {
    let admin = Router::new()
        .route("/bootstrap", get(bootstrap))
        .route("/pages", post(create_page))
        .route("/pages/{id}", put(update_page).delete(delete_page))
        .route("/pages/{id}/publish", post(publish_page))
        .route("/pages/{id}/unpublish", post(unpublish_page))
        .route(
            "/pages/{id}/blocks/{block_id}/unpublish",
            post(unpublish_block),
        )
        .route(
            "/pages/{id}/schedule",
            put(schedule::schedule_page).delete(schedule::cancel),
        )
        .route("/pages/{id}/submit", post(workflow::submit))
        .route("/reviews/{id}", post(workflow::decide))
        .route(
            "/organization",
            get(workflow::get_organization).put(workflow::save_organization),
        )
        .route("/members/{id}/password", put(workflow::set_password))
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
        .route("/media/{id}", put(media::update).delete(media::delete))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));
    // Merged after the 1 MiB layer below, which would otherwise cap uploads too.
    let upload = Router::new()
        .route("/api/admin/media", post(media::upload))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
        .layer(DefaultBodyLimit::max(media::MAX_BYTES))
        .with_state(state.clone());
    let index = static_dir.as_ref().join("index.html");
    Router::new()
        .route("/health", get(health))
        .route("/api/content", get(content))
        .route("/media/{id}", get(media::serve))
        .nest("/api/headless", headless::routes(state.clone()))
        .nest("/api/admin", admin)
        .fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(index)))
        .with_state(state.clone())
        .merge(state.auth.routes())
        .layer(middleware::from_fn_with_state(state, redirect_alias))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .merge(upload)
}

async fn redirect_alias(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    if matches!(*request.method(), Method::GET | Method::HEAD) && SitePath::parse(&path).is_some() {
        let target = state
            .db
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
        .db
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
            let schedules = schedule::load(db)?
                .into_iter()
                .filter(|schedule| visible.contains(schedule.page_id.as_str()))
                .collect();
            Ok(Json(Bootstrap {
                pages: data.pages,
                templates: data.templates,
                components: data.components,
                access,
                reviews,
                schedules,
                media: media::list(db)?,
            }))
        })
        .await
}

/// Parses an ID from a URL path segment. An invalid ID names nothing, so it is a 404.
fn path_id<T: std::str::FromStr>(raw: &str) -> Result<T> {
    raw.parse().map_err(|_| ApiError::not_found())
}
