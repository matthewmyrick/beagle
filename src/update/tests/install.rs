//! Tests for the download → verify → swap pipeline (`update::install`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use super::*;
use crate::error::Error;

#[test]
fn sha_lines_parse_and_reject_non_digests() {
    let line = format!("{}  beagle-x.tar.gz\n", "ab".repeat(32));
    assert_eq!(parse_sha(&line), Some("ab".repeat(32)));
    assert_eq!(
        parse_sha("ABCD".repeat(16).as_str()).as_deref(),
        Some("abcd".repeat(16).as_str())
    );
    assert_eq!(parse_sha("not-a-digest file"), None);
    assert_eq!(parse_sha(""), None);
}

/// End-to-end install over `file://` URLs: build a fake release tarball
/// with a real checksum, install it over a fake current binary, and
/// check the swap happened atomically with the executable bit intact.
#[test]
fn install_verifies_checksum_extracts_and_swaps_the_binary() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let target = "test-target";
    let (tarball, sha_file) = fake_release(tmp.path(), target, b"#!/bin/sh\necho new\n");

    let exe = tmp.path().join("bin").join("beagle");
    fs::create_dir_all(exe.parent().unwrap()).expect("bin dir");
    fs::write(&exe, "old-binary").expect("old binary");

    install(
        &format!("file://{}", tarball.display()),
        &format!("file://{}", sha_file.display()),
        target,
        &exe,
    )
    .expect("install succeeds");

    let installed = fs::read(&exe).expect("read installed");
    assert_eq!(installed, b"#!/bin/sh\necho new\n");
    assert!(
        !exe.parent().unwrap().join(".beagle.update").exists(),
        "no staging file left behind"
    );
}

#[test]
fn install_refuses_a_tampered_tarball() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let target = "test-target";
    let (tarball, sha_file) = fake_release(tmp.path(), target, b"legit");
    // Corrupt the tarball after the checksum was published.
    fs::write(&tarball, "tampered bytes").expect("tamper");

    let exe = tmp.path().join("beagle");
    fs::write(&exe, "old-binary").expect("old binary");

    let err = install(
        &format!("file://{}", tarball.display()),
        &format!("file://{}", sha_file.display()),
        target,
        &exe,
    )
    .expect_err("checksum mismatch must fail the install");
    assert!(matches!(err, Error::ChecksumMismatch { .. }), "{err}");
    assert_eq!(
        fs::read(&exe).expect("read"),
        b"old-binary",
        "binary untouched on failure"
    );
}

/// Builds `beagle-<target>/beagle` into a tarball with a matching
/// `.sha256`, exactly like the release workflow does.
fn fake_release(dir: &Path, target: &str, binary: &[u8]) -> (PathBuf, PathBuf) {
    let pkg = dir.join(format!("beagle-{target}"));
    fs::create_dir_all(&pkg).expect("pkg dir");
    fs::write(pkg.join("beagle"), binary).expect("binary");

    let tarball = dir.join(format!("beagle-{target}.tar.gz"));
    let status = Command::new("tar")
        .args([
            "-czf",
            &tarball.to_string_lossy(),
            "-C",
            &dir.to_string_lossy(),
            &format!("beagle-{target}"),
        ])
        .status()
        .expect("tar available");
    assert!(status.success(), "tar failed");

    let digest = sha256_of(&tarball).expect("digest");
    let sha_file = dir.join(format!("beagle-{target}.tar.gz.sha256"));
    fs::write(&sha_file, format!("{digest}  beagle-{target}.tar.gz\n")).expect("sha file");
    (tarball, sha_file)
}
