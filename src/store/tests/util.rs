//! Shared fixtures for the `store` test modules.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use crate::model::{RcaId, RcaMeta, Severity, Status};
use time::OffsetDateTime;

pub(crate) fn test_meta(title: &str, severity: Severity) -> RcaMeta {
    RcaMeta {
        title: title.to_owned(),
        severity,
        status: Status::Investigating,
        created: OffsetDateTime::from_unix_timestamp(1_780_000_000).expect("valid test timestamp"),
        updated: None,
        systems: vec!["payments-api".to_owned()],
        tags: vec!["latency".to_owned()],
        prs: Vec::new(),
    }
}

pub(crate) fn test_id(slug: &str) -> RcaId {
    RcaId::new(slug).expect("valid test id")
}
