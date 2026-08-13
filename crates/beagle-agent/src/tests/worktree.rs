//! Tests for `worktree`, against a throwaway local git repo with a real
//! `origin` remote.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::path::Path;
use std::process::Command;

use super::*;

/// Runs a git command in `dir`, asserting success.
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

/// Builds a repo with a bare `origin` holding a single `main` commit, and
/// returns the temp dir plus the working repo and worktree-root paths.
fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let origin = tmp.path().join("origin.git");
    let repo = tmp.path().join("repo");

    git_ok(
        tmp.path(),
        &["init", "--bare", "-b", "main", &origin.to_string_lossy()],
    );
    git_ok(
        tmp.path(),
        &["clone", &origin.to_string_lossy(), &repo.to_string_lossy()],
    );
    git_ok(&repo, &["config", "user.email", "t@example.com"]);
    git_ok(&repo, &["config", "user.name", "Test"]);
    std::fs::write(repo.join("README.md"), "hello").expect("write");
    git_ok(&repo, &["add", "."]);
    git_ok(&repo, &["commit", "-m", "init"]);
    git_ok(&repo, &["branch", "-M", "main"]);
    git_ok(&repo, &["push", "-u", "origin", "main"]);

    let root = tmp.path().join("worktrees");
    (tmp, repo, root)
}

/// The current branch name checked out at `dir`.
fn current_branch(dir: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .expect("branch");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn create_makes_worktree_off_origin_main() {
    let (_tmp, repo, root) = fixture();
    let manager = Manager::new(repo, root, "agent".to_string());

    let wt = manager.create("rca-1").expect("create");
    assert_eq!(wt.branch, "agent/rca-1");
    assert!(wt.path.is_dir());
    assert_eq!(current_branch(&wt.path), "agent/rca-1");

    // Freshly branched off origin/main: not ahead yet.
    assert!(!manager.has_commits(&wt).expect("has_commits"));

    // A commit in the worktree makes it ahead, and HEAD resolves.
    std::fs::write(wt.path.join("fix.txt"), "patch").expect("write");
    git_ok(&wt.path, &["add", "."]);
    git_ok(&wt.path, &["commit", "-m", "fix"]);
    assert!(manager.has_commits(&wt).expect("has_commits"));
    assert_eq!(manager.head_sha(&wt).expect("head").len(), 40);
}

#[test]
fn remove_deletes_worktree_and_branch() {
    let (_tmp, repo, root) = fixture();
    let manager = Manager::new(repo.clone(), root, "agent".to_string());

    let wt = manager.create("rca-2").expect("create");
    assert!(wt.path.is_dir());

    manager.remove(&wt).expect("remove");
    assert!(!wt.path.exists(), "worktree dir should be gone");

    let branches = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["branch", "--list", "agent/rca-2"])
        .output()
        .expect("branch list");
    assert!(
        String::from_utf8_lossy(&branches.stdout).trim().is_empty(),
        "branch should be deleted"
    );
}

#[test]
fn create_is_idempotent_after_a_stale_worktree() {
    let (_tmp, repo, root) = fixture();
    let manager = Manager::new(repo, root, "agent".to_string());

    // First attempt leaves a live worktree behind (as a crash would).
    let first = manager.create("rca-3").expect("first create");
    assert!(first.path.is_dir());

    // A second create for the same id must clear the stale one and succeed.
    let second = manager.create("rca-3").expect("second create");
    assert_eq!(second.path, first.path);
    assert!(second.path.is_dir());
    assert_eq!(current_branch(&second.path), "agent/rca-3");
}

#[test]
fn invalid_ids_are_rejected() {
    let (_tmp, repo, root) = fixture();
    let manager = Manager::new(repo, root, "agent".to_string());

    for bad in [
        "",
        "../evil",
        "has space",
        "UPPER",
        "weird!",
        &"x".repeat(65),
    ] {
        let err = manager.create(bad).expect_err("should reject");
        assert!(
            matches!(err, WorktreeError::InvalidId(_)),
            "{bad:?}: {err:?}"
        );
    }
}
