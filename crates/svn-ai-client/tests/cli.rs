use std::{path::Path, process::Command};

use tempfile::tempdir;

fn svn_ai() -> Command {
    Command::new(env!("CARGO_BIN_EXE_svn-ai"))
}

fn run(command: &mut Command) -> std::process::Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn configure_status_and_events_use_the_selected_home() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("client home");
    run(svn_ai().args([
        "--home",
        home.to_str().unwrap(),
        "configure",
        "--device-id",
        "device-1",
        "--svn-username",
        "zhengjie",
        "--fingerprint-key",
        "0707070707070707070707070707070707070707070707070707070707070707",
        "--svn",
        "C:/Program Files/Subversion/bin/svn.exe",
    ]));

    let status = run(svn_ai().args(["--home", home.to_str().unwrap(), "status", "--json"]));
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["configured"], true);
    assert_eq!(status["device_id"], "device-1");
    assert_eq!(status["svn_username"], "zhengjie");
    assert_eq!(status["pending_transactions"], 0);
    assert_eq!(status["attribution_events"], 0);

    let events = run(svn_ai().args(["--home", home.to_str().unwrap(), "events", "--json"]));
    let events: serde_json::Value = serde_json::from_slice(&events.stdout).unwrap();
    assert_eq!(events, serde_json::json!([]));
}

#[test]
fn install_hooks_command_updates_only_the_selected_settings_file() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let settings = temp
        .path()
        .join("project")
        .join(".claude")
        .join("settings.json");
    let executable = Path::new(env!("CARGO_BIN_EXE_svn-ai"));

    let output = run(svn_ai().args([
        "--home",
        home.to_str().unwrap(),
        "install-hooks",
        "--settings",
        settings.to_str().unwrap(),
        "--executable",
        executable.to_str().unwrap(),
    ]));

    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["installed_hooks"], 3);
    assert!(settings.is_file());
}

#[test]
fn malformed_hook_input_never_blocks_claude_and_writes_a_diagnostic() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let mut command = svn_ai();
    command
        .args(["--home", home.to_str().unwrap(), "hook"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(b"{not-json").unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let diagnostics = std::fs::read_to_string(home.join("diagnostics.log")).unwrap();
    assert!(diagnostics.contains("invalid hook JSON"));
    assert!(!diagnostics.contains("{not-json"));
}
