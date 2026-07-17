//! Tests for desktop-notification construction (`notify`).
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::command_for;

#[test]
fn notifier_invocation_matches_the_platform() {
    let (program, args) = command_for("beagle — new incident", "Payments p99 regression", None)
        .expect("macOS and Linux both have a notifier");
    if cfg!(target_os = "macos") {
        assert_eq!(program, "osascript");
        assert!(args[1].contains("display notification"));
        assert!(args[1].contains("Payments p99 regression"));
        assert!(args[1].contains("beagle — new incident"));
    } else {
        assert_eq!(program, "notify-send");
        assert_eq!(args, ["beagle — new incident", "Payments p99 regression"]);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_passes_the_icon_when_available() {
    let (program, args) = command_for("t", "b", Some("/cache/beagle/icon.png")).expect("notifier");
    assert_eq!(program, "notify-send");
    assert_eq!(args, ["-i", "/cache/beagle/icon.png", "t", "b"]);
}

#[cfg(target_os = "macos")]
#[test]
fn applescript_quoting_survives_hostile_titles() {
    let (_, args) = command_for(r#"a "quoted" title"#, r"back\slash", None).expect("notifier");
    let script = &args[1];
    assert!(script.contains(r#"\"quoted\""#), "quotes escaped: {script}");
    assert!(
        script.contains(r"back\\slash"),
        "backslashes escaped: {script}"
    );
}
