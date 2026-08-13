//! The RCA trigger poller: reads the beagle RCA store, finds workspaces sitting
//! at the trigger status (v1: `agent`), and enqueues each as a durable job.
//!
//! This is the read side of the loop — it never mutates an RCA. Enqueuing goes
//! through [`crate::store::Store::upsert_pending`], whose `INSERT OR IGNORE`
//! semantics make re-polling safe: a job already `done`, `failed`, or
//! `processing` is never dragged back to `pending`, so the same RCA is picked
//! up exactly once until it is finished.

use beagle::model::{RcaId, SectionKind};
use beagle::store::Store as RcaStore;

use crate::store::{DbError, Store as JobStore};

/// An RCA currently sitting at the trigger status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triggered {
    /// The RCA id (workspace slug).
    pub id: String,
    /// The RCA title, for display.
    pub title: Option<String>,
}

/// An error polling the RCA store or enqueuing jobs.
#[derive(Debug, thiserror::Error)]
pub enum PollError {
    /// Reading the beagle RCA store failed.
    #[error("reading rca store: {0}")]
    Rcas(#[from] beagle::Error),
    /// Writing to the job store failed.
    #[error("job store: {0}")]
    Jobs(#[from] DbError),
}

/// Reads the RCA store and enqueues the RCAs at a configured trigger status.
pub struct Poller<'a> {
    rcas: &'a RcaStore,
    trigger_status: &'a str,
}

impl<'a> Poller<'a> {
    /// Creates a poller over `rcas` that fires for RCAs whose status equals
    /// `trigger_status` (e.g. `agent`).
    #[must_use]
    pub fn new(rcas: &'a RcaStore, trigger_status: &'a str) -> Self {
        Self {
            rcas,
            trigger_status,
        }
    }

    /// Returns the active (non-archived) RCAs currently at the trigger status.
    ///
    /// # Errors
    /// Returns [`PollError`] if the RCA store cannot be listed.
    pub fn triggered(&self) -> Result<Vec<Triggered>, PollError> {
        let listing = self.rcas.list()?;
        let triggered = listing
            .summaries
            .into_iter()
            .filter(|summary| summary.meta.status.as_str() == self.trigger_status)
            .map(|summary| Triggered {
                id: summary.id.as_str().to_string(),
                title: Some(summary.meta.title),
            })
            .collect();
        Ok(triggered)
    }

    /// Enqueues every triggered RCA into `jobs` and returns them. Safe to call on
    /// every poll: `upsert_pending` ignores RCAs already tracked, so nothing is
    /// re-run.
    ///
    /// # Errors
    /// Returns [`PollError`] if the RCA store cannot be read or a job cannot be
    /// enqueued.
    pub fn enqueue(&self, jobs: &JobStore) -> Result<Vec<Triggered>, PollError> {
        let triggered = self.triggered()?;
        for rca in &triggered {
            jobs.upsert_pending(&rca.id, rca.title.as_deref())?;
        }
        Ok(triggered)
    }

    /// Reads an RCA's `remediation.md` — the payload the agent implements.
    /// Returns `None` if the RCA has no remediation section yet.
    ///
    /// # Errors
    /// Returns [`PollError`] if `id` is not a valid RCA id or the section cannot
    /// be read.
    pub fn read_remediation(&self, id: &str) -> Result<Option<String>, PollError> {
        let rca_id = RcaId::new(id)?;
        Ok(self.rcas.read_section(&rca_id, SectionKind::Remediation)?)
    }
}

#[cfg(test)]
#[path = "tests/poller.rs"]
mod tests;
