//! Desktop notifications, opt-in via config `notify = true`.
//!
//! Same shell-out philosophy as the clipboard and `gh`: `osascript` on
//! macOS, `notify-send` elsewhere, spawned detached and fire-and-forget.
//! A missing tool (or a failure) is a silent no-op — notifications are a
//! convenience, never an error source. When the platform notifier supports
//! a custom icon, the beagle logo is used (embedded and written to the
//! cache once).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The beagle logo, embedded for use as the notification icon.
const ICON_PNG: &[u8] = include_bytes!("../assets/beagle-icon.png");

/// Path to the beagle icon on disk, written to the cache once. `None` if it
/// cannot be written (icons are best-effort — a notification without one is
/// fine).
fn icon_path() -> Option<&'static std::path::Path> {
    static ICON: OnceLock<Option<PathBuf>> = OnceLock::new();
    ICON.get_or_init(|| {
        let dir = cache_dir()?.join("beagle");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("icon.png");
        if !path.exists() {
            std::fs::write(&path, ICON_PNG).ok()?;
        }
        Some(path)
    })
    .as_deref()
}

/// `$XDG_CACHE_HOME`, else `$HOME/.cache`.
fn cache_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(xdg));
    }
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".cache"))
}

/// Whether `program` is on the `PATH`.
fn has(program: &str) -> bool {
    Command::new(program)
        .arg("-h")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// The platform notifier invocation for a title/body pair, or `None` on
/// platforms without one. `icon` is the beagle logo path when available.
/// Split from [`send`] so the construction (and its quoting) is testable
/// without popping real notifications.
#[must_use]
pub fn command_for(
    title: &str,
    body: &str,
    icon: Option<&str>,
) -> Option<(&'static str, Vec<String>)> {
    if cfg!(target_os = "macos") {
        // osascript can't set a custom icon; terminal-notifier can. Use it
        // when present (and an icon is available), else plain osascript.
        if let Some(icon) = icon.filter(|_| has("terminal-notifier")) {
            return Some((
                "terminal-notifier",
                vec![
                    "-title".to_owned(),
                    title.to_owned(),
                    "-message".to_owned(),
                    body.to_owned(),
                    "-appIcon".to_owned(),
                    icon.to_owned(),
                ],
            ));
        }
        // AppleScript string literals: escape backslashes and quotes.
        let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        Some((
            "osascript",
            vec![
                "-e".to_owned(),
                format!(
                    "display notification \"{}\" with title \"{}\"",
                    escape(body),
                    escape(title),
                ),
            ],
        ))
    } else if cfg!(target_os = "linux") {
        let mut args = Vec::new();
        if let Some(icon) = icon {
            args.push("-i".to_owned());
            args.push(icon.to_owned());
        }
        args.push(title.to_owned());
        args.push(body.to_owned());
        Some(("notify-send", args))
    } else {
        None
    }
}

/// Fires a desktop notification and forgets it. Never blocks, never fails
/// loudly — the child is detached from our stdio so it cannot corrupt the
/// terminal. Uses the beagle logo as the icon where supported.
pub fn send(title: &str, body: &str) {
    let icon = icon_path().and_then(|p| p.to_str());
    let Some((program, args)) = command_for(title, body, icon) else {
        return;
    };
    let _ = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
#[path = "tests/notify.rs"]
mod tests;
