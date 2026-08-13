//! Durable job state (`SQLite`): the `pending → processing → done/failed`
//! lifecycle for the agents the daemon runs.
//!
//! One row per job, keyed by RCA id. The store is a single writer over a
//! WAL-mode connection with a busy timeout, so concurrent readers never block
//! it and a contended write waits instead of failing. On [`Store::open`] any
//! job left `processing` by a previous run — i.e. one whose agent died — is
//! returned to `pending`, so a crash or restart never strands work.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection};

/// The schema, applied on every open. `IF NOT EXISTS` makes it safe to re-run.
const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS jobs (
    id          TEXT PRIMARY KEY,                -- RCA id (workspace slug)
    title       TEXT,
    state       TEXT NOT NULL DEFAULT 'pending', -- pending | processing | done | failed
    attempts    INTEGER NOT NULL DEFAULT 0,
    pr_url      TEXT,
    fail_kind   TEXT,
    fail_reason TEXT,
    updated     TEXT
);";

/// One agent job's lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// Queued and actionable.
    Pending,
    /// A session is (or was) working on it.
    Processing,
    /// Finished successfully.
    Done,
    /// Gave up after a failure.
    Failed,
}

impl JobState {
    /// Parses the on-disk `state` text, returning `None` for an unknown value.
    fn from_db(text: &str) -> Option<Self> {
        match text {
            "pending" => Some(Self::Pending),
            "processing" => Some(Self::Processing),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// A full job row, as read back from the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The RCA id this job tracks.
    pub id: String,
    /// The RCA title, if known.
    pub title: Option<String>,
    /// Where the job is in its lifecycle.
    pub state: JobState,
    /// How many times a session has been started for it.
    pub attempts: u32,
    /// The remediation PR URL, set once the job is `done`.
    pub pr_url: Option<String>,
    /// On failure, a short classification (e.g. `ci`, `pipeline`).
    pub fail_kind: Option<String>,
    /// On failure, a human-readable reason.
    pub fail_reason: Option<String>,
    /// When the row last changed (`datetime('now')` text), if ever.
    pub updated: Option<String>,
}

/// A pending job the poller may act on: the subset [`Store::select_actionable`]
/// returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actionable {
    /// The RCA id.
    pub id: String,
    /// The RCA title, if known.
    pub title: Option<String>,
    /// How many attempts have already been made.
    pub attempts: u32,
}

/// An error from the job store (an underlying `SQLite` failure).
#[derive(Debug, thiserror::Error)]
#[error("job store: {0}")]
pub struct DbError(#[from] rusqlite::Error);

/// A durable, single-writer `SQLite` store of agent job state.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (creating if needed) the store at `path`, enabling WAL and a busy
    /// timeout, ensuring the schema, and running crash recovery: any job left
    /// `processing` by a previous run is returned to `pending`.
    ///
    /// # Errors
    /// Returns [`DbError`] if the database cannot be opened or initialized.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\nPRAGMA synchronous = NORMAL;\nPRAGMA foreign_keys = ON;",
        )?;
        conn.execute_batch(SCHEMA)?;
        let store = Self { conn };
        store.reset_processing()?;
        Ok(store)
    }

    /// Registers a job as `pending`. `INSERT OR IGNORE` keeps any existing row
    /// untouched, so a job already `done`/`failed`/`processing` is never reset
    /// by re-seeing it.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn upsert_pending(&self, id: &str, title: Option<&str>) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO jobs (id, title, state) VALUES (?1, ?2, 'pending')",
            params![id, title],
        )?;
        Ok(())
    }

    /// Marks a job `processing` and clears any prior failure fields.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn mark_processing(&self, id: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE jobs SET state = 'processing', fail_kind = NULL, fail_reason = NULL, \
             updated = datetime('now') WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Marks a job `done`, recording its remediation PR URL.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn mark_done(&self, id: &str, pr_url: Option<&str>) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE jobs SET state = 'done', pr_url = ?2, fail_kind = NULL, fail_reason = NULL, \
             updated = datetime('now') WHERE id = ?1",
            params![id, pr_url],
        )?;
        Ok(())
    }

    /// Marks a job `failed`, recording a classification and a reason.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn mark_failed(&self, id: &str, fail_kind: &str, fail_reason: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE jobs SET state = 'failed', fail_kind = ?2, fail_reason = ?3, \
             updated = datetime('now') WHERE id = ?1",
            params![id, fail_kind, fail_reason],
        )?;
        Ok(())
    }

    /// Returns a job to `pending` for a retry, clearing any failure fields.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn mark_pending(&self, id: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE jobs SET state = 'pending', fail_kind = NULL, fail_reason = NULL, \
             updated = datetime('now') WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Increments a job's attempt counter and returns the new count.
    ///
    /// # Errors
    /// Returns [`DbError`] if the job does not exist or on a `SQLite` failure.
    pub fn increment_attempts(&self, id: &str) -> Result<u32, DbError> {
        let attempts: i64 = self.conn.query_row(
            "UPDATE jobs SET attempts = attempts + 1, updated = datetime('now') \
             WHERE id = ?1 RETURNING attempts",
            [id],
            |row| row.get(0),
        )?;
        Ok(u32::try_from(attempts).unwrap_or(u32::MAX))
    }

    /// Returns the pending jobs still under the retry cap (`attempts <
    /// max_attempts`), ordered by id. A job that has hit the cap stays in the
    /// table but is no longer actionable.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn select_actionable(&self, max_attempts: u32) -> Result<Vec<Actionable>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, attempts FROM jobs \
             WHERE state = 'pending' AND attempts < ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([max_attempts], |row| {
            let attempts: i64 = row.get(2)?;
            Ok(Actionable {
                id: row.get(0)?,
                title: row.get(1)?,
                attempts: u32::try_from(attempts).unwrap_or(u32::MAX),
            })
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    /// Reads one job by id, or `None` if there is no such row.
    ///
    /// # Errors
    /// Returns [`DbError`] on a `SQLite` failure.
    pub fn job(&self, id: &str) -> Result<Option<Job>, DbError> {
        let result = self.conn.query_row(
            "SELECT id, title, state, attempts, pr_url, fail_kind, fail_reason, updated \
             FROM jobs WHERE id = ?1",
            [id],
            row_to_job,
        );
        match result {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Crash recovery: return every `processing` job to `pending`, since a job
    /// left `processing` has no live session behind it. Run once on open.
    fn reset_processing(&self) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE jobs SET state = 'pending', updated = datetime('now') WHERE state = 'processing'",
            [],
        )?;
        Ok(())
    }
}

/// Maps a full `jobs` row to a [`Job`]. A stored `state` that is not one of the
/// known values is a data-integrity error and surfaces as a conversion failure.
fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    let state_text: String = row.get(2)?;
    let state = JobState::from_db(&state_text).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            format!("unknown job state `{state_text}`").into(),
        )
    })?;
    let attempts: i64 = row.get(3)?;
    Ok(Job {
        id: row.get(0)?,
        title: row.get(1)?,
        state,
        attempts: u32::try_from(attempts).unwrap_or(u32::MAX),
        pr_url: row.get(4)?,
        fail_kind: row.get(5)?,
        fail_reason: row.get(6)?,
        updated: row.get(7)?,
    })
}

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;
