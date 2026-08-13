//! Agent configuration: the `agents.toml` schema and its loader.
//!
//! The file lists the agents the daemon runs — one `[[agent]]` table each. It
//! is optional: an absent file loads as an empty set, so the daemon starts
//! cleanly with nothing configured. Unknown top-level keys are rejected at
//! parse time so typos surface immediately instead of being silently ignored.
//!
//! Example:
//!
//! ```toml
//! [[agent]]
//! id = "rca-remediation"
//! trigger = { kind = "rca-status", status = "agent" }
//! target_repo = "~/GitHub/matthewmyrick/beagle"
//! prompt = "~/.config/beagle/prompts/rca-remediation.txt"
//! allowed_tools = ["Read", "Edit", "Bash(git:*)", "Bash(gh:*)"]
//! poll_interval = "60s"
//! max_concurrent = 2
//! ```

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// The parsed `agents.toml`: the full list of configured agents.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The configured agents, one per `[[agent]]` table. Empty when the file
    /// is absent.
    #[serde(default, rename = "agent")]
    pub agents: Vec<AgentDef>,
}

/// One agent's configuration: what triggers it, which repo it works in, and
/// the prompt and limits it runs under.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDef {
    /// Stable identifier, unique within the file; used as the job key and in
    /// the UI.
    pub id: String,
    /// Whether the daemon should run this agent. Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// What causes this agent to fire.
    pub trigger: Trigger,
    /// The git repository the agent implements changes in. A leading `~/` is
    /// expanded to `$HOME` at load time.
    pub target_repo: PathBuf,
    /// Path to the custom prompt file handed to the headless `claude` session.
    /// A leading `~/` is expanded to `$HOME` at load time.
    pub prompt: PathBuf,
    /// The `--allowedTools` list passed to the session. Empty means "inherit
    /// the runner's default".
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// How often the trigger is polled. Accepts `ms`, `s`, `m`, or `h`
    /// suffixes (e.g. `60s`, `5m`). Defaults to 60 seconds.
    #[serde(default = "default_poll_interval", deserialize_with = "de_duration")]
    pub poll_interval: Duration,
    /// The most concurrent sessions this agent may run. Must be at least 1;
    /// defaults to 2.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
}

/// What causes an agent to fire. Internally tagged on `kind`, so a trigger
/// table reads `{ kind = "rca-status", status = "agent" }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Trigger {
    /// Fires for every RCA workspace sitting at the given lifecycle status
    /// (v1 uses `agent`).
    RcaStatus {
        /// The RCA `status` value that makes a workspace actionable.
        status: String,
    },
}

/// An error loading or validating the agent config file.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The file exists but could not be read.
    #[error("reading agent config {}: {source}", .path.display())]
    Read {
        /// The config path.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The file could not be parsed as TOML, or contained unknown keys.
    #[error("parsing agent config {}: {message}", .path.display())]
    Parse {
        /// The config path.
        path: PathBuf,
        /// The parser's message.
        message: String,
    },
    /// An agent was valid TOML but semantically invalid (e.g. empty id or
    /// zero concurrency).
    #[error("invalid agent `{id}`: {message}")]
    Invalid {
        /// The offending agent's id.
        id: String,
        /// What was wrong.
        message: String,
    },
}

fn default_enabled() -> bool {
    true
}

fn default_max_concurrent() -> u32 {
    2
}

fn default_poll_interval() -> Duration {
    Duration::from_secs(60)
}

/// Loads the agent config from the default path
/// (`$XDG_CONFIG_HOME/beagle/agents.toml`, else `~/.config/beagle/agents.toml`).
/// A missing file is not an error — it loads as an empty config.
///
/// # Errors
/// Returns [`LoadError`] if the file exists but cannot be read, parsed, or
/// validated.
pub fn load_default() -> Result<Config, LoadError> {
    match default_path() {
        Some(path) => load(&path),
        None => Ok(Config::default()),
    }
}

/// Loads and validates the agent config from `path`. A missing file loads as
/// an empty config; every present agent has its `~/` paths expanded and its
/// fields validated.
///
/// # Errors
/// Returns [`LoadError`] on read, parse, or validation failure.
pub fn load(path: &Path) -> Result<Config, LoadError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Config::default());
        }
        Err(source) => {
            return Err(LoadError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let mut config: Config = toml::from_str(&text).map_err(|err| LoadError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    let home = std::env::var_os("HOME");
    for agent in &mut config.agents {
        agent.expand_paths(home.as_deref());
        agent.validate()?;
    }
    Ok(config)
}

/// The default config path: `$XDG_CONFIG_HOME/beagle/agents.toml` when that
/// variable is set and non-empty, otherwise `~/.config/beagle/agents.toml`.
/// Returns `None` only when neither `XDG_CONFIG_HOME` nor `HOME` is set.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    default_path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

fn default_path_from(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    if let Some(xdg) = xdg_config_home {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("beagle").join("agents.toml"));
        }
    }
    let home = home?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("beagle")
            .join("agents.toml"),
    )
}

/// Parses a human duration like `500ms`, `60s`, `5m`, or `1h`.
///
/// # Errors
/// Returns a descriptive message for empty input, a missing or unknown unit, a
/// non-numeric value, or arithmetic overflow.
pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err("duration must not be empty".to_string());
    }
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: u64 = number
        .parse()
        .map_err(|_| format!("duration `{raw}` must start with a number"))?;
    let seconds = match unit {
        "ms" => return Ok(Duration::from_millis(value)),
        "s" => value,
        "m" => value
            .checked_mul(60)
            .ok_or_else(|| format!("duration `{raw}` is too large"))?,
        "h" => value
            .checked_mul(3600)
            .ok_or_else(|| format!("duration `{raw}` is too large"))?,
        "" => return Err(format!("duration `{raw}` is missing a unit (ms, s, m, h)")),
        other => {
            return Err(format!(
                "duration `{raw}` has an unknown unit `{other}` (use ms, s, m, h)"
            ));
        }
    };
    Ok(Duration::from_secs(seconds))
}

fn de_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    parse_duration(&raw).map_err(serde::de::Error::custom)
}

/// Expands a leading `~` component of `path` to `home`. Paths without a
/// leading `~`, and any path when `home` is unset, are returned unchanged.
fn expand_tilde(path: &Path, home: Option<&OsStr>) -> PathBuf {
    let Ok(rest) = path.strip_prefix("~") else {
        return path.to_path_buf();
    };
    match home {
        Some(home) => PathBuf::from(home).join(rest),
        None => path.to_path_buf(),
    }
}

impl AgentDef {
    /// Expands a leading `~/` in `target_repo` and `prompt` to `home`.
    fn expand_paths(&mut self, home: Option<&OsStr>) {
        self.target_repo = expand_tilde(&self.target_repo, home);
        self.prompt = expand_tilde(&self.prompt, home);
    }

    /// Checks the semantic constraints the schema alone cannot express.
    fn validate(&self) -> Result<(), LoadError> {
        let invalid = |message: String| LoadError::Invalid {
            id: self.id.clone(),
            message,
        };
        if self.id.trim().is_empty() {
            return Err(invalid("id must not be empty".to_string()));
        }
        if self.max_concurrent == 0 {
            return Err(invalid("max_concurrent must be at least 1".to_string()));
        }
        if self.poll_interval.is_zero() {
            return Err(invalid(
                "poll_interval must be greater than zero".to_string(),
            ));
        }
        match &self.trigger {
            Trigger::RcaStatus { status } => {
                if status.trim().is_empty() {
                    return Err(invalid("trigger.status must not be empty".to_string()));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
