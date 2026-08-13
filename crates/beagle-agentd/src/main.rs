//! `beagle-agentd` — the always-on daemon that hosts beagle's agent engine.
//!
//! On startup it kills any process groups a previous run leaked, loads the
//! agent config, and builds one [`Orchestrator`] per enabled agent. Then it
//! ticks them: `beagle-agentd once` runs a single tick per agent and exits
//! (handy for a manual end-to-end), while `beagle-agentd` loops, ticking every
//! poll interval and serving the control socket so the TUI and desktop can
//! query status and start/stop agents (epic #137).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beagle::store::Store as RcaStore;
use beagle_agent::config::{self, AgentDef, Trigger};
use beagle_agent::control::{self, Handler, Server};
use beagle_agent::orchestrator::{
    default_precheck, GhPublisher, JobOutcome, JobResult, Orchestrator, RunPolicy, Tick,
};
use beagle_agent::protocol::{AgentStatus, DaemonStatus, Request, Response};
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
/// How many recent job summaries to keep per agent.
const RESULT_HISTORY: usize = 8;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("version" | "--version" | "-V") => {
            println!("beagle-agentd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("once") => dispatch(true),
        None => dispatch(false),
        Some(other) => {
            eprintln!(
                "beagle-agentd: unknown argument `{other}` (use `once`, `version`, or no argument)"
            );
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

/// One agent's live state, shared between the tick loop (writer) and the control
/// server (reader).
struct AgentState {
    enabled: AtomicBool,
    running: AtomicBool,
    last_tick: Mutex<Option<String>>,
    last_results: Mutex<Vec<String>>,
}

/// A built agent: its orchestrator, poll interval, and shared state.
struct Runtime {
    agent_id: String,
    poll: Duration,
    orchestrator: Orchestrator,
    state: Arc<AgentState>,
}

/// The state the control [`Handler`] reads and mutates.
struct Shared {
    version: String,
    agents: Vec<(String, Arc<AgentState>)>,
}

/// Sets up state, builds the enabled agents, then ticks them once or forever.
fn run(once: bool) -> Result<(), String> {
    let state_dir = state_dir()?;
    std::fs::create_dir_all(&state_dir).map_err(|err| format!("creating state dir: {err}"))?;

    match cleanup_orphans(&state_dir.join("runs")) {
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
        runtimes.push(build(agent, &state_dir)?);
    }

    if once {
        for runtime in &runtimes {
            tick_agent(runtime);
        }
        return Ok(());
    }

    serve_and_loop(&runtimes)
}

/// Loop mode: bind the control socket, serve it on a background thread, then
/// tick the agents forever.
fn serve_and_loop(runtimes: &[Runtime]) -> Result<(), String> {
    let shared = Arc::new(Shared {
        version: env!("CARGO_PKG_VERSION").to_string(),
        agents: runtimes
            .iter()
            .map(|runtime| (runtime.agent_id.clone(), Arc::clone(&runtime.state)))
            .collect(),
    });

    let socket = control::default_socket_path();
    match control::bind(&socket) {
        Ok(listener) => {
            let handler: Arc<dyn Handler> = Arc::new(DaemonHandler {
                shared: Arc::clone(&shared),
            });
            let server = Server::new(handler);
            eprintln!("beagle-agentd: control socket at {}", socket.display());
            thread::spawn(move || {
                let _ = server.serve(&listener);
            });
        }
        Err(err) => {
            eprintln!("beagle-agentd: control socket unavailable ({err}); running headless");
        }
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
        for runtime in runtimes {
            tick_agent(runtime);
        }
        thread::sleep(interval);
    }
}

/// Ticks one agent (unless paused) and folds the outcome into its shared state.
fn tick_agent(runtime: &Runtime) {
    if !runtime.state.running.load(Ordering::Acquire) {
        return;
    }
    let tick = runtime.orchestrator.run_once();
    log_tick(&runtime.agent_id, &tick);
    match &tick {
        Tick::Skipped { reason } => {
            *lock(&runtime.state.last_results) = vec![format!("idle-skip: {reason}")];
        }
        Tick::Ran { results } if !results.is_empty() => {
            let mut summaries: Vec<String> = results.iter().map(summarize).collect();
            summaries.truncate(RESULT_HISTORY);
            *lock(&runtime.state.last_results) = summaries;
        }
        Tick::Ran { .. } => {}
    }
    *lock(&runtime.state.last_tick) = Some(epoch_seconds());
}

/// Builds one agent's orchestrator and shared state from its config.
fn build(agent: &AgentDef, state_dir: &Path) -> Result<Runtime, String> {
    let Trigger::RcaStatus { status } = &agent.trigger;
    let rcas = RcaStore::open(&rca_root()?).map_err(|err| err.to_string())?;
    let base_prompt = std::fs::read_to_string(&agent.prompt)
        .map_err(|err| format!("reading prompt {}: {err}", agent.prompt.display()))?;
    let jobs = JobStore::open(&state_dir.join(format!("jobs-{}.db", agent.id)))
        .map_err(|err| err.to_string())?;
    let worktrees = Worktrees::new(
        agent.target_repo.clone(),
        state_dir.join("worktrees").join(&agent.id),
        BRANCH_PREFIX.to_string(),
    );
    let runner = Runner::new(state_dir.join("runs"));

    let policy = RunPolicy {
        trigger_status: status.clone(),
        base_prompt,
        allowed_tools: agent.allowed_tools.clone(),
        timeout: SESSION_TIMEOUT,
        max_concurrent: usize::try_from(agent.max_concurrent).unwrap_or(1).max(1),
        max_per_poll: MAX_PER_POLL,
        max_attempts: MAX_ATTEMPTS,
        logs_dir: state_dir.join("logs").join(&agent.id),
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
        state: Arc::new(AgentState {
            enabled: AtomicBool::new(true),
            running: AtomicBool::new(true),
            last_tick: Mutex::new(None),
            last_results: Mutex::new(Vec::new()),
        }),
    })
}

/// The control handler: serves status and start/stop/reload against the shared
/// agent state.
struct DaemonHandler {
    shared: Arc<Shared>,
}

impl DaemonHandler {
    fn agent_statuses(&self) -> Vec<AgentStatus> {
        self.shared
            .agents
            .iter()
            .map(|(id, state)| AgentStatus {
                id: id.clone(),
                enabled: state.enabled.load(Ordering::Acquire),
                running: state.running.load(Ordering::Acquire),
                last_tick: lock(&state.last_tick).clone(),
                active_sessions: 0,
                last_results: lock(&state.last_results).clone(),
            })
            .collect()
    }

    fn set_running(&self, id: &str, running: bool) -> Response {
        match self.shared.agents.iter().find(|(aid, _)| aid == id) {
            Some((_, state)) => {
                state.running.store(running, Ordering::Release);
                Response::Ok
            }
            None => Response::Error {
                message: format!("unknown agent `{id}`"),
            },
        }
    }

    /// Re-reads the config and applies enabled/paused changes to known agents.
    /// New or removed agents require a restart; this reports success if the
    /// config parses.
    fn reload(&self) -> Response {
        let cfg = match config::load_default() {
            Ok(cfg) => cfg,
            Err(err) => {
                return Response::Error {
                    message: format!("reload failed: {err}"),
                }
            }
        };
        for (id, state) in &self.shared.agents {
            let enabled = cfg
                .agents
                .iter()
                .find(|agent| &agent.id == id)
                .is_some_and(|agent| agent.enabled);
            state.enabled.store(enabled, Ordering::Release);
            state.running.store(enabled, Ordering::Release);
        }
        Response::Ok
    }
}

impl Handler for DaemonHandler {
    fn handle(&self, request: Request) -> Response {
        match request {
            Request::GetStatus => Response::Status {
                status: self.snapshot(),
            },
            Request::ListAgents => Response::Agents {
                agents: self.agent_statuses(),
            },
            Request::StartAgent { id } => self.set_running(&id, true),
            Request::StopAgent { id } => self.set_running(&id, false),
            Request::ReloadConfig => self.reload(),
            // Subscribe is handled by the server, never dispatched here.
            Request::Subscribe => Response::Ok,
        }
    }

    fn snapshot(&self) -> DaemonStatus {
        DaemonStatus {
            version: self.shared.version.clone(),
            agents: self.agent_statuses(),
        }
    }
}

/// Prints a tick's outcome to stderr (the launchd/systemd log stream).
fn log_tick(agent_id: &str, tick: &Tick) {
    match tick {
        Tick::Skipped { reason } => eprintln!("beagle-agentd[{agent_id}]: idle-skip: {reason}"),
        Tick::Ran { results } => {
            for result in results {
                eprintln!("beagle-agentd[{agent_id}]: {}", summarize(result));
            }
        }
    }
}

/// A short one-line summary of a job result.
fn summarize(result: &JobResult) -> String {
    let kind = match &result.outcome {
        JobOutcome::Published { .. } => "published",
        JobOutcome::WillRetry { .. } => "will-retry",
        JobOutcome::GaveUp { .. } => "gave-up",
    };
    format!("{} -> {kind}", result.id)
}

/// Recovers a mutex even if a previous holder panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The current time as epoch-seconds text (a coarse `last_tick` marker).
fn epoch_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
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
