//! `beagle-agentd` — the always-on daemon that hosts beagle's agent engine.
//!
//! P0 scaffold: it prints its version and exits. The poll loop, control
//! socket, and RCA→PR orchestration arrive in later issues under epic #137.

/// Prints the daemon's version and exits. This is a placeholder entry point so
/// the workspace builds end to end; real behavior lands in follow-up issues.
fn main() {
    println!("beagle-agentd {}", env!("CARGO_PKG_VERSION"));
}
