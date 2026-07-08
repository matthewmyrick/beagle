//! Tests for the single-file markdown export (`store::export`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;

use crate::model::Severity;
use crate::store::testutil::{test_id, test_meta};
use crate::store::{Store, DIAGRAMS_DIR, EXPORTS_DIR};

#[test]
fn export_is_deterministic_with_frontmatter_sections_and_clean_diagrams() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("export-me");
    let mut meta = test_meta("Export \"quoted\" title", Severity::High);
    meta.tags = vec!["webhooks".to_owned(), "data-loss".to_owned()];
    store.scaffold(&id, &meta).expect("scaffold");
    fs::write(
        store.workspace_dir(&id).join(DIAGRAMS_DIR).join("01-x.txt"),
        "a \u{1b}[1;31mBUG\u{1b}[0m b",
    )
    .expect("write diagram");

    let doc = store.export_markdown(&id).expect("export");
    assert!(doc.starts_with("---\n"), "frontmatter first");
    assert!(doc.contains("title: \"Export \\\"quoted\\\" title\""));
    assert!(doc.contains("severity: high"));
    assert!(doc.contains("tags: [\"webhooks\", \"data-loss\"]"));
    assert!(doc.contains("# Summary"), "sections included");
    assert!(doc.contains("## Diagram: 01-x.txt"));
    assert!(doc.contains("a BUG b"), "ANSI stripped from diagrams");
    assert!(!doc.contains('\u{1b}'), "no raw escape bytes in export");

    let again = store.export_markdown(&id).expect("export twice");
    assert_eq!(doc, again, "deterministic");
}

#[test]
fn export_to_writes_default_path_and_honors_out() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("to-file");
    store
        .scaffold(&id, &test_meta("To file", Severity::Low))
        .expect("scaffold");

    let default_path = store.export_to(&id, None).expect("default export");
    assert_eq!(
        default_path,
        tmp.path().join(EXPORTS_DIR).join("to-file.md")
    );
    assert!(default_path.is_file());

    let custom = tmp.path().join("vault").join("note.md");
    let custom_path = store.export_to(&id, Some(&custom)).expect("custom export");
    assert_eq!(custom_path, custom);
    assert!(custom.is_file(), "parent dirs created");
}
