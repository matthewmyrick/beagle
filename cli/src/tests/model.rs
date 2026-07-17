//! Tests for `model`.
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
fn pre_0_6_status_names_still_parse_but_are_never_written() {
    // Old manifests keep loading (serde aliases + FromStr), so nothing on
    // disk breaks when the vocabulary changes...
    for (old, new) in [
        ("identified", Status::Review),
        ("monitoring", Status::FinalReview),
        ("resolved", Status::Finished),
    ] {
        let toml_value = toml::Value::String(old.to_owned());
        let parsed: Status = toml_value.try_into().expect("legacy name deserializes");
        assert_eq!(parsed, new);
        assert_eq!(old.parse::<Status>(), Ok(new), "CLI accepts legacy names");
    }
    // ...while serialization always emits the new names.
    let serialized = toml::Value::try_from(Status::FinalReview).expect("serializes");
    assert_eq!(serialized, toml::Value::String("final-review".to_owned()));
    assert_eq!(Status::Finished.as_str(), "finished");
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
fn manifest_prs_parse_and_are_omitted_when_empty() {
    let toml_src = r#"
        title = "t"
        severity = "high"
        status = "identified"
        created = "2026-07-05T14:32:00Z"
        prs = ["https://github.com/o/r/pull/9"]
    "#;
    let meta: RcaMeta = toml::from_str(toml_src).expect("prs parse");
    assert_eq!(meta.prs, ["https://github.com/o/r/pull/9"]);

    let mut without = meta.clone();
    without.prs.clear();
    let serialized = toml::to_string(&without).expect("serializes");
    assert!(
        !serialized.contains("prs"),
        "empty prs omitted so old binaries keep parsing: {serialized}"
    );
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
            prs: Vec::new(),
        },
        archived: false,
    };
    let resolved = mk(Status::Finished, Severity::Critical, 100);
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
