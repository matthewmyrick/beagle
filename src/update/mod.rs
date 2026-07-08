//! Self-update: fetch released versions and swap the running binary.
//!
//! Follows the crate's no-heavy-dependencies philosophy: networking shells
//! out to `curl` (the same tool the README install uses), checksums to
//! `shasum`/`sha256sum`, and extraction to `tar`. Every downloaded artifact
//! is verified against the sha256 the release workflow publishes before a
//! single byte replaces the installed binary, and the final swap is an
//! atomic same-directory rename — an interrupted update can never leave a
//! half-written executable.
//!
//! Versions are plain `MAJOR.MINOR.PATCH`; picking what to install is the
//! user's call (`--version`, or the interactive `beagle version list`), and
//! moving to an older version is as supported as moving to a newer one.
//!
//! Submodules: `install` (download, verify, swap) and `picker` (the
//! interactive `beagle version list` TUI).

mod install;
mod picker;

pub use install::{install, update_to};
pub use picker::pick_version;

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

use install::curl_stdout;

/// The GitHub repository releases come from, as `owner/name`. Derived from
/// the crate metadata so a fork updates itself from the fork.
#[must_use]
pub fn repo_slug() -> &'static str {
    REPO_URL
        .strip_prefix("https://github.com/")
        .unwrap_or(REPO_URL)
}

/// The repository URL from `Cargo.toml`.
pub const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// A released version: `MAJOR.MINOR.PATCH`. Ordered so newer sorts greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Incompatible-change counter (0 while the format is still settling).
    pub major: u64,
    /// Feature counter.
    pub minor: u64,
    /// Fix counter.
    pub patch: u64,
}

impl Version {
    /// The version this binary was built as.
    #[must_use]
    pub fn current() -> Self {
        // CARGO_PKG_VERSION is always well-formed x.y.z for this crate.
        env!("CARGO_PKG_VERSION").parse().unwrap_or(Self {
            major: 0,
            minor: 0,
            patch: 0,
        })
    }

    /// The git tag for this version, e.g. `v0.2.0`.
    #[must_use]
    pub fn tag(self) -> String {
        format!("v{self}")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let bare = s.strip_prefix('v').unwrap_or(s);
        let mut parts = bare.split('.');
        let mut next = |name: &str| {
            parts
                .next()
                .and_then(|p| p.parse::<u64>().ok())
                .ok_or_else(|| format!("invalid version `{s}`: bad {name} (expected X.Y.Z)"))
        };
        let version = Self {
            major: next("major")?,
            minor: next("minor")?,
            patch: next("patch")?,
        };
        if parts.next().is_some() {
            return Err(format!("invalid version `{s}`: too many components"));
        }
        Ok(version)
    }
}

/// One published release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Release {
    /// The released version (from the tag).
    pub version: Version,
}

/// The compile-time target triple, exactly as the release workflow names its
/// assets. `None` on platforms the release workflow does not build for.
#[must_use]
pub fn release_target() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("aarch64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("x86_64-unknown-linux-musl")
    } else {
        None
    }
}

/// Fetches every published (non-draft, non-prerelease) release, newest
/// first.
///
/// # Errors
/// [`Error::Tool`] if `curl` is missing or the request fails,
/// [`Error::ParseReleases`] if the response is not the expected JSON.
pub fn fetch_releases() -> Result<Vec<Release>> {
    let url = format!(
        "https://api.github.com/repos/{}/releases?per_page=100",
        repo_slug()
    );
    let json = curl_stdout(&[
        "-fsSL",
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "User-Agent: beagle",
        &url,
    ])?;
    parse_releases(&json)
}

/// Parses the GitHub releases JSON. Split from [`fetch_releases`] so the
/// parsing is testable without a network.
///
/// # Errors
/// [`Error::ParseReleases`] when the body is not a JSON array of releases.
pub fn parse_releases(json: &str) -> Result<Vec<Release>> {
    #[derive(serde::Deserialize)]
    struct Raw {
        tag_name: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        prerelease: bool,
    }
    let raw: Vec<Raw> = serde_json::from_str(json).map_err(|e| {
        let head: String = json.chars().take(120).collect();
        Error::ParseReleases(format!("{e} (body starts: {head})"))
    })?;
    let mut releases: Vec<Release> = raw
        .iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| r.tag_name.parse().ok())
        .map(|version| Release { version })
        .collect();
    releases.sort_by_key(|release| std::cmp::Reverse(release.version));
    Ok(releases)
}

#[cfg(test)]
#[path = "tests/version.rs"]
mod tests;
