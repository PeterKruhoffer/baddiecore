//! Page rows are written only by `src/store.rs`, which validates every draft and
//! claims its paths. A write elsewhere could skip those checks.

#[test]
fn only_the_store_writes_page_rows() {
    let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut offenders = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            if path.ends_with("src/store.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap().to_ascii_uppercase();
            let source = source.split_whitespace().collect::<Vec<_>>().join(" ");
            for write in ["INTO PAGES", "UPDATE PAGES", "DELETE FROM PAGES"] {
                if source.contains(write) {
                    offenders.push(format!("{}: {write}", path.display()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "write page rows through store::save_pages, store::mark_published or store::delete_page, not raw SQL: {offenders:?}"
    );
}
