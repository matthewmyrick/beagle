//! The user config file: `~/.config/beagle/config.toml`.
//!
//! Optional and tiny by design — every setting has a flag or a sensible
//! default, so beagle works with no config at all. `beagle config` opens the
//! file in an editor and validates it on close; unknown fields are rejected
//! at parse time so typos surface immediately instead of being silently
//! ignored.
//!
//! Precedence everywhere: explicit CLI flag → config file → built-in default.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Environment variable overriding the config file location. Used by tests
/// and by anyone who wants per-project configs.
pub const CONFIG_ENV: &str = "BEAGLE_CONFIG";

/// The template written when `beagle config` runs for the first time.
/// Everything is commented out: the empty config is the default config.
pub const TEMPLATE: &str = "\
# beagle configuration.
#
# Every setting is optional and every setting has a CLI flag that overrides
# it. Uncomment what you need; unknown keys are rejected.

# Default workspace root: the directory containing `rcas/` and `exports/`.
# Overridden by --root. Without either, beagle uses the current directory.
# root = \"/path/to/oncall\"

# Editor for `beagle config` (may include arguments, e.g. \"code -w\").
# Falls back to $VISUAL, then $EDITOR, then vim.
# editor = \"vim\"

# Desktop notifications while the TUI runs: new incidents and status
# changes (osascript on macOS, notify-send on Linux). Off by default.
# notify = true

# Which events notify (only when notify is on). Omit this table to get all
# of them; include it to fire only the events you set to true.
# [notify_events]
# new_incident = true
# investigating = true
# review = false
# agent = false
# final_review = true
# finished = true

# Agent hand-off (`beagle handoff <slug>`): the agent beagle launches on a
# reviewed RCA. The composed prompt (the prompt file below, then the RCA
# write-up) is piped to the command's stdin; it runs in the store root with
# BEAGLE_RCA_SLUG / BEAGLE_RCA_DIR set.
# [handoff]
# command = [\"codex\", \"exec\"]      # or [\"claude\", \"-p\"], or any argv
# prompt = \"~/.config/beagle/handoff-prompt.md\"
";

/// Parsed contents of the config file. All fields optional; the empty file
/// is valid and means "all defaults".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default workspace root (the directory containing `rcas/`), used when
    /// `--root` is not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    /// Editor command for `beagle config`, possibly with arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// Desktop notifications from the TUI (new incidents, status changes).
    /// Off unless explicitly enabled. The master switch — see
    /// `notify_events` to pick which events fire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<bool>,
    /// Which lifecycle events fire a notification, when `notify` is on. An
    /// absent table means every event fires; a present one fires only the
    /// events set to `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_events: Option<NotifyEvents>,
    /// The agent hand-off (`beagle handoff <slug>`): what to launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<HandoffConfig>,
}

/// Per-event notification flags. Any omitted event defaults to off, so a
/// `[notify_events]` table with only `final_review = true` notifies on
/// nothing else. Use [`NotifyEvents::all`] for "every event" (the default
/// when the table is absent entirely).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // independent per-event flags, not an encoded state
pub struct NotifyEvents {
    /// A new incident appeared under `rcas/`.
    #[serde(default)]
    pub new_incident: bool,
    /// A workspace moved to `investigating`.
    #[serde(default)]
    pub investigating: bool,
    /// A workspace moved to `review`.
    #[serde(default)]
    pub review: bool,
    /// A workspace moved to `agent`.
    #[serde(default)]
    pub agent: bool,
    /// A workspace moved to `final-review`.
    #[serde(default)]
    pub final_review: bool,
    /// A workspace moved to `finished`.
    #[serde(default)]
    pub finished: bool,
}

impl NotifyEvents {
    /// Every event enabled — the default when no `[notify_events]` table is
    /// configured but notifications are on.
    #[must_use]
    pub fn all() -> Self {
        Self {
            new_incident: true,
            investigating: true,
            review: true,
            agent: true,
            final_review: true,
            finished: true,
        }
    }
}

/// Configures `beagle handoff`: the agent beagle launches on a reviewed
/// RCA, and the prompt that drives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffConfig {
    /// The agent command as argv, e.g. `["codex", "exec"]` or
    /// `["claude", "-p"]`. beagle pipes the composed prompt (your prompt
    /// file plus the RCA write-up) to its stdin and runs it in the store
    /// root, with `BEAGLE_RCA_SLUG` / `BEAGLE_RCA_DIR` in the environment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    /// A markdown prompt file with the task instructions for the agent.
    /// `~` is expanded. Optional — without it, only the write-up is sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<PathBuf>,
}

/// Where the config file lives: `$BEAGLE_CONFIG` if set, else
/// `$XDG_CONFIG_HOME/beagle/config.toml`, else `$HOME/.config/beagle/config.toml`,
/// else `.beagle.toml` in the current directory as a last resort.
#[must_use]
pub fn path() -> PathBuf {
    if let Some(explicit) = env::var_os(CONFIG_ENV) {
        return PathBuf::from(explicit);
    }
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("beagle").join("config.toml");
    }
    if let Some(home) = env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home)
            .join(".config")
            .join("beagle")
            .join("config.toml");
    }
    PathBuf::from(".beagle.toml")
}

/// Loads and validates the config at `path`. `Ok(None)` when the file does
/// not exist — no config is a perfectly good config.
///
/// # Errors
/// [`Error::Io`] if the file exists but cannot be read,
/// [`Error::ParseConfig`] if it does not parse or contains unknown fields.
pub fn load(path: &Path) -> Result<Option<Config>> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(path, e)),
    };
    let config = parse(&raw).map_err(|source| Error::ParseConfig {
        path: path.to_owned(),
        source,
    })?;
    Ok(Some(config))
}

/// Loads the config from the default [`path`].
///
/// # Errors
/// As for [`load`].
pub fn load_default() -> Result<Option<Config>> {
    load(&path())
}

/// Parses config TOML. Split out from [`load`] so validation is testable
/// without a filesystem.
///
/// # Errors
/// Returns the TOML error on invalid syntax or unknown fields.
pub fn parse(raw: &str) -> std::result::Result<Config, Box<toml::de::Error>> {
    toml::from_str(raw).map_err(Box::new)
}

/// Sets one `key = value` assignment in the config file, preserving every
/// other line (comments included): an existing active assignment is
/// replaced, a commented-out template line (`# key = ...`) is uncommented,
/// and a missing key is appended. The result is validated before anything
/// is written, and the write is atomic (temp + rename) — an invalid value
/// can never corrupt the file. Returns the freshly parsed config.
///
/// `raw_value` is inserted verbatim, so strings must arrive quoted
/// (`"\"vim\""`) and booleans bare (`"true"`).
///
/// # Errors
/// [`Error::ParseConfig`] if the resulting file would not validate,
/// [`Error::Io`] on write failures.
pub fn upsert(path: &Path, key: &str, raw_value: &str) -> Result<Config> {
    let original = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => TEMPLATE.to_owned(),
        Err(e) => return Err(Error::io(path, e)),
    };
    let assignment = format!("{key} = {raw_value}");
    let mut lines: Vec<String> = original.lines().map(str::to_owned).collect();

    let is_active = |line: &str| {
        let trimmed = line.trim_start();
        trimmed.starts_with(&format!("{key} =")) || trimmed.starts_with(&format!("{key}="))
    };
    let is_commented = |line: &str| {
        let trimmed = line.trim_start();
        trimmed
            .strip_prefix('#')
            .map(str::trim_start)
            .is_some_and(|rest| {
                rest.starts_with(&format!("{key} =")) || rest.starts_with(&format!("{key}="))
            })
    };

    if let Some(line) = lines.iter_mut().find(|line| is_active(line)) {
        line.clone_from(&assignment);
    } else if let Some(line) = lines.iter_mut().find(|line| is_commented(line)) {
        line.clone_from(&assignment);
    } else {
        lines.push(assignment);
    }
    let mut updated = lines.join("\n");
    updated.push('\n');

    // Validate before touching the file.
    let config = parse(&updated).map_err(|source| Error::ParseConfig {
        path: path.to_owned(),
        source,
    })?;

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, &updated).map_err(|e| Error::io(&tmp, e))?;
    fs::rename(&tmp, path).map_err(|e| Error::io(path, e))?;
    Ok(config)
}

/// The editor command for `beagle config`, resolved as: config `editor` (if
/// the config currently parses) → `$VISUAL` → `$EDITOR` → `vim`. A broken
/// config falls through to the environment — that is exactly the situation
/// where the user needs the editor to open.
#[must_use]
pub fn editor(config: Option<&Config>) -> String {
    if let Some(editor) = config.and_then(|c| c.editor.clone()) {
        return editor;
    }
    for var in ["VISUAL", "EDITOR"] {
        if let Some(value) = env::var_os(var) {
            let value = value.to_string_lossy().trim().to_owned();
            if !value.is_empty() {
                return value;
            }
        }
    }
    "vim".to_owned()
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
