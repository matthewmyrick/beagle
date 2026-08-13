//! Tests for `agentd`: the pure plist rendering and status formatting.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::path::PathBuf;

use super::*;

#[test]
fn plist_has_label_program_and_keepalive() {
    let plist = render_plist(&LaunchdConfig {
        label: "com.beagle.agentd".to_string(),
        program: PathBuf::from("/usr/local/bin/beagle-agentd"),
        log_dir: PathBuf::from("/home/me/.local/state/beagle-agent/logs"),
    });

    assert!(plist.contains("<string>com.beagle.agentd</string>"));
    assert!(plist.contains("<string>/usr/local/bin/beagle-agentd</string>"));
    // Autostart on login and restart on crash.
    assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));
    assert!(plist.contains("<key>KeepAlive</key>\n    <true/>"));
    // Logs point under the state dir.
    assert!(plist.contains("beagle-agent/logs/agentd.out.log"));
    assert!(plist.contains("beagle-agent/logs/agentd.err.log"));
    // Well-formed enough to be a plist.
    assert!(plist.starts_with("<?xml"));
    assert!(plist.trim_end().ends_with("</plist>"));
}

#[test]
fn unit_has_execstart_and_restart() {
    let unit = render_unit(&PathBuf::from("/usr/local/bin/beagle-agentd"));
    assert!(unit.contains("ExecStart=/usr/local/bin/beagle-agentd"));
    assert!(unit.contains("Restart=always"));
    // Starts on login.
    assert!(unit.contains("WantedBy=default.target"));
    assert!(unit.starts_with("[Unit]"));
    assert!(unit.contains("[Service]"));
    assert!(unit.contains("[Install]"));
}

#[test]
fn format_status_summarizes_agents() {
    let line = r#"{"type":"status","status":{"version":"0.1.0","agents":[
        {"id":"rca","enabled":true,"running":true,"last_tick":"1723550400","active_sessions":0,"last_results":[]},
        {"id":"other","enabled":true,"running":false,"last_tick":null,"active_sessions":0,"last_results":[]}
    ]}}"#;
    let out = format_status(line).expect("format");
    assert!(out.contains("beagle-agentd 0.1.0"));
    assert!(out.contains("rca: running (last tick 1723550400)"), "{out}");
    assert!(out.contains("other: paused"), "{out}");
}

#[test]
fn format_status_handles_no_agents() {
    let line = r#"{"type":"status","status":{"version":"0.1.0","agents":[]}}"#;
    let out = format_status(line).expect("format");
    assert!(out.contains("(no agents)"), "{out}");
}

#[test]
fn format_status_rejects_garbage() {
    assert!(format_status("not json").is_err());
    assert!(format_status(r#"{"type":"ok"}"#).is_err());
}
