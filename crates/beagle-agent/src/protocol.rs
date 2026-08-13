//! The control protocol shared by the daemon and its clients (the TUI and
//! desktop app). Messages are **newline-delimited JSON**: one JSON object per
//! line over a loopback unix socket. The types live here so every client links
//! the exact same contract.

use serde::{Deserialize, Serialize};

/// A request from a client to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Request {
    /// Ask for a one-shot daemon status snapshot.
    GetStatus,
    /// List the configured agents and their status.
    ListAgents,
    /// Resume ticking the given agent.
    StartAgent {
        /// The agent id.
        id: String,
    },
    /// Pause ticking the given agent.
    StopAgent {
        /// The agent id.
        id: String,
    },
    /// Re-read the agent config file.
    ReloadConfig,
    /// Open a live stream: the daemon pushes [`Response::Update`] snapshots
    /// until the client disconnects.
    Subscribe,
}

/// A response from the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Response {
    /// A one-shot status snapshot.
    Status {
        /// The snapshot.
        status: DaemonStatus,
    },
    /// The configured agents.
    Agents {
        /// One entry per agent.
        agents: Vec<AgentStatus>,
    },
    /// The request succeeded and has no payload.
    Ok,
    /// The request failed (e.g. malformed, or an unknown agent id).
    Error {
        /// A human-readable reason.
        message: String,
    },
    /// A pushed status update (only on a subscribe stream).
    Update {
        /// The latest snapshot.
        status: DaemonStatus,
    },
}

/// A snapshot of the whole daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// The daemon version.
    pub version: String,
    /// One entry per configured agent.
    pub agents: Vec<AgentStatus>,
}

/// One agent's current status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStatus {
    /// The agent id.
    pub id: String,
    /// Whether the config marks the agent enabled.
    pub enabled: bool,
    /// Whether the daemon is currently ticking it (not paused via `stop`).
    pub running: bool,
    /// When the agent last ticked (RFC-3339-ish text), if ever.
    pub last_tick: Option<String>,
    /// The number of sessions currently running for this agent.
    pub active_sessions: u32,
    /// Short summaries of the most recent job outcomes.
    pub last_results: Vec<String>,
}

/// Serializes `value` as a single JSON line terminated with `\n`.
///
/// # Errors
/// Returns a [`serde_json::Error`] if the value cannot be serialized.
pub fn encode_line<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    Ok(line)
}

/// Parses one newline-delimited JSON message. Trailing whitespace (the
/// delimiter) is ignored.
///
/// # Errors
/// Returns a [`serde_json::Error`] if the line is not valid JSON for `T`.
pub fn decode_line<T: for<'de> Deserialize<'de>>(line: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line.trim_end())
}

#[cfg(test)]
#[path = "tests/protocol.rs"]
mod tests;
