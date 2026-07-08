//! Tests for listing, reading, and mtime tracking (`store::mod`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::testutil::{test_id, test_meta};
use super::*;
use crate::model::Severity;

#[test]
fn missing_section_is_none_not_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("bare");
    // Hand-build a workspace with a manifest but no section files.
    let dir = store.workspace_dir(&id);
    fs::create_dir_all(&dir).expect("mkdir");
    let manifest = toml::to_string_pretty(&test_meta("Bare", Severity::Info)).expect("toml");
    fs::write(dir.join(MANIFEST_FILE), manifest).expect("write manifest");

    assert_eq!(
        store.read_section(&id, SectionKind::Summary).expect("read"),
        None
    );
}

#[test]
fn corrupt_manifest_becomes_warning_not_failure() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    store
        .scaffold(&test_id("good"), &test_meta("Good", Severity::Low))
        .expect("scaffold");

    let bad_dir = store.workspace_dir(&test_id("bad"));
    fs::create_dir_all(&bad_dir).expect("mkdir");
    fs::write(bad_dir.join(MANIFEST_FILE), "title = unclosed").expect("write corrupt");

    let (summaries, warnings) = store.list().expect("list");
    assert_eq!(summaries.len(), 1, "the good workspace still lists");
    assert_eq!(warnings.len(), 1, "the bad one is reported");
    assert!(warnings[0].0.contains("bad"));
}

#[test]
fn oversized_file_is_rejected_before_reading() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("big");
    store
        .scaffold(&id, &test_meta("Big", Severity::Low))
        .expect("scaffold");

    let path = store
        .workspace_dir(&id)
        .join(SectionKind::Notes.file_name());
    let file = fs::File::create(&path).expect("create");
    file.set_len(MAX_FILE_BYTES + 1).expect("grow file"); // sparse: no real 4 MB write
    drop(file);

    assert!(matches!(
        store.read_section(&id, SectionKind::Notes),
        Err(Error::FileTooLarge { .. })
    ));
}

#[test]
fn section_mtimes_cover_scaffolded_sections_and_skip_absent_ones() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("mtimes");
    store
        .scaffold(&id, &test_meta("Mtimes", Severity::Low))
        .expect("scaffold");

    let mtimes = store.section_mtimes(&id);
    assert_eq!(mtimes.len(), SectionKind::ALL.len());

    fs::remove_file(
        store
            .workspace_dir(&id)
            .join(SectionKind::Notes.file_name()),
    )
    .expect("remove");
    let mtimes = store.section_mtimes(&id);
    assert!(!mtimes.contains_key(&SectionKind::Notes));
}

#[test]
fn diagrams_list_sorted_and_missing_dir_is_empty() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("diag");
    store
        .scaffold(&id, &test_meta("Diag", Severity::Low))
        .expect("scaffold");

    let ddir = store.workspace_dir(&id).join(DIAGRAMS_DIR);
    fs::write(ddir.join("02-flow.txt"), "b").expect("write");
    fs::write(ddir.join("01-topology.txt"), "a").expect("write");

    let diagrams = store.list_diagrams(&id).expect("list diagrams");
    let names: Vec<&str> = diagrams.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["01-topology.txt", "02-flow.txt"]);

    let no_dir = test_id("nodir");
    fs::create_dir_all(store.workspace_dir(&no_dir)).expect("mkdir");
    assert!(store.list_diagrams(&no_dir).expect("empty").is_empty());
}
