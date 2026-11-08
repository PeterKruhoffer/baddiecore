use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use axum::{
    Extension, Router,
    extract::{
        DefaultBodyLimit, FromRequest, Path as AxumPath, Query, Request, State,
        rejection::JsonRejection,
    },
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

use definitions::{
    create_component, create_template, delete_component, delete_template, update_component,
    update_template,
};

use db::{
    Database, Locked, TransactionMode, constraint_error, db_error, json, load_all, load_data,
    migrate, parse, require_page,
};
pub use ids::{ComponentId, PageId, SitePath, TemplateId};
use seed::seed;

pub mod auth;
mod db;
mod definitions;
pub mod headless;
pub mod ids;
pub mod media;
mod package;
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

#[derive(Deserialize)]
struct NewPage {
    title: String,
    slug: SitePath,
    template_id: TemplateId,
}
async fn create_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(input): Json<NewPage>,
) -> Result<(StatusCode, Json<Page>)> {
    state
        .run_as(editor, move |db, access| {
            access.page(&input.slug)?;
            let page = Page {
                id: PageId::new(),
                title: input.title,
                slug: input.slug,
                aliases: vec![],
                template_id: input.template_id,
                blocks: vec![],
                revision: 1,
                published_revision: None,
            };
            store::save_pages(db, std::slice::from_ref(&page))?;
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
            if page.id != id.as_str() {
                return Err(ApiError::bad("page id does not match path"));
            }
            let old = require_page(db, &page.id)?;
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
            page.revision += 1;
            let mut changed = Vec::new();
            if page.slug != old.slug {
                if old.slug == SitePath::root() {
                    return Err(ApiError::bad("the root page cannot be moved"));
                }
                if page.slug.is_descendant_of(&old.slug) {
                    return Err(ApiError::bad("a page cannot move into its own subtree"));
                }
                let mut pages: Vec<Page> = load_all(db, "pages")?;
                for child in &mut pages {
                    if child.id == page.id {
                        *child = page.clone();
                    } else if child.slug.is_descendant_of(&old.slug) {
                        access.page(&child.slug)?;
                        child.slug = child.slug.rebase(&old.slug, &page.slug).ok_or_else(|| {
                            ApiError::bad(format!(
                                "moving {} would give it an invalid path",
                                child.slug
                            ))
                        })?;
                        child.revision += 1;
                        access.page(&child.slug)?;
                        changed.push(child.clone());
                    }
                }
                let mut paths = HashSet::new();
                if pages.iter().any(|p| !paths.insert(&p.slug)) {
                    return Err(ApiError::conflict("a destination path already exists"));
                }
            }
            changed.push(page.clone());
            store::save_pages(db, &changed)?;
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
            let page = require_page(db, &path_id(&id)?)?;
            if page.revision != input.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            let snapshot = workflow::current_content(db, page)?;
            Ok(Json(publish(db, snapshot)?))
        })
        .await
}
/// Takes the page and its aliases off the live site and drops any pending
/// schedule. The draft and child pages are untouched.
async fn unpublish_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<Page>> {
    state
        .run_as(editor, move |db, access| {
            let id: PageId = path_id(&id)?;
            let page = require_page(db, &id)?;
            access.page(&page.slug)?;
            let live: Option<String> = db
                .exec_first("SELECT data FROM snapshots WHERE page_id=?", (&id,))
                .map_err(db_error)?;
            if let Some(live) = live {
                access.page(&parse::<Content>(&live)?.page.slug)?;
            }
            for table in ["snapshots", "route_aliases", "scheduled_publications"] {
                db.exec_drop(format!("DELETE FROM {table} WHERE page_id=?"), (&id,))
                    .map_err(db_error)?;
            }
            Ok(Json(store::mark_published(db, &id, None)?))
        })
        .await
}
/// Hides one block in the draft and removes it from the live and scheduled snapshots.
async fn unpublish_block(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath((id, block_id)): AxumPath<(String, String)>,
    Json(input): Json<Revision>,
) -> Result<Json<Page>> {
    state
        .run_as(editor, move |db, access| {
            let id: PageId = path_id(&id)?;
            let mut page = require_page(db, &id)?;
            access.page(&page.slug)?;
            let live: Option<String> = db
                .exec_first("SELECT data FROM snapshots WHERE page_id=?", (&id,))
                .map_err(db_error)?;
            let live: Option<Content> = live.map(|data| parse(&data)).transpose()?;
            if let Some(live) = &live {
                access.page(&live.page.slug)?;
            }
            if page.revision != input.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            page.blocks
                .iter_mut()
                .find(|block| block.id == block_id)
                .ok_or_else(ApiError::not_found)?
                .hidden = true;
            let in_sync = page.published_revision == Some(page.revision);
            page.revision += 1;
            // The live page still matches the draft minus hidden blocks, so keep it "Published".
            if in_sync {
                page.published_revision = Some(page.revision);
            }
            store::save_pages(db, std::slice::from_ref(&page))?;
            if let Some(mut live) = live {
                remove_block(&mut live, &block_id);
                if in_sync {
                    live.page.revision = page.revision;
                    live.page.published_revision = Some(page.revision);
                }
                db.exec_drop(
                    "UPDATE snapshots SET data=? WHERE page_id=?",
                    (json(&live)?, &id),
                )
                .map_err(db_error)?;
            }
            let scheduled: Option<String> = db
                .exec_first(
                    "SELECT data FROM scheduled_publications WHERE page_id=?",
                    (&id,),
                )
                .map_err(db_error)?;
            if let Some(data) = scheduled {
                let mut scheduled: Content = parse(&data)?;
                remove_block(&mut scheduled, &block_id);
                db.exec_drop(
                    "UPDATE scheduled_publications SET data=? WHERE page_id=?",
                    (json(&scheduled)?, &id),
                )
                .map_err(db_error)?;
            }
            Ok(Json(page))
        })
        .await
}
/// Removes a block from a snapshot along with the components and media only it used.
fn remove_block(content: &mut Content, block_id: &str) {
    content.page.blocks.retain(|block| block.id != block_id);
    let components: HashMap<ComponentId, Component> = content
        .components
        .iter()
        .filter(|c| content.page.blocks.iter().any(|b| b.component_id == c.id))
        .map(|c| (c.id.clone(), c.clone()))
        .collect();
    let media: HashSet<media::MediaId> = media::referenced(&content.page, &components).collect();
    content
        .components
        .retain(|c| components.contains_key(&c.id));
    content.media.retain(|m| media.contains(&m.id));
}
/// Rejects a publication whose path or aliases another page's publication uses.
fn check_published_paths(db: &mut impl Queryable, page: &Page) -> Result<()> {
    let occupied: Vec<String> = db
        .exec(
            "SELECT slug FROM snapshots WHERE page_id<>? UNION ALL SELECT slug FROM route_aliases WHERE page_id<>?",
            (&page.id, &page.id),
        )
        .map_err(db_error)?;
    if occupied
        .iter()
        .any(|path| page.slug == path.as_str() || page.aliases.iter().any(|a| a == path.as_str()))
    {
        return Err(ApiError::conflict("a published path already exists"));
    }
    // A scheduled snapshot may hold paths its draft has since released. Another
    // page's draft may have claimed them; another draft's canonical path may
    // still match this page's published path, as after a move.
    for path in std::iter::once(&page.slug).chain(&page.aliases) {
        let owner_slug_matches: Option<bool> = db
            .exec_first(
                "SELECT p.slug=pp.path FROM page_paths pp JOIN pages p ON p.id=pp.page_id WHERE pp.path=? AND pp.page_id<>?",
                (path, &page.id),
            )
            .map_err(db_error)?;
        if owner_slug_matches.is_some_and(|canonical| path != &page.slug || !canonical) {
            return Err(ApiError::conflict(
                "another page's draft now uses this path",
            ));
        }
    }
    Ok(())
}
// Callers build and validate the snapshot within this same locked transaction.
// Returns the current draft, which a scheduled snapshot may predate.
fn publish(db: &mut Locked<'_>, mut snapshot: Content) -> Result<Page> {
    snapshot.page.published_revision = Some(snapshot.page.revision);
    let page = &snapshot.page;
    check_published_paths(db, page)?;
    let draft = store::mark_published(db, &page.id, Some(page.revision))?;
    // A newer publication supersedes any pending schedule for this page.
    db.exec_drop(
        "DELETE FROM scheduled_publications WHERE page_id=?",
        (&page.id,),
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
    Ok(draft)
}
async fn delete_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            store::delete_page(db, &path_id(&id)?)?;
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
    let slug: SitePath = query.slug.parse().map_err(ApiError::bad)?;
    state
        .db
        .read(move |db| {
            let raw: Option<String> = db
                .exec_first("SELECT data FROM snapshots WHERE slug=?", (slug,))
                .map_err(db_error)?;
            Ok(Json(parse(&raw.ok_or_else(ApiError::not_found)?)?))
        })
        .await
}

/// Parses an ID from a URL path segment. An invalid ID names nothing, so it is a 404.
fn path_id<T: std::str::FromStr>(raw: &str) -> Result<T> {
    raw.parse().map_err(|_| ApiError::not_found())
}
