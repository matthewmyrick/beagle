//! Tests for `poller`, against a real beagle RCA store scaffolded via the
//! `beagle` domain crate.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use beagle::model::{Severity, Status};

use crate::store::JobState;

use super::*;

/// Builds a temp RCA store with one workspace per `(id, status)` pair.
fn rca_store_with(entries: &[(&str, Status)]) -> (tempfile::TempDir, RcaStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = RcaStore::open(dir.path()).expect("open rca store");
    for (id, status) in entries {
        let rca_id = RcaId::new(*id).expect("valid id");
        let meta = beagle::store::new_meta(format!("Title {id}"), Severity::Medium);
        store.scaffold(&rca_id, &meta).expect("scaffold");
        store.set_status(&rca_id, *status).expect("set status");
    }
    (dir, store)
}

/// Opens a fresh job store under a temp dir.
fn job_store() -> (tempfile::TempDir, JobStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JobStore::open(&dir.path().join("jobs.db")).expect("open jobs");
    (dir, store)
}

#[test]
fn triggered_returns_only_matching_status() {
    let (_dir, rcas) = rca_store_with(&[
        ("2026-01-01-alpha", Status::Agent),
        ("2026-01-02-beta", Status::Investigating),
        ("2026-01-03-gamma", Status::Review),
        ("2026-01-04-delta", Status::Agent),
    ]);
    let poller = Poller::new(&rcas, "agent");

    let mut ids: Vec<String> = poller
        .triggered()
        .expect("triggered")
        .into_iter()
        .map(|t| t.id)
        .collect();
    ids.sort();
    assert_eq!(ids, ["2026-01-01-alpha", "2026-01-04-delta"]);
}

#[test]
fn enqueue_inserts_triggered_and_skips_others() {
    let (_dir, rcas) = rca_store_with(&[
        ("2026-01-01-alpha", Status::Agent),
        ("2026-01-02-beta", Status::Investigating),
    ]);
    let (_jdir, jobs) = job_store();
    let poller = Poller::new(&rcas, "agent");

    poller.enqueue(&jobs).expect("enqueue");

    let alpha = jobs.job("2026-01-01-alpha").expect("read").expect("exists");
    assert_eq!(alpha.state, JobState::Pending);
    assert_eq!(alpha.title.as_deref(), Some("Title 2026-01-01-alpha"));
    // A non-agent RCA is never enqueued.
    assert!(jobs.job("2026-01-02-beta").expect("read").is_none());
}

#[test]
fn re_enqueue_does_not_resurrect_finished_jobs() {
    let (_dir, rcas) = rca_store_with(&[
        ("2026-01-01-alpha", Status::Agent),
        ("2026-01-04-delta", Status::Agent),
    ]);
    let (_jdir, jobs) = job_store();
    let poller = Poller::new(&rcas, "agent");

    poller.enqueue(&jobs).expect("first enqueue");
    jobs.mark_done("2026-01-01-alpha", None).expect("done");

    // Re-polling must not drag the finished job back to pending.
    poller.enqueue(&jobs).expect("second enqueue");
    assert_eq!(
        jobs.job("2026-01-01-alpha")
            .expect("read")
            .expect("row")
            .state,
        JobState::Done
    );
    assert_eq!(
        jobs.job("2026-01-04-delta")
            .expect("read")
            .expect("row")
            .state,
        JobState::Pending
    );
}

#[test]
fn read_remediation_returns_the_section() {
    let (_dir, rcas) = rca_store_with(&[("2026-01-01-alpha", Status::Agent)]);
    let rca_id = RcaId::new("2026-01-01-alpha").expect("id");
    std::fs::write(
        rcas.workspace_dir(&rca_id).join("remediation.md"),
        "# Fix\n\nRestart the worker and raise the memory limit.\n",
    )
    .expect("write remediation");

    let poller = Poller::new(&rcas, "agent");
    let remediation = poller
        .read_remediation("2026-01-01-alpha")
        .expect("read")
        .expect("some content");
    assert!(
        remediation.contains("raise the memory limit"),
        "{remediation:?}"
    );
}
