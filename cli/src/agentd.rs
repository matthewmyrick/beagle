//! Managing the `beagle-agentd` daemon from the CLI: install/uninstall it as an
//! OS service, start/stop it, and query its status.
//!
//! On macOS the service is a launchd `LaunchAgent` (`KeepAlive` +
//! `RunAtLoad`), so it starts on login and restarts on crash. Linux (systemd)
//! support lands in a follow-up. Status talks to a running daemon directly over
//! its loopback control socket (newline-JSON), so the CLI needs no dependency
//! on the daemon crate — it just speaks the wire format.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;

/// The launchd label / service name.
const LABEL: &str = "com.beagle.agentd";

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

/// Installs and loads the launchd agent so the daemon runs on login.
///
/// # Errors
/// Returns a message if not on macOS, if `beagle-agentd` cannot be found, or if
/// writing the plist or `launchctl` fails.
pub fn install() -> Result<String, String> {
    require_macos("install")?;
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

/// Unloads and removes the launchd agent.
///
/// # Errors
/// Returns a message if not on macOS or if removing the plist fails.
pub fn uninstall() -> Result<String, String> {
    require_macos("uninstall")?;
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

/// (Re)loads the agent so it runs now.
///
/// # Errors
/// Returns a message if not on macOS, the agent is not installed, or
/// `launchctl` fails.
pub fn start() -> Result<String, String> {
    require_macos("start")?;
    let path = plist_path()?;
    if !path.is_file() {
        return Err("agent is not installed; run `beagle agent install` first".to_string());
    }
    launchctl(&["bootstrap", &gui_domain()?, &path.to_string_lossy()])?;
    Ok(format!("started {LABEL}"))
}

/// Unloads the agent so it stops (and stays stopped until `start`/`install`).
///
/// # Errors
/// Returns a message if not on macOS or `launchctl` fails.
pub fn stop() -> Result<String, String> {
    require_macos("stop")?;
    launchctl(&["bootout", &format!("{}/{LABEL}", gui_domain()?)])?;
    Ok(format!("stopped {LABEL}"))
}

/// Queries the running daemon over its control socket and formats the status.
///
/// # Errors
/// Returns a message if the daemon is not reachable or its response is
/// malformed.
pub fn status() -> Result<String, String> {
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
    writer
        .write_all(b"{\"type\":\"get-status\"}\n")
        .map_err(|err| format!("socket write: {err}"))?;
    writer
        .flush()
        .map_err(|err| format!("socket flush: {err}"))?;
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|err| format!("socket read: {err}"))?;
    format_status(&line)
}

/// Formats a `get-status` JSON response into a human-readable summary.
fn format_status(line: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|err| format!("bad daemon response: {err}"))?;
    let status = value
        .get("status")
        .ok_or("unexpected daemon response (no status)")?;
    let version = status
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let mut out = format!("beagle-agentd {version}");
    match status.get("agents").and_then(|a| a.as_array()) {
        Some(agents) if !agents.is_empty() => {
            for agent in agents {
                let id = agent.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let running = agent
                    .get("running")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let state = if running { "running" } else { "paused" };
                let _ = write!(out, "\n  {id}: {state}");
                if let Some(tick) = agent.get("last_tick").and_then(|v| v.as_str()) {
                    let _ = write!(out, " (last tick {tick})");
                }
            }
        }
        _ => out.push_str("\n  (no agents)"),
    }
    Ok(out)
}

/// Errors unless running on macOS.
fn require_macos(action: &str) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(format!(
            "`beagle agent {action}` currently supports macOS only (Linux/systemd support is tracked separately)"
        ))
    }
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
