//! Tests for the machine-readable rendering (`cli::json`).
#![allow(clippy::expect_used, clippy::panic)] // panicking is the correct failure mode in tests

use beagle::model::{RcaId, RcaMeta, RcaSummary, Severity, Status};
use time::macros::datetime;

use super::list;

fn summary(id: &str, archived: bool) -> RcaSummary {
    RcaSummary {
        id: RcaId::new(id).expect("valid test id"),
        meta: RcaMeta {
            title: "Payments API p99 latency 40x regression".to_owned(),
            severity: Severity::High,
            status: Status::Review,
            created: datetime!(2026-07-17 13:40:34.352325 UTC),
            updated: Some(datetime!(2026-07-17 19:11:15.477876 UTC)),
            systems: vec!["payments-api".to_owned(), "redis-sessions".to_owned()],
            tags: Vec::new(),
            prs: vec!["https://github.com/acme/infra/pull/4212".to_owned()],
            published: false,
            published_at: None,
        },
        archived,
    }
}

#[test]
fn renders_one_object_per_workspace() {
    let first = summary("2026-07-15-payments-api-p99-latency", false);
    let second = summary("2026-01-02-old-incident", true);
    let rendered = list(&[&first, &second]).expect("plain data serializes");

    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("valid JSON");
    let entries = parsed.as_array().expect("a JSON array");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["id"], "2026-07-15-payments-api-p99-latency");
    assert_eq!(
        entries[0]["title"],
        "Payments API p99 latency 40x regression"
    );
    assert_eq!(entries[0]["severity"], "high");
    assert_eq!(entries[0]["status"], "review");
    assert_eq!(entries[0]["created"], "2026-07-17T13:40:34.352325Z");
    assert_eq!(entries[0]["updated"], "2026-07-17T19:11:15.477876Z");
    assert_eq!(entries[0]["systems"][1], "redis-sessions");
    assert_eq!(
        entries[0]["prs"][0],
        "https://github.com/acme/infra/pull/4212"
    );
    assert_eq!(entries[0]["archived"], false);
    assert_eq!(entries[1]["archived"], true);
}

#[test]
fn absent_values_are_null_or_empty_not_missing() {
    let mut rca = summary("2026-07-15-payments-api-p99-latency", false);
    rca.meta.updated = None;
    rca.meta.prs.clear();
    let rendered = list(&[&rca]).expect("plain data serializes");

    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("valid JSON");
    let entry = &parsed[0];
    assert_eq!(entry["updated"], serde_json::Value::Null);
    assert_eq!(entry["published_at"], serde_json::Value::Null);
    assert_eq!(entry["published"], false);
    assert_eq!(entry["tags"], serde_json::json!([]));
    assert_eq!(entry["prs"], serde_json::json!([]));
}

#[test]
fn no_workspaces_renders_an_empty_array() {
    assert_eq!(list(&[]).expect("plain data serializes"), "[]");
}
