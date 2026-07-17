//! Tests for `fuzzy`.
#![allow(clippy::expect_used, clippy::unwrap_used)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn empty_needle_matches_everything() {
    assert_eq!(score("", "anything"), Some(0));
    assert_eq!(score("", ""), Some(0));
}

#[test]
fn subsequence_matches_and_out_of_order_does_not() {
    assert!(score("payapi", "payments-api latency").is_some());
    assert!(score("apipay", "payments-api").is_none());
    assert!(score("xyz", "payments-api").is_none());
}

#[test]
fn matching_is_case_insensitive() {
    assert!(score("SENDGRID", "SendGrid webhook failures").is_some());
    assert!(score("sendgrid", "SENDGRID WEBHOOK").is_some());
}

#[test]
fn consecutive_run_beats_scattered_letters() {
    let tight = score("redis", "redis-sessions").expect("matches");
    let scattered = score("redis", "remote dispatch worker sync").expect("matches");
    assert!(tight > scattered, "{tight} should beat {scattered}");
}

#[test]
fn word_boundary_hits_rank_higher() {
    let boundary = score("pa", "payments api").expect("matches");
    let midword = score("pa", "krpamdx").expect("matches");
    assert!(boundary > midword);
}
