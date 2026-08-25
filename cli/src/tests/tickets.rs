//! Tests for the ticket → PR discovery parsers (`tickets`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn github_issue_parts_from_url() {
    assert_eq!(
        github_issue_parts("https://github.com/acme/infra/issues/42"),
        Some(("acme".to_owned(), "infra".to_owned(), "42".to_owned()))
    );
    assert_eq!(
        github_issue_parts("https://github.com/a/b/issues/7#issuecomment-1"),
        Some(("a".to_owned(), "b".to_owned(), "7".to_owned()))
    );
    // A PR URL is not an issue.
    assert_eq!(github_issue_parts("https://github.com/a/b/pull/7"), None);
}

#[test]
fn github_timeline_extracts_cross_referenced_prs() {
    let json = r#"[
      {"event":"labeled"},
      {"event":"cross-referenced","source":{"issue":{
          "html_url":"https://github.com/a/b/pull/10",
          "pull_request":{"url":"x"}}}},
      {"event":"cross-referenced","source":{"issue":{
          "html_url":"https://github.com/a/b/issues/9"}}},
      {"event":"cross-referenced","source":{"issue":{
          "html_url":"https://github.com/a/b/pull/10",
          "pull_request":{"url":"x"}}}}
    ]"#;
    // Only the PR cross-reference, de-duplicated; the issue one is ignored.
    assert_eq!(
        github_timeline_prs(json),
        vec!["https://github.com/a/b/pull/10".to_owned()]
    );
    assert!(github_timeline_prs("not json").is_empty());
}

#[test]
fn linear_id_from_url_or_bare() {
    assert_eq!(
        linear_issue_id("https://linear.app/acme/issue/ENG-123/some-title"),
        Some("ENG-123".to_owned())
    );
    assert_eq!(linear_issue_id("ENG-123"), Some("ENG-123".to_owned()));
    assert_eq!(linear_issue_id("not-a-ticket"), None);
}

#[test]
fn linear_attachments_keep_only_pr_links() {
    let json = r#"{"data":{"issue":{"attachments":{"nodes":[
      {"url":"https://github.com/a/b/pull/5"},
      {"url":"https://docs.example.com/spec"},
      {"url":"https://gitlab.com/a/b/-/merge_requests/3"}
    ]}}}}"#;
    assert_eq!(
        linear_attachment_prs(json),
        vec![
            "https://github.com/a/b/pull/5".to_owned(),
            "https://gitlab.com/a/b/-/merge_requests/3".to_owned()
        ]
    );
}
