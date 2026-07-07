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
mod tests {
    #![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

    use super::*;

    #[test]
    fn template_is_a_valid_all_defaults_config() {
        let config = parse(TEMPLATE).expect("template must parse");
        assert_eq!(config, Config::default(), "template is fully commented");
    }

    #[test]
    fn empty_config_is_valid() {
        assert_eq!(parse("").expect("empty parses"), Config::default());
    }

    #[test]
    fn full_config_round_trips() {
        let config = parse("root = \"/oncall\"\neditor = \"code -w\"\n").expect("parses");
        assert_eq!(config.root.as_deref(), Some(Path::new("/oncall")));
        assert_eq!(config.editor.as_deref(), Some("code -w"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let err = parse("roots = \"/typo\"").expect_err("typo must be caught");
        assert!(err.to_string().contains("roots"), "error names the field");
    }

    #[test]
    fn invalid_types_are_rejected() {
        assert!(parse("root = 7").is_err());
    }

    #[test]
    fn missing_file_loads_as_none() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("nope.toml");
        assert_eq!(load(&missing).expect("absent is fine"), None);
    }

    #[test]
    fn config_editor_wins_over_fallback() {
        let config = Config {
            root: None,
            editor: Some("hx".to_owned()),
        };
        assert_eq!(editor(Some(&config)), "hx");
    }
}
