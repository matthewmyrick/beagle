//! Desktop notifications, opt-in via config `notify = true`.
//!
//! Same shell-out philosophy as the clipboard and `gh`: `osascript` on
//! macOS, `notify-send` elsewhere, spawned detached and fire-and-forget.
//! A missing tool (or a failure) is a silent no-op — notifications are a
//! convenience, never an error source.

use std::process::{Command, Stdio};

/// The platform notifier invocation for a title/body pair, or `None` on
/// platforms without one. Split from [`send`] so the construction (and its
/// quoting) is testable without popping real notifications.
#[must_use]
pub fn command_for(title: &str, body: &str) -> Option<(&'static str, Vec<String>)> {
    if cfg!(target_os = "macos") {
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
        Some(("notify-send", vec![title.to_owned(), body.to_owned()]))
    } else {
        None
    }
}

/// Fires a desktop notification and forgets it. Never blocks, never fails
/// loudly — the child is detached from our stdio so it cannot corrupt the
/// terminal.
pub fn send(title: &str, body: &str) {
    let Some((program, args)) = command_for(title, body) else {
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
