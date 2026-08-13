//! Tests for `store`.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

/// Opens a store in a fresh temp directory, returning the store and the dir
/// guard (kept alive so the file survives for the test's duration).
fn temp_store() -> (Store, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("jobs.db")).expect("open store");
    (store, dir)
}

#[test]
fn open_enables_wal() {
    let (store, _dir) = temp_store();
    let mode: String = store
        .conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal_mode");
    assert_eq!(mode, "wal");
}

#[test]
fn upsert_then_lifecycle_transitions() {
    let (store, _dir) = temp_store();
    store.upsert_pending("rca-1", Some("boom")).expect("upsert");

    let job = store.job("rca-1").expect("read").expect("exists");
    assert_eq!(job.state, JobState::Pending);
    assert_eq!(job.attempts, 0);
    assert_eq!(job.title.as_deref(), Some("boom"));

    store.mark_processing("rca-1").expect("processing");
    assert_eq!(
        store.job("rca-1").expect("read").expect("row").state,
        JobState::Processing
    );

    store
        .mark_done("rca-1", Some("https://pr/1"))
        .expect("done");
    let done = store.job("rca-1").expect("read").expect("row");
    assert_eq!(done.state, JobState::Done);
    assert_eq!(done.pr_url.as_deref(), Some("https://pr/1"));
}

#[test]
fn upsert_ignores_existing_row() {
    let (store, _dir) = temp_store();
    store
        .upsert_pending("rca-1", Some("first"))
        .expect("upsert");
    store.mark_done("rca-1", None).expect("done");

    // Re-seeing the job must not reset it back to pending or change its title.
    store
        .upsert_pending("rca-1", Some("second"))
        .expect("re-upsert");
    let job = store.job("rca-1").expect("read").expect("row");
    assert_eq!(job.state, JobState::Done);
    assert_eq!(job.title.as_deref(), Some("first"));
}

#[test]
fn increment_attempts_returns_new_count() {
    let (store, _dir) = temp_store();
    store.upsert_pending("rca-1", None).expect("upsert");
    assert_eq!(store.increment_attempts("rca-1").expect("inc"), 1);
    assert_eq!(store.increment_attempts("rca-1").expect("inc"), 2);
    assert_eq!(store.job("rca-1").expect("read").expect("row").attempts, 2);
}

#[test]
fn mark_failed_records_kind_and_reason() {
    let (store, _dir) = temp_store();
    store.upsert_pending("rca-1", None).expect("upsert");
    store
        .mark_failed("rca-1", "ci", "failing checks: clippy")
        .expect("failed");
    let job = store.job("rca-1").expect("read").expect("row");
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.fail_kind.as_deref(), Some("ci"));
    assert_eq!(job.fail_reason.as_deref(), Some("failing checks: clippy"));
}

#[test]
fn select_actionable_only_pending_under_cap() {
    let (store, _dir) = temp_store();
    store.upsert_pending("a", None).expect("a");
    store.upsert_pending("b", None).expect("b");
    store.upsert_pending("c", None).expect("c");

    // b is being worked on; c has exhausted the cap of 3.
    store.mark_processing("b").expect("b processing");
    for _ in 0..3 {
        store.increment_attempts("c").expect("c inc");
    }

    let actionable = store.select_actionable(3).expect("select");
    assert_eq!(actionable.len(), 1);
    assert_eq!(actionable[0].id, "a");
    assert_eq!(actionable[0].attempts, 0);
}

#[test]
fn missing_job_reads_as_none() {
    let (store, _dir) = temp_store();
    assert!(store.job("nope").expect("read").is_none());
}

#[test]
fn processing_resets_to_pending_on_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("jobs.db");

    {
        let store = Store::open(&path).expect("open");
        store.upsert_pending("rca-1", None).expect("upsert");
        store.mark_processing("rca-1").expect("processing");
        // Simulate a crash: the store (and its live session) vanish while the
        // job is still `processing`.
    }

    let reopened = Store::open(&path).expect("reopen");
    let job = reopened.job("rca-1").expect("read").expect("row");
    assert_eq!(job.state, JobState::Pending);
}
