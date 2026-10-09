//! Portable zip packages of page drafts, for moving content between installations.
//!
//! A package uses the same layout as a `pull` export: `pages/`, plus the `templates/` and
//! `components/` those pages use. Installing merges pages as drafts and only adds definitions
//! the target lacks; existing definitions stay under Git's control.

use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
};

use axum::{
    Extension, Json,
    body::Bytes,
    extract::{Query, State},
    http::header,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    ApiError, AppState, Result, auth, is_descendant, load_data,
    serialization::{
        self, DefinitionPolicy, Items, MAX_FILE_SIZE, Merged, export_files, export_path,
    },
    validate_slug,
};

const MAX_ENTRIES: usize = 10_000;
const MAX_UNPACKED_SIZE: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
pub(crate) struct ExportQuery {
    path: String,
}

/// Download the page at `path` and every page below it as a zip package.
pub(crate) async fn export(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Query(query): Query<ExportQuery>,
) -> Result<Response> {
    let path = query.path;
    validate_slug(&path)?;
    let filename = package_filename(&path);
    let bytes = state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let mut data = load_data(db, true)?;
            data.pages
                .retain(|page| page.slug == path || is_descendant(&page.slug, &path));
            if data.pages.is_empty() {
                return Err(ApiError::bad("no pages at or below this path"));
            }
            let templates: HashSet<&str> = data
                .pages
                .iter()
                .map(|page| page.template_id.as_str())
                .collect();
            data.templates.retain(|t| templates.contains(t.id.as_str()));
            let components: HashSet<&str> = data
                .templates
                .iter()
                .flat_map(|t| &t.regions)
                .flat_map(|r| &r.allowed_components)
                .chain(
                    data.pages
                        .iter()
                        .flat_map(|p| &p.blocks)
                        .map(|b| &b.component_id),
                )
                .map(String::as_str)
                .collect();
            data.components
                .retain(|c| components.contains(c.id.as_str()));
            let files = export_files(&data.components, &data.templates, &data.pages)
                .map_err(|e| ApiError::bad(e.0))?;
            zip_files(&files)
        })
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

/// Install a zip package. Pages become drafts; nothing is published.
pub(crate) async fn install(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    body: Bytes,
) -> Result<Json<Merged>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            // Unpack only after the admin check, so others cannot make the server inflate archives.
            let items = unzip(&body).map_err(|e| ApiError::bad(e.0))?;
            if items.pages() == 0 {
                return Err(ApiError::bad("package contains no pages"));
            }
            Ok(Json(serialization::merge_items(
                db,
                items,
                DefinitionPolicy::AddMissing,
            )?))
        })
        .await
}

fn package_filename(path: &str) -> String {
    let name: String = path
        .trim_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if name.is_empty() {
        "baddiecore-site.zip".into()
    } else {
        format!("baddiecore-{name}.zip")
    }
}

fn zip_files(files: &[serialization::ExportFile<'_>]) -> Result<Vec<u8>> {
    let failed = |_| ApiError::bad("could not write package");
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (group, id, bytes) in files {
        zip.start_file(export_path(group, id), options)
            .map_err(failed)?;
        zip.write_all(bytes)
            .map_err(|_| ApiError::bad("could not write package"))?;
    }
    Ok(zip.finish().map_err(failed)?.into_inner())
}

fn unzip(bytes: &[u8]) -> std::result::Result<Items, serialization::Error> {
    let error = |message: &str| serialization::Error(message.into());
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| error("not a valid zip file"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(error("package has too many files"));
    }
    let mut items = Items::new();
    let mut unpacked = 0;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| error("could not read package"))?;
        let name = file
            .name()
            .map_err(|_| error("package has an unreadable file name"))?
            .into_owned();
        // Tolerate directory entries and the litter macOS adds when re-zipping.
        if file.is_dir() || name.starts_with("__MACOSX/") || name.ends_with(".DS_Store") {
            continue;
        }
        if file.is_symlink() {
            return Err(serialization::Error(format!("unexpected file: {name}")));
        }
        // Never trust declared sizes; read at most one byte past each limit.
        let mut data = Vec::new();
        (&mut file)
            .take(MAX_FILE_SIZE + 1)
            .read_to_end(&mut data)
            .map_err(|_| error("could not read package"))?;
        if data.len() as u64 > MAX_FILE_SIZE {
            return Err(serialization::Error(format!("file too large: {name}")));
        }
        unpacked += data.len() as u64;
        if unpacked > MAX_UNPACKED_SIZE {
            return Err(error("package is too large"));
        }
        items.add(&name, &data)?;
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(files: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    const PAGE: &str = "schema_version: 1\nkind: page\ndata:\n  id: about\n  title: About\n  slug: /about\n  template_id: homepage\n  blocks: []\n";

    #[test]
    fn filenames_follow_the_exported_path() {
        assert_eq!(package_filename("/"), "baddiecore-site.zip");
        assert_eq!(package_filename("/news/2026"), "baddiecore-news-2026.zip");
    }

    #[test]
    fn unpacks_pages_and_ignores_macos_litter() {
        let items = unzip(&package(&[
            ("pages/about.yaml", PAGE),
            ("__MACOSX/pages/._about.yaml", "junk"),
            ("pages/.DS_Store", "junk"),
        ]))
        .unwrap();
        assert_eq!(items.pages(), 1);
    }

    #[test]
    fn rejects_unexpected_paths_and_mismatched_ids() {
        for name in [
            "../pages/about.yaml",
            "about.yaml",
            "web/about.yaml",
            "pages/other.yaml",
        ] {
            assert!(unzip(&package(&[(name, PAGE)])).is_err(), "{name}");
        }
        assert!(unzip(b"not a zip").is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for name in ["pages/about.yaml", "pages/about.yaml"] {
            if zip.start_file(name, SimpleFileOptions::default()).is_err() {
                return; // The writer itself refuses duplicates; nothing more to check.
            }
            zip.write_all(PAGE.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        assert!(unzip(&bytes).is_err());
    }

    #[test]
    fn rejects_oversized_files() {
        let big = "x".repeat(MAX_FILE_SIZE as usize + 1);
        let error = unzip(&package(&[("pages/about.yaml", &big)]))
            .err()
            .unwrap();
        assert!(error.0.contains("too large"));
    }
}
