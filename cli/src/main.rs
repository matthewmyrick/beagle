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

use beagle::skill::{self, Agent, SkillStatus};

use cli::{Command, SkillAction, USAGE};

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
            let config = config::load_default().ok().flatten();
            let notify = config.as_ref().and_then(|c| c.notify).unwrap_or(false);
            // Absent `[notify_events]` means every event fires.
            let events = config
                .and_then(|c| c.notify_events)
                .unwrap_or_else(config::NotifyEvents::all);
            ui::run(Store::open(&effective_root(root)?)?, notify, events)
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
            archived,
        } => run_list(root, status, severity, archived),
        Command::Archive { root, id, force } => run_archive(root, &id, force),
        Command::Unarchive { root, id } => run_unarchive(root, &id),
        Command::SetPublished {
            root,
            id,
            published,
        } => run_set_published(root, &id, published),
        Command::Handoff { root, id } => run_handoff(root, &id),
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
        Command::Init { root } => run_init(root),
        Command::Config => run_config(),
        Command::Skill { action } => run_skill(action),
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
    archived: bool,
) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let listing = if archived {
        store.list_all()?
    } else {
        store.list()?
    };
    let (summaries, warnings) = (listing.summaries, listing.warnings);
    for rca in summaries
        .iter()
        .filter(|rca| status.map_or(true, |s| rca.meta.status == s))
        .filter(|rca| severity.map_or(true, |s| rca.meta.severity == s))
    {
        let marker = if rca.archived { "  [archived]" } else { "" };
        println!(
            "{:<10} {:<14} {:<30} {}{marker}",
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

/// `beagle init`: scaffold toolbox.md + systems/ agent context templates.
fn run_init(root: Option<PathBuf>) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let created = store.init_context()?;
    if created.is_empty() {
        println!("toolbox.md and systems/ already present; nothing created");
    } else {
        for path in created {
            println!("created {}", path.display());
        }
        println!(
            "fill these in — agents read them before every investigation (T shows them in the TUI)"
        );
    }
    Ok(())
}

/// `beagle archive`: move a finished workspace to `rcas/archive/`.
fn run_archive(root: Option<PathBuf>, id: &RcaId, force: bool) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let dest = store.archive(id, force)?;
    println!("archived {id} → {}", dest.display());
    Ok(())
}

/// `beagle unarchive`: move an archived workspace back to the active list.
fn run_unarchive(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let dest = store.unarchive(id)?;
    println!("unarchived {id} → {}", dest.display());
    Ok(())
}

/// `beagle publish` / `beagle unpublish`: toggle an RCA's public flag.
fn run_set_published(root: Option<PathBuf>, id: &RcaId, published: bool) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    store.set_published(id, published)?;
    if published {
        println!("{id} is now public — rebuild the web app to include it");
    } else {
        println!("{id} is private again");
    }
    Ok(())
}

/// `beagle handoff <slug>`: launch the configured agent on a reviewed RCA.
/// The composed prompt (config prompt file + the RCA write-up) is piped to
/// the agent's stdin; it runs in the store root with the RCA's identity in
/// the environment, and its output streams to the terminal. Book-ended in
/// the workspace log so the live view shows the hand-off.
fn run_handoff(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let handoff = config::load_default()?
        .and_then(|c| c.handoff)
        .filter(|h| !h.command.is_empty())
        .ok_or_else(|| Error::Tool {
            tool: "handoff",
            message: "no agent configured — add a [handoff] section with a \
                      `command` to your config (`beagle config`)"
                .to_owned(),
        })?;

    let store = Store::open(&effective_root(root)?)?;
    store.read_meta(id)?; // proves the workspace exists

    let prompt = match handoff.prompt {
        Some(path) => {
            let path = expand_tilde(&path);
            Some(fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?)
        }
        None => None,
    };
    let write_up = store.export_markdown(id)?;
    let input = beagle::handoff::compose_input(id.as_str(), prompt.as_deref(), &write_up);

    let workspace = store.workspace_dir(id);
    // `command` is non-empty (filtered above), so `split_first` is `Some`.
    let Some((program, args)) = handoff.command.split_first() else {
        return Ok(());
    };
    println!("→ handing {id} to `{program}` …");
    let _ = store.append_log(id, &format!("agent hand-off started (`{program}`)"));

    let status = run_agent(program, args, store.root(), id, &workspace, &input);
    match status {
        Ok(status) if status.success() => {
            let _ = store.append_log(id, "agent hand-off finished");
            println!("✓ hand-off complete");
            Ok(())
        }
        Ok(status) => {
            let _ = store.append_log(id, &format!("agent hand-off exited with {status}"));
            Err(Error::Tool {
                tool: "handoff",
                message: format!("`{program}` exited with {status}"),
            })
        }
        Err(e) => Err(Error::Tool {
            tool: "handoff",
            message: format!("could not launch `{program}`: {e}"),
        }),
    }
}

/// Spawns the hand-off agent: stdin fed the composed prompt, stdout/stderr
/// inherited so its work streams live, run in the store root with the RCA
/// in the environment.
fn run_agent(
    program: &str,
    args: &[String],
    root: &std::path::Path,
    id: &RcaId,
    workspace: &std::path::Path,
    input: &str,
) -> std::io::Result<std::process::ExitStatus> {
    use std::io::Write as _;
    use std::process::{Command as Proc, Stdio};

    let mut child = Proc::new(program)
        .args(args)
        .current_dir(root)
        .env("BEAGLE_RCA_SLUG", id.as_str())
        .env("BEAGLE_RCA_DIR", workspace)
        .env("BEAGLE_RCA_ROOT", root)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    } // drop closes stdin so the agent sees EOF
    child.wait()
}

/// Expands a leading `~` in a path to `$HOME`.
fn expand_tilde(path: &std::path::Path) -> PathBuf {
    if let Ok(rest) = path.strip_prefix("~") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    path.to_owned()
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

/// `beagle similar`: related workspaces, highest score first. Archived
/// incidents stay in the candidate set — the archive *is* the knowledge
/// base this command exists to mine.
fn run_similar(root: Option<PathBuf>, id: &RcaId) -> Result<(), Error> {
    let store = Store::open(&effective_root(root)?)?;
    let summaries = store.list_all()?.summaries;
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
/// nearest ancestor of the working directory that already contains
/// `rcas/` → the working directory. An invalid config surfaces here
/// rather than being silently ignored — otherwise beagle would quietly
/// open the wrong root.
fn effective_root(explicit: Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(root) = explicit {
        return Ok(root);
    }
    if let Some(config) = config::load_default()? {
        if let Some(root) = config.root {
            return Ok(root);
        }
    }
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    Ok(beagle::store::discover_root(&cwd))
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
        offer_skill_install();
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
    offer_skill_install();
    Ok(())
}

/// `beagle skill`: report where each agent's copy of the skill stands, or
/// write the bundled skill for each.
fn run_skill(action: SkillAction) -> Result<(), Error> {
    let home = skill::home()?;
    match action {
        SkillAction::Status => {
            for agent in Agent::ALL {
                let note = if agent.is_present(&home) {
                    match skill::status(agent, &home) {
                        SkillStatus::Current => "up to date",
                        SkillStatus::Outdated => "outdated — `beagle skill install` refreshes it",
                        SkillStatus::Missing => "not installed — `beagle skill install` adds it",
                    }
                } else {
                    "agent not detected here"
                };
                println!(
                    "{:<12} {note}\n             {}",
                    agent.name(),
                    agent.skill_path(&home).display()
                );
            }
        }
        SkillAction::Install => {
            // Explicit intent: install for detected agents, or for all when
            // none are detected (so a fresh setup still gets the skill).
            let detected: Vec<Agent> = Agent::ALL
                .into_iter()
                .filter(|a| a.is_present(&home))
                .collect();
            let targets = if detected.is_empty() {
                Agent::ALL.to_vec()
            } else {
                detected
            };
            for agent in targets {
                let path = skill::install(agent, &home)?;
                println!("✓ {} → {}", agent.name(), path.display());
            }
        }
    }
    Ok(())
}

/// After an install, offer to install or refresh the `/beagle` skill for
/// every detected agent whose copy is missing or stale. Interactive only —
/// piped output just prints a hint — and best-effort: a decline or a write
/// failure never fails the update.
fn offer_skill_install() {
    let Ok(home) = skill::home() else { return };
    let pending: Vec<Agent> = Agent::ALL
        .into_iter()
        .filter(|a| a.is_present(&home))
        .filter(|a| skill::status(*a, &home) != SkillStatus::Current)
        .collect();
    if pending.is_empty() {
        return;
    }
    let names: Vec<&str> = pending.iter().map(|a| a.name()).collect();
    let list = names.join(" and ");

    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        println!("\nthe bundled /beagle skill is newer than what's installed for {list}.");
        println!("run `beagle skill install` to update it.");
        return;
    }

    print!("\ninstall/update the /beagle skill for {list}? [y/N] ");
    if std::io::Write::flush(&mut std::io::stdout()).is_err() {
        return;
    }
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return;
    }
    if !matches!(answer.trim(), "y" | "Y" | "yes") {
        println!("skipped — run `beagle skill install` anytime.");
        return;
    }
    for agent in pending {
        match skill::install(agent, &home) {
            Ok(path) => println!("✓ {} → {}", agent.name(), path.display()),
            Err(e) => eprintln!("could not install for {}: {e}", agent.name()),
        }
    }
}
