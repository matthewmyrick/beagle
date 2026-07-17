//! The typed data model for RCA workspaces.
//!
//! Everything external (TOML manifests, CLI arguments) is parsed into these
//! types at the boundary; past this module, invalid states are
//! unrepresentable. This module depends on nothing else in the crate except
//! [`crate::error`].

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::Error;

/// A validated RCA workspace identifier: a lowercase slug, `[a-z0-9-]`,
/// 1..=64 characters, doubling as the workspace directory name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RcaId(String);

impl RcaId {
    /// Maximum identifier length in bytes.
    pub const MAX_LEN: usize = 64;

    /// Validates and wraps a slug.
    ///
    /// # Errors
    /// Returns [`Error::InvalidId`] if the slug is empty, longer than
    /// [`Self::MAX_LEN`], or contains characters outside `[a-z0-9-]`.
    pub fn new(slug: impl Into<String>) -> Result<Self, Error> {
        let slug = slug.into();
        let valid_len = !slug.is_empty() && slug.len() <= Self::MAX_LEN;
        let valid_chars = slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid_len && valid_chars {
            Ok(Self(slug))
        } else {
            Err(Error::InvalidId(slug))
        }
    }

    /// The slug as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RcaId {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RcaId> for String {
    fn from(id: RcaId) -> Self {
        id.0
    }
}

impl fmt::Display for RcaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How bad the incident is. Ordering is most-severe-first so sorting a list
/// of workspaces by severity puts fires at the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Full outage or data loss; all hands.
    Critical,
    /// Major degradation with user impact.
    High,
    /// Partial degradation or a broken non-critical path.
    Medium,
    /// Minor issue, cosmetic impact, or near miss.
    Low,
    /// Informational write-up; no live impact.
    Info,
}

impl Severity {
    /// Every severity, most severe first.
    pub const ALL: [Self; 5] = [
        Self::Critical,
        Self::High,
        Self::Medium,
        Self::Low,
        Self::Info,
    ];

    /// Stable lowercase name, matching the serde representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Info => "info",
        }
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|sev| sev.as_str() == s)
            .ok_or_else(|| {
                format!(
                    "unknown severity `{s}` (expected one of: critical, high, medium, low, info)"
                )
            })
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where the investigation currently stands. Ordering is
/// most-active-first so open investigations sort above finished ones —
/// `investigating` tops the sidebar, `finished` sinks to the very bottom.
///
/// The pre-0.6 names still parse (`identified` → `review`, `monitoring` →
/// `final-review`, `resolved` → `finished`) so old manifests keep loading;
/// writes always use the new names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Actively debugging; root cause unknown.
    Investigating,
    /// Root cause found; the fix (usually a PR) is out for review.
    #[serde(alias = "identified")]
    Review,
    /// The fix has merged — verify it actually worked via the Final
    /// Review checklist, then sign off.
    #[serde(alias = "monitoring")]
    FinalReview,
    /// Verified and closed; the write-up is the record.
    #[serde(alias = "resolved")]
    Finished,
}

impl Status {
    /// Every status, most active first.
    pub const ALL: [Self; 4] = [
        Self::Investigating,
        Self::Review,
        Self::FinalReview,
        Self::Finished,
    ];

    /// Stable name, matching the serde representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Investigating => "investigating",
            Self::Review => "review",
            Self::FinalReview => "final-review",
            Self::Finished => "finished",
        }
    }

    /// The pre-0.6 name this status replaces, accepted as CLI/manifest
    /// input for compatibility.
    fn legacy_alias(self) -> Option<&'static str> {
        match self {
            Self::Investigating => None,
            Self::Review => Some("identified"),
            Self::FinalReview => Some("monitoring"),
            Self::Finished => Some("resolved"),
        }
    }
}

impl FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|st| st.as_str() == s || st.legacy_alias() == Some(s))
            .ok_or_else(|| {
                format!(
                    "unknown status `{s}` (expected one of: investigating, review, \
                     final-review, finished)"
                )
            })
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The parsed contents of an `rca.toml` manifest.
///
/// Unknown fields are rejected at parse time so typos surface immediately
/// rather than being silently dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RcaMeta {
    /// Human-readable one-line incident title.
    pub title: String,
    /// Incident severity.
    pub severity: Severity,
    /// Investigation status.
    pub status: Status,
    /// When the investigation was opened (RFC 3339 string in TOML).
    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,
    /// When the workspace was last meaningfully updated, if tracked.
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated: Option<OffsetDateTime>,
    /// Systems involved (service names, hosts, queues, ...).
    #[serde(default)]
    pub systems: Vec<String>,
    /// Free-form tags for grouping and search.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Pull requests remediating this incident, as URLs. Attach with
    /// `beagle pr add`; the TUI shows live merge status when `gh` is
    /// available. Omitted from the manifest while empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prs: Vec<String>,
    /// Whether this incident is published to the public web app. Opt-in
    /// per incident (`beagle publish`); the static site only includes
    /// flagged RCAs, and only their client-safe sections. Defaults to
    /// `false` and is omitted from the manifest while unset, so existing
    /// workspaces and internal-only incidents stay private.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub published: bool,
    /// When the incident was published, if it is. Stamped by
    /// `beagle publish`; shown as the "published" date on the public page.
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    pub published_at: Option<OffsetDateTime>,
}

/// A workspace as listed in the sidebar: identity plus manifest, no section
/// content (that is loaded lazily by the store when a tab is opened).
#[derive(Debug, Clone, PartialEq)]
pub struct RcaSummary {
    /// The workspace identifier (and directory name).
    pub id: RcaId,
    /// The parsed manifest.
    pub meta: RcaMeta,
    /// Whether the workspace lives under `rcas/archive/` — out of the way
    /// but never deleted; the TUI hides archived incidents by default.
    pub archived: bool,
}

impl RcaSummary {
    /// Sort key: active before archived, open investigations first, then by
    /// severity, then newest first. This is the sidebar ordering.
    #[must_use]
    pub fn sort_key(&self) -> (bool, Status, Severity, i64) {
        // Negate the timestamp so larger (newer) sorts first under `Ord`.
        (
            self.archived,
            self.meta.status,
            self.meta.severity,
            -self.meta.created.unix_timestamp(),
        )
    }
}

/// The markdown sections every workspace can contain. Each maps to one file
/// in the workspace directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionKind {
    /// What broke, in three sentences. The tab a responder reads first.
    Summary,
    /// What happened when, as observed in telemetry.
    Timeline,
    /// Why it broke: the causal chain down to the root.
    RootCause,
    /// Who and what was affected, quantified.
    Impact,
    /// How to fix it: immediate mitigation and durable fix.
    Remediation,
    /// The verification checklist: concrete, checkable predictions of what
    /// "fixed" looks like, written **during** the investigation so the
    /// `final-review` phase knows exactly what to confirm once the fix
    /// PR merges.
    FinalReview,
    /// Raw evidence, queries, links, and loose ends.
    Notes,
    /// Append-only investigation log: what the investigator did, when.
    /// The live "watch the agent think" stream.
    Log,
}

impl SectionKind {
    /// Every section, in tab order.
    pub const ALL: [Self; 8] = [
        Self::Summary,
        Self::Timeline,
        Self::RootCause,
        Self::Impact,
        Self::Remediation,
        Self::FinalReview,
        Self::Notes,
        Self::Log,
    ];

    /// The file name backing this section inside the workspace directory.
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Summary => "summary.md",
            Self::Timeline => "timeline.md",
            Self::RootCause => "root-cause.md",
            Self::Impact => "impact.md",
            Self::Remediation => "remediation.md",
            Self::FinalReview => "final-review.md",
            Self::Notes => "notes.md",
            Self::Log => "log.md",
        }
    }

    /// The human-readable tab title.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Timeline => "Timeline",
            Self::RootCause => "Root Cause",
            Self::Impact => "Impact",
            Self::Remediation => "Fix",
            Self::FinalReview => "Final Review",
            Self::Notes => "Notes",
            Self::Log => "Log",
        }
    }
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
