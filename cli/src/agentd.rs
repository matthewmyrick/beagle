//! Managing the `beagle-agentd` daemon from the CLI: install/uninstall it as an
//! OS service, start/stop it, and query its status.
//!
//! On macOS the service is a launchd `LaunchAgent` (`KeepAlive` + `RunAtLoad`);
//! on Linux it is a `systemd --user` unit (`Restart=always`,
//! `WantedBy=default.target`). Either way it starts on login and restarts on
//! crash. Status talks to a running daemon directly over its loopback control
//! socket (newline-JSON), so the CLI needs no dependency on the daemon crate —
//! it just speaks the wire format.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

/// The launchd label / service name.
const LABEL: &str = "com.beagle.agentd";

/// How often the TUI status poller re-queries the daemon.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// A snapshot of the whole daemon, decoded from a `get-status` response. Mirrors
/// the daemon's `DaemonStatus` wire type (the CLI can't depend on the daemon
/// crate, so it re-declares the shape it reads).
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonSnapshot {
    /// The daemon version.
    pub version: String,
    /// One entry per configured agent.
    pub agents: Vec<AgentSnapshot>,
}

/// One agent's status within a [`DaemonSnapshot`].
#[derive(Debug, Clone, Deserialize)]
pub struct AgentSnapshot {
    /// The agent id.
    pub id: String,
    /// Whether the config marks it enabled.
    pub enabled: bool,
    /// Whether the daemon is currently ticking it (not paused).
    pub running: bool,
    /// When it last ticked, if ever.
    pub last_tick: Option<String>,
    /// Sessions currently running for it.
    pub active_sessions: u32,
    /// Short summaries of the most recent job outcomes.
    pub last_results: Vec<String>,
}

/// The live connection state the TUI shows for the daemon.
#[derive(Debug, Clone, Default)]
pub enum AgentsStatus {
    /// No snapshot yet — the first poll is in flight.
    #[default]
    Connecting,
    /// The daemon answered with a snapshot.
    Connected(DaemonSnapshot),
    /// The daemon could not be reached (with the reason).
    Offline(String),
}

/// The inputs for a launchd plist.
pub struct LaunchdConfig {
    /// The launchd label.
    pub label: String,
    /// Absolute path to the `beagle-agentd` binary.
    pub program: PathBuf,
    /// Directory for the daemon's stdout/stderr logs.
    pub log_dir: PathBuf,
}

/// Renders a launchd `LaunchAgent` plist that starts the daemon on login and
/// restarts it on crash.
#[must_use]
pub fn render_plist(config: &LaunchdConfig) -> String {
    let out_log = config.log_dir.join("agentd.out.log");
    let err_log = config.log_dir.join("agentd.err.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{program}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>{out}</string>
    <key>StandardErrorPath</key>
    <string>{err}</string>
</dict>
</plist>
"#,
        label = config.label,
        program = config.program.display(),
        out = out_log.display(),
        err = err_log.display(),
    )
}

/// Installs and enables the daemon as an OS service (launchd on macOS,
/// `systemd --user` on Linux) so it runs on login and restarts on crash.
///
/// # Errors
/// Returns a message on an unsupported platform, if `beagle-agentd` cannot be
/// found, or if writing the unit / running the service manager fails.
pub fn install() -> Result<String, String> {
    dispatch("install", launchd_install, systemd_install)
}

/// Disables and removes the daemon service.
///
/// # Errors
/// Returns a message on an unsupported platform or if removal fails.
pub fn uninstall() -> Result<String, String> {
    dispatch("uninstall", launchd_uninstall, systemd_uninstall)
}

/// (Re)starts the installed service.
///
/// # Errors
/// Returns a message on an unsupported platform, if the service is not
/// installed, or if the service manager fails.
pub fn start() -> Result<String, String> {
    dispatch("start", launchd_start, systemd_start)
}

/// Stops the service.
///
/// # Errors
/// Returns a message on an unsupported platform or if the service manager fails.
pub fn stop() -> Result<String, String> {
    dispatch("stop", launchd_stop, systemd_stop)
}

/// Picks the macOS or Linux backend for an action, or errors on an unsupported
/// platform.
fn dispatch(
    action: &str,
    macos: fn() -> Result<String, String>,
    linux: fn() -> Result<String, String>,
) -> Result<String, String> {
    if cfg!(target_os = "macos") {
        macos()
    } else if cfg!(target_os = "linux") {
        linux()
    } else {
        Err(format!(
            "`beagle agent {action}` is only supported on macOS and Linux"
        ))
    }
}

// ---- macOS (launchd) ----

fn launchd_install() -> Result<String, String> {
    let program = find_agentd()?;
    let log_dir = state_dir()?.join("logs");
    std::fs::create_dir_all(&log_dir).map_err(|err| format!("creating log dir: {err}"))?;
    let config = LaunchdConfig {
        label: LABEL.to_string(),
        program,
        log_dir,
    };
    let path = plist_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("creating LaunchAgents dir: {err}"))?;
    }
    std::fs::write(&path, render_plist(&config))
        .map_err(|err| format!("writing plist {}: {err}", path.display()))?;

    let domain = gui_domain()?;
    // Reload cleanly if a previous copy was loaded.
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
    Ok(format!(
        "installed launchd agent {LABEL}\n  plist: {}\n  starts on login, restarts on crash",
        path.display()
    ))
}

fn launchd_uninstall() -> Result<String, String> {
    let domain = gui_domain()?;
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    let path = plist_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("removing plist {}: {err}", path.display())),
    }
    Ok(format!("uninstalled launchd agent {LABEL}"))
}

fn launchd_start() -> Result<String, String> {
    let path = plist_path()?;
    if !path.is_file() {
        return Err("agent is not installed; run `beagle agent install` first".to_string());
    }
    launchctl(&["bootstrap", &gui_domain()?, &path.to_string_lossy()])?;
    Ok(format!("started {LABEL}"))
}

fn launchd_stop() -> Result<String, String> {
    launchctl(&["bootout", &format!("{}/{LABEL}", gui_domain()?)])?;
    Ok(format!("stopped {LABEL}"))
}

// ---- Linux (systemd --user) ----

/// The systemd user unit name.
const UNIT: &str = "beagle-agentd.service";

fn systemd_install() -> Result<String, String> {
    let program = find_agentd()?;
    let path = unit_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("creating systemd user dir: {err}"))?;
    }
    std::fs::write(&path, render_unit(&program))
        .map_err(|err| format!("writing unit {}: {err}", path.display()))?;
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", UNIT])?;
    Ok(format!(
        "installed systemd --user unit {UNIT}\n  unit: {}\n  starts on login, restarts on crash\n  tip: `loginctl enable-linger $USER` keeps it running without an active login",
        path.display()
    ))
}

fn systemd_uninstall() -> Result<String, String> {
    let _ = systemctl(&["disable", "--now", UNIT]);
    let path = unit_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("removing unit {}: {err}", path.display())),
    }
    let _ = systemctl(&["daemon-reload"]);
    Ok(format!("uninstalled systemd --user unit {UNIT}"))
}

fn systemd_start() -> Result<String, String> {
    let path = unit_path()?;
    if !path.is_file() {
        return Err("agent is not installed; run `beagle agent install` first".to_string());
    }
    systemctl(&["start", UNIT])?;
    Ok(format!("started {UNIT}"))
}

fn systemd_stop() -> Result<String, String> {
    systemctl(&["stop", UNIT])?;
    Ok(format!("stopped {UNIT}"))
}

/// Renders a `systemd --user` service unit that restarts the daemon on crash
/// and starts it on login (`WantedBy=default.target`).
#[must_use]
pub fn render_unit(program: &std::path::Path) -> String {
    format!(
        "[Unit]\n\
         Description=beagle agent daemon (beagle-agentd)\n\
         After=default.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={program}\n\
         Restart=always\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        program = program.display()
    )
}

/// The systemd user-unit path: `$XDG_CONFIG_HOME/systemd/user/<unit>`, else
/// `~/.config/systemd/user/<unit>`.
fn unit_path() -> Result<PathBuf, String> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("systemd/user").join(UNIT));
        }
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config/systemd/user").join(UNIT))
}

/// Runs `systemctl --user args...`, mapping a non-zero exit to an error.
fn systemctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|err| format!("running systemctl: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Queries the running daemon and formats its status for `beagle agent status`.
///
/// # Errors
/// Returns a message if the daemon is not reachable or its response is
/// malformed.
pub fn status() -> Result<String, String> {
    Ok(format_snapshot(&fetch_status()?))
}

/// Connects to the control socket, sends `get-status`, and decodes the reply.
///
/// # Errors
/// Returns a message if the daemon is not reachable or its response cannot be
/// parsed.
pub fn fetch_status() -> Result<DaemonSnapshot, String> {
    parse_status(&send_line(&serde_json::json!({ "type": "get-status" }))?)
}

/// Resumes ticking `id`.
///
/// # Errors
/// Returns a message if the daemon is unreachable or rejects the request.
pub fn start_agent(id: &str) -> Result<(), String> {
    request_ok(&serde_json::json!({ "type": "start-agent", "id": id }))
}

/// Pauses `id`.
///
/// # Errors
/// Returns a message if the daemon is unreachable or rejects the request.
pub fn stop_agent(id: &str) -> Result<(), String> {
    request_ok(&serde_json::json!({ "type": "stop-agent", "id": id }))
}

/// Asks the daemon to re-read its config.
///
/// # Errors
/// Returns a message if the daemon is unreachable or rejects the request.
pub fn reload_config() -> Result<(), String> {
    request_ok(&serde_json::json!({ "type": "reload-config" }))
}

/// Sends one request and requires an `ok` response.
fn request_ok(payload: &serde_json::Value) -> Result<(), String> {
    let line = send_line(payload)?;
    let value: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|err| format!("bad daemon response: {err}"))?;
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("ok") => Ok(()),
        Some("error") => Err(value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("daemon error")
            .to_string()),
        _ => Err("unexpected daemon response".to_string()),
    }
}

/// Connects to the control socket, writes one JSON request line, and returns the
/// single response line.
fn send_line(payload: &serde_json::Value) -> Result<String, String> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).map_err(|err| {
        format!(
            "daemon not reachable at {} ({err}); is it running? try `beagle agent start`",
            path.display()
        )
    })?;
    let mut writer = stream
        .try_clone()
        .map_err(|err| format!("socket error: {err}"))?;
    let mut reader = BufReader::new(stream);
    let mut request = payload.to_string();
    request.push('\n');
    writer
        .write_all(request.as_bytes())
        .map_err(|err| format!("socket write: {err}"))?;
    writer
        .flush()
        .map_err(|err| format!("socket flush: {err}"))?;
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|err| format!("socket read: {err}"))?;
    Ok(line)
}

/// The newest `.log` file for `agent_id` under the daemon's log dir, if any.
#[must_use]
pub fn latest_log(agent_id: &str) -> Option<PathBuf> {
    let dir = state_dir().ok()?.join("logs").join(agent_id);
    newest_log_in(&dir)
}

/// The path to the agents config file (`~/.config/beagle/agents.toml`, XDG-aware),
/// matching what the daemon reads.
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("beagle").join("agents.toml"));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("beagle")
            .join("agents.toml"),
    )
}

/// The most recently modified `.log` in `dir`, or `None` if there are none.
fn newest_log_in(dir: &std::path::Path) -> Option<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension() != Some(std::ffi::OsStr::new("log")) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        let replace = match &newest {
            Some((newest_time, _)) => modified > *newest_time,
            None => true,
        };
        if replace {
            newest = Some((modified, path));
        }
    }
    newest.map(|(_, path)| path)
}

/// Spawns a background thread that polls the daemon every couple of seconds and
/// reports the connection state over `tx`. Exits when the receiver is dropped.
pub fn spawn_status_poller(tx: Sender<AgentsStatus>) {
    thread::spawn(move || loop {
        let status = match fetch_status() {
            Ok(snapshot) => AgentsStatus::Connected(snapshot),
            Err(reason) => AgentsStatus::Offline(reason),
        };
        if tx.send(status).is_err() {
            return; // the UI is gone
        }
        thread::sleep(POLL_INTERVAL);
    });
}

/// Decodes a `get-status` response line into a [`DaemonSnapshot`], turning an
/// `error` response into an `Err`.
fn parse_status(line: &str) -> Result<DaemonSnapshot, String> {
    let value: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|err| format!("bad daemon response: {err}"))?;
    match value.get("type").and_then(serde_json::Value::as_str) {
        Some("status") => {
            let status = value
                .get("status")
                .ok_or("unexpected daemon response (no status)")?;
            serde_json::from_value(status.clone())
                .map_err(|err| format!("bad status payload: {err}"))
        }
        Some("error") => Err(value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("daemon error")
            .to_string()),
        _ => Err("unexpected daemon response".to_string()),
    }
}

/// Formats a snapshot into a human-readable summary for the CLI.
fn format_snapshot(snapshot: &DaemonSnapshot) -> String {
    let mut out = format!("beagle-agentd {}", snapshot.version);
    if snapshot.agents.is_empty() {
        out.push_str("\n  (no agents)");
        return out;
    }
    for agent in &snapshot.agents {
        let state = if agent.running { "running" } else { "paused" };
        let _ = write!(out, "\n  {}: {state}", agent.id);
        if let Some(tick) = &agent.last_tick {
            let _ = write!(out, " (last tick {tick})");
        }
    }
    out
}

/// Resolves the `beagle-agentd` binary: `$BEAGLE_AGENTD`, else a `PATH` search.
fn find_agentd() -> Result<PathBuf, String> {
    if let Some(explicit) = std::env::var_os("BEAGLE_AGENTD") {
        if !explicit.is_empty() {
            return Ok(PathBuf::from(explicit));
        }
    }
    let path = std::env::var_os("PATH").ok_or("PATH is not set")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("beagle-agentd");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err("beagle-agentd not found on PATH; build it (`cargo build -p beagle-agentd`) and put it on PATH, or set BEAGLE_AGENTD".to_string())
}

/// The launchd plist path: `~/Library/LaunchAgents/<label>.plist`.
fn plist_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

/// The launchd GUI domain target for the current user (`gui/<uid>`).
fn gui_domain() -> Result<String, String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|err| format!("running id: {err}"))?;
    if !output.status.success() {
        return Err("could not determine the current uid".to_string());
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("gui/{uid}"))
}

/// Runs `launchctl args...`, mapping a non-zero exit to an error.
fn launchctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .map_err(|err| format!("running launchctl: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "launchctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The daemon's control-socket path (mirrors `beagle-agent`'s default).
fn socket_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("beagle-agentd.sock");
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/state/beagle/agentd.sock");
    }
    PathBuf::from("beagle-agentd.sock")
}

/// The daemon's state directory (mirrors `beagle-agentd`'s default).
fn state_dir() -> Result<PathBuf, String> {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("beagle-agent"));
        }
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/state/beagle-agent"))
}

#[cfg(test)]
#[path = "tests/agentd.rs"]
mod tests;
