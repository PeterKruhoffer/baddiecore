//! The only module that writes `pages` rows. Every draft write runs the full
//! page validation and path claims here, so a new write path cannot skip one.
//! `tests/store_boundary.rs` rejects `pages` SQL writes anywhere else.

use mysql::prelude::Queryable;

use crate::{
    ApiError, Definitions, Page, Result, constraint_error, db::Locked, db_error, is_descendant,
    json, load_all, require_page,
};

/// Validates `pages` against the stored definitions, then inserts or updates each
/// draft and claims its paths. Pages may swap or move onto each other's paths.
pub(crate) fn save_pages(db: &mut Locked<'_>, pages: &[Page]) -> Result<()> {
    let definitions = Definitions::load(db, pages)?;
    for page in pages {
        definitions.validate(page)?;
    }
    let mut stored = Vec::with_capacity(pages.len());
    for page in pages {
        let slug: Option<String> = db
            .exec_first("SELECT slug FROM pages WHERE id=?", (&page.id,))
            .map_err(db_error)?;
        stored.push(slug);
    }
    // Temporary non-public paths avoid transient unique-key collisions. Snapshots remain untouched.
    for (page, slug) in pages.iter().zip(&stored) {
        if slug.as_ref().is_some_and(|slug| slug != &page.slug) {
            db.exec_drop(
                "UPDATE pages SET slug=? WHERE id=?",
                (format!("__move_{}", page.id), &page.id),
            )
            .map_err(db_error)?;
        }
    }
    for (page, slug) in pages.iter().zip(&stored) {
        let statement = if slug.is_some() {
            "UPDATE pages SET slug=?,data=? WHERE id=?"
        } else {
            "INSERT INTO pages(slug,data,id) VALUES(?,?,?)"
        };
        db.exec_drop(statement, (&page.slug, json(page)?, &page.id))
            .map_err(constraint_error)?;
    }
    claim_paths(db, pages)
}

/// Records that the draft's `revision` is live. Leaves its content and paths alone.
pub(crate) fn mark_published(db: &mut Locked<'_>, id: &str, revision: i64) -> Result<Page> {
    let mut draft = require_page(db, id)?;
    draft.published_revision = Some(revision);
    db.exec_drop("UPDATE pages SET data=? WHERE id=?", (json(&draft)?, id))
        .map_err(db_error)?;
    Ok(draft)
}

pub(crate) fn delete_page(db: &mut Locked<'_>, id: &str) -> Result<()> {
    let page = require_page(db, id)?;
    let pages: Vec<Page> = load_all(db, "pages")?;
    if pages.iter().any(|p| is_descendant(&p.slug, &page.slug)) {
        return Err(ApiError::conflict("move or delete child pages first"));
    }
    db.exec_drop("DELETE FROM pages WHERE id=?", (id,))
        .map_err(db_error)?;
    if db.affected_rows() == 0 {
        return Err(ApiError::not_found());
    }
    Ok(())
}

/// One-time index of existing drafts for databases created before page_paths existed.
pub(crate) fn backfill_paths(db: &mut Locked<'_>) -> Result<()> {
    let pages: Vec<Page> = load_all(db, "pages")?;
    claim_paths(db, &pages)
}

/// Records the draft slugs and aliases of `pages`, which the caller is writing, in
/// page_paths, after checking them against every other page's draft and published paths.
fn claim_paths(db: &mut Locked<'_>, pages: &[Page]) -> Result<()> {
    for page in pages {
        db.exec_drop("DELETE FROM page_paths WHERE page_id=?", (&page.id,))
            .map_err(db_error)?;
    }
    for page in pages {
        for path in std::iter::once(&page.slug).chain(&page.aliases) {
            db.exec_drop(
                "INSERT INTO page_paths(path,page_id) VALUES(?,?)",
                (path, &page.id),
            )
            .map_err(|error| match &error {
                mysql::Error::MySqlError(e) if e.code == 1062 => {
                    ApiError::conflict("a page or alias already uses this path")
                }
                _ => db_error(error),
            })?;
        }
    }
    for page in pages {
        for path in std::iter::once(&page.slug).chain(&page.aliases) {
            let owner: Option<String> = db
                .exec_first("SELECT page_id FROM route_aliases WHERE slug=?", (path,))
                .map_err(db_error)?;
            if owner.is_some_and(|owner| owner != page.id) {
                return Err(ApiError::conflict(
                    "a published alias already uses this path",
                ));
            }
        }
        for path in &page.aliases {
            let owner: Option<String> = db
                .exec_first("SELECT page_id FROM snapshots WHERE slug=?", (path,))
                .map_err(db_error)?;
            if owner.is_some_and(|owner| owner != page.id) {
                return Err(ApiError::conflict(
                    "a published page already uses this alias path",
                ));
            }
        }
    }
    Ok(())
}
