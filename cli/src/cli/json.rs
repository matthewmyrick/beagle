//! Machine-readable rendering of what the CLI prints.
//!
//! Pure like the parser beside it: typed model in, `String` out, no I/O.
//!
//! The shape here is the CLI's own contract, deliberately independent of the
//! on-disk manifest. `rca.toml` omits empty and unset fields to stay tidy; a
//! script reading this output should never have to tell "absent" from
//! "empty", so every key is always present — `null` for unset timestamps,
//! `[]` for empty lists.

use serde::Serialize;
use time::OffsetDateTime;

use beagle::model::{RcaSummary, Severity, Status};
use beagle::Error;

/// One workspace, as emitted by `beagle list --json`.
#[derive(Debug, Serialize)]
struct ListEntry<'a> {
    /// The workspace slug, which is also its directory name under `rcas/`.
    id: &'a str,
    /// One-line incident title.
    title: &'a str,
    /// `critical` | `high` | `medium` | `low` | `info`.
    severity: Severity,
    /// `investigating` | `review` | `agent` | `final-review` | `finished`.
    status: Status,
    /// When the investigation was opened, RFC 3339.
    #[serde(with = "time::serde::rfc3339")]
    created: OffsetDateTime,
    /// Last meaningful update, RFC 3339, or `null` when untracked.
    #[serde(with = "time::serde::rfc3339::option")]
    updated: Option<OffsetDateTime>,
    /// Systems involved (service names, hosts, queues, ...).
    systems: &'a [String],
    /// Free-form tags.
    tags: &'a [String],
    /// Attached remediation PR URLs.
    prs: &'a [String],
    /// Whether the incident is published to the public web app.
    published: bool,
    /// When it was published, RFC 3339, or `null` when it is not.
    #[serde(with = "time::serde::rfc3339::option")]
    published_at: Option<OffsetDateTime>,
    /// Whether the workspace lives under `rcas/archive/`.
    archived: bool,
}

impl<'a> From<&'a RcaSummary> for ListEntry<'a> {
    fn from(rca: &'a RcaSummary) -> Self {
        Self {
            id: rca.id.as_str(),
            title: &rca.meta.title,
            severity: rca.meta.severity,
            status: rca.meta.status,
            created: rca.meta.created,
            updated: rca.meta.updated,
            systems: &rca.meta.systems,
            tags: &rca.meta.tags,
            prs: &rca.meta.prs,
            published: rca.meta.published,
            published_at: rca.meta.published_at,
            archived: rca.archived,
        }
    }
}

/// Renders workspaces as a pretty-printed JSON array, one object per
/// workspace, in the order given.
///
/// Broken workspaces are deliberately not representable: the array stays
/// parseable as a list of workspaces, and their reasons go to stderr with
/// the other warnings.
///
/// # Errors
/// Returns [`Error::SerializeJson`] if serialization fails. The entries are
/// plain data with string keys, so that is a programmer error rather than
/// anything a workspace on disk can trigger.
pub fn list(summaries: &[&RcaSummary]) -> Result<String, Error> {
    let entries: Vec<ListEntry<'_>> = summaries.iter().copied().map(ListEntry::from).collect();
    Ok(serde_json::to_string_pretty(&entries)?)
}

#[cfg(test)]
#[path = "tests/json.rs"]
mod tests;
