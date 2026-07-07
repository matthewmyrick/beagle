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
/// most-active-first so open investigations sort above resolved ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Actively debugging; root cause unknown.
    Investigating,
    /// Root cause identified; fix not yet applied.
    Identified,
    /// Fix applied; watching telemetry for recurrence.
    Monitoring,
    /// Closed out; the write-up is the record.
    Resolved,
}

impl Status {
    /// Every status, most active first.
    pub const ALL: [Self; 4] = [
        Self::Investigating,
        Self::Identified,
        Self::Monitoring,
        Self::Resolved,
    ];

    /// Stable lowercase name, matching the serde representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Investigating => "investigating",
            Self::Identified => "identified",
            Self::Monitoring => "monitoring",
            Self::Resolved => "resolved",
        }
    }
}

impl FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|st| st.as_str() == s).ok_or_else(|| {
            format!("unknown status `{s}` (expected one of: investigating, identified, monitoring, resolved)")
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
}

/// A workspace as listed in the sidebar: identity plus manifest, no section
/// content (that is loaded lazily by the store when a tab is opened).
#[derive(Debug, Clone, PartialEq)]
pub struct RcaSummary {
    /// The workspace identifier (and directory name).
    pub id: RcaId,
    /// The parsed manifest.
    pub meta: RcaMeta,
}

impl RcaSummary {
    /// Sort key: open investigations first, then by severity, then newest
    /// first. This is the sidebar ordering.
    #[must_use]
    pub fn sort_key(&self) -> (Status, Severity, i64) {
        // Negate the timestamp so larger (newer) sorts first under `Ord`.
        (
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
    /// Raw evidence, queries, links, and loose ends.
    Notes,
}

impl SectionKind {
    /// Every section, in tab order.
    pub const ALL: [Self; 6] = [
        Self::Summary,
        Self::Timeline,
        Self::RootCause,
        Self::Impact,
        Self::Remediation,
        Self::Notes,
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
            Self::Notes => "notes.md",
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
            Self::Notes => "Notes",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

    use super::*;

    #[test]
    fn id_accepts_valid_slugs() {
        for slug in ["a", "payments-api-p99", "2026-07-05-outage", "x-1"] {
            assert!(RcaId::new(slug).is_ok(), "expected `{slug}` to be valid");
        }
    }

    #[test]
    fn id_rejects_invalid_slugs() {
        let too_long = "a".repeat(RcaId::MAX_LEN + 1);
        for slug in [
            "",
            "Has-Caps",
            "under_score",
            "sp ace",
            "dot.dot",
            "emoji-💥",
            too_long.as_str(),
        ] {
            assert!(
                RcaId::new(slug).is_err(),
                "expected `{slug}` to be rejected"
            );
        }
    }

    #[test]
    fn severity_round_trips_through_serde_and_fromstr() {
        for sev in Severity::ALL {
            let toml_value = toml::Value::try_from(sev).expect("severity serializes");
            let back: Severity = toml_value.try_into().expect("severity deserializes");
            assert_eq!(back, sev);
            assert_eq!(sev.as_str().parse::<Severity>(), Ok(sev));
        }
        assert!("catastrophic".parse::<Severity>().is_err());
    }

    #[test]
    fn status_round_trips_through_serde_and_fromstr() {
        for st in Status::ALL {
            let toml_value = toml::Value::try_from(st).expect("status serializes");
            let back: Status = toml_value.try_into().expect("status deserializes");
            assert_eq!(back, st);
            assert_eq!(st.as_str().parse::<Status>(), Ok(st));
        }
        assert!("closed".parse::<Status>().is_err());
    }

    #[test]
    fn manifest_rejects_unknown_fields() {
        let toml_src = r#"
            title = "t"
            severity = "high"
            status = "investigating"
            created = "2026-07-05T14:32:00Z"
            oops = "typo"
        "#;
        assert!(toml::from_str::<RcaMeta>(toml_src).is_err());
    }

    #[test]
    fn manifest_parses_minimal_and_defaults_lists() {
        let toml_src = r#"
            title = "Payments p99 latency"
            severity = "high"
            status = "identified"
            created = "2026-07-05T14:32:00Z"
        "#;
        let meta: RcaMeta = toml::from_str(toml_src).expect("minimal manifest parses");
        assert_eq!(meta.severity, Severity::High);
        assert!(meta.systems.is_empty());
        assert!(meta.tags.is_empty());
        assert!(meta.updated.is_none());
    }

    #[test]
    fn sort_key_puts_active_severe_recent_first() {
        let mk = |status, severity, ts| RcaSummary {
            id: RcaId::new("x").expect("valid test id"),
            meta: RcaMeta {
                title: String::new(),
                severity,
                status,
                created: OffsetDateTime::from_unix_timestamp(ts).expect("valid test timestamp"),
                updated: None,
                systems: Vec::new(),
                tags: Vec::new(),
            },
        };
        let resolved = mk(Status::Resolved, Severity::Critical, 100);
        let active_high = mk(Status::Investigating, Severity::High, 100);
        let active_crit_old = mk(Status::Investigating, Severity::Critical, 50);
        let active_crit_new = mk(Status::Investigating, Severity::Critical, 200);

        let mut list = vec![
            resolved.clone(),
            active_high.clone(),
            active_crit_old.clone(),
            active_crit_new.clone(),
        ];
        list.sort_by_key(RcaSummary::sort_key);
        assert_eq!(
            list,
            vec![active_crit_new, active_crit_old, active_high, resolved]
        );
    }
}
