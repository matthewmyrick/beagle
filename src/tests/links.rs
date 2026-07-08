//! Tests for `links`.
use super::*;

#[test]
fn urls_are_found_trimmed_and_deduplicated() {
    let text = "\
See <https://grafana.example.com/d/abc?from=now-1h> and the Sentry issue
(https://sentry.io/org/proj/issues/42). Also https://sentry.io/org/proj/issues/42,
plus `https://github.com/org/repo/pull/7` — but not http:/broken.";
    let urls = extract_urls(text);
    assert_eq!(
        urls,
        [
            "https://grafana.example.com/d/abc?from=now-1h",
            "https://sentry.io/org/proj/issues/42",
            "https://github.com/org/repo/pull/7",
        ]
    );
}

#[test]
fn bare_scheme_and_empty_text_yield_nothing() {
    assert!(extract_urls("").is_empty());
    assert!(extract_urls("https:// is not a url; httpx://nope").is_empty());
}
