//! Isolated git worktrees of a target repo, so concurrent agents never share a
//! checkout. Deterministic Rust owns the worktree lifecycle end to end;
//! `claude` only edits files inside the worktree it is handed.
//!
//! [`Manager::create`] branches a fresh worktree off `origin/main`;
//! [`Manager::remove`] tears the worktree and its branch back down. Creation is
//! idempotent — a retry after a crash clears any stale worktree or branch
//! first — so a failed run never leaves a dangling checkout. Operations that
//! mutate the shared base repo (fetch, `worktree add`/`remove`, `branch -D`)
//! are serialized behind a mutex, since concurrent `git fetch` races on the
//! `refs/remotes/origin/main` lock.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The longest an RCA id (and thus a branch/dir component) may be.
const MAX_ID_LEN: usize = 64;

/// A created worktree: its checkout directory and the branch checked out there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// The absolute worktree directory.
    pub path: PathBuf,
    /// The branch created for it (`<prefix>/<id>`).
    pub branch: String,
}

/// An error managing a worktree.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    /// The RCA id is not a safe slug for a branch/path component.
    #[error("invalid worktree id `{0}`: must be a non-empty [a-z0-9-] slug up to 64 chars")]
    InvalidId(String),
    /// `git` could not be launched.
    #[error("running git: {source}")]
    Spawn {
        /// The underlying I/O error.
        source: io::Error,
    },
    /// The worktree root directory could not be created.
    #[error("creating worktree root {}: {source}", .path.display())]
    Root {
        /// The root path.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// A git command exited non-zero.
    #[error("git {args} failed: {stderr}")]
    Git {
        /// The git arguments that failed.
        args: String,
        /// Git's stderr.
        stderr: String,
    },
}

/// Creates and removes isolated worktrees of one target repository.
pub struct Manager {
    repo_dir: PathBuf,
    root: PathBuf,
    branch_prefix: String,
    repo_lock: Mutex<()>,
}

impl Manager {
    /// Manages worktrees of `repo_dir`, placing each checkout under `root` and
    /// naming branches `<branch_prefix>/<id>`.
    #[must_use]
    pub fn new(repo_dir: PathBuf, root: PathBuf, branch_prefix: String) -> Self {
        Self {
            repo_dir,
            root,
            branch_prefix,
            repo_lock: Mutex::new(()),
        }
    }

    /// Creates a fresh worktree for `id`, branched off `origin/main`. Idempotent:
    /// any stale worktree, directory, or branch from a previous attempt is
    /// cleared first.
    ///
    /// # Errors
    /// Returns [`WorktreeError`] if `id` is not a valid slug, the root cannot be
    /// created, git cannot be launched, or `git fetch`/`worktree add` fails.
    pub fn create(&self, id: &str) -> Result<Worktree, WorktreeError> {
        if !valid_id(id) {
            return Err(WorktreeError::InvalidId(id.to_string()));
        }
        let branch = format!("{}/{}", self.branch_prefix, id);
        let path = self.root.join(id);

        let _guard = self.lock();
        std::fs::create_dir_all(&self.root).map_err(|source| WorktreeError::Root {
            path: self.root.clone(),
            source,
        })?;

        // Idempotent cleanup of anything a crashed prior attempt left behind.
        let _ = Self::git(
            &self.repo_dir,
            &[
                "worktree".as_ref(),
                "remove".as_ref(),
                "--force".as_ref(),
                path.as_os_str(),
            ],
        );
        let _ = Self::git(&self.repo_dir, &["worktree".as_ref(), "prune".as_ref()]);
        let _ = Self::git(
            &self.repo_dir,
            &["branch".as_ref(), "-D".as_ref(), branch.as_ref()],
        );
        let _ = std::fs::remove_dir_all(&path);

        Self::git(
            &self.repo_dir,
            &["fetch".as_ref(), "origin".as_ref(), "main".as_ref()],
        )?;
        Self::git(
            &self.repo_dir,
            &[
                "worktree".as_ref(),
                "add".as_ref(),
                "-b".as_ref(),
                branch.as_ref(),
                path.as_os_str(),
                "origin/main".as_ref(),
            ],
        )?;
        Ok(Worktree { path, branch })
    }

    /// Tears down `worktree` and deletes its branch. Best-effort per step, so a
    /// partially-removed worktree still gets cleaned up.
    ///
    /// # Errors
    /// Returns [`WorktreeError`] if `git worktree remove` fails.
    pub fn remove(&self, worktree: &Worktree) -> Result<(), WorktreeError> {
        let _guard = self.lock();
        let result = Self::git(
            &self.repo_dir,
            &[
                "worktree".as_ref(),
                "remove".as_ref(),
                "--force".as_ref(),
                worktree.path.as_os_str(),
            ],
        );
        let _ = Self::git(&self.repo_dir, &["worktree".as_ref(), "prune".as_ref()]);
        let _ = Self::git(
            &self.repo_dir,
            &["branch".as_ref(), "-D".as_ref(), worktree.branch.as_ref()],
        );
        let _ = std::fs::remove_dir_all(&worktree.path);
        result.map(|_| ())
    }

    /// Reports whether the worktree branch is ahead of `origin/main` (i.e. the
    /// session actually committed something).
    ///
    /// # Errors
    /// Returns [`WorktreeError`] if the count cannot be read.
    pub fn has_commits(&self, worktree: &Worktree) -> Result<bool, WorktreeError> {
        let out = Self::git(
            &worktree.path,
            &[
                "rev-list".as_ref(),
                "--count".as_ref(),
                "origin/main..HEAD".as_ref(),
            ],
        )?;
        Ok(out.trim().parse::<u64>().unwrap_or(0) > 0)
    }

    /// Returns the worktree's current `HEAD` commit SHA.
    ///
    /// # Errors
    /// Returns [`WorktreeError`] if `git rev-parse` fails.
    pub fn head_sha(&self, worktree: &Worktree) -> Result<String, WorktreeError> {
        Self::git(&worktree.path, &["rev-parse".as_ref(), "HEAD".as_ref()])
    }

    /// Recovers the repo lock even if a previous holder panicked — a poisoned
    /// lock is not a reason to wedge the daemon.
    fn lock(&self) -> MutexGuard<'_, ()> {
        self.repo_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `git -C <dir> <args...>`, returning trimmed stdout on success.
    fn git(dir: &Path, args: &[&OsStr]) -> Result<String, WorktreeError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .map_err(|source| WorktreeError::Spawn { source })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let rendered = args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ");
            Err(WorktreeError::Git {
                args: rendered,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }
}

/// Whether `id` is a safe branch/path component: a non-empty `[a-z0-9-]` slug of
/// at most 64 chars (beagle's RCA-id rules), which also blocks path traversal.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
#[path = "tests/worktree.rs"]
mod tests;
