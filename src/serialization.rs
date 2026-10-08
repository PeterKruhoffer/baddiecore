//! Local, file-based import and export of CMS drafts.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
};

use mysql::{Pool, Transaction, prelude::Queryable};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{
    ApiError, Block, Component, Definitions, Page, Template, TransactionMode, database_transaction,
    insert_page, json, load_data, validate_component, validate_template,
};

const SCHEMA_VERSION: u32 = 1;
const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub components: usize,
    pub templates: usize,
    pub pages: usize,
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

/// Export components and templates, and optionally page drafts.
pub fn pull(db: &Pool, directory: &Path, pages: bool, force: bool) -> Result<Counts, Error> {
    let data = database_transaction(db, TransactionMode::Commit, |tx| load_data(tx, pages))
        .map_err(api_error)?;
    let mut files = Vec::new();
    for item in &data.components {
        files.push(("components", &item.id, yaml("component", item)?));
    }
    for item in &data.templates {
        files.push(("templates", &item.id, yaml("template", item)?));
    }
    if pages {
        for item in &data.pages {
            files.push(("pages", &item.id, yaml("page", PageDraft::from(item))?));
        }
    }
    files.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    ensure_directory(directory)?;
    for (group, id, bytes) in &files {
        let dir = directory.join(group);
        ensure_directory(&dir)?;
        let path = dir.join(file_name(id));
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(Error(format!("refusing unsafe output: {}", path.display())));
            }
            if !force
                && (meta.len() != bytes.len() as u64
                    || fs::read(&path).map_err(|e| io_error("read", &path, e))? != *bytes)
            {
                return Err(Error(format!(
                    "{} has local changes (use --force to overwrite)",
                    path.display()
                )));
            }
        }
    }
    // A pull is a snapshot of the selected kinds. Do not silently keep deleted
    // pages as files that a later push would recreate.
    let expected: HashSet<PathBuf> = files
        .iter()
        .map(|(group, id, _)| directory.join(group).join(file_name(id)))
        .collect();
    let groups = if pages {
        &["components", "templates", "pages"][..]
    } else {
        &["components", "templates"][..]
    };
    let stale = stale_exports(directory, groups, &expected)?;
    if !stale.is_empty() && !force {
        return Err(Error(
            "export contains items absent from the database; use --force to remove their files"
                .into(),
        ));
    }
    // Stage the complete snapshot before replacing anything. Renames are atomic per
    // file, but a pull is intentionally not atomic across the whole directory.
    let mut staged = Vec::with_capacity(files.len());
    for (group, id, bytes) in files {
        let path = directory.join(group).join(file_name(id));
        let mut temp = NamedTempFile::new_in(path.parent().expect("output has parent"))
            .map_err(|e| io_error("create temporary output", &path, e))?;
        temp.write_all(&bytes)
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
    for path in stale {
        fs::remove_file(&path).map_err(|e| io_error("remove stale export", &path, e))?;
    }
    Ok(Counts {
        components: data.components.len(),
        templates: data.templates.len(),
        pages: if pages { data.pages.len() } else { 0 },
    })
}

fn stale_exports(
    directory: &Path,
    groups: &[&str],
    expected: &HashSet<PathBuf>,
) -> Result<Vec<PathBuf>, Error> {
    let mut stale = Vec::new();
    for group in groups {
        let dir = directory.join(group);
        if fs::symlink_metadata(&dir).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
            continue;
        }
        reject_unsafe_directory(&dir)?;
        let entries = fs::read_dir(&dir).map_err(|e| io_error("read directory", &dir, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| io_error("read directory entry", &dir, e))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| io_error("inspect", &path, e))?;
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(Error(format!(
                    "unexpected or unsafe file: {}",
                    path.display()
                )));
            }
            if is_canonical_filename(&path) && !expected.contains(&path) {
                stale.push(path);
            }
        }
    }
    Ok(stale)
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

/// Validate and merge an export into the database. Missing files never delete records.
pub fn push(db: &Pool, directory: &Path, pages: bool, dry_run: bool) -> Result<Counts, Error> {
    reject_unsafe_directory(directory)?;
    let components: Vec<Component> = read_group(directory, "components", "component")?;
    let templates: Vec<Template> = read_group(directory, "templates", "template")?;
    let drafts: Vec<PageDraft> = if pages {
        read_group(directory, "pages", "page")?
    } else {
        Vec::new()
    };
    let counts = Counts {
        components: components.len(),
        templates: templates.len(),
        pages: drafts.len(),
    };
    for id in components
        .iter()
        .map(|x| &x.id)
        .chain(templates.iter().map(|x| &x.id))
        .chain(drafts.iter().map(|x| &x.id))
    {
        if id.trim().is_empty() || id.chars().count() > 255 {
            return Err(Error("item ids must contain 1 to 255 characters".into()));
        }
        // Duplicate IDs are rejected within a kind by read_group; IDs may intentionally overlap across kinds.
    }
    let mode = if dry_run {
        TransactionMode::Rollback
    } else {
        TransactionMode::Commit
    };
    database_transaction(db, mode, move |tx| {
        merge(tx, components, templates, drafts)?;
        Ok(())
    })
    .map_err(api_error)?;
    Ok(counts)
}

fn merge(
    tx: &mut Transaction<'_>,
    components: Vec<Component>,
    templates: Vec<Template>,
    drafts: Vec<PageDraft>,
) -> crate::Result<()> {
    for item in &components {
        validate_component(item)?;
        tx.exec_drop(
            "INSERT INTO components(id,data) VALUES(?,?) ON DUPLICATE KEY UPDATE data=VALUES(data)",
            (&item.id, json(item)?),
        )
        .map_err(crate::db_error)?;
    }
    for item in &templates {
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
            Some(old) if PageDraft::from(*old) == draft => (*old).clone(),
            Some(old) => draft.page(old.revision + 1, old.published_revision),
            None => draft.page(1, None),
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
    let pages = crate::load_all::<Page>(tx, "pages")?;
    let definitions = Definitions::load(tx, &pages)?;
    for page in &pages {
        definitions.validate(page)?;
    }
    crate::validate_route_paths(tx, &pages)?;
    Ok(())
}

fn read_group<T: DeserializeOwned + Serialize + HasId>(
    root: &Path,
    group: &str,
    kind: &str,
) -> Result<Vec<T>, Error> {
    let dir = root.join(group);
    if fs::symlink_metadata(&dir).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        return Ok(Vec::new());
    }
    reject_unsafe_directory(&dir)?;
    let mut paths: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| io_error("read directory", &dir, e))?
        .map(|e| {
            e.map(|x| x.path())
                .map_err(|e| io_error("read directory entry", &dir, e))
        })
        .collect::<Result<_, _>>()?;
    paths.sort();
    let mut result = Vec::new();
    let mut ids = HashSet::new();
    for path in paths {
        let meta = fs::symlink_metadata(&path).map_err(|e| io_error("inspect", &path, e))?;
        if meta.file_type().is_symlink()
            || !meta.is_file()
            || path.extension().and_then(|x| x.to_str()) != Some("yaml")
        {
            return Err(Error(format!(
                "unexpected or unsafe file: {}",
                path.display()
            )));
        }
        if meta.len() > MAX_FILE_SIZE {
            return Err(Error(format!("file too large: {}", path.display())));
        }
        let bytes = fs::read(&path).map_err(|e| io_error("read", &path, e))?;
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_slice(&bytes)
            .map_err(|_| Error(format!("malformed YAML: {}", path.display())))?;
        let file: File<T> = serde_yaml_ng::from_value(value.clone())
            .map_err(|_| Error(format!("invalid item: {}", path.display())))?;
        // Keep API models reusable while rejecting ignored or misspelled YAML keys,
        // including keys inside fields and regions.
        if serde_yaml_ng::to_value(&file).map_err(|_| Error("invalid item".into()))? != value {
            return Err(Error(format!(
                "unknown fields or invalid types: {}",
                path.display()
            )));
        }
        if file.schema_version != SCHEMA_VERSION || file.kind != kind {
            return Err(Error(format!("wrong schema or kind: {}", path.display())));
        }
        if path.file_name().and_then(|x| x.to_str()) != Some(&file_name(file.data.id())) {
            return Err(Error(format!(
                "filename does not match id: {}",
                path.display()
            )));
        }
        if !ids.insert(file.data.id().to_owned()) {
            return Err(Error(format!("duplicate id: {}", file.data.id())));
        }
        result.push(file.data);
    }
    Ok(result)
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
    fn stale_detection_uses_names_without_parsing_and_preserves_unrelated_files() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("components");
        fs::create_dir(&dir).unwrap();
        let expected_path = dir.join("expected.yaml");
        fs::write(&expected_path, "malformed: [").unwrap();
        let stale_path = dir.join("deleted.yaml");
        fs::write(&stale_path, "also not yaml: [").unwrap();
        fs::write(dir.join("editor-notes.txt"), "keep me").unwrap();
        fs::write(dir.join("not canonical.yaml"), "keep me too").unwrap();

        let expected = HashSet::from([expected_path]);
        assert_eq!(
            stale_exports(root.path(), &["components"], &expected).unwrap(),
            vec![stale_path]
        );
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
        assert_eq!(
            read_group::<Template>(dir.path(), "templates", "template")
                .unwrap()
                .len(),
            1
        );
        for bad in [
            good.replace("max_components: 2", "max_components: 2, typo: true"),
            good.replace("schema_version: 1", "schema_version: 7"),
            good.replace("id: t", "id: other"),
        ] {
            fs::write(&path, bad).unwrap();
            assert!(read_group::<Template>(dir.path(), "templates", "template").is_err());
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
        fs::create_dir(dir.path().join("pages")).unwrap();
        fs::write(target.path().join("home.yaml"), "unchanged").unwrap();
        symlink(
            target.path().join("home.yaml"),
            dir.path().join("pages/home.yaml"),
        )
        .unwrap();
        assert!(read_group::<PageDraft>(dir.path(), "pages", "page").is_err());
        assert_eq!(
            fs::read_to_string(target.path().join("home.yaml")).unwrap(),
            "unchanged"
        );
    }
}
