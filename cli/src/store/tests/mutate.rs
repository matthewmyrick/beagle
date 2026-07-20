//! Tests for scaffolding and manifest edits (`store::mutate`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crate::error::Error;
use crate::model::{SectionKind, Severity, Status};
use crate::store::testutil::{test_id, test_meta};
use crate::store::Store;

#[test]
fn scaffold_then_list_round_trips() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("payments-latency");
    let meta = test_meta("Payments p99 latency", Severity::High);

    store.scaffold(&id, &meta).expect("scaffold");
    let listing = store.list().expect("list");
    let (summaries, warnings) = (listing.summaries, listing.warnings);

    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, id);
    assert_eq!(summaries[0].meta, meta);
}

#[test]
fn scaffold_refuses_to_overwrite() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("dup");
    let meta = test_meta("dup", Severity::Low);

    store.scaffold(&id, &meta).expect("first scaffold");
    assert!(matches!(
        store.scaffold(&id, &meta),
        Err(Error::AlreadyExists(_))
    ));
}

#[test]
fn scaffold_creates_all_sections_and_they_read_back() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("sections");
    store
        .scaffold(&id, &test_meta("Sections", Severity::Medium))
        .expect("scaffold");

    for kind in SectionKind::ALL {
        let content = store.read_section(&id, kind).expect("read section");
        let content = content.unwrap_or_else(|| panic!("section {kind:?} missing"));
        assert!(content.starts_with(&format!("# {}", kind.title())));
    }
}

#[test]
fn set_status_rewrites_only_status_and_stamps_updated() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("flip");
    let meta = test_meta("Flip", Severity::High);
    store.scaffold(&id, &meta).expect("scaffold");

    let written = store.set_status(&id, Status::Review).expect("set status");
    assert_eq!(written.status, Status::Review);

    let back = store.read_meta(&id).expect("re-read");
    assert_eq!(back.status, Status::Review);
    assert!(back.updated.is_some(), "updated stamped");
    assert_eq!(back.title, meta.title, "other fields preserved");
    assert_eq!(back.created, meta.created);
    assert_eq!(back.systems, meta.systems);
    assert_eq!(back.tags, meta.tags);
}

#[test]
fn set_status_on_missing_workspace_is_an_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    assert!(store
        .set_status(&test_id("ghost"), Status::Finished)
        .is_err());
}

#[test]
fn add_pr_appends_once_stamps_updated_and_rejects_non_urls() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("fixed");
    store
        .scaffold(&id, &test_meta("Fixed", Severity::High))
        .expect("scaffold");

    let url = "https://github.com/org/repo/pull/12";
    assert!(store.add_pr(&id, url).expect("attach"), "first add");
    assert!(!store.add_pr(&id, url).expect("re-attach"), "idempotent");

    let meta = store.read_meta(&id).expect("read");
    assert_eq!(meta.prs, [url]);
    assert!(meta.updated.is_some(), "updated stamped");
    assert_eq!(meta.title, "Fixed", "other fields preserved");

    assert!(store.add_pr(&id, "not-a-url").is_err());
    assert!(store.add_pr(&test_id("ghost"), url).is_err());
}

#[test]
fn append_log_creates_then_appends_timestamped_bullets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("logged");
    store
        .scaffold(&id, &test_meta("Logged", Severity::Low))
        .expect("scaffold");

    store
        .append_log(&id, "checked p99 dashboard")
        .expect("append");
    store
        .append_log(&id, "  querying loki  ")
        .expect("append again");
    let content = store
        .read_section(&id, SectionKind::Log)
        .expect("read")
        .expect("present");
    let bullets: Vec<&str> = content.lines().filter(|l| l.starts_with("- **")).collect();
    assert_eq!(bullets.len(), 2);
    assert!(bullets[0].contains("UTC** — checked p99 dashboard"));
    assert!(
        bullets[1].ends_with("— querying loki"),
        "message trimmed: {}",
        bullets[1]
    );
}

#[test]
fn append_log_requires_an_existing_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    assert!(store.append_log(&test_id("ghost"), "hello").is_err());
}

#[test]
fn archive_moves_a_finished_workspace_and_listing_splits() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("done-rca");
    store
        .scaffold(&id, &test_meta("Done", Severity::Low))
        .expect("scaffold");
    store.set_status(&id, Status::Finished).expect("finish");

    let dest = store.archive(&id, false).expect("archive");
    assert!(dest.ends_with("archive/done-rca"), "moved to {dest:?}");
    assert!(dest.join("rca.toml").exists());

    let active = store.list().expect("list");
    assert!(
        active.summaries.is_empty(),
        "archived leaves the active list"
    );
    assert!(
        active.broken.is_empty(),
        "archive/ must not be reported as a broken workspace"
    );
    let archived = store.list_archived().expect("archived");
    assert_eq!(archived.summaries.len(), 1);
    assert!(archived.summaries[0].archived);

    let all = store.list_all().expect("all");
    assert_eq!(all.summaries.len(), 1);
}

#[test]
fn archive_refuses_unfinished_unless_forced_and_never_twice() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("live-rca");
    store
        .scaffold(&id, &test_meta("Live", Severity::High))
        .expect("scaffold");

    let err = store.archive(&id, false).expect_err("investigating");
    assert!(err.to_string().contains("finished"), "explains: {err}");

    store.archive(&id, true).expect("force overrides");
    let err = store.archive(&id, true).expect_err("already archived");
    assert!(err.to_string().contains("already archived"), "got: {err}");
}

#[test]
fn archived_workspaces_read_and_export_transparently() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("old-rca");
    store
        .scaffold(&id, &test_meta("Old", Severity::Low))
        .expect("scaffold");
    store.set_status(&id, Status::Finished).expect("finish");
    store.archive(&id, false).expect("archive");

    let summary = store
        .read_section(&id, SectionKind::Summary)
        .expect("read")
        .expect("present");
    assert!(summary.contains("Old"), "sections resolve into archive/");
    let doc = store.export_markdown(&id).expect("export");
    assert!(doc.contains("title: \"Old\""));
    store.append_log(&id, "post-archive note").expect("log");
}

#[test]
fn scaffold_refuses_the_archive_slug_and_archived_collisions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let reserved = test_id("archive");
    let err = store
        .scaffold(&reserved, &test_meta("Nope", Severity::Low))
        .expect_err("reserved");
    assert!(err.to_string().contains("reserved"), "got: {err}");

    let id = test_id("gone-rca");
    store
        .scaffold(&id, &test_meta("Gone", Severity::Low))
        .expect("scaffold");
    store.archive(&id, true).expect("archive");
    store
        .scaffold(&id, &test_meta("Again", Severity::Low))
        .expect_err("archived slug still occupied");
}

#[test]
fn unarchive_restores_a_workspace_and_round_trips() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("back-rca");
    store
        .scaffold(&id, &test_meta("Back", Severity::Low))
        .expect("scaffold");
    store.archive(&id, true).expect("archive");

    let dest = store.unarchive(&id).expect("unarchive");
    assert!(dest.ends_with("rcas/back-rca"));
    let listing = store.list().expect("list");
    assert_eq!(listing.summaries.len(), 1);
    assert!(!listing.summaries[0].archived);

    let err = store.unarchive(&id).expect_err("not archived anymore");
    assert!(err.to_string().contains("not archived"), "got: {err}");
    let missing = test_id("never-was");
    let err = store.unarchive(&missing).expect_err("missing");
    assert!(err.to_string().contains("no workspace"), "got: {err}");
}

#[test]
fn publish_sets_the_flag_and_stamps_a_date_idempotently() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("public-rca");
    store
        .scaffold(&id, &test_meta("Public", Severity::High))
        .expect("scaffold");

    // Fresh workspaces are private.
    assert!(!store.read_meta(&id).expect("meta").published);

    assert!(store.set_published(&id, true).expect("publish"));
    let meta = store.read_meta(&id).expect("meta");
    assert!(meta.published);
    let stamp = meta.published_at.expect("published_at stamped");

    // Re-publishing keeps the original timestamp (idempotent).
    store.set_published(&id, true).expect("republish");
    assert_eq!(
        store.read_meta(&id).expect("meta").published_at,
        Some(stamp),
        "published_at is not re-stamped"
    );

    // Unpublishing clears both.
    assert!(!store.set_published(&id, false).expect("unpublish"));
    let meta = store.read_meta(&id).expect("meta");
    assert!(!meta.published);
    assert!(meta.published_at.is_none());
}

#[test]
fn delete_removes_active_and_archived_workspaces_permanently() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");

    let active = test_id("mistake");
    store
        .scaffold(&active, &test_meta("Mistake", Severity::Low))
        .expect("scaffold");
    let dir = store.delete(&active).expect("delete active");
    assert!(!dir.exists(), "directory gone: {dir:?}");
    assert!(store.list_all().expect("list").summaries.is_empty());

    // Archived workspaces delete from rcas/archive/ transparently.
    let archived = test_id("old-news");
    store
        .scaffold(&archived, &test_meta("Old news", Severity::Low))
        .expect("scaffold");
    store
        .set_status(&archived, Status::Finished)
        .expect("finish");
    store.archive(&archived, false).expect("archive");
    let dir = store.delete(&archived).expect("delete archived");
    assert!(dir.ends_with("archive/old-news") && !dir.exists());

    // No workspace, no delete — and the reserved archive dir is refused
    // even if an `archive` id were constructed.
    assert!(store.delete(&test_id("ghost")).is_err());
    assert!(matches!(
        store.delete(&test_id("archive")),
        Err(Error::Tool { tool: "delete", .. })
    ));
}

#[test]
fn set_tags_trims_dedupes_and_drops_empties() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("open store");
    let id = test_id("taggy");
    store
        .scaffold(&id, &test_meta("Taggy", Severity::Low))
        .expect("scaffold");

    let meta = store
        .set_tags(
            &id,
            vec![
                " redis ".to_owned(),
                "redis".to_owned(),
                String::new(),
                "  ".to_owned(),
                "config".to_owned(),
            ],
        )
        .expect("set");
    assert_eq!(meta.tags, ["redis", "config"]);
    assert_eq!(
        store.read_meta(&id).expect("read").tags,
        ["redis", "config"]
    );
    assert!(meta.updated.is_some(), "updated stamped");
}
