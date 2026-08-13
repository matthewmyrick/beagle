//! `beagle-agentd` — the always-on daemon that hosts beagle's agent engine.
//!
//! On startup it kills any process groups a previous run leaked, loads the
//! agent config, and builds one [`Orchestrator`] per enabled agent. Then it
//! ticks them: `beagle-agentd once` runs a single tick per agent and exits
//! (handy for a manual end-to-end), while `beagle-agentd` loops, ticking every
//! poll interval. The control socket and autostart wiring arrive in later
//! issues (epic #137).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use beagle::store::Store as RcaStore;
use beagle_agent::config::{self, AgentDef, Trigger};
use beagle_agent::orchestrator::{default_precheck, GhPublisher, Orchestrator, RunPolicy, Tick};
use beagle_agent::runner::{cleanup_orphans, Runner};
use beagle_agent::store::Store as JobStore;
use beagle_agent::worktree::Manager as Worktrees;

/// Per-session hard timeout (mirrors the Go pipeline's agent timeout).
const SESSION_TIMEOUT: Duration = Duration::from_secs(45 * 60);
/// Attempts before a job is marked failed.
const MAX_ATTEMPTS: u32 = 3;
/// Jobs launched per tick per agent.
const MAX_PER_POLL: usize = 5;
/// Branch namespace for pipeline-owned worktrees.
const BRANCH_PREFIX: &str = "agent";

fn main() -> ExitCode {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("version" | "--version" | "-V") => {
            println!("beagle-agentd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("once") => dispatch(true),
        None => dispatch(false),
        Some(other) => {
            eprintln!("beagle-agentd: unknown argument `{other}` (use `once`, `version`, or no argument to run the loop)");
            ExitCode::from(2)
        }
    }
}

/// Runs the daemon, reporting a fatal setup error as a non-zero exit.
fn dispatch(once: bool) -> ExitCode {
    match run(once) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("beagle-agentd: {err}");
            ExitCode::FAILURE
        }
    }
}

/// A built agent: its id, poll interval, and orchestrator.
struct Runtime {
    agent_id: String,
    poll: Duration,
    orchestrator: Orchestrator,
}

/// Sets up state, builds the enabled agents, then ticks them once or forever.
fn run(once: bool) -> Result<(), String> {
    let state = state_dir()?;
    std::fs::create_dir_all(&state).map_err(|err| format!("creating state dir: {err}"))?;

    match cleanup_orphans(&state.join("runs")) {
        Ok(cleaned) if cleaned > 0 => {
            eprintln!("beagle-agentd: cleaned {cleaned} orphaned session(s) on startup");
        }
        Ok(_) => {}
        Err(err) => eprintln!("beagle-agentd: orphan cleanup failed: {err}"),
    }

    let cfg = config::load_default().map_err(|err| err.to_string())?;
    let agents: Vec<AgentDef> = cfg
        .agents
        .into_iter()
        .filter(|agent| agent.enabled)
        .collect();
    if agents.is_empty() {
        eprintln!("beagle-agentd: no enabled agents configured; nothing to do");
        return Ok(());
    }

    let mut runtimes = Vec::with_capacity(agents.len());
    for agent in &agents {
        runtimes.push(build(agent, &state)?);
    }

    if once {
        for runtime in &runtimes {
            log_tick(&runtime.agent_id, &runtime.orchestrator.run_once());
        }
        return Ok(());
    }

    let interval = runtimes
        .iter()
        .map(|runtime| runtime.poll)
        .min()
        .unwrap_or(Duration::from_secs(60));
    eprintln!(
        "beagle-agentd: running {} agent(s), ticking every {}s",
        runtimes.len(),
        interval.as_secs()
    );
    loop {
        for runtime in &runtimes {
            log_tick(&runtime.agent_id, &runtime.orchestrator.run_once());
        }
        std::thread::sleep(interval);
    }
}

/// Builds one agent's orchestrator from its config.
fn build(agent: &AgentDef, state: &Path) -> Result<Runtime, String> {
    let Trigger::RcaStatus { status } = &agent.trigger;
    let rcas = RcaStore::open(&rca_root()?).map_err(|err| err.to_string())?;
    let base_prompt = std::fs::read_to_string(&agent.prompt)
        .map_err(|err| format!("reading prompt {}: {err}", agent.prompt.display()))?;
    let jobs = JobStore::open(&state.join(format!("jobs-{}.db", agent.id)))
        .map_err(|err| err.to_string())?;
    let worktrees = Worktrees::new(
        agent.target_repo.clone(),
        state.join("worktrees").join(&agent.id),
        BRANCH_PREFIX.to_string(),
    );
    let runner = Runner::new(state.join("runs"));

    let policy = RunPolicy {
        trigger_status: status.clone(),
        base_prompt,
        allowed_tools: agent.allowed_tools.clone(),
        timeout: SESSION_TIMEOUT,
        max_concurrent: usize::try_from(agent.max_concurrent).unwrap_or(1).max(1),
        max_per_poll: MAX_PER_POLL,
        max_attempts: MAX_ATTEMPTS,
        logs_dir: state.join("logs").join(&agent.id),
    };
    let orchestrator = Orchestrator::new(
        policy,
        rcas,
        jobs,
        worktrees,
        runner,
        Box::new(GhPublisher),
        Box::new(default_precheck),
    );
    Ok(Runtime {
        agent_id: agent.id.clone(),
        poll: agent.poll_interval,
        orchestrator,
    })
}

/// Prints a tick's outcome to stderr (the launchd/systemd log stream).
fn log_tick(agent_id: &str, tick: &Tick) {
    match tick {
        Tick::Skipped { reason } => {
            eprintln!("beagle-agentd[{agent_id}]: idle-skip: {reason}");
        }
        Tick::Ran { results } => {
            for result in results {
                eprintln!(
                    "beagle-agentd[{agent_id}]: {} -> {:?}",
                    result.id, result.outcome
                );
            }
        }
    }
}

/// The daemon's state directory: `$XDG_STATE_HOME/beagle-agent`, else
/// `~/.local/state/beagle-agent`.
fn state_dir() -> Result<PathBuf, String> {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("beagle-agent"));
        }
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/state/beagle-agent"))
}

/// The RCA store root: `$BEAGLE_AGENT_RCA_ROOT` if set, else the current dir.
fn rca_root() -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("BEAGLE_AGENT_RCA_ROOT") {
        if !root.is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    std::env::current_dir().map_err(|err| format!("resolving current dir: {err}"))
}
