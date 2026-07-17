//! Merge-status tracking for PRs attached to workspaces.
//!
//! Status comes from the `gh` CLI on a background thread — the TUI thread
//! never blocks on the network. When `gh` is missing or unauthenticated the
//! poller simply exits and attached PRs render as plain links: degraded,
//! never broken.

use std::collections::HashMap;
use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

/// How often the poller re-queries `gh` for PR states.
pub const POLL_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Where a pull request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    /// Open and ready for review.
    Open,
    /// Open, but marked draft.
    Draft,
    /// Merged — the fix has landed.
    Merged,
    /// Closed without merging.
    Closed,
}

impl PrState {
    /// One-cell status glyph.
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Open => "○",
            Self::Draft => "◌",
            Self::Merged => "✓",
            Self::Closed => "✗",
        }
    }

    /// Lowercase label for display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Draft => "draft",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

/// Short display form of a PR URL: `#123` for anything with a `/pull/123`
/// path, otherwise the URL truncated to a status-line-friendly length.
#[must_use]
pub fn short_label(url: &str) -> String {
    if let Some(rest) = url.split("/pull/").nth(1) {
        let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !number.is_empty() {
            return format!("#{number}");
        }
    }
    let mut short: String = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .take(32)
        .collect();
    if short.len() < url.trim_start_matches("https://").len() {
        short.push('…');
    }
    short
}

/// Parses `gh pr view --json state,isDraft` output into a [`PrState`].
#[must_use]
pub fn parse_state_json(json: &str) -> Option<PrState> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        state: String,
        #[serde(default)]
        is_draft: bool,
    }
    let raw: Raw = serde_json::from_str(json).ok()?;
    match raw.state.as_str() {
        "OPEN" if raw.is_draft => Some(PrState::Draft),
        "OPEN" => Some(PrState::Open),
        "MERGED" => Some(PrState::Merged),
        "CLOSED" => Some(PrState::Closed),
        _ => None,
    }
}

/// Queries one PR's state via `gh`. `None` on any failure — a PR whose
/// state can't be determined renders as a plain link.
#[must_use]
pub fn state_of(url: &str) -> Option<PrState> {
    let output = Command::new("gh")
        .args(["pr", "view", url, "--json", "state,isDraft"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_state_json(&String::from_utf8_lossy(&output.stdout))
}

/// Whether the `gh` CLI is installed and runnable.
#[must_use]
pub fn gh_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Spawns the background poller. It receives URL-set updates on `urls_rx`
/// (poll immediately on every update), re-polls the last set every
/// [`POLL_INTERVAL`], and sends `url → state` maps back on `results_tx`.
/// Exits quietly when `gh` is unavailable or either channel closes.
#[allow(clippy::implicit_hasher)] // a channel payload, not a generic API
pub fn spawn_poller(urls_rx: Receiver<Vec<String>>, results_tx: Sender<HashMap<String, PrState>>) {
    thread::spawn(move || {
        if !gh_available() {
            return;
        }
        let mut urls: Vec<String> = Vec::new();
        loop {
            match urls_rx.recv_timeout(POLL_INTERVAL) {
                Ok(update) => urls = update,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            if urls.is_empty() {
                continue;
            }
            let states: HashMap<String, PrState> = urls
                .iter()
                .filter_map(|url| state_of(url).map(|state| (url.clone(), state)))
                .collect();
            if results_tx.send(states).is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
#[path = "tests/prs.rs"]
mod tests;
