//! Serializable views of the CLI crate's types, shaped for the frontend.
//!
//! The frontend never sees the domain types directly: these DTOs are the
//! IPC contract, mirrored by `src/types.ts` on the TypeScript side. Keep
//! the two in sync when either changes.

use serde::Serialize;
use time::format_description::well_known::Rfc3339;

/// One workspace row for the sidebar.
#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub status: String,
    /// RFC 3339; empty string if the timestamp cannot format (never in
    /// practice — `created` round-trips from RFC 3339).
    pub created: String,
    pub systems: Vec<String>,
    pub tags: Vec<String>,
    pub prs: Vec<String>,
    pub archived: bool,
}

impl From<&beagle::model::RcaSummary> for Workspace {
    fn from(rca: &beagle::model::RcaSummary) -> Self {
        Self {
            id: rca.id.to_string(),
            title: rca.meta.title.clone(),
            severity: rca.meta.severity.to_string(),
            status: rca.meta.status.to_string(),
            created: rca.meta.created.format(&Rfc3339).unwrap_or_default(),
            systems: rca.meta.systems.clone(),
            tags: rca.meta.tags.clone(),
            prs: rca.meta.prs.clone(),
            archived: rca.archived,
        }
    }
}

/// A workspace directory that exists but could not load.
#[derive(Debug, Clone, Serialize)]
pub struct Broken {
    pub dir_name: String,
    pub reason: String,
}

/// Everything `list_workspaces` returns in one call.
#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub root: String,
    pub workspaces: Vec<Workspace>,
    pub broken: Vec<Broken>,
    pub warnings: Vec<String>,
}

/// One searchable line for the global finder: where it lives and what it
/// says. Title entries use the summary file and line 0.
#[derive(Debug, Clone, Serialize)]
pub struct CorpusLine {
    pub id: String,
    pub title: String,
    pub file: String,
    pub line: usize,
    pub text: String,
}
