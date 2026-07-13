//! Agent-skill installation: `beagle install --skills`.
//!
//! The `/beagle` skill teaches an agent the whole authoring workflow
//! (toolbox → scaffold → narrate → attach PRs). It ships inside this repo
//! at `.claude/skills/beagle/`, which only helps when the agent is working
//! *in this repo* — so the skill markdown is embedded in the binary and
//! this module installs it globally for every agent CLI found on the
//! machine: Claude Code, Codex, and opencode.
//!
//! An agent counts as present when its binary is on `PATH` **or** its
//! config directory exists (shell aliases and wrapper functions hide
//! binaries from `PATH` scans). Installing overwrites: the skill tracks
//! the binary's version, and `install --skills` is how they stay in sync.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The `/beagle` skill, embedded at build time so the installed binary can
/// write it anywhere without a checkout.
pub const SKILL_MD: &str = include_str!("../.claude/skills/beagle/SKILL.md");

/// One agent CLI the skill can be installed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTarget {
    /// Display name (also the binary name): `claude`, `codex`, `opencode`.
    pub name: &'static str,
    /// The agent's config directory; its existence counts as "installed".
    pub config_dir: PathBuf,
    /// Where the skill file belongs for this agent.
    pub skill_file: PathBuf,
}

/// What happened for one agent during installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Skill written for the first time.
    Created,
    /// Existing skill overwritten (kept in sync with this binary).
    Updated,
    /// Agent not found on this machine; nothing written.
    NotFound,
}

/// The supported agents and their skill locations, resolved against `home`
/// and (for XDG-style agents) `xdg_config` when set.
#[must_use]
pub fn agent_targets(home: &Path, xdg_config: Option<&Path>) -> Vec<AgentTarget> {
    let xdg = xdg_config.map_or_else(|| home.join(".config"), Path::to_path_buf);
    vec![
        AgentTarget {
            name: "claude",
            config_dir: home.join(".claude"),
            skill_file: home
                .join(".claude")
                .join("skills")
                .join("beagle")
                .join("SKILL.md"),
        },
        AgentTarget {
            name: "codex",
            config_dir: home.join(".codex"),
            skill_file: home
                .join(".codex")
                .join("skills")
                .join("beagle")
                .join("SKILL.md"),
        },
        AgentTarget {
            name: "opencode",
            config_dir: xdg.join("opencode"),
            skill_file: xdg
                .join("opencode")
                .join("skill")
                .join("beagle")
                .join("SKILL.md"),
        },
    ]
}

/// Whether `binary` is an executable file on any `PATH` entry.
#[must_use]
pub fn binary_on_path(binary: &str, path_var: Option<&OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    std::env::split_paths(path_var).any(|dir| dir.join(binary).is_file())
}

/// Whether this agent is present: binary on `PATH` or config dir on disk
/// (wrapper functions and aliases hide binaries from `PATH` scans).
#[must_use]
pub fn detected(target: &AgentTarget, path_var: Option<&OsStr>) -> bool {
    binary_on_path(target.name, path_var) || target.config_dir.is_dir()
}

/// Installs the embedded skill for one agent, or reports it missing.
///
/// # Errors
/// I/O failures creating the skill directory or writing the file.
pub fn install(target: &AgentTarget, path_var: Option<&OsStr>) -> io::Result<Outcome> {
    if !detected(target, path_var) {
        return Ok(Outcome::NotFound);
    }
    let existed = target.skill_file.exists();
    if let Some(parent) = target.skill_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target.skill_file, SKILL_MD)?;
    Ok(if existed {
        Outcome::Updated
    } else {
        Outcome::Created
    })
}

#[cfg(test)]
#[path = "tests/skills.rs"]
mod tests;
