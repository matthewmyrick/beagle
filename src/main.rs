//! CLI entry point: `beagle` opens the TUI; the subcommands give scripts
//! (and Claude) a typed way to create, inspect, and update workspaces — and
//! the binary itself (`beagle update`).
//!
//! Argument parsing lives in [`cli`] and is pure (no filesystem, no
//! network); `run` does the I/O. `--root` resolution consults the config
//! file: explicit flag → config `root` → current directory.

mod cli;

use std::env;
use std::fs;
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::process::ExitCode;

use beagle::model::{RcaId, Severity, Status};
use beagle::store::{new_meta, Store};
use beagle::{config, ui, update, Error};

use cli::{Command, USAGE};

fn main() -> ExitCode {
    let command = match cli::parse_args(env::args().skip(1)) {
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
        Command::Tui { root } => {
            let notify = config::load_default()
                .ok()
                .flatten()
                .and_then(|c| c.notify)
                .unwrap_or(false);
            ui::run(Store::open(&effective_root(root)?)?, notify)
        }
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
        } => run_list(root, status, severity),
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
        Command::PrAdd { root, id, url } => {
            let store = Store::open(&effective_root(root)?)?;
            if store.add_pr(&id, &url)? {
                println!("attached {url} to {id}");
            } else {
                println!("{url} is already attached to {id}");
            }
            Ok(())
        }
        Command::PrList { root, id } => run_pr_list(root, &id),
        Command::Similar { root, id } => run_similar(root, &id),
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

/// `beagle list`: workspaces to stdout, warnings to stderr.
fn run_list(
    root: Option<PathBuf>,
    status: Option<Status>,
    severity: Option<Severity>,
) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let listing = store.list()?;
    let (summaries, warnings) = (listing.summaries, listing.warnings);
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
    for broken in &listing.broken {
        println!("⚠ broken     {:<30} {}", broken.dir_name, broken.reason);
    }
    for warning in &warnings {
        eprintln!("warning: {}", warning.0);
    }
    Ok(())
}

/// `beagle pr list`: attached PRs, with live state when `gh` works.
fn run_pr_list(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let meta = store.read_meta(id)?;
    if meta.prs.is_empty() {
        println!("no PRs attached to {id} (use `beagle pr add {id} <url>`)");
        return Ok(());
    }
    let gh = beagle::prs::gh_available();
    for url in &meta.prs {
        match gh.then(|| beagle::prs::state_of(url)).flatten() {
            Some(state) => println!("{} {:<8} {url}", state.glyph(), state.label()),
            None => println!("  {:<8} {url}", "-"),
        }
    }
    Ok(())
}

/// `beagle similar`: related workspaces, highest score first.
fn run_similar(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let summaries = store.list()?.summaries;
    let target = summaries
        .iter()
        .find(|rca| rca.id == *id)
        .ok_or_else(|| Error::Tool {
            tool: "similar",
            message: format!("no workspace `{id}` under this root"),
        })?;
    let related = beagle::similar::rank(target, &summaries);
    if related.is_empty() {
        println!("no related incidents (nothing shares systems or tags with {id})");
        return Ok(());
    }
    for entry in &related {
        println!(
            "{:<3} {:<10} {:<14} {:<40} {}",
            entry.score,
            entry.rca.meta.severity,
            entry.rca.meta.status,
            entry.rca.id,
            beagle::similar::shared_label(entry),
        );
    }
    Ok(())
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
