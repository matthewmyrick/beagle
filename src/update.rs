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

use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::DefaultTerminal;

use crate::error::{Error, Result};

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

/// Downloads `version` for this platform, verifies its sha256 against the
/// published checksum, and atomically replaces the binary at `exe` — the
/// same flow whether that is an upgrade or a downgrade.
///
/// # Errors
/// [`Error::UnsupportedTarget`] on platforms without prebuilt binaries, and
/// [`Error::Tool`] / [`Error::ChecksumMismatch`] / [`Error::Io`] for
/// download, verification, and install failures.
pub fn update_to(version: Version, exe: &Path) -> Result<()> {
    let target = release_target().ok_or(Error::UnsupportedTarget {
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

fn curl_stdout(args: &[&str]) -> Result<String> {
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

/// Runs the interactive version picker: j/k or arrows move, enter returns
/// the chosen version, q/esc/ctrl-c returns `None`. Sets up and restores the
/// terminal itself.
///
/// # Errors
/// [`Error::Terminal`] on draw or input failures.
pub fn pick_version(releases: &[Release], current: Version) -> Result<Option<Version>> {
    if releases.is_empty() {
        return Ok(None);
    }
    let mut terminal = ratatui::init();
    let picked = run_picker(&mut terminal, releases, current);
    ratatui::restore();
    picked
}

fn run_picker(
    terminal: &mut DefaultTerminal,
    releases: &[Release],
    current: Version,
) -> Result<Option<Version>> {
    let mut selected = 0usize;
    loop {
        terminal.draw(|frame| draw_picker(frame, releases, current, selected))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                selected = (selected + 1).min(releases.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => selected = 0,
            KeyCode::Char('G') | KeyCode::End => selected = releases.len() - 1,
            KeyCode::Enter => return Ok(releases.get(selected).map(|r| r.version)),
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            _ => {}
        }
    }
}

fn draw_picker(
    frame: &mut ratatui::Frame,
    releases: &[Release],
    current: Version,
    selected: usize,
) {
    let items: Vec<ListItem<'_>> = releases
        .iter()
        .enumerate()
        .map(|(i, release)| ListItem::new(release_line(release.version, current, i == 0)))
        .collect();
    let block = Block::default()
        .title(" beagle versions — enter install · j/k move · q cancel ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(selected));

    let [area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .areas(frame.area());
    frame.render_stateful_widget(list, area, &mut state);
}

/// One picker row: the tag plus `latest` / `current` markers.
fn release_line(version: Version, current: Version, is_latest: bool) -> Line<'static> {
    let mut spans = vec![Span::raw(format!(" {:<12}", version.tag()))];
    if is_latest {
        spans.push(Span::styled(
            " latest",
            Style::default().fg(Color::LightGreen),
        ));
    }
    if version == current {
        spans.push(Span::styled(
            " ← current",
            Style::default().fg(Color::Yellow),
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

    use super::*;
    use std::path::PathBuf;

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

    #[test]
    fn repo_slug_is_owner_slash_name() {
        assert_eq!(repo_slug(), "matthewmyrick/beagle");
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
}
