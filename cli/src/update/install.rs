//! The download → verify → swap pipeline behind `beagle update`.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::{Error, Result};

use super::{Version, REPO_URL};

/// A human description of the build target, for the unsupported-target error.
const TARGET_DESCRIPTION: &str = if cfg!(target_os = "macos") {
    "this macOS architecture"
} else if cfg!(target_os = "linux") {
    "this Linux architecture"
} else {
    "this platform"
};

/// Distinguishes concurrent installs (tests run in parallel in one process).
static INSTALL_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Downloads `version` for this platform, verifies its sha256 against the
/// published checksum, and atomically replaces the binary at `exe` — the
/// same flow whether that is an upgrade or a downgrade.
///
/// # Errors
/// [`Error::UnsupportedTarget`] on platforms without prebuilt binaries, and
/// [`Error::Tool`] / [`Error::ChecksumMismatch`] / [`Error::Io`] for
/// download, verification, and install failures.
pub fn update_to(version: Version, exe: &Path) -> Result<()> {
    let target = super::release_target().ok_or(Error::UnsupportedTarget {
        target: TARGET_DESCRIPTION,
        repo: REPO_URL,
    })?;
    let base = format!("{REPO_URL}/releases/download/{}", version.tag());
    install(
        &format!("{base}/beagle-{target}.tar.gz"),
        &format!("{base}/beagle-{target}.tar.gz.sha256"),
        target,
        exe,
    )
}

/// Downloads a release tarball and its checksum from the given URLs,
/// verifies, extracts, and atomically swaps the binary at `exe`. URL-based
/// so tests can drive the whole flow with `file://` URLs.
///
/// # Errors
/// As for [`update_to`], minus the target lookup.
pub fn install(tarball_url: &str, sha_url: &str, target: &str, exe: &Path) -> Result<()> {
    let workdir = std::env::temp_dir().join(format!(
        "beagle-update-{}-{}",
        std::process::id(),
        INSTALL_SEQ.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&workdir).map_err(|e| Error::io(&workdir, e))?;
    let result = install_in(&workdir, tarball_url, sha_url, target, exe);
    let _ = fs::remove_dir_all(&workdir); // best effort; temp dir anyway
    result
}

fn install_in(
    workdir: &Path,
    tarball_url: &str,
    sha_url: &str,
    target: &str,
    exe: &Path,
) -> Result<()> {
    let tarball = workdir.join("release.tar.gz");
    download(tarball_url, &tarball)?;
    let sha_file = workdir.join("release.tar.gz.sha256");
    download(sha_url, &sha_file)?;

    let expected = read_expected_sha(&sha_file)?;
    let actual = sha256_of(&tarball)?;
    if expected != actual {
        return Err(Error::ChecksumMismatch {
            path: tarball,
            expected,
            actual,
        });
    }

    run_tool(
        "tar",
        &[
            "-xzf",
            &tarball.to_string_lossy(),
            "-C",
            &workdir.to_string_lossy(),
        ],
    )?;
    let new_binary = workdir.join(format!("beagle-{target}")).join("beagle");
    if !new_binary.is_file() {
        return Err(Error::Tool {
            tool: "tar",
            message: format!("extracted archive has no `beagle-{target}/beagle` binary inside"),
        });
    }

    // Stage next to the destination so the final rename is atomic (same
    // filesystem); `fs::copy` carries the executable bit over from the
    // extracted file.
    let staging = exe
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".beagle.update");
    fs::copy(&new_binary, &staging).map_err(|e| Error::io(&staging, e))?;
    fs::rename(&staging, exe).map_err(|e| {
        let _ = fs::remove_file(&staging);
        Error::io(exe, e)
    })?;
    Ok(())
}

/// Downloads `url` to `to` via curl. `file://` URLs work too, which is how
/// the install flow is tested without a network.
fn download(url: &str, to: &Path) -> Result<()> {
    curl_stdout(&["-fsSL", "--retry", "2", "-o", &to.to_string_lossy(), url]).map(|_| ())
}

/// Runs curl with `args` and returns its stdout. Shared with the release
/// listing in the parent module.
pub(super) fn curl_stdout(args: &[&str]) -> Result<String> {
    let output = Command::new("curl").args(args).output().map_err(|e| {
        let message = if e.kind() == std::io::ErrorKind::NotFound {
            "curl not found on PATH; install curl or update via cargo install".to_owned()
        } else {
            e.to_string()
        };
        Error::Tool {
            tool: "curl",
            message,
        }
    })?;
    if !output.status.success() {
        return Err(Error::Tool {
            tool: "curl",
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_tool(tool: &'static str, args: &[&str]) -> Result<()> {
    let output = Command::new(tool)
        .args(args)
        .output()
        .map_err(|e| Error::Tool {
            tool,
            message: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(Error::Tool {
            tool,
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

/// Reads the expected checksum from a published `.sha256` file (format:
/// `<hex>  <filename>`).
fn read_expected_sha(path: &Path) -> Result<String> {
    let raw = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    parse_sha(&raw).ok_or_else(|| Error::Tool {
        tool: "sha256",
        message: format!("`{}` does not contain a sha256 hex digest", path.display()),
    })
}

/// Extracts the leading 64-hex-char digest from a checksum line, lowercased.
fn parse_sha(line: &str) -> Option<String> {
    let token = line.split_whitespace().next()?;
    (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| token.to_ascii_lowercase())
}

/// Computes a file's sha256 via `shasum -a 256` (macOS) or `sha256sum`
/// (Linux), whichever exists.
fn sha256_of(path: &Path) -> Result<String> {
    let path_str = path.to_string_lossy();
    let attempts: [(&str, Vec<&str>); 2] = [
        ("shasum", vec!["-a", "256", &path_str]),
        ("sha256sum", vec![&path_str]),
    ];
    for (tool, args) in attempts {
        match Command::new(tool).args(&args).output() {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return parse_sha(&stdout).ok_or(Error::Tool {
                    tool: "sha256",
                    message: "checksum tool produced no digest".to_owned(),
                });
            }
            Ok(output) => {
                return Err(Error::Tool {
                    tool: "sha256",
                    message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::Tool {
                    tool: "sha256",
                    message: e.to_string(),
                });
            }
        }
    }
    Err(Error::Tool {
        tool: "sha256",
        message: "neither shasum nor sha256sum found on PATH".to_owned(),
    })
}

#[cfg(test)]
#[path = "tests/install.rs"]
mod tests;
