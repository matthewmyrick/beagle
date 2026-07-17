//! Tests for version parsing and release listing (`update::mod`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn v(s: &str) -> Version {
    s.parse().expect("valid test version")
}

#[test]
fn version_parses_with_and_without_the_v_prefix() {
    assert_eq!(v("0.2.0"), v("v0.2.0"));
    assert_eq!(v("1.12.3").to_string(), "1.12.3");
    assert_eq!(v("0.2.0").tag(), "v0.2.0");
}

#[test]
fn version_rejects_garbage() {
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "a.b.c",
        "1.2.x",
        "v",
        "0.2.0-rc1",
    ] {
        assert!(bad.parse::<Version>().is_err(), "`{bad}` must be rejected");
    }
}

#[test]
fn versions_order_newest_greatest() {
    assert!(v("0.10.0") > v("0.9.9"), "numeric, not lexicographic");
    assert!(v("1.0.0") > v("0.99.99"));
    assert!(v("0.2.1") > v("0.2.0"));
}

#[test]
fn current_version_matches_the_crate() {
    assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn releases_parse_filter_and_sort_newest_first() {
    let json = r#"[
        {"tag_name": "v0.1.0"},
        {"tag_name": "v0.3.0-rc1", "prerelease": true},
        {"tag_name": "v0.4.0", "draft": true},
        {"tag_name": "weird-tag"},
        {"tag_name": "v0.2.0"}
    ]"#;
    let releases = parse_releases(json).expect("parses");
    let tags: Vec<String> = releases.iter().map(|r| r.version.tag()).collect();
    assert_eq!(tags, ["v0.2.0", "v0.1.0"]);
}

#[test]
fn non_array_release_bodies_error_with_context() {
    let err = parse_releases(r#"{"message": "API rate limit exceeded"}"#)
        .expect_err("objects are not release lists");
    assert!(
        err.to_string().contains("rate limit"),
        "body surfaced: {err}"
    );
}

#[test]
fn repo_slug_is_owner_slash_name() {
    assert_eq!(repo_slug(), "matthewmyrick/beagle");
}

#[test]
fn parse_releases_skips_desktop_tags() {
    // The repo publishes desktop releases under desktop-v* tags in the
    // same GitHub release list; the CLI updater must ignore them — this
    // invariant keeps `beagle update` safe in the multi-component repo.
    let json = r#"[
        {"tag_name": "desktop-v0.1.0"},
        {"tag_name": "v0.7.1"},
        {"tag_name": "desktop-v0.2.0"},
        {"tag_name": "v0.7.0"}
    ]"#;
    let releases = parse_releases(json).expect("parses");
    let tags: Vec<String> = releases.iter().map(|r| r.version.tag()).collect();
    assert_eq!(tags, ["v0.7.1", "v0.7.0"], "desktop tags are skipped");
}
