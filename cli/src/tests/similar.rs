//! Tests for related-incident ranking (`similar`).
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use time::OffsetDateTime;

use crate::model::{RcaId, RcaMeta, RcaSummary, Severity, Status};

use super::{rank, shared_label};

fn summary(id: &str, systems: &[&str], tags: &[&str], created: i64) -> RcaSummary {
    RcaSummary {
        id: RcaId::new(id).expect("valid test id"),
        meta: RcaMeta {
            title: format!("Incident {id}"),
            severity: Severity::Medium,
            status: Status::Finished,
            created: OffsetDateTime::from_unix_timestamp(created).expect("valid ts"),
            updated: None,
            systems: systems.iter().map(ToString::to_string).collect(),
            tags: tags.iter().map(ToString::to_string).collect(),
            prs: Vec::new(),
        },
        archived: false,
    }
}

#[test]
fn systems_outweigh_tags_and_ties_break_newest_first() {
    let target = summary("target", &["alloy", "mimir"], &["ingestion"], 0);
    let all = vec![
        target.clone(),
        summary("tag-only", &[], &["ingestion"], 100),
        summary("one-system-old", &["alloy"], &[], 50),
        summary("one-system-new", &["alloy"], &[], 200),
        summary("system-and-tag", &["mimir"], &["ingestion"], 10),
        summary("unrelated", &["sendgrid"], &["webhooks"], 300),
    ];
    let ranked = rank(&target, &all);
    let ids: Vec<&str> = ranked.iter().map(|r| r.rca.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "system-and-tag", // 3 + 1 = 4
            "one-system-new", // 3, newer
            "one-system-old", // 3, older
            "tag-only",       // 1
        ],
        "self and zero-score are excluded, systems beat tags, ties by recency"
    );
    assert_eq!(ranked[0].score, 4);
    assert_eq!(ranked[0].shared_systems, ["mimir"]);
    assert_eq!(ranked[0].shared_tags, ["ingestion"]);
}

#[test]
fn matching_is_case_insensitive() {
    let target = summary("target", &["Alloy"], &[], 0);
    let all = vec![target.clone(), summary("other", &["alloy"], &[], 1)];
    let ranked = rank(&target, &all);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].score, 3);
}

#[test]
fn shared_labels_read_naturally() {
    let target = summary("t", &["alloy", "mimir"], &["ingestion", "429"], 0);
    let all = vec![
        target.clone(),
        summary("a", &["alloy", "mimir"], &["ingestion"], 1),
        summary("b", &["alloy"], &[], 2),
    ];
    let ranked = rank(&target, &all);
    assert_eq!(shared_label(&ranked[0]), "2 systems, tag: ingestion");
    assert_eq!(shared_label(&ranked[1]), "system: alloy");
}
