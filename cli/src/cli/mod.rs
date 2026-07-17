//! Argument parsing for the `beagle` binary.
//!
//! Parsing here is pure (no filesystem, no network); `main.rs` does the I/O.
//! Everything is parsed into the typed [`Command`] at this boundary, so the
//! rest of the binary never re-inspects raw strings.

mod subcommands;

use std::path::PathBuf;

use beagle::model::{RcaId, Severity, Status};
use beagle::update;

/// The `--help` text.
pub const USAGE: &str = "\
beagle — a TUI for root-cause analysis workspaces

USAGE:
    beagle [--root <dir>]                 open the TUI (default)
    beagle new <id> --title <title>       scaffold a new RCA workspace
                  [--severity <sev>]           critical|high|medium|low|info (default: medium)
                  [--system <name>]...         systems involved (repeatable)
                  [--root <dir>]
    beagle list [--status <status>]       print workspaces to stdout,
                  [--severity <sev>]           optionally filtered
                  [--archived]                 include archived workspaces
                  [--root <dir>]
    beagle archive <id> [--force]         move a finished RCA to
                  [--root <dir>]            rcas/archive/ (kept, but out of
                                            the sidebar; --force skips the
                                            finished-status check)
    beagle unarchive <id> [--root <dir>]  move an archived RCA back to the
                                            active list
    beagle publish <id> [--root <dir>]    mark an RCA public (the web app
                                            builds only published RCAs)
    beagle unpublish <id> [--root <dir>]  make an RCA private again
    beagle status <id> <status>           set a workspace's status; a running
                  [--root <dir>]            TUI picks the change up live
                                            (investigating|review|final-review|finished)
    beagle log <id> <message...>          append a timestamped bullet to the
                  [--root <dir>]            workspace's log.md (the Log tab)
    beagle pr add <id> <url>              attach a remediation PR to the RCA;
                  [--root <dir>]            the TUI tracks merge status via gh
    beagle pr list <id> [--root <dir>]    print attached PRs (live state
                                            included when gh is available)
    beagle similar <id> [--root <dir>]    print past RCAs related to this one
                                            (shared systems and tags, ranked)
    beagle export <id> [--out <file>]     export one RCA as a single markdown
                  [--root <dir>]            document (default: exports/<id>.md)
    beagle init [--root <dir>]            scaffold toolbox.md + systems/ agent
                                            context templates at the root
    beagle config                         edit the config file and validate it
    beagle skill [status|install]         show or install the /beagle skill
                                            for Claude Code and Codex
    beagle update [--version <ver>]       install the latest release, or move
                                            to <ver> (upgrade or downgrade)
    beagle version                        print the installed version
    beagle version list                   browse releases; enter installs one
    beagle banner                         print the BEAGLE banner
    beagle --help | --version

The <id> is a lowercase slug ([a-z0-9-], max 64 chars) and becomes the
directory name under <root>/rcas/. Without --root, the config file's `root`
is used, then the current directory.";

/// What `beagle skill` should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillAction {
    /// Report where each agent's skill stands (default).
    Status,
    /// Write the bundled skill for each agent.
    Install,
}

/// A fully parsed invocation. Parsing happens once, here at the boundary;
/// everything downstream takes typed values. `root` stays optional until
/// `main::run`, where it is resolved against the config file.
#[derive(Debug)]
pub enum Command {
    /// `beagle` (no subcommand): open the TUI.
    Tui {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
    },
    /// `beagle new`: scaffold a workspace.
    New {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// Incident title (`--title`, required).
        title: String,
        /// Incident severity (`--severity`, default medium).
        severity: Severity,
        /// Systems involved (`--system`, repeatable).
        systems: Vec<String>,
    },
    /// `beagle list`: print workspaces to stdout.
    List {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// Only workspaces with this status (`--status`).
        status: Option<Status>,
        /// Only workspaces with this severity (`--severity`).
        severity: Option<Severity>,
        /// Include archived workspaces (`--archived`).
        archived: bool,
    },
    /// `beagle archive`: move a finished workspace to `rcas/archive/`.
    Archive {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// Skip the finished-status check (`--force`).
        force: bool,
    },
    /// `beagle unarchive`: move an archived workspace back.
    Unarchive {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
    },
    /// `beagle publish` / `beagle unpublish`: toggle an RCA's public flag.
    SetPublished {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// Whether to publish (`true`) or unpublish (`false`).
        published: bool,
    },
    /// `beagle status`: set a workspace's status.
    SetStatus {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// The new status.
        status: Status,
    },
    /// `beagle log`: append a timestamped bullet to `log.md`.
    Log {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// The log message (non-flag arguments joined by spaces).
        message: String,
    },
    /// `beagle pr add`: attach a remediation PR URL.
    PrAdd {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// The PR URL.
        url: String,
    },
    /// `beagle pr list`: print attached PRs.
    PrList {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
    },
    /// `beagle similar`: print related workspaces, ranked.
    Similar {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
    },
    /// `beagle export`: write the single-file markdown export.
    Export {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
        /// The workspace slug.
        id: RcaId,
        /// Explicit output path (`--out`).
        out: Option<PathBuf>,
    },
    /// `beagle init`: scaffold toolbox.md + systems/.
    Init {
        /// Explicit `--root`, if given.
        root: Option<PathBuf>,
    },
    /// `beagle config`: edit and validate the config file.
    Config,
    /// `beagle skill`: show or install the `/beagle` skill for agents.
    Skill {
        /// What to do: report status or install.
        action: SkillAction,
    },
    /// `beagle update`: install a release over the running binary.
    Update {
        /// Target version (`--version`); latest when absent.
        version: Option<update::Version>,
    },
    /// `beagle version list`: browse releases.
    VersionList,
    /// `beagle banner`: print the BEAGLE banner.
    Banner,
    /// `beagle --help`.
    Help,
    /// `beagle version` / `--version`.
    Version,
}

/// Parses the raw arguments (without the program name) into a [`Command`].
///
/// # Errors
/// Returns a human-readable message for unknown commands or flags, missing
/// values, and invalid slugs/severities/statuses/versions.
pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut args = args.peekable();
    let mut root: Option<PathBuf> = None;

    // The subcommand, if any, is the first non-flag argument.
    let subcommand = match args.peek().map(String::as_str) {
        Some("--help" | "-h") => return Ok(Command::Help),
        Some("--version" | "-V") => return Ok(Command::Version),
        Some(s) if !s.starts_with('-') => {
            let sub = s.to_owned();
            args.next();
            Some(sub)
        }
        _ => None,
    };

    match subcommand.as_deref() {
        None => {
            parse_common_flags(&mut args, &mut root)?;
            Ok(Command::Tui { root })
        }
        Some("list") => subcommands::parse_list(&mut args, root),
        Some("archive") => subcommands::parse_archive(&mut args, root),
        Some("unarchive") => subcommands::parse_unarchive(&mut args, root),
        Some("publish") => subcommands::parse_set_published(&mut args, root, "publish", true),
        Some("unpublish") => subcommands::parse_set_published(&mut args, root, "unpublish", false),
        Some("status") => subcommands::parse_status(&mut args, root),
        Some("log") => subcommands::parse_log(&mut args, root),
        Some("pr") => subcommands::parse_pr(&mut args, root),
        Some("similar") => subcommands::parse_similar(&mut args, root),
        Some("export") => subcommands::parse_export(&mut args, root),
        Some("new") => subcommands::parse_new(&mut args, root),
        Some("init") => {
            parse_common_flags(&mut args, &mut root)?;
            Ok(Command::Init { root })
        }
        Some("config") => no_arguments(&mut args, "config", Command::Config),
        Some("skill") => subcommands::parse_skill(&mut args),
        Some("update") => subcommands::parse_update(&mut args),
        Some("version") => match args.next().as_deref() {
            None => Ok(Command::Version),
            Some("list") => no_arguments(&mut args, "version list", Command::VersionList),
            Some(other) => Err(format!(
                "unknown `version` subcommand `{other}` (expected `list`)"
            )),
        },
        Some("banner") => no_arguments(&mut args, "banner", Command::Banner),
        Some(other) => Err(format!("unknown command `{other}`")),
    }
}

fn no_arguments(
    args: &mut impl Iterator<Item = String>,
    name: &str,
    command: Command,
) -> Result<Command, String> {
    match args.next() {
        Some(extra) => Err(format!("`{name}` takes no arguments (got `{extra}`)")),
        None => Ok(command),
    }
}

fn parse_common_flags(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    root: &mut Option<PathBuf>,
) -> Result<(), String> {
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => *root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}`")),
        }
    }
    Ok(())
}

/// Consumes the next argument as `flag`'s value.
fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

#[cfg(test)]
#[path = "tests/parse.rs"]
mod tests;
