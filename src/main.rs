//! CLI entry point: `beagle` opens the TUI; the subcommands give scripts
//! (and Claude) a typed way to create, inspect, and update workspaces — and
//! the binary itself (`beagle update`).
//!
//! Parsing here is pure (no filesystem, no network); `run` does the I/O.
//! `--root` resolution consults the config file: explicit flag → config
//! `root` → current directory.

use std::env;
use std::fs;
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::process::ExitCode;

use beagle::model::{RcaId, Severity, Status};
use beagle::store::{new_meta, Store};
use beagle::{config, ui, update, Error};

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
    beagle log <id> <message...>          append a timestamped bullet to the
                  [--root <dir>]            workspace's log.md (the Log tab)
    beagle export <id> [--out <file>]     export one RCA as a single markdown
                  [--root <dir>]            document (default: exports/<id>.md)
    beagle init [--root <dir>]            scaffold toolbox.md + systems/ agent
                                            context templates at the root
    beagle config                         edit the config file and validate it
    beagle update [--version <ver>]       install the latest release, or move
                                            to <ver> (upgrade or downgrade)
    beagle version                        print the installed version
    beagle version list                   browse releases; enter installs one
    beagle banner                         print the BEAGLE banner
    beagle --help | --version

The <id> is a lowercase slug ([a-z0-9-], max 64 chars) and becomes the
directory name under <root>/rcas/. Without --root, the config file's `root`
is used, then the current directory.";

/// A fully parsed invocation. Parsing happens once, here at the boundary;
/// everything downstream takes typed values. `root` stays optional until
/// `run`, where it is resolved against the config file.
#[derive(Debug)]
enum Command {
    Tui {
        root: Option<PathBuf>,
    },
    New {
        root: Option<PathBuf>,
        id: RcaId,
        title: String,
        severity: Severity,
        systems: Vec<String>,
    },
    List {
        root: Option<PathBuf>,
        status: Option<Status>,
        severity: Option<Severity>,
    },
    SetStatus {
        root: Option<PathBuf>,
        id: RcaId,
        status: Status,
    },
    Log {
        root: Option<PathBuf>,
        id: RcaId,
        message: String,
    },
    Export {
        root: Option<PathBuf>,
        id: RcaId,
        out: Option<PathBuf>,
    },
    Init {
        root: Option<PathBuf>,
    },
    Config,
    Update {
        version: Option<update::Version>,
    },
    VersionList,
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

fn run(command: Command) -> Result<(), Error> {
    match command {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            println!(
                "beagle {} ({})",
                update::Version::current(),
                update::release_target().unwrap_or("source build"),
            );
            Ok(())
        }
        Command::Tui { root } => ui::run(Store::open(&effective_root(root)?)?),
        Command::New {
            root,
            id,
            title,
            severity,
            systems,
        } => {
            let store = Store::open(&effective_root(root)?)?;
            let mut meta = new_meta(title, severity);
            meta.systems = systems;
            let dir = store.scaffold(&id, &meta)?;
            println!("created {}", dir.display());
            Ok(())
        }
        Command::Export { root, id, out } => {
            let store = Store::open(&effective_root(root)?)?;
            let path = store.export_to(&id, out.as_deref())?;
            println!("{}", path.display());
            Ok(())
        }
        Command::List {
            root,
            status,
            severity,
        } => {
            let store = Store::open(&effective_root(root)?)?;
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
            let store = Store::open(&effective_root(root)?)?;
            store.set_status(&id, status)?;
            println!("{id}: status → {status}");
            Ok(())
        }
        Command::Log { root, id, message } => {
            let store = Store::open(&effective_root(root)?)?;
            let path = store.append_log(&id, &message)?;
            println!("logged to {}", path.display());
            Ok(())
        }
        Command::Init { root } => {
            let store = Store::open(&effective_root(root)?)?;
            let created = store.init_context()?;
            if created.is_empty() {
                println!("toolbox.md and systems/ already present; nothing created");
            } else {
                for path in created {
                    println!("created {}", path.display());
                }
                println!("fill these in — agents read them before every investigation (T shows them in the TUI)");
            }
            Ok(())
        }
        Command::Config => run_config(),
        Command::Update { version } => {
            let version = match version {
                Some(version) => version,
                None => latest_release()?,
            };
            install_version(version)
        }
        Command::VersionList => run_version_list(),
        Command::Banner => {
            println!(
                "{}\nbeagle {} — a TUI for root-cause analysis workspaces",
                beagle::banner::BANNER,
                update::Version::current(),
            );
            Ok(())
        }
    }
}

/// Resolves the workspace root: explicit `--root` → config file `root` →
/// current directory. An invalid config surfaces here rather than being
/// silently ignored — otherwise beagle would quietly open the wrong root.
fn effective_root(explicit: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(root) = explicit {
        return Ok(root);
    }
    if let Some(config) = config::load_default()? {
        if let Some(root) = config.root {
            return Ok(root);
        }
    }
    env::current_dir().map_err(|e| Error::io(".", e))
}

/// `beagle config`: create the file from the template if absent, open it in
/// the user's editor, and validate it when the editor closes.
fn run_config() -> Result<(), Error> {
    let path = config::path();
    if !path.exists() {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        fs::write(&path, config::TEMPLATE).map_err(|e| Error::io(&path, e))?;
        println!("created {}", path.display());
    }

    // Choose the editor from the pre-edit config when it parses; a broken
    // config falls back to $VISUAL/$EDITOR/vim — that broken state is
    // exactly when the editor most needs to open.
    let pre_edit = config::load(&path).ok().flatten();
    let editor = config::editor(pre_edit.as_ref());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vim");
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .map_err(|e| Error::Tool {
            tool: "editor",
            message: format!("could not launch `{editor}`: {e}"),
        })?;
    if !status.success() {
        return Err(Error::Tool {
            tool: "editor",
            message: format!("`{editor}` exited with {status}; config not validated"),
        });
    }

    match config::load(&path) {
        Ok(Some(config)) => {
            println!("✓ config OK ({})", path.display());
            if let Some(root) = &config.root {
                println!("  root   = {}", root.display());
            }
            if let Some(editor) = &config.editor {
                println!("  editor = {editor}");
            }
            Ok(())
        }
        Ok(None) => {
            println!("config file removed; beagle will use defaults");
            Ok(())
        }
        Err(e) => {
            eprintln!("your edits are saved, but the config does not validate;");
            eprintln!("run `beagle config` again to fix it:");
            Err(e)
        }
    }
}

/// `beagle version list`: interactive picker on a terminal (enter installs
/// the selected version), plain listing when piped.
fn run_version_list() -> Result<(), Error> {
    let releases = update::fetch_releases()?;
    if releases.is_empty() {
        println!("no releases published yet — {}/releases", update::REPO_URL);
        return Ok(());
    }
    let current = update::Version::current();
    if std::io::stdout().is_terminal() {
        match update::pick_version(&releases, current)? {
            Some(version) => install_version(version),
            None => Ok(()),
        }
    } else {
        for (i, release) in releases.iter().enumerate() {
            let mut markers = String::new();
            if i == 0 {
                markers.push_str("  latest");
            }
            if release.version == current {
                markers.push_str("  current");
            }
            println!("{}{markers}", release.version.tag());
        }
        Ok(())
    }
}

fn latest_release() -> Result<update::Version, Error> {
    update::fetch_releases()?
        .first()
        .map(|release| release.version)
        .ok_or_else(|| Error::Tool {
            tool: "update",
            message: format!("no releases published yet — {}/releases", update::REPO_URL),
        })
}

/// Installs `version` over the running binary — the same path for upgrades
/// and downgrades. A no-op (with a message) when already on that version.
fn install_version(version: update::Version) -> Result<(), Error> {
    let current = update::Version::current();
    if version == current {
        println!("already on beagle {current}; nothing to do");
        return Ok(());
    }
    let exe = env::current_exe().map_err(|e| Error::io("beagle", e))?;
    let verb = if version > current {
        "updating"
    } else {
        "downgrading"
    };
    println!("{verb} beagle {current} → {version} …");
    update::update_to(version, &exe)?;
    println!("✓ beagle {version} installed at {}", exe.display());
    println!("  restart any running beagle TUIs to pick it up");
    Ok(())
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
            Ok(Command::Tui { root })
        }
        Some("list") => parse_list(&mut args, root),
        Some("status") => parse_status(&mut args, root),
        Some("log") => parse_log(&mut args, root),
        Some("export") => parse_export(&mut args, root),
        Some("new") => parse_new(&mut args, root),
        Some("init") => {
            parse_common_flags(&mut args, &mut root)?;
            Ok(Command::Init { root })
        }
        Some("config") => no_arguments(&mut args, "config", Command::Config),
        Some("update") => parse_update(&mut args),
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

fn parse_update(args: &mut impl Iterator<Item = String>) -> Result<Command, String> {
    let mut version: Option<update::Version> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--version" => version = Some(take_value(args, "--version")?.parse()?),
            other => return Err(format!("unknown flag `{other}` for `update`")),
        }
    }
    Ok(Command::Update { version })
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
        root,
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
    Ok(Command::SetStatus { root, id, status })
}

/// `log <id> <message...>`: everything that isn't a flag becomes the
/// message, so multi-word messages work without quoting.
fn parse_log(
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
    Ok(Command::Export { root, id, out })
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
        root,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

    use super::*;

    fn parse(argv: &[&str]) -> Result<Command, String> {
        parse_args(argv.iter().map(ToString::to_string))
    }

    #[test]
    fn bare_invocation_is_tui_with_no_explicit_root() {
        assert!(matches!(parse(&[]), Ok(Command::Tui { root: None })));
        assert!(matches!(
            parse(&["--root", "/x"]),
            Ok(Command::Tui { root: Some(_) })
        ));
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
                assert_eq!(root, Some(PathBuf::from("/tmp/x")));
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
                assert_eq!(root, Some(PathBuf::from("/x")));
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
                assert_eq!(root, Some(PathBuf::from("/x")));
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
    fn log_parses_multi_word_messages_and_root() {
        match parse(&[
            "log",
            "my-rca",
            "checked",
            "the",
            "dashboard",
            "--root",
            "/x",
        ]) {
            Ok(Command::Log { root, id, message }) => {
                assert_eq!(root, Some(PathBuf::from("/x")));
                assert_eq!(id.as_str(), "my-rca");
                assert_eq!(message, "checked the dashboard");
            }
            other => panic!("unexpected parse: {other:?}"),
        }
        assert!(parse(&["log"]).is_err(), "missing id");
        assert!(parse(&["log", "my-rca"]).is_err(), "missing message");
        assert!(parse(&["log", "my-rca", "msg", "--force"]).is_err());
    }

    #[test]
    fn banner_parses_and_rejects_arguments() {
        assert!(matches!(parse(&["banner"]), Ok(Command::Banner)));
        assert!(parse(&["banner", "--loud"]).is_err());
    }

    #[test]
    fn config_parses_and_rejects_arguments() {
        assert!(matches!(parse(&["config"]), Ok(Command::Config)));
        assert!(parse(&["config", "extra"]).is_err());
    }

    #[test]
    fn init_parses_with_optional_root() {
        assert!(matches!(parse(&["init"]), Ok(Command::Init { root: None })));
        assert!(matches!(
            parse(&["init", "--root", "/x"]),
            Ok(Command::Init { root: Some(_) })
        ));
        assert!(parse(&["init", "--frob"]).is_err());
    }

    #[test]
    fn version_and_version_list_parse() {
        assert!(matches!(parse(&["version"]), Ok(Command::Version)));
        assert!(matches!(
            parse(&["version", "list"]),
            Ok(Command::VersionList)
        ));
        assert!(parse(&["version", "bump"]).is_err());
        assert!(parse(&["version", "list", "extra"]).is_err());
    }

    #[test]
    fn update_parses_an_optional_target_version() {
        assert!(matches!(
            parse(&["update"]),
            Ok(Command::Update { version: None })
        ));
        match parse(&["update", "--version", "v0.1.0"]) {
            Ok(Command::Update {
                version: Some(version),
            }) => assert_eq!(version.tag(), "v0.1.0"),
            other => panic!("unexpected parse: {other:?}"),
        }
        assert!(parse(&["update", "--version", "latest"]).is_err());
        assert!(parse(&["update", "--force"]).is_err());
    }
}
