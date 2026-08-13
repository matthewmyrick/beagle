//! The headless `claude` runner: spawns a session in its own process group,
//! streams its output to a per-run log file, and enforces a hard timeout by
//! killing the whole group.
//!
//! Every session is its own process group (via `Command::process_group`),
//! so a timeout or shutdown kills `claude` *and* every subprocess it spawned —
//! no stragglers. Each live session records its group id under a runs
//! directory; [`cleanup_orphans`] reads those on daemon startup and kills any
//! group a previous run left behind, so a crash or restart never leaks a
//! token-burning session. Deterministic Rust owns this lifecycle; `claude`
//! only edits files.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;

/// How long, after a kill signal, to keep reaping before giving up. Mirrors the
/// Go pipeline's 10s wait-delay.
const REAP_GRACE: Duration = Duration::from_secs(10);

/// How often the wait loop polls the child while watching the deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The filename extension for a session's registry file under the runs dir.
const REGISTRY_EXT: &str = "session";

/// What a finished (or killed) session produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The process exited on its own with this status code.
    Exited(i32),
    /// The process was terminated by this signal number.
    Signaled(i32),
    /// The process ran past its timeout and was killed by the runner.
    TimedOut,
}

impl Outcome {
    /// Whether the session finished successfully (exit code 0).
    #[must_use]
    pub fn is_success(self) -> bool {
        self == Outcome::Exited(0)
    }
}

/// The inputs for one `claude` session.
#[derive(Debug, Clone, Copy)]
pub struct SessionSpec<'a> {
    /// The isolated worktree the session runs in (its working directory and
    /// `--add-dir`).
    pub working_dir: &'a Path,
    /// The prompt handed to `claude -p`.
    pub prompt: &'a str,
    /// The `--allowedTools` list; empty omits the flag.
    pub allowed_tools: &'a [String],
    /// Where the session's combined stdout+stderr is written.
    pub log_path: &'a Path,
}

/// An error spawning or supervising a session.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The runs directory could not be created or read.
    #[error("runs directory {}: {source}", .path.display())]
    RunsDir {
        /// The runs directory path.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// The log file (or its parent directory) could not be created.
    #[error("log file {}: {source}", .path.display())]
    Log {
        /// The log path.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// `claude` could not be spawned at all.
    #[error("spawning {bin}: {source}")]
    Spawn {
        /// The program that failed to start.
        bin: String,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// Waiting on the child failed.
    #[error("waiting on session: {0}")]
    Wait(io::Error),
}

/// Spawns headless `claude` sessions and supervises their process groups.
pub struct Runner {
    claude_bin: OsString,
    runs_dir: PathBuf,
}

impl Runner {
    /// Creates a runner that records live sessions under `runs_dir` and invokes
    /// `claude` from `$PATH`.
    #[must_use]
    pub fn new(runs_dir: PathBuf) -> Self {
        Self {
            claude_bin: OsString::from("claude"),
            runs_dir,
        }
    }

    /// Overrides the `claude` executable (used by tests to point at a shim).
    #[must_use]
    pub fn with_claude_bin(mut self, bin: impl Into<OsString>) -> Self {
        self.claude_bin = bin.into();
        self
    }

    /// Spawns a session and supervises it to completion or `timeout`, whichever
    /// comes first. On timeout the entire process group is killed.
    ///
    /// # Errors
    /// Returns [`RunError`] if the runs dir or log file cannot be created, if
    /// `claude` cannot be spawned, or if waiting on the child fails.
    pub fn run(&self, spec: SessionSpec, timeout: Duration) -> Result<Outcome, RunError> {
        self.spawn(spec)?.wait(timeout)
    }

    /// Spawns a session in its own process group and records it under the runs
    /// dir, returning a handle to supervise it.
    ///
    /// # Errors
    /// Returns [`RunError`] if the runs dir or log file cannot be created or if
    /// `claude` cannot be spawned.
    pub fn spawn(&self, spec: SessionSpec) -> Result<Session, RunError> {
        std::fs::create_dir_all(&self.runs_dir).map_err(|source| RunError::RunsDir {
            path: self.runs_dir.clone(),
            source,
        })?;
        if let Some(parent) = spec.log_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| RunError::Log {
                path: spec.log_path.to_path_buf(),
                source,
            })?;
        }
        let log = File::create(spec.log_path).map_err(|source| RunError::Log {
            path: spec.log_path.to_path_buf(),
            source,
        })?;
        let log_err = log.try_clone().map_err(|source| RunError::Log {
            path: spec.log_path.to_path_buf(),
            source,
        })?;

        let mut command = Command::new(&self.claude_bin);
        command
            .current_dir(spec.working_dir)
            .arg("-p")
            .arg(spec.prompt)
            .arg("--add-dir")
            .arg(spec.working_dir)
            .args(["--output-format", "text"]);
        if !spec.allowed_tools.is_empty() {
            command
                .arg("--allowedTools")
                .arg(spec.allowed_tools.join(" "));
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .process_group(0); // new process group, pgid == child pid

        let child = command.spawn().map_err(|source| RunError::Spawn {
            bin: self.claude_bin.to_string_lossy().into_owned(),
            source,
        })?;

        // A PID always fits in an i32 on Unix; if it somehow does not, refuse to
        // proceed rather than risk signalling the wrong group.
        let Ok(pgid) = i32::try_from(child.id()) else {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(RunError::Spawn {
                bin: self.claude_bin.to_string_lossy().into_owned(),
                source: io::Error::other("child pid does not fit in i32"),
            });
        };

        let registry_file = self.runs_dir.join(format!("{pgid}.{REGISTRY_EXT}"));
        // Best-effort: the in-process wait() is the primary reaper; this file is
        // only the cross-restart backstop that cleanup_orphans reads.
        let _ = std::fs::write(&registry_file, spec.log_path.to_string_lossy().as_bytes());

        Ok(Session {
            child,
            pgid,
            registry_file,
        })
    }
}

/// A live, supervised `claude` session.
pub struct Session {
    child: Child,
    pgid: i32,
    registry_file: PathBuf,
}

impl Session {
    /// The session's process-group id (equal to the child's pid).
    #[must_use]
    pub fn pgid(&self) -> i32 {
        self.pgid
    }

    /// Waits for the session to finish, killing its whole process group if it
    /// runs past `timeout`. Consumes the session and removes its registry file.
    ///
    /// # Errors
    /// Returns [`RunError::Wait`] if waiting on the child fails.
    pub fn wait(mut self, timeout: Duration) -> Result<Outcome, RunError> {
        let deadline = Instant::now() + timeout;
        let outcome = loop {
            match self.child.try_wait().map_err(RunError::Wait)? {
                Some(status) => break outcome_from_status(status),
                None if Instant::now() >= deadline => {
                    break self.kill_group()?;
                }
                None => sleep(POLL_INTERVAL),
            }
        };
        let _ = std::fs::remove_file(&self.registry_file);
        Ok(outcome)
    }

    /// Kills the process group and reaps the leader, returning [`Outcome::TimedOut`].
    fn kill_group(&mut self) -> Result<Outcome, RunError> {
        signal_group(self.pgid, Signal::SIGKILL);
        let reap_deadline = Instant::now() + REAP_GRACE;
        loop {
            match self.child.try_wait().map_err(RunError::Wait)? {
                Some(_) => return Ok(Outcome::TimedOut),
                None if Instant::now() >= reap_deadline => return Ok(Outcome::TimedOut),
                None => sleep(POLL_INTERVAL),
            }
        }
    }
}

/// Kills any process groups recorded under `runs_dir` by sessions that a
/// previous run left behind, then removes their registry files. Safe to call on
/// every daemon startup: a missing directory is not an error, and signalling an
/// already-dead group is ignored.
///
/// Returns the number of registry files processed.
///
/// # Errors
/// Returns [`RunError::RunsDir`] if the directory exists but cannot be read.
pub fn cleanup_orphans(runs_dir: &Path) -> Result<usize, RunError> {
    let entries = match std::fs::read_dir(runs_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(RunError::RunsDir {
                path: runs_dir.to_path_buf(),
                source,
            });
        }
    };
    let mut cleaned = 0;
    for entry in entries {
        let path = entry
            .map_err(|source| RunError::RunsDir {
                path: runs_dir.to_path_buf(),
                source,
            })?
            .path();
        if path.extension() != Some(OsStr::new(REGISTRY_EXT)) {
            continue;
        }
        if let Some(pgid) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(parse_pgid)
        {
            signal_group(pgid, Signal::SIGKILL);
            cleaned += 1;
        }
        let _ = std::fs::remove_file(&path);
    }
    Ok(cleaned)
}

/// Sends `signal` to the whole process group `pgid`, ignoring the error a
/// group that has already exited produces.
fn signal_group(pgid: i32, signal: Signal) {
    let _ = killpg(Pid::from_raw(pgid), signal);
}

/// Parses a positive process-group id from a registry filename stem.
fn parse_pgid(stem: &str) -> Option<i32> {
    match stem.parse::<i32>() {
        Ok(pgid) if pgid > 0 => Some(pgid),
        _ => None,
    }
}

/// Maps a finished child's exit status to an [`Outcome`].
fn outcome_from_status(status: ExitStatus) -> Outcome {
    if let Some(code) = status.code() {
        Outcome::Exited(code)
    } else if let Some(signal) = status.signal() {
        Outcome::Signaled(signal)
    } else {
        Outcome::Signaled(0)
    }
}

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
