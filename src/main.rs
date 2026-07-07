//! CLI entry point: `beagle` opens the TUI; `new` and `list` give
//! scripts (and Claude) a typed way to create and inspect workspaces.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use beagle::model::{RcaId, Severity, Status};
use beagle::store::{new_meta, Store};
use beagle::ui;

const USAGE: &str = "\
beagle — a TUI for root-cause analysis workspaces

USAGE:
    beagle [--root <dir>]                 open the TUI (default)
    beagle new <id> --title <title>       scaffold a new RCA workspace
                  [--severity <sev>]           critical|high|medium|low|info (default: medium)
                  [--system <name>]...         systems involved (repeatable)
                  [--root <dir>]
    beagle list [--status <status>]       print workspaces to stdout,
                  [--severity <sev>]           optionally filtered
                  [--root <dir>]
    beagle status <id> <status>           set a workspace's status; a running
                  [--root <dir>]            TUI picks the change up live
                                            (investigating|identified|monitoring|resolved)
    beagle export <id> [--out <file>]     export one RCA as a single markdown
                  [--root <dir>]            document (default: exports/<id>.md)
    beagle banner                         print the BEAGLE banner
    beagle --help | --version

The <id> is a lowercase slug ([a-z0-9-], max 64 chars) and becomes the
directory name under <root>/rcas/.";

/// A fully parsed invocation. Parsing happens once, here at the boundary;
/// everything downstream takes typed values.
#[derive(Debug)]
enum Command {
    Tui {
        root: PathBuf,
    },
    New {
        root: PathBuf,
        id: RcaId,
        title: String,
        severity: Severity,
        systems: Vec<String>,
    },
    List {
        root: PathBuf,
        status: Option<Status>,
        severity: Option<Severity>,
    },
    SetStatus {
        root: PathBuf,
        id: RcaId,
        status: Status,
    },
    Export {
        root: PathBuf,
        id: RcaId,
        out: Option<PathBuf>,
    },
    Banner,
    Help,
    Version,
}

fn main() -> ExitCode {
    let command = match parse_args(env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), beagle::Error> {
    match command {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            println!("beagle {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Tui { root } => ui::run(Store::open(&root)?),
        Command::New {
            root,
            id,
            title,
            severity,
            systems,
        } => {
            let store = Store::open(&root)?;
            let mut meta = new_meta(title, severity);
            meta.systems = systems;
            let dir = store.scaffold(&id, &meta)?;
            println!("created {}", dir.display());
            Ok(())
        }
        Command::Export { root, id, out } => {
            let store = Store::open(&root)?;
            let path = store.export_to(&id, out.as_deref())?;
            println!("{}", path.display());
            Ok(())
        }
        Command::List {
            root,
            status,
            severity,
        } => {
            let store = Store::open(&root)?;
            let (summaries, warnings) = store.list()?;
            for rca in summaries
                .iter()
                .filter(|rca| status.map_or(true, |s| rca.meta.status == s))
                .filter(|rca| severity.map_or(true, |s| rca.meta.severity == s))
            {
                println!(
                    "{:<10} {:<14} {:<30} {}",
                    rca.meta.severity, rca.meta.status, rca.id, rca.meta.title,
                );
            }
            for warning in &warnings {
                eprintln!("warning: {}", warning.0);
            }
            Ok(())
        }
        Command::SetStatus { root, id, status } => {
            let store = Store::open(&root)?;
            store.set_status(&id, status)?;
            println!("{id}: status → {status}");
            Ok(())
        }
        Command::Banner => {
            println!(
                "{}\nbeagle {} — a TUI for root-cause analysis workspaces",
                beagle::banner::BANNER,
                env!("CARGO_PKG_VERSION"),
            );
            Ok(())
        }
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Command, String> {
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
            Ok(Command::Tui {
                root: resolve_root(root)?,
            })
        }
        Some("list") => parse_list(&mut args, root),
        Some("status") => parse_status(&mut args, root),
        Some("export") => parse_export(&mut args, root),
        Some("new") => parse_new(&mut args, root),
        Some("banner") => {
            if let Some(extra) = args.next() {
                return Err(format!("`banner` takes no arguments (got `{extra}`)"));
            }
            Ok(Command::Banner)
        }
        Some(other) => Err(format!("unknown command `{other}`")),
    }
}

fn parse_list(
    args: &mut impl Iterator<Item = String>,
    mut root: Option<PathBuf>,
) -> Result<Command, String> {
    let mut status: Option<Status> = None;
    let mut severity: Option<Severity> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--status" => status = Some(take_value(args, "--status")?.parse()?),
            "--severity" => severity = Some(take_value(args, "--severity")?.parse()?),
            "--root" => root = Some(PathBuf::from(take_value(args, "--root")?)),
            other => return Err(format!("unknown flag `{other}` for `list`")),
        }
    }
    Ok(Command::List {
        root: resolve_root(root)?,
        status,
        severity,
    })
}

fn parse_status(
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
    Ok(Command::SetStatus {
        root: resolve_root(root)?,
        id,
        status,
    })
}

fn parse_export(
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
    Ok(Command::Export {
        root: resolve_root(root)?,
        id,
        out,
    })
}

fn parse_new(
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
        root: resolve_root(root)?,
        id,
        title,
        severity,
        systems,
    })
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

fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn resolve_root(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    match explicit {
        Some(root) => Ok(root),
        None => env::current_dir().map_err(|e| format!("cannot determine working directory: {e}")),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

    use super::*;

    fn parse(argv: &[&str]) -> Result<Command, String> {
        parse_args(argv.iter().map(ToString::to_string))
    }

    #[test]
    fn bare_invocation_is_tui() {
        assert!(matches!(parse(&[]), Ok(Command::Tui { .. })));
    }

    #[test]
    fn new_parses_all_flags() {
        let parsed = parse(&[
            "new",
            "pay-latency",
            "--title",
            "Payments latency",
            "--severity",
            "high",
            "--system",
            "payments-api",
            "--system",
            "redis",
            "--root",
            "/tmp/x",
        ]);
        match parsed {
            Ok(Command::New {
                root,
                id,
                title,
                severity,
                systems,
            }) => {
                assert_eq!(root, PathBuf::from("/tmp/x"));
                assert_eq!(id.as_str(), "pay-latency");
                assert_eq!(title, "Payments latency");
                assert_eq!(severity, Severity::High);
                assert_eq!(systems, ["payments-api", "redis"]);
            }
            other => panic!("unexpected parse: {other:?}"),
        }
    }

    #[test]
    fn new_rejects_bad_input() {
        assert!(parse(&["new"]).is_err(), "missing id");
        assert!(parse(&["new", "ok-id"]).is_err(), "missing --title");
        assert!(
            parse(&["new", "Bad_Id", "--title", "t"]).is_err(),
            "invalid slug"
        );
        assert!(parse(&["new", "ok-id", "--title", "t", "--severity", "huge"]).is_err());
        assert!(
            parse(&["new", "ok-id", "--title", "   "]).is_err(),
            "blank title"
        );
    }

    #[test]
    fn export_parses_id_out_and_root() {
        let parsed = parse(&[
            "export",
            "my-rca",
            "--out",
            "/tmp/vault/note.md",
            "--root",
            "/x",
        ]);
        match parsed {
            Ok(Command::Export { root, id, out }) => {
                assert_eq!(root, PathBuf::from("/x"));
                assert_eq!(id.as_str(), "my-rca");
                assert_eq!(out, Some(PathBuf::from("/tmp/vault/note.md")));
            }
            other => panic!("unexpected parse: {other:?}"),
        }
        assert!(parse(&["export"]).is_err(), "missing id");
        assert!(parse(&["export", "Bad Slug"]).is_err(), "invalid slug");
    }

    #[test]
    fn unknown_flags_and_commands_are_rejected() {
        assert!(parse(&["--frobnicate"]).is_err());
        assert!(parse(&["destroy"]).is_err());
    }

    #[test]
    fn list_parses_filters() {
        match parse(&["list", "--status", "investigating", "--severity", "high"]) {
            Ok(Command::List {
                status, severity, ..
            }) => {
                assert_eq!(status, Some(Status::Investigating));
                assert_eq!(severity, Some(Severity::High));
            }
            other => panic!("unexpected parse: {other:?}"),
        }
        match parse(&["list"]) {
            Ok(Command::List {
                status, severity, ..
            }) => {
                assert_eq!(status, None);
                assert_eq!(severity, None);
            }
            other => panic!("unexpected parse: {other:?}"),
        }
        assert!(parse(&["list", "--status", "closed"]).is_err());
        assert!(parse(&["list", "--out", "x"]).is_err());
    }

    #[test]
    fn status_parses_id_status_and_root() {
        match parse(&["status", "my-rca", "investigating", "--root", "/x"]) {
            Ok(Command::SetStatus { root, id, status }) => {
                assert_eq!(root, PathBuf::from("/x"));
                assert_eq!(id.as_str(), "my-rca");
                assert_eq!(status, Status::Investigating);
            }
            other => panic!("unexpected parse: {other:?}"),
        }
        assert!(parse(&["status"]).is_err(), "missing id");
        assert!(parse(&["status", "my-rca"]).is_err(), "missing status");
        assert!(parse(&["status", "my-rca", "closed"]).is_err());
        assert!(parse(&["status", "Bad Slug", "resolved"]).is_err());
    }

    #[test]
    fn banner_parses_and_rejects_arguments() {
        assert!(matches!(parse(&["banner"]), Ok(Command::Banner)));
        assert!(parse(&["banner", "--loud"]).is_err());
    }
}
