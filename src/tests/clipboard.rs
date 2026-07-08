//! Tests for `clipboard`.
#![allow(clippy::expect_used, clippy::unwrap_used)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn base64_matches_rfc4648_test_vectors() {
    let vectors = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (input, expected) in vectors {
        assert_eq!(base64(input.as_bytes()), expected, "input `{input}`");
    }
}

#[test]
fn base64_handles_non_ascii_bytes() {
    assert_eq!(base64("héllo — ✓".as_bytes()), "aMOpbGxvIOKAlCDinJM=");
}

#[test]
fn missing_binary_is_skipped_not_fatal() {
    assert!(!pipe_to(&["definitely-not-a-real-clipboard-tool"], "x"));
}
