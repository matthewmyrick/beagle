//! Tests for `runner`. These drive a fake `claude` shim so no real session is
//! ever started.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::Command;
use std::time::{Duration, Instant};

use super::*;

/// Writes an executable shell shim into `dir` and returns its path.
fn write_shim(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write shim");
    let mut perms = std::fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).expect("chmod");
    path
}

/// A spec pointing at `dir` for both the worktree and the log file.
fn spec<'a>(dir: &'a Path, log: &'a Path) -> SessionSpec<'a> {
    SessionSpec {
        working_dir: dir,
        prompt: "do the thing",
        allowed_tools: &[],
        log_path: log,
    }
}

#[test]
fn clean_exit_captures_log() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shim = write_shim(
        dir.path(),
        "claude",
        "#!/bin/sh\necho hello-from-shim\nexit 0\n",
    );
    let log = dir.path().join("session.log");

    let runner = Runner::new(dir.path().join("runs")).with_claude_bin(shim);
    let outcome = runner
        .run(spec(dir.path(), &log), Duration::from_secs(10))
        .expect("run");

    assert_eq!(outcome, Outcome::Exited(0));
    assert!(outcome.is_success());
    let logged = std::fs::read_to_string(&log).expect("read log");
    assert!(logged.contains("hello-from-shim"), "log was: {logged:?}");
}

#[test]
fn non_zero_exit_is_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shim = write_shim(dir.path(), "claude", "#!/bin/sh\nexit 7\n");
    let log = dir.path().join("session.log");

    let runner = Runner::new(dir.path().join("runs")).with_claude_bin(shim);
    let outcome = runner
        .run(spec(dir.path(), &log), Duration::from_secs(10))
        .expect("run");

    assert_eq!(outcome, Outcome::Exited(7));
    assert!(!outcome.is_success());
}

#[test]
fn timeout_kills_the_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let shim = write_shim(dir.path(), "claude", "#!/bin/sh\nsleep 30\n");
    let log = dir.path().join("session.log");

    let runner = Runner::new(dir.path().join("runs")).with_claude_bin(shim);
    let started = Instant::now();
    let outcome = runner
        .run(spec(dir.path(), &log), Duration::from_millis(300))
        .expect("run");

    assert_eq!(outcome, Outcome::TimedOut);
    // The sleep was 30s; we must have killed it far sooner.
    assert!(started.elapsed() < Duration::from_secs(15), "took too long");
    // The registry file is cleaned up once wait() returns.
    let leftover: Vec<_> = std::fs::read_dir(dir.path().join("runs"))
        .expect("read runs")
        .collect();
    assert!(leftover.is_empty(), "registry not cleaned: {leftover:?}");
}

#[test]
fn spawn_failure_surfaces_as_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("session.log");

    let runner =
        Runner::new(dir.path().join("runs")).with_claude_bin("/no/such/claude/binary/anywhere");
    let err = runner
        .run(spec(dir.path(), &log), Duration::from_secs(5))
        .expect_err("spawn should fail");

    assert!(matches!(err, RunError::Spawn { .. }), "got {err:?}");
}

#[test]
fn cleanup_orphans_kills_recorded_groups() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = dir.path().join("runs");
    std::fs::create_dir_all(&runs).expect("runs dir");

    // A leaked session: a long sleep in its own process group, recorded under
    // the runs dir exactly as Runner::spawn would have.
    let mut child = Command::new("sh")
        .args(["-c", "sleep 30"])
        .process_group(0)
        .spawn()
        .expect("spawn sleeper");
    let pgid = i32::try_from(child.id()).expect("pid fits i32");
    let registry = runs.join(format!("{pgid}.session"));
    std::fs::write(&registry, "some-log-path").expect("write registry");

    let cleaned = cleanup_orphans(&runs).expect("cleanup");
    assert_eq!(cleaned, 1);
    assert!(!registry.exists(), "registry file should be removed");

    // The leaked process was force-killed.
    let status = child.wait().expect("reap");
    assert_eq!(status.signal(), Some(Signal::SIGKILL as i32));
}

#[test]
fn cleanup_orphans_on_missing_dir_is_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cleaned = cleanup_orphans(&dir.path().join("does-not-exist")).expect("cleanup");
    assert_eq!(cleaned, 0);
}
