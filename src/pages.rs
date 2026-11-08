//! Page drafts and publication: the admin page handlers, `publish` and its
//! published-path check, unpublishing a page or block, and the public content
//! endpoint that serves snapshots.
use std::collections::{HashMap, HashSet};

use axum::{
    Extension,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
};
use mysql::prelude::Queryable;
use serde::Deserialize;

use crate::{
    ApiError, AppState, Component, ComponentId, Content, Json, Page, PageId, Result, SitePath,
    TemplateId, auth,
    db::{Locked, constraint_error, db_error, json, load_all, parse, require_page},
    media, path_id, store, workflow,
};

#[derive(Deserialize)]
pub(crate) struct NewPage {
    title: String,
    slug: SitePath,
    template_id: TemplateId,
}
pub(crate) async fn create_page(
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
pub(crate) async fn update_page(
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
pub(crate) struct Revision {
    revision: i64,
}
pub(crate) async fn publish_page(
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
pub(crate) async fn unpublish_page(
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
pub(crate) async fn unpublish_block(
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
pub(crate) fn check_published_paths(db: &mut impl Queryable, page: &Page) -> Result<()> {
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
pub(crate) fn publish(db: &mut Locked<'_>, mut snapshot: Content) -> Result<Page> {
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
pub(crate) async fn delete_page(
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
pub(crate) struct ContentQuery {
    slug: String,
}
pub(crate) async fn content(
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
