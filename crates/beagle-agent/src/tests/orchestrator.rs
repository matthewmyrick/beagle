//! Tests for `orchestrator`. The whole flow runs against a real beagle store, a
//! real git repo, and a `claude` shim that actually commits — only the publish
//! step (push + `gh`) is faked.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use beagle::model::Severity;

use crate::store::JobState;

use super::*;

/// A publisher that records the RCA ids it was asked to publish and returns a
/// canned URL, without touching a real remote or GitHub.
struct FakePublisher {
    calls: Arc<Mutex<Vec<String>>>,
    url: String,
}

impl Publisher for FakePublisher {
    fn publish(&self, request: &PrRequest) -> Result<String, PublishError> {
        self.calls
            .lock()
            .expect("calls lock")
            .push(request.rca_id.to_string());
        Ok(self.url.clone())
    }
}

fn git_ok(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_shim(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write shim");
    let mut perms = std::fs::metadata(path).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod");
}

/// Everything a test needs: the built orchestrator plus the paths and handles to
/// inspect the outcome. The `TempDir` keeps all of it alive.
struct Harness {
    orch: Orchestrator,
    _tmp: tempfile::TempDir,
    rca_root: std::path::PathBuf,
    wts: std::path::PathBuf,
    jobs_db: std::path::PathBuf,
    calls: Arc<Mutex<Vec<String>>>,
}

const PR_URL: &str = "https://github.com/matthewmyrick/beagle/pull/999";
const RCA_ID: &str = "2026-01-01-alpha";

/// Builds a harness: a git repo with an origin/main, a beagle store holding one
/// `agent`-status RCA with a `remediation.md`, and a `claude` shim.
fn harness(shim_body: &str, max_attempts: u32, precheck_blocked: Option<&str>) -> Harness {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path();

    // Target repo with a bare origin holding one main commit.
    let origin = base.join("origin.git");
    let repo = base.join("repo");
    git_ok(
        base,
        &["init", "--bare", "-b", "main", &origin.to_string_lossy()],
    );
    git_ok(
        base,
        &["clone", &origin.to_string_lossy(), &repo.to_string_lossy()],
    );
    git_ok(&repo, &["config", "user.email", "t@example.com"]);
    git_ok(&repo, &["config", "user.name", "Test"]);
    std::fs::write(repo.join("README.md"), "hi").expect("write");
    git_ok(&repo, &["add", "."]);
    git_ok(&repo, &["commit", "-m", "init"]);
    git_ok(&repo, &["branch", "-M", "main"]);
    git_ok(&repo, &["push", "-u", "origin", "main"]);

    // Beagle store with one agent-status RCA + remediation.md.
    let rca_root = base.join("store");
    let rcas = RcaStore::open(&rca_root).expect("rca store");
    let rca_id = RcaId::new(RCA_ID).expect("id");
    let meta = beagle::store::new_meta("Payments API OOM".to_string(), Severity::High);
    rcas.scaffold(&rca_id, &meta).expect("scaffold");
    rcas.set_status(&rca_id, Status::Agent).expect("status");
    std::fs::write(
        rcas.workspace_dir(&rca_id).join("remediation.md"),
        "# Fix\n\nRaise the memory limit.\n",
    )
    .expect("write remediation");

    // The claude shim.
    let shim = base.join("claude");
    write_shim(&shim, shim_body);

    let wts = base.join("worktrees");
    let jobs_db = base.join("jobs.db");
    let calls = Arc::new(Mutex::new(Vec::new()));

    let policy = RunPolicy {
        trigger_status: "agent".to_string(),
        base_prompt: "You are the beagle remediation agent.".to_string(),
        allowed_tools: Vec::new(),
        timeout: std::time::Duration::from_secs(30),
        max_concurrent: 2,
        max_per_poll: 5,
        max_attempts,
        logs_dir: base.join("logs"),
    };
    let blocked = precheck_blocked.map(str::to_string);
    let orch = Orchestrator::new(
        policy,
        rcas,
        JobStore::open(&jobs_db).expect("jobs"),
        Worktrees::new(repo, wts.clone(), "agent".to_string()),
        Runner::new(base.join("runs")).with_claude_bin(shim),
        Box::new(FakePublisher {
            calls: Arc::clone(&calls),
            url: PR_URL.to_string(),
        }),
        Box::new(move || blocked.clone()),
    );

    Harness {
        orch,
        _tmp: tmp,
        rca_root,
        wts,
        jobs_db,
        calls,
    }
}

/// The shim that "implements" the fix: writes a file and commits, as claude
/// would inside the worktree.
const COMMITTING_SHIM: &str =
    "#!/bin/sh\necho patch > applied.txt\ngit add -A\ngit commit -m 'agent: remediate' >/dev/null 2>&1\n";

fn rca_status(root: &Path) -> Status {
    let rcas = RcaStore::open(root).expect("reopen store");
    rcas.read_meta(&RcaId::new(RCA_ID).expect("id"))
        .expect("meta")
        .status
}

#[test]
fn happy_path_publishes_attaches_and_advances() {
    let h = harness(COMMITTING_SHIM, 3, None);
    let tick = h.orch.run_once();

    let results = match tick {
        Tick::Ran { results } => results,
        Tick::Skipped { reason } => panic!("unexpected skip: {reason}"),
    };
    assert_eq!(results.len(), 1);
    match &results[0].outcome {
        JobOutcome::Published { pr_url, warning } => {
            assert_eq!(pr_url, PR_URL);
            assert!(warning.is_none(), "warning: {warning:?}");
        }
        other => panic!("expected Published, got {other:?}"),
    }

    // The PR was attached and the RCA advanced to final-review.
    let rcas = RcaStore::open(&h.rca_root).expect("reopen");
    let meta = rcas
        .read_meta(&RcaId::new(RCA_ID).expect("id"))
        .expect("meta");
    assert_eq!(meta.status, Status::FinalReview);
    assert!(
        meta.prs.iter().any(|url| url == PR_URL),
        "prs: {:?}",
        meta.prs
    );

    // The job is done, the publisher ran once, and no worktree leaked.
    let jobs = JobStore::open(&h.jobs_db).expect("jobs");
    assert_eq!(
        jobs.job(RCA_ID).expect("read").expect("row").state,
        JobState::Done
    );
    assert_eq!(h.calls.lock().expect("calls").len(), 1);
    assert!(!h.wts.join(RCA_ID).exists(), "worktree leaked");
}

#[test]
fn session_failure_gives_up_without_publishing() {
    // A shim that fails: no commit, non-zero exit. Cap of 1 → give up.
    let h = harness("#!/bin/sh\nexit 1\n", 1, None);
    let tick = h.orch.run_once();

    let results = match tick {
        Tick::Ran { results } => results,
        Tick::Skipped { reason } => panic!("unexpected skip: {reason}"),
    };
    assert!(matches!(results[0].outcome, JobOutcome::GaveUp { .. }));

    // Nothing published, RCA untouched, job failed, no leaked worktree.
    assert!(h.calls.lock().expect("calls").is_empty());
    assert_eq!(rca_status(&h.rca_root), Status::Agent);
    let jobs = JobStore::open(&h.jobs_db).expect("jobs");
    assert_eq!(
        jobs.job(RCA_ID).expect("read").expect("row").state,
        JobState::Failed
    );
    assert!(!h.wts.join(RCA_ID).exists(), "worktree leaked");
}

#[test]
fn no_commit_retries_under_the_cap() {
    // Shim exits 0 but makes no commit → failure, but under the cap → retry.
    let h = harness("#!/bin/sh\nexit 0\n", 3, None);
    let tick = h.orch.run_once();

    let results = match tick {
        Tick::Ran { results } => results,
        Tick::Skipped { reason } => panic!("unexpected skip: {reason}"),
    };
    match &results[0].outcome {
        JobOutcome::WillRetry { reason } => assert!(reason.contains("no commits"), "{reason}"),
        other => panic!("expected WillRetry, got {other:?}"),
    }

    // Returned to pending for another attempt; nothing published.
    let jobs = JobStore::open(&h.jobs_db).expect("jobs");
    assert_eq!(
        jobs.job(RCA_ID).expect("read").expect("row").state,
        JobState::Pending
    );
    assert!(h.calls.lock().expect("calls").is_empty());
}

#[test]
fn precheck_block_skips_the_tick() {
    let h = harness(COMMITTING_SHIM, 3, Some("gh not authed"));
    match h.orch.run_once() {
        Tick::Skipped { reason } => assert_eq!(reason, "gh not authed"),
        Tick::Ran { results } => panic!("expected skip, ran {results:?}"),
    }
    // Nothing was touched.
    assert!(h.calls.lock().expect("calls").is_empty());
    assert_eq!(rca_status(&h.rca_root), Status::Agent);
}
