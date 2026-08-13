//! `beagle-agent` — the engine for beagle's native agent runtime.
//!
//! This crate holds the pieces that the [`beagle-agentd`] daemon assembles into
//! an always-on loop: agent configuration, durable job state, the RCA trigger
//! poller, git-worktree isolation, the headless `claude` runner, and the
//! control-socket protocol shared with the TUI and desktop clients.
//!
//! It is the Rust-native successor to the Go `ai-pipelines/beagle` feature
//! agent (tracked by epic #137). Subsystems land module by module; see the
//! epic's child issues.
//!
//! [`beagle-agentd`]: https://github.com/matthewmyrick/beagle

pub mod config;
pub mod control;
pub mod orchestrator;
pub mod poller;
pub mod protocol;
pub mod runner;
pub mod store;
pub mod worktree;
