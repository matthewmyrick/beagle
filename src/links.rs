//! URL extraction and opening, for the `o` link popup.
//!
//! Agents cite Grafana/Sentry/GitHub URLs constantly; the TUI renders links
//! as plain text, so this module finds them in raw section content and hands
//! them to the platform opener.

use std::io;
use std::process::{Command, Stdio};

/// Extracts `http(s)://` URLs from free text, in order of appearance,
/// de-duplicated. URLs end at whitespace or a closing delimiter, and
/// trailing punctuation is trimmed (so `<https://x> and (https://y).`
/// both come out clean).
#[must_use]
pub fn extract_urls(text: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for (index, _) in text.match_indices("http") {
        let rest = &text[index..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            continue;
        }
        let end = rest
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '>' | ')' | ']' | '}' | '"' | '\'' | '`')
            })
            .unwrap_or(rest.len());
        let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if url.len() > "https://".len() && !urls.iter().any(|u| u == url) {
            urls.push(url.to_owned());
        }
    }
    urls
}

/// Opens a URL in the default browser: `open` on macOS, `xdg-open`
/// elsewhere. Fire-and-forget; the child is detached from our stdio so it
/// can never corrupt the terminal.
///
/// # Errors
/// Returns the spawn error when the opener is missing.
pub fn open_url(url: &str) -> io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
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
}
