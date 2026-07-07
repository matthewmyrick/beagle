//! Copy text to the system clipboard without adding a dependency.
//!
//! Strategy: pipe to the platform's clipboard command if one exists
//! (`pbcopy`, `wl-copy`, `xclip`, `xsel`); otherwise emit an OSC 52 escape
//! sequence, which modern terminals (`iTerm2`, `kitty`, `WezTerm`,
//! `Alacritty`) translate into a clipboard write — including over SSH.

use std::io::{self, Write as _};
use std::process::{Command, Stdio};

/// Clipboard commands to try, in order. First element is the binary name.
const COMMANDS: &[&[&str]] = &[
    &["pbcopy"],
    &["wl-copy"],
    &["xclip", "-selection", "clipboard"],
    &["xsel", "--clipboard", "--input"],
];

/// Copies `text` to the clipboard, returning the mechanism used (for the
/// status bar). Falls back to OSC 52 when no clipboard command is present.
///
/// # Errors
/// Returns [`io::Error`] only if every command fails *and* writing the
/// OSC 52 sequence to stdout fails.
pub fn copy(text: &str) -> io::Result<&'static str> {
    for cmd in COMMANDS {
        if pipe_to(cmd, text) {
            return Ok(cmd[0]);
        }
    }
    osc52(text)?;
    Ok("osc 52")
}

/// Pipes `text` into a spawned command; false if the binary is missing or
/// the command fails (both mean "try the next mechanism").
fn pipe_to(cmd: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(stdin) = child.stdin.take() {
        let mut stdin = stdin;
        if stdin.write_all(text.as_bytes()).is_err() {
            let _ = child.kill(); // best effort; the copy already failed
            let _ = child.wait();
            return false;
        }
    }
    child.wait().is_ok_and(|status| status.success())
}

/// Writes the OSC 52 clipboard sequence to stdout. Works from inside the
/// alternate screen; the terminal interprets it rather than displaying it.
fn osc52(text: &str) -> io::Result<()> {
    let mut out = io::stdout().lock();
    write!(out, "\u{1b}]52;c;{}\u{7}", base64(text.as_bytes()))?;
    out.flush()
}

/// Standard base64 (RFC 4648, with padding). Hand-rolled because 20 lines
/// beat a dependency for a single call site.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        let sextet = |shift: u32| {
            // `& 63` bounds the value to 0..64, so the cast cannot truncate.
            usize::from(u8::try_from((n >> shift) & 63).unwrap_or(0))
        };
        out.push(ALPHABET[sextet(18)] as char);
        out.push(ALPHABET[sextet(12)] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[sextet(6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[sextet(0)] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
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
}
