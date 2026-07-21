//! Tests for the toolbox and systems context (`store::context`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;

use super::{SYSTEM_TEMPLATE, TOOLBOX_TEMPLATE};
use crate::store::{Store, SYSTEMS_DIR, TOOLBOX_FILE};

#[test]
fn init_context_scaffolds_once_and_never_overwrites() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");

    let created = store.init_context().expect("init");
    assert_eq!(created.len(), 2, "toolbox + example system: {created:?}");
    assert!(tmp.path().join(TOOLBOX_FILE).is_file());
    assert!(tmp
        .path()
        .join(SYSTEMS_DIR)
        .join("example-service.md")
        .is_file());

    // Second run creates nothing and touches nothing.
    fs::write(tmp.path().join(TOOLBOX_FILE), "customized").expect("customize");
    let again = store.init_context().expect("re-init");
    assert!(again.is_empty(), "no overwrites: {again:?}");
    assert_eq!(
        fs::read_to_string(tmp.path().join(TOOLBOX_FILE)).expect("read"),
        "customized"
    );
}

#[test]
fn init_context_skips_the_example_when_real_system_docs_exist() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let systems = tmp.path().join(SYSTEMS_DIR);
    fs::create_dir_all(&systems).expect("mkdir");
    fs::write(systems.join("payments-api.md"), "# payments-api").expect("write");

    let created = store.init_context().expect("init");
    assert_eq!(created.len(), 1, "only the toolbox is missing");
    assert!(
        !systems.join("example-service.md").exists(),
        "no example next to real docs"
    );
}

#[test]
fn toolbox_and_system_docs_read_back() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");

    assert_eq!(store.read_toolbox().expect("absent ok"), None);
    assert!(store.list_system_docs().expect("missing dir ok").is_empty());

    store.init_context().expect("init");
    let toolbox = store.read_toolbox().expect("read").expect("present");
    assert!(toolbox.starts_with("# Toolbox"));

    let systems = tmp.path().join(SYSTEMS_DIR);
    fs::write(systems.join("api.md"), "# api").expect("write");
    fs::write(systems.join("notes.txt"), "not markdown").expect("write");
    let docs = store.list_system_docs().expect("list");
    let names: Vec<&str> = docs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["api", "example-service"], "sorted, .md only");
    assert_eq!(
        store.read_system_doc(&docs[0]).expect("read").as_deref(),
        Some("# api")
    );
}

#[test]
fn context_templates_are_valid_markdown_seeds() {
    assert!(TOOLBOX_TEMPLATE.starts_with("# Toolbox"));
    assert!(SYSTEM_TEMPLATE.starts_with("# example-service"));
}

#[test]
fn review_context_bundles_writeup_toolbox_and_relevant_systems() {
    use crate::model::Severity;
    use crate::store::testutil::{test_id, test_meta};

    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("payments-latency");
    // test_meta lists the `payments-api` system.
    store
        .scaffold(&id, &test_meta("Payments p99 latency", Severity::High))
        .expect("scaffold");

    // Toolbox + two system docs, only one of which this RCA touches.
    fs::write(
        tmp.path().join(TOOLBOX_FILE),
        "# Toolbox\n\nGrafana at /d/payments\n",
    )
    .expect("toolbox");
    let systems = tmp.path().join(SYSTEMS_DIR);
    fs::create_dir_all(&systems).expect("mkdir");
    fs::write(
        systems.join("payments-api.md"),
        "# payments-api\n\nRust axum service\n",
    )
    .expect("sys1");
    fs::write(
        systems.join("unrelated.md"),
        "# unrelated\n\nshould not appear\n",
    )
    .expect("sys2");

    let bundle = store.review_context(&id).expect("context");
    assert!(
        bundle.contains("title: \"Payments p99 latency\""),
        "has the writeup frontmatter"
    );
    assert!(bundle.contains("# Toolbox"), "includes the toolbox");
    assert!(bundle.contains("Grafana at /d/payments"));
    assert!(
        bundle.contains("<!-- systems/payments-api.md -->"),
        "includes the touched system"
    );
    assert!(bundle.contains("Rust axum service"));
    assert!(
        !bundle.contains("should not appear"),
        "omits unrelated systems"
    );
}
