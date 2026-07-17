//! The bundled `/beagle` skill and installing it for coding agents.
//!
//! The skill markdown is embedded into the binary at compile time from the
//! repo's canonical `.claude/skills/beagle/SKILL.md`, so a release carries
//! a snapshot of the skill and `beagle update` can offer to install or
//! refresh it wherever an agent looks for it — Claude Code
//! (`~/.claude/skills/beagle/SKILL.md`) and Codex
//! (`~/.codex/prompts/beagle.md`). Everything here is pure given a home
//! directory, so it tests against a temp dir without touching the real one.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The `/beagle` skill markdown, embedded from the repo's single source of
/// truth. Rebuilding the binary re-snapshots it; that is how a newer
/// release ships a newer skill.
pub const SKILL_MD: &str = include_str!("../../.claude/skills/beagle/SKILL.md");

/// A coding agent beagle can install its skill for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// Anthropic's Claude Code — skills live under `~/.claude/skills/`.
    Claude,
    /// The Codex coding agent — custom prompts live under
    /// `~/.codex/prompts/`.
    Codex,
}

impl Agent {
    /// Every agent, in display order.
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    /// The agent's config directory under `home`; its existence is how we
    /// detect that the agent is set up on this machine.
    fn config_dir(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home.join(".claude"),
            Self::Codex => home.join(".codex"),
        }
    }

    /// Where this agent reads the beagle skill / prompt from.
    #[must_use]
    pub fn skill_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home
                .join(".claude")
                .join("skills")
                .join("beagle")
                .join("SKILL.md"),
            Self::Codex => home.join(".codex").join("prompts").join("beagle.md"),
        }
    }

    /// The content to write for this agent. Claude keeps the YAML
    /// frontmatter (it drives skill discovery); a Codex prompt is injected
    /// verbatim, so it gets the body only.
    #[must_use]
    pub fn content(self) -> &'static str {
        match self {
            Self::Claude => SKILL_MD,
            Self::Codex => body_without_frontmatter(SKILL_MD),
        }
    }

    /// Whether this agent appears set up (its config directory exists).
    #[must_use]
    pub fn is_present(self, home: &Path) -> bool {
        self.config_dir(home).is_dir()
    }
}

/// How the installed skill compares to the bundled one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillStatus {
    /// No skill file for this agent yet.
    Missing,
    /// A skill file exists but differs from the bundled version.
    Outdated,
    /// The installed skill matches the bundled version.
    Current,
}

/// Compares the on-disk skill for `agent` against the bundled content.
#[must_use]
pub fn status(agent: Agent, home: &Path) -> SkillStatus {
    match std::fs::read_to_string(agent.skill_path(home)) {
        Ok(existing) if existing == agent.content() => SkillStatus::Current,
        Ok(_) => SkillStatus::Outdated,
        Err(_) => SkillStatus::Missing,
    }
}

/// Writes the bundled skill for `agent`, creating parent directories.
///
/// # Errors
/// [`Error::Io`] if the directories or file cannot be written.
pub fn install(agent: Agent, home: &Path) -> Result<PathBuf> {
    let path = agent.skill_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(&path, agent.content()).map_err(|e| Error::io(&path, e))?;
    Ok(path)
}

/// The user's home directory from `$HOME`.
///
/// # Errors
/// [`Error::Tool`] when `$HOME` is unset or empty.
pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| Error::Tool {
            tool: "skill",
            message: "cannot locate your home directory ($HOME is unset)".to_owned(),
        })
}

/// The markdown with a leading `---\n … \n---` YAML frontmatter block
/// removed (and the blank lines after it). Returns the input unchanged when
/// there is no frontmatter.
#[must_use]
pub fn body_without_frontmatter(md: &str) -> &str {
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + "\n---\n".len()..].trim_start_matches('\n');
        }
    }
    md
}

#[cfg(test)]
#[path = "tests/skill.rs"]
mod tests;
