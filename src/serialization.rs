//! Git-friendly export and import of components and templates as YAML files. Content pages
//! use the same page format, but travel between installations in zip packages instead.
//!
//! `pull` copies database changes into files and `push` copies file changes into the
//! database. Each compares three hashes per export path: the database, the files, and the
//! sync base, which records what both sides held after this database's last pull or push.
//! A side that differs from the base changed since then, deletions included, so neither
//! command reverts the other side's work or resurrects deleted items. When both sides
//! changed, the command stops; `--force` makes the target mirror the source instead.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt, fs,
    io::Write,
    path::Path,
};

use mysql::{Pool, Transaction, prelude::Queryable};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{
    ApiError, Block, Component, Definitions, Page, StoredData, Template, TransactionMode,
    database_transaction, insert_page, json, load_data, validate_component, validate_template,
};

const SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;
/// Marks a directory as an export that has history with this database.
const MANIFEST: &str = "baddiecore.yaml";
/// The groups `pull` and `push` sync. Pages are never synced through Git.
const GROUPS: [&str; 2] = ["components", "templates"];

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        ApiError::bad(e.0)
    }
}

/// Which side changed an export path since the last sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Database,
    Files,
    Both,
}

/// An export path that differs between the database and the files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub side: Side,
    /// `added`, `modified` or `deleted`: what happened on the changed side, or for applied
    /// changes, what happened to the target.
    pub action: &'static str,
}

/// The outcome of a pull or push.
#[derive(Debug, Default)]
pub struct Sync {
    /// Changes copied to the target.
    pub applied: Vec<Change>,
    /// Changes on the target side, left for the opposite command.
    pub pending: Vec<Change>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File<T> {
    schema_version: u32,
    kind: String,
    data: T,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PageDraft {
    id: String,
    title: String,
    slug: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    template_id: String,
    blocks: Vec<BlockDraft>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BlockDraft {
    id: String,
    component_id: String,
    region: String,
    fields: BTreeMap<String, String>,
}

impl From<&Page> for PageDraft {
    fn from(page: &Page) -> Self {
        Self {
            id: page.id.clone(),
            title: page.title.clone(),
            slug: page.slug.clone(),
            aliases: page.aliases.clone(),
            template_id: page.template_id.clone(),
            blocks: page
                .blocks
                .iter()
                .map(|b| BlockDraft {
                    id: b.id.clone(),
                    component_id: b.component_id.clone(),
                    region: b.region.clone(),
                    fields: b
                        .fields
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                })
                .collect(),
        }
    }
}

impl PageDraft {
    fn page(self, revision: i64, published_revision: Option<i64>) -> Page {
        Page {
            id: self.id,
            title: self.title,
            slug: self.slug,
            aliases: self.aliases,
            template_id: self.template_id,
            blocks: self
                .blocks
                .into_iter()
                .map(|b| Block {
                    id: b.id,
                    component_id: b.component_id,
                    region: b.region,
                    fields: b.fields.into_iter().collect(),
                })
                .collect(),
            revision,
            published_revision,
        }
    }
}

fn yaml<T: Serialize>(kind: &str, data: T) -> Result<Vec<u8>, Error> {
    serde_yaml_ng::to_string(&File {
        schema_version: SCHEMA_VERSION,
        kind: kind.into(),
        data,
    })
    .map(|s| s.into_bytes())
    .map_err(|_| Error("could not serialize YAML".into()))
}

fn file_name(id: &str) -> String {
    if !id.is_empty()
        && id.len() <= 200
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        format!("{id}.yaml")
    } else {
        format!("id.{:x}.yaml", Sha256::digest(id.as_bytes()))
    }
}

/// One exported item: its group directory, id and YAML bytes.
pub(crate) type ExportFile<'a> = (&'static str, &'a str, Vec<u8>);

/// Encode items in the export layout, sorted by group and id. Shared by `pull` and packages.
pub(crate) fn export_files<'a>(
    components: &'a [Component],
    templates: &'a [Template],
    pages: &'a [Page],
) -> Result<Vec<ExportFile<'a>>, Error> {
    let mut files = Vec::new();
    for item in components {
        files.push(("components", item.id.as_str(), yaml("component", item)?));
    }
    for item in templates {
        files.push(("templates", item.id.as_str(), yaml("template", item)?));
    }
    for item in pages {
        files.push((
            "pages",
            item.id.as_str(),
            yaml("page", PageDraft::from(item))?,
        ));
    }
    files.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    Ok(files)
}

/// The path of an exported item relative to the export root.
pub(crate) fn export_path(group: &str, id: &str) -> String {
    format!("{group}/{}", file_name(id))
}

/// Hashes keyed by export path, such as `templates/homepage.yaml`.
type Hashes = BTreeMap<String, String>;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Database items by export path: group, id and canonical YAML.
type DatabaseItems = BTreeMap<String, (&'static str, String, Vec<u8>)>;

fn database_items(data: &StoredData) -> Result<DatabaseItems, Error> {
    Ok(
        export_files(&data.components, &data.templates, &data.pages)?
            .into_iter()
            .map(|(group, id, bytes)| (export_path(group, id), (group, id.to_owned(), bytes)))
            .collect(),
    )
}

/// Classify every path that differs between the database and the files.
fn compare(db: &Hashes, files: &Hashes, base: &Hashes) -> Vec<Change> {
    let paths: BTreeSet<&String> = db.keys().chain(files.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let (d, f, b) = (db.get(path), files.get(path), base.get(path));
            let (side, action) = if d == f {
                return None;
            } else if f == b {
                (Side::Database, action(b, d))
            } else if d == b {
                (Side::Files, action(b, f))
            } else {
                (Side::Both, "changed")
            };
            Some(Change {
                path: path.clone(),
                side,
                action,
            })
        })
        .collect()
}

fn action(before: Option<&String>, after: Option<&String>) -> &'static str {
    match (before, after) {
        (_, None) => "deleted",
        (None, _) => "added",
        _ => "modified",
    }
}

/// Split changes into those copied to the target and those left pending.
fn plan(
    changes: Vec<Change>,
    source: Side,
    force: bool,
    target: &Hashes,
    incoming: &Hashes,
) -> Result<Sync, Error> {
    let conflicts: Vec<&str> = changes
        .iter()
        .filter(|c| c.side == Side::Both)
        .map(|c| c.path.as_str())
        .collect();
    if !conflicts.is_empty() && !force {
        return Err(Error(format!(
            "changed in both the database and the files since the last sync:\n  {}\n\
             Use pull --force to keep the database versions, or push --force to keep the files.",
            conflicts.join("\n  ")
        )));
    }
    let mut sync = Sync::default();
    for mut change in changes {
        if force || change.side == source {
            change.action = action(target.get(&change.path), incoming.get(&change.path));
            sync.applied.push(change);
        } else {
            sync.pending.push(change);
        }
    }
    Ok(sync)
}

/// Apply a plan's outcome to a side's hashes.
fn after(hashes: &Hashes, applied: &[Change], source: &Hashes) -> Hashes {
    let mut result = hashes.clone();
    for change in applied {
        match source.get(&change.path) {
            Some(hash) => result.insert(change.path.clone(), hash.clone()),
            None => result.remove(&change.path),
        };
    }
    result
}

fn load_base(tx: &mut Transaction<'_>) -> crate::Result<Hashes> {
    let rows: Vec<(String, String)> = tx
        .query("SELECT path,hash FROM sync_base")
        .map_err(crate::db_error)?;
    Ok(rows
        .into_iter()
        .filter(|(path, _)| {
            path.split_once('/')
                .is_some_and(|(group, _)| GROUPS.contains(&group))
        })
        .collect())
}

/// Record every path where both sides now agree. Paths that still differ keep their base.
fn save_base(
    tx: &mut Transaction<'_>,
    db: &Hashes,
    files: &Hashes,
    base: &Hashes,
) -> crate::Result<()> {
    let paths: BTreeSet<&String> = db.keys().chain(files.keys()).chain(base.keys()).collect();
    for path in paths {
        let hash = db.get(path);
        if hash != files.get(path) || hash == base.get(path) {
            continue;
        }
        match hash {
            Some(hash) => tx.exec_drop(
                "INSERT INTO sync_base(path,hash) VALUES(?,?) ON DUPLICATE KEY UPDATE hash=VALUES(hash)",
                (path, hash),
            ),
            None => tx.exec_drop("DELETE FROM sync_base WHERE path=?", (path,)),
        }
        .map_err(crate::db_error)?;
    }
    Ok(())
}

/// Treat freshly seeded items as already synced, so a repository that deleted or changed
/// them updates a new installation instead of having its starter items exported back.
pub(crate) fn record_base(
    tx: &mut Transaction<'_>,
    components: &[Component],
    templates: &[Template],
) -> crate::Result<()> {
    for (group, id, bytes) in export_files(components, templates, &[])? {
        tx.exec_drop(
            "INSERT INTO sync_base(path,hash) VALUES(?,?)",
            (export_path(group, id), digest(&bytes)),
        )
        .map_err(crate::db_error)?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {}

/// Whether the directory holds a manifest, validating it if so.
fn has_manifest(directory: &Path) -> Result<bool, Error> {
    let path = directory.join(MANIFEST);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(io_error("inspect", &path, e)),
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
            return Err(Error(format!(
                "unexpected or unsafe file: {}",
                path.display()
            )));
        }
        Ok(_) => {}
    }
    let bytes = fs::read(&path).map_err(|e| io_error("read", &path, e))?;
    let file: File<Manifest> = serde_yaml_ng::from_slice(&bytes)
        .map_err(|_| Error(format!("invalid manifest: {}", path.display())))?;
    if file.schema_version != SCHEMA_VERSION || file.kind != "export" {
        return Err(Error(format!("wrong schema or kind: {}", path.display())));
    }
    Ok(true)
}

/// Raw bytes of the canonical export files in `groups`, by export path. Unrelated files are
/// skipped, or rejected when `strict`.
fn read_files(
    directory: &Path,
    groups: &[&str],
    strict: bool,
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let mut files = BTreeMap::new();
    for group in groups {
        let dir = directory.join(group);
        if fs::symlink_metadata(&dir).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
            continue;
        }
        reject_unsafe_directory(&dir)?;
        let entries = fs::read_dir(&dir).map_err(|e| io_error("read directory", &dir, e))?;
        for entry in entries {
            let path = entry
                .map_err(|e| io_error("read directory entry", &dir, e))?
                .path();
            let meta = fs::symlink_metadata(&path).map_err(|e| io_error("inspect", &path, e))?;
            let canonical = is_canonical_filename(&path);
            if meta.file_type().is_symlink() || !meta.is_file() || (strict && !canonical) {
                return Err(Error(format!(
                    "unexpected or unsafe file: {}",
                    path.display()
                )));
            }
            if !canonical {
                continue;
            }
            if meta.len() > MAX_FILE_SIZE {
                return Err(Error(format!("file too large: {}", path.display())));
            }
            let name = path
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or_default();
            let bytes = fs::read(&path).map_err(|e| io_error("read", &path, e))?;
            files.insert(format!("{group}/{name}"), bytes);
        }
    }
    Ok(files)
}

/// Parse an export file and re-encode it, so formatting-only edits are not changes.
fn canonical(path: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let (group, name) = path.split_once('/').unwrap_or_default();
    match group {
        "components" => yaml(
            "component",
            parse_item::<Component>(path, name, bytes, "component")?,
        ),
        "templates" => yaml(
            "template",
            parse_item::<Template>(path, name, bytes, "template")?,
        ),
        _ => Err(Error(format!("unexpected file: {path}"))),
    }
}

/// Write and remove export files. Writes are staged first; each replacement is atomic, but
/// the directory as a whole is not.
fn write_files(directory: &Path, writes: &[(&str, &[u8])], removals: &[&str]) -> Result<(), Error> {
    let mut staged = Vec::with_capacity(writes.len());
    for (relative, bytes) in writes {
        let path = directory.join(relative);
        let parent = path.parent().expect("output has parent");
        ensure_directory(parent)?;
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
            return Err(Error(format!("refusing unsafe output: {}", path.display())));
        }
        let mut temp = NamedTempFile::new_in(parent)
            .map_err(|e| io_error("create temporary output", &path, e))?;
        temp.write_all(bytes)
            .map_err(|e| io_error("write temporary output", &path, e))?;
        temp.as_file()
            .sync_all()
            .map_err(|e| io_error("sync temporary output", &path, e))?;
        // Close each file before staging the next; large exports must not exhaust FDs.
        staged.push((temp.into_temp_path(), path));
    }
    for (temp, path) in staged {
        temp.persist(&path)
            .map_err(|e| io_error("replace", &path, e.error))?;
    }
    for relative in removals {
        let path = directory.join(relative);
        fs::remove_file(&path).map_err(|e| io_error("remove", &path, e))?;
    }
    Ok(())
}

/// List what differs between the database and an export directory, without changing either.
pub fn status(db: &Pool, directory: &Path) -> Result<Vec<Change>, Error> {
    let known = has_manifest(directory)?;
    let files = file_hashes(&read_files(directory, &GROUPS, false)?);
    database_transaction(db, TransactionMode::Rollback, |tx| {
        let db_hashes = database_hashes(&database_items(&load_data(tx, false)?)?);
        let base = if known { load_base(tx)? } else { Hashes::new() };
        Ok(compare(&db_hashes, &files, &base))
    })
    .map_err(api_error)
}

fn database_hashes(items: &DatabaseItems) -> Hashes {
    items
        .iter()
        .map(|(path, (_, _, bytes))| (path.clone(), digest(bytes)))
        .collect()
}

/// Hash files leniently: malformed files still count as changes.
fn file_hashes(files: &BTreeMap<String, Vec<u8>>) -> Hashes {
    files
        .iter()
        .map(|(path, bytes)| {
            let hash = canonical(path, bytes).map_or_else(|_| digest(bytes), |c| digest(&c));
            (path.clone(), hash)
        })
        .collect()
}

/// Copy database changes since the last sync into the files. Changes made only in the files
/// are left for `push`; `force` makes the files mirror the database.
pub fn pull(db: &Pool, directory: &Path, force: bool) -> Result<Sync, Error> {
    ensure_directory(directory)?;
    let known = has_manifest(directory)?;
    let files = file_hashes(&read_files(directory, &GROUPS, false)?);
    database_transaction(db, TransactionMode::Commit, |tx| {
        // A directory without a manifest shares no history with this database.
        if !known {
            tx.query_drop("DELETE FROM sync_base")
                .map_err(crate::db_error)?;
        }
        let items = database_items(&load_data(tx, false)?)?;
        let db_hashes = database_hashes(&items);
        let base = load_base(tx)?;
        let sync = plan(
            compare(&db_hashes, &files, &base),
            Side::Database,
            force,
            &files,
            &db_hashes,
        )?;
        let mut writes = Vec::new();
        let mut removals = Vec::new();
        for change in &sync.applied {
            match items.get(&change.path) {
                Some((_, _, bytes)) => writes.push((change.path.as_str(), bytes.as_slice())),
                None => removals.push(change.path.as_str()),
            }
        }
        write_files(directory, &writes, &removals)?;
        save_base(
            tx,
            &db_hashes,
            &after(&files, &sync.applied, &db_hashes),
            &base,
        )?;
        if !known {
            let manifest = yaml("export", Manifest {})?;
            write_files(directory, &[(MANIFEST, &manifest)], &[])?;
        }
        Ok(sync)
    })
    .map_err(api_error)
}

/// Copy file changes since the last sync into the database, including deletions. Changes
/// made only in the database are left for `pull`; `force` makes the database mirror the
/// files. Nothing is published. Any failure rolls back the whole push.
pub fn push(db: &Pool, directory: &Path, force: bool, dry_run: bool) -> Result<Sync, Error> {
    reject_unsafe_directory(directory)?;
    if !has_manifest(directory)? {
        return Err(Error(format!(
            "{} is not an export yet; run pull first",
            directory.display()
        )));
    }
    let files = read_files(directory, &GROUPS, true)?;
    let mut file_hashes = Hashes::new();
    for (path, bytes) in &files {
        file_hashes.insert(path.clone(), digest(&canonical(path, bytes)?));
    }
    let mode = if dry_run {
        TransactionMode::Rollback
    } else {
        TransactionMode::Commit
    };
    database_transaction(db, mode, |tx| {
        let items = database_items(&load_data(tx, false)?)?;
        let db_hashes = database_hashes(&items);
        let base = load_base(tx)?;
        let sync = plan(
            compare(&db_hashes, &file_hashes, &base),
            Side::Files,
            force,
            &db_hashes,
            &file_hashes,
        )?;
        let mut changes = Items::new();
        for change in &sync.applied {
            match files.get(&change.path) {
                Some(bytes) => changes.add(&change.path, bytes)?,
                None => {
                    let (group, id, _) = &items[&change.path];
                    changes.deleted.push((group, id.clone()));
                }
            }
        }
        changes.check_ids()?;
        merge(tx, changes, DefinitionPolicy::Replace)?;
        save_base(
            tx,
            &after(&db_hashes, &sync.applied, &file_hashes),
            &file_hashes,
            &base,
        )?;
        Ok(sync)
    })
    .map_err(api_error)
}

fn is_canonical_filename(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(stem) = name.strip_suffix(".yaml") else {
        return false;
    };
    (!stem.is_empty()
        && stem.len() <= 200
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')))
        || stem.strip_prefix("id.").is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// Parsed export items, ready to merge.
pub(crate) struct Items {
    components: Vec<Component>,
    templates: Vec<Template>,
    drafts: Vec<PageDraft>,
    /// Components and templates to delete, by group and id.
    deleted: Vec<(&'static str, String)>,
}

impl Items {
    pub(crate) fn new() -> Self {
        Self {
            components: Vec::new(),
            templates: Vec::new(),
            drafts: Vec::new(),
            deleted: Vec::new(),
        }
    }
    /// Parse one file of the export layout, given as `group/filename`.
    pub(crate) fn add(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error> {
        let (group, name) = path
            .split_once('/')
            .filter(|(_, name)| is_canonical_filename(Path::new(name)))
            .ok_or_else(|| Error(format!("unexpected file: {path}")))?;
        let duplicate = match group {
            "components" => add_unique(
                &mut self.components,
                parse_item(path, name, bytes, "component")?,
            ),
            "templates" => add_unique(
                &mut self.templates,
                parse_item(path, name, bytes, "template")?,
            ),
            "pages" => add_unique(&mut self.drafts, parse_item(path, name, bytes, "page")?),
            _ => return Err(Error(format!("unexpected file: {path}"))),
        };
        match duplicate {
            Some(id) => Err(Error(format!("duplicate id: {id}"))),
            None => Ok(()),
        }
    }
    pub(crate) fn pages(&self) -> usize {
        self.drafts.len()
    }
    fn check_ids(&self) -> Result<(), Error> {
        for id in self
            .components
            .iter()
            .map(|x| &x.id)
            .chain(self.templates.iter().map(|x| &x.id))
            .chain(self.drafts.iter().map(|x| &x.id))
        {
            if id.trim().is_empty() || id.chars().count() > 255 {
                return Err(Error("item ids must contain 1 to 255 characters".into()));
            }
            // Duplicate IDs are rejected within a kind; IDs may intentionally overlap across kinds.
        }
        Ok(())
    }
}

fn add_unique<T: HasId>(items: &mut Vec<T>, item: T) -> Option<String> {
    if items.iter().any(|x| x.id() == item.id()) {
        return Some(item.id().to_owned());
    }
    items.push(item);
    None
}

/// How a merge treats component and template definitions that already exist.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DefinitionPolicy {
    /// Overwrite them; Git exports are the source of truth for definitions.
    Replace,
    /// Keep them; content packages only add definitions the target lacks.
    AddMissing,
}

/// What a merge changed. Pages are listed by path.
#[derive(Default, Serialize)]
pub(crate) struct Merged {
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub components_added: usize,
    pub templates_added: usize,
}

pub(crate) fn merge_items(
    tx: &mut Transaction<'_>,
    items: Items,
    definitions: DefinitionPolicy,
) -> crate::Result<Merged> {
    items.check_ids().map_err(|e| ApiError::bad(e.0))?;
    merge(tx, items, definitions)
}

fn exists(tx: &mut Transaction<'_>, table: &str, id: &str) -> crate::Result<bool> {
    tx.exec_first::<String, _, _>(format!("SELECT id FROM {table} WHERE id=?"), (id,))
        .map(|row| row.is_some())
        .map_err(crate::db_error)
}

fn merge(
    tx: &mut Transaction<'_>,
    items: Items,
    definitions: DefinitionPolicy,
) -> crate::Result<Merged> {
    let Items {
        components,
        templates,
        drafts,
        deleted,
    } = items;
    let mut merged = Merged::default();
    for item in &components {
        if definitions == DefinitionPolicy::AddMissing {
            if exists(tx, "components", &item.id)? {
                continue;
            }
            merged.components_added += 1;
        }
        validate_component(item)?;
        tx.exec_drop(
            "INSERT INTO components(id,data) VALUES(?,?) ON DUPLICATE KEY UPDATE data=VALUES(data)",
            (&item.id, json(item)?),
        )
        .map_err(crate::db_error)?;
    }
    for item in &templates {
        if definitions == DefinitionPolicy::AddMissing {
            if exists(tx, "templates", &item.id)? {
                continue;
            }
            merged.templates_added += 1;
        }
        validate_template(tx, item)?;
        tx.exec_drop(
            "INSERT INTO templates(id,data) VALUES(?,?) ON DUPLICATE KEY UPDATE data=VALUES(data)",
            (&item.id, json(item)?),
        )
        .map_err(crate::db_error)?;
    }
    let old: Vec<Page> = crate::load_all(tx, "pages")?;
    let old_by_id: HashMap<_, _> = old.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut desired = Vec::new();
    let mut slugs = HashSet::new();
    for draft in drafts {
        if !slugs.insert(draft.slug.clone()) {
            return Err(ApiError::bad("duplicate page paths in import"));
        }
        let page = match old_by_id.get(draft.id.as_str()) {
            Some(old) if PageDraft::from(*old) == draft => {
                merged.unchanged.push(draft.slug);
                (*old).clone()
            }
            Some(old) => {
                merged.updated.push(draft.slug.clone());
                draft.page(old.revision + 1, old.published_revision)
            }
            None => {
                merged.created.push(draft.slug.clone());
                draft.page(1, None)
            }
        };
        desired.push(page);
    }
    // Free all changed paths first, allowing atomic swaps without touching snapshot rows.
    for page in &desired {
        if let Some(old) = old_by_id.get(page.id.as_str())
            && old.slug != page.slug
        {
            tx.exec_drop(
                "UPDATE pages SET slug=? WHERE id=?",
                (
                    format!("__baddiecore_import_{}", file_name(&page.id)),
                    &page.id,
                ),
            )
            .map_err(crate::db_error)?;
        }
    }
    for page in &desired {
        match old_by_id.get(page.id.as_str()) {
            Some(old) if **old == *page => {}
            Some(_) => tx
                .exec_drop(
                    "UPDATE pages SET slug=?,data=? WHERE id=?",
                    (&page.slug, json(page)?, &page.id),
                )
                .map_err(crate::constraint_error)?,
            None => insert_page(tx, page)?,
        }
    }
    // Delete definitions last, once pages and templates have moved off them.
    for group in ["templates", "components"] {
        for (_, id) in deleted.iter().filter(|(g, _)| *g == group) {
            crate::delete_definition(tx, group, id)?;
        }
    }
    let pages = crate::load_all::<Page>(tx, "pages")?;
    let definitions = Definitions::load(tx, &pages)?;
    for page in &pages {
        definitions.validate(page)?;
    }
    crate::validate_route_paths(tx, &pages)?;
    for paths in [
        &mut merged.created,
        &mut merged.updated,
        &mut merged.unchanged,
    ] {
        paths.sort();
    }
    Ok(merged)
}

/// Parse one export file. `label` names it in errors; `name` is its filename, which must match the id.
fn parse_item<T: DeserializeOwned + Serialize + HasId>(
    label: &str,
    name: &str,
    bytes: &[u8],
    kind: &str,
) -> Result<T, Error> {
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_slice(bytes).map_err(|_| Error(format!("malformed YAML: {label}")))?;
    let file: File<T> = serde_yaml_ng::from_value(value.clone())
        .map_err(|_| Error(format!("invalid item: {label}")))?;
    // Keep API models reusable while rejecting ignored or misspelled YAML keys,
    // including keys inside fields and regions.
    if serde_yaml_ng::to_value(&file).map_err(|_| Error("invalid item".into()))? != value {
        return Err(Error(format!("unknown fields or invalid types: {label}")));
    }
    if file.schema_version != SCHEMA_VERSION || file.kind != kind {
        return Err(Error(format!("wrong schema or kind: {label}")));
    }
    if name != file_name(file.data.id()) {
        return Err(Error(format!("filename does not match id: {label}")));
    }
    Ok(file.data)
}

trait HasId {
    fn id(&self) -> &str;
}
impl HasId for Component {
    fn id(&self) -> &str {
        &self.id
    }
}
impl HasId for Template {
    fn id(&self) -> &str {
        &self.id
    }
}
impl HasId for PageDraft {
    fn id(&self) -> &str {
        &self.id
    }
}

fn ensure_directory(path: &Path) -> Result<(), Error> {
    reject_symlink_ancestors(path)?;
    if fs::symlink_metadata(path).is_ok() {
        reject_unsafe_directory(path)
    } else {
        fs::create_dir_all(path).map_err(|e| io_error("create directory", path, e))
    }
}
fn reject_unsafe_directory(path: &Path) -> Result<(), Error> {
    reject_symlink_ancestors(path)?;
    let meta = fs::symlink_metadata(path).map_err(|e| io_error("inspect", path, e))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        Err(Error(format!("not a safe directory: {}", path.display())))
    } else {
        Ok(())
    }
}
fn reject_symlink_ancestors(path: &Path) -> Result<(), Error> {
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error(format!("refusing symlink: {}", ancestor.display())));
        }
    }
    Ok(())
}
fn io_error(operation: &str, path: &Path, error: std::io::Error) -> Error {
    Error(format!("{operation} {}: {error}", path.display()))
}
fn api_error(e: ApiError) -> Error {
    Error(e.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_are_stable_and_safe() {
        assert_eq!(file_name("homepage"), "homepage.yaml");
        assert_eq!(file_name("../ café").len(), 72);
        assert!(!file_name("../ café").contains('/'));
        assert_ne!(file_name("a/b"), file_name("a_b"));
        assert!(file_name(&"é".repeat(255)).len() < 255);
    }

    #[test]
    fn canonical_filename_shape_is_exact() {
        let hashed = file_name("not/a/direct/id");
        assert!(is_canonical_filename(Path::new("home.yaml")));
        assert!(is_canonical_filename(Path::new(&hashed)));
        assert!(!is_canonical_filename(Path::new("notes.txt")));
        assert!(!is_canonical_filename(Path::new("not canonical.yaml")));
        assert!(!is_canonical_filename(Path::new(
            "id.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.yaml"
        )));
    }

    #[test]
    fn compare_uses_the_base_to_tell_which_side_changed() {
        let h = |pairs: &[(&str, &str)]| -> Hashes {
            pairs
                .iter()
                .map(|(p, v)| (p.to_string(), v.to_string()))
                .collect()
        };
        let base = h(&[
            ("same", "1"),
            ("db-edit", "1"),
            ("db-del", "1"),
            ("file-edit", "1"),
            ("file-del", "1"),
            ("both", "1"),
            ("both-del", "1"),
        ]);
        let db = h(&[
            ("same", "1"),
            ("db-edit", "2"),
            ("db-new", "1"),
            ("file-edit", "1"),
            ("file-del", "1"),
            ("both", "2"),
            ("both-del", "2"),
            ("untracked", "1"),
        ]);
        let files = h(&[
            ("same", "1"),
            ("db-edit", "1"),
            ("db-del", "1"),
            ("file-edit", "2"),
            ("file-new", "1"),
            ("both", "3"),
            ("untracked", "2"),
        ]);
        let changes: Vec<(String, Side, &str)> = compare(&db, &files, &base)
            .into_iter()
            .map(|c| (c.path, c.side, c.action))
            .collect();
        let expected = [
            ("both", Side::Both, "changed"),
            ("both-del", Side::Both, "changed"),
            ("db-del", Side::Database, "deleted"),
            ("db-edit", Side::Database, "modified"),
            ("db-new", Side::Database, "added"),
            ("file-del", Side::Files, "deleted"),
            ("file-edit", Side::Files, "modified"),
            ("file-new", Side::Files, "added"),
            // Without a base, differing content on both sides cannot be ordered.
            ("untracked", Side::Both, "changed"),
        ];
        assert_eq!(
            changes,
            expected
                .iter()
                .map(|(p, s, a)| (p.to_string(), *s, *a))
                .collect::<Vec<_>>()
        );
        assert!(
            plan(
                compare(&db, &files, &base),
                Side::Database,
                false,
                &files,
                &db
            )
            .is_err()
        );
        let sync = plan(compare(&db, &files, &base), Side::Files, true, &db, &files).unwrap();
        assert_eq!(sync.applied.len(), 9);
        // Applied actions describe the target: the database gains file-new and loses db-new.
        let action = |p: &str| sync.applied.iter().find(|c| c.path == p).unwrap().action;
        assert_eq!((action("file-new"), action("db-new")), ("added", "deleted"));
    }

    #[test]
    fn deterministic_page_yaml() {
        let draft = PageDraft {
            id: "p".into(),
            title: "T".into(),
            slug: "/".into(),
            aliases: vec!["/welcome".into()],
            template_id: "t".into(),
            blocks: vec![BlockDraft {
                id: "b".into(),
                component_id: "c".into(),
                region: "r".into(),
                fields: BTreeMap::from([("z".into(), "1".into()), ("a".into(), "2".into())]),
            }],
        };
        let text = String::from_utf8(yaml("page", &draft).unwrap()).unwrap();
        assert!(text.find("a: '2'").unwrap() < text.find("z: '1'").unwrap());
        assert!(!text.contains("revision"));
        let parsed: File<PageDraft> = serde_yaml_ng::from_str(&text).unwrap();
        assert_eq!(yaml("page", parsed.data).unwrap(), text.as_bytes());
    }
    #[test]
    fn rejects_unknown_yaml_fields() {
        let bad = b"schema_version: 1\nkind: page\ndata: {id: p, title: T, slug: /, template_id: t, blocks: [], extra: true}\n";
        assert!(serde_yaml_ng::from_slice::<File<PageDraft>>(bad).is_err());
    }

    #[test]
    fn rejects_unknown_nested_fields_wrong_filenames_and_versions() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("templates")).unwrap();
        let path = dir.path().join("templates/t.yaml");
        let good = "schema_version: 1\nkind: template\ndata:\n  id: t\n  name: T\n  description: ''\n  regions: [{name: main, allowed_components: [], max_components: 2}]\n";
        fs::write(&path, good).unwrap();
        assert!(canonical("templates/t.yaml", good.as_bytes()).is_ok());
        for bad in [
            good.replace("max_components: 2", "max_components: 2, typo: true"),
            good.replace("schema_version: 1", "schema_version: 7"),
            good.replace("id: t", "id: other"),
        ] {
            assert!(canonical("templates/t.yaml", bad.as_bytes()).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_files_and_directory_ancestors() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        symlink(target.path(), dir.path().join("linked")).unwrap();
        assert!(ensure_directory(&dir.path().join("linked/nested")).is_err());
        fs::create_dir(dir.path().join("templates")).unwrap();
        fs::write(target.path().join("home.yaml"), "unchanged").unwrap();
        symlink(
            target.path().join("home.yaml"),
            dir.path().join("templates/home.yaml"),
        )
        .unwrap();
        assert!(read_files(dir.path(), &["templates"], false).is_err());
        assert_eq!(
            fs::read_to_string(target.path().join("home.yaml")).unwrap(),
            "unchanged"
        );
    }
}
