//! Tests for hand-off input composition (`handoff`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::compose_input;

#[test]
fn composes_prompt_then_write_up_with_a_separator() {
    let out = compose_input(
        "2026-05-12-example",
        Some("Open a PR for each fix."),
        "# Summary\n\nboom",
    );
    assert!(out.starts_with("# Beagle agent hand-off: 2026-05-12-example"));
    assert!(out.contains("Open a PR for each fix."));
    assert!(out.contains("\n---\n"), "prompt and write-up are separated");
    assert!(out.contains("# Summary"), "the write-up is included");
    // The prompt comes before the write-up.
    assert!(out.find("Open a PR").unwrap() < out.find("# Summary").unwrap());
}

#[test]
fn an_empty_or_missing_prompt_is_skipped() {
    let no_prompt = compose_input("slug", None, "body");
    assert!(
        !no_prompt.contains("\n---\n"),
        "no separator without a prompt"
    );
    assert!(no_prompt.contains("body"));

    let blank = compose_input("slug", Some("   \n  "), "body");
    assert!(
        !blank.contains("\n---\n"),
        "whitespace prompt counts as absent"
    );
}
