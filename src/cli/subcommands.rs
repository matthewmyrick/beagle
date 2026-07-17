//! Per-subcommand argument parsers. Each consumes the remaining arguments
//! after the subcommand word and returns a fully typed [`Command`].

use std::path::PathBuf;

use beagle::model::{RcaId, Severity, Status};
use beagle::update;

use super::{take_value, Command};

pub(super) fn parse_update(args: &mut impl Iterator<Item = String>) -> Result<Command, String> {
    let mut version: Option<update::Version> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--version" => version = Some(take_value(args, "--version")?.parse()?),
            other => return Err(format!("unknown flag `{other}` for `update`")),
        }
    }
    Ok(Command::Update { version })
}

pub(super) fn parse_list(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let mut status: Option<Status> = None;
    let mut severity: Option<Severity> = None;
    let mut archived = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--status" => status = Some(take_value(args, "--status")?.parse()?),
            "--severity" => severity = Some(take_value(args, "--severity")?.parse()?),
            "--archived" => archived = true,
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `list`")),
        }
    }
    Ok(Command::List {
        root,
        status,
        severity,
        archived,
    })
}

pub(super) fn parse_archive(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`archive` requires an <id> slug as its first argument")?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    let mut force = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--force" => force = true,
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `archive`")),
        }
    }
    Ok(Command::Archive { root, id, force })
}

pub(super) fn parse_status(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`status` requires an <id> slug as its first argument")?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    let status: Status = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`status` requires a <status> as its second argument")?
        .parse()?;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `status`")),
        }
    }
    Ok(Command::SetStatus { root, id, status })
}

pub(super) fn parse_similar(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`similar` requires an <id> slug as its first argument")?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `similar`")),
        }
    }
    Ok(Command::Similar { root, id })
}

/// `log <id> <message...>`: everything that isn't a flag becomes the
/// message, so multi-word messages work without quoting.
pub(super) fn parse_log(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`log` requires an <id> slug as its first argument")?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    let mut words: Vec<String> = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag `{flag}` for `log`"));
            }
            _ => words.push(arg),
        }
    }
    if words.is_empty() {
        return Err("`log` requires a message after the <id>".to_owned());
    }
    Ok(Command::Log {
        root,
        id,
        message: words.join(" "),
    })
}

pub(super) fn parse_pr(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let action = args
        .next()
        .ok_or("`pr` requires a subcommand: `add` or `list`")?;
    if action != "add" && action != "list" {
        return Err(format!(
            "unknown `pr` subcommand `{action}` (expected add|list)"
        ));
    }
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| format!("`pr {action}` requires an <id> slug"))?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    let url = if action == "add" {
        Some(
            args.next()
                .filter(|a| !a.starts_with('-'))
                .ok_or("`pr add` requires a <url> after the <id>")?,
        )
    } else {
        None
    };
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `pr {action}`")),
        }
    }
    match url {
        // `url` is Some exactly when the action is `add`.
        Some(url) => Ok(Command::PrAdd { root, id, url }),
        None => Ok(Command::PrList { root, id }),
    }
}

pub(super) fn parse_export(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`export` requires an <id> slug as its first argument")?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;
    let mut out: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(take_value(args, "--out")?)),
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `export`")),
        }
    }
    Ok(Command::Export { root, id, out })
}

pub(super) fn parse_new(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let id_raw = args
        .next()
        .filter(|a| !a.starts_with('-'))
        .ok_or("`new` requires an <id> slug as its first argument")?;
    let id = RcaId::new(id_raw).map_err(|e| e.to_string())?;

    let mut title: Option<String> = None;
    let mut severity = Severity::Medium;
    let mut systems = Vec::new();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--title" => title = Some(take_value(args, "--title")?),
            "--severity" => severity = take_value(args, "--severity")?.parse()?,
            "--system" => systems.push(take_value(args, "--system")?),
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `new`")),
        }
    }
    let title = title.ok_or("`new` requires --title")?;
    if title.trim().is_empty() {
        return Err("--title must not be empty".to_owned());
    }
    Ok(Command::New {
        root,
        id,
        title,
        severity,
        systems,
    })
}
