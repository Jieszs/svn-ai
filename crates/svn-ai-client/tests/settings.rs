use std::path::Path;

use svn_ai::{InstallHooksResult, install_claude_hooks};
use tempfile::tempdir;

#[test]
fn installation_preserves_unrelated_settings_and_existing_hooks() {
    let temp = tempdir().unwrap();
    let settings = temp.path().join(".claude").join("settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let original = r#"{
  "env": {"ANTHROPIC_BASE_URL": "http://offline-model"},
  "includeCoAuthoredBy": false,
  "hooks": {
    "Stop": [{"hooks": [{"type": "command", "command": "existing-stop"}]}]
  }
}"#;
    std::fs::write(&settings, original).unwrap();

    let result = install_claude_hooks(
        &settings,
        Path::new("C:/Program Files/SVN AI/svn-ai.exe"),
        Path::new("C:/Users/Test User/AppData/Local/svn-ai"),
    )
    .unwrap();

    assert_eq!(
        result,
        InstallHooksResult {
            changed: true,
            installed_hooks: 3,
        }
    );
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
    assert_eq!(value["env"]["ANTHROPIC_BASE_URL"], "http://offline-model");
    assert_eq!(value["includeCoAuthoredBy"], false);
    assert_eq!(
        value["hooks"]["Stop"][0]["hooks"][0]["command"],
        "existing-stop"
    );
    for event in ["PreToolUse", "PostToolUse", "PostToolUseFailure"] {
        assert_eq!(value["hooks"][event].as_array().unwrap().len(), 1);
        assert_eq!(value["hooks"][event][0]["matcher"], "Edit|Write");
        let command = value["hooks"][event][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(command.contains("Program Files/SVN AI/svn-ai.exe"));
        assert!(command.contains("Test User/AppData/Local/svn-ai"));
        assert!(command.ends_with(" hook"));
    }
    let backup = settings.with_file_name("settings.json.svn-ai.bak");
    assert_eq!(std::fs::read_to_string(backup).unwrap(), original);
}

#[test]
fn repeated_installation_does_not_add_duplicate_hooks() {
    let temp = tempdir().unwrap();
    let settings = temp.path().join("settings.json");
    let executable = Path::new("C:/svn-ai/svn-ai.exe");
    let home = Path::new("C:/svn-ai/home");
    install_claude_hooks(&settings, executable, home).unwrap();

    let second = install_claude_hooks(&settings, executable, home).unwrap();

    assert_eq!(second.installed_hooks, 0);
    assert!(!second.changed);
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(settings).unwrap()).unwrap();
    for event in ["PreToolUse", "PostToolUse", "PostToolUseFailure"] {
        assert_eq!(value["hooks"][event].as_array().unwrap().len(), 1);
    }
}

#[test]
fn creates_a_new_settings_file_when_none_exists() {
    let temp = tempdir().unwrap();
    let settings = temp.path().join("nested").join("settings.json");

    let result = install_claude_hooks(
        &settings,
        Path::new("/opt/svn ai/bin/svn-ai"),
        Path::new("/var/lib/svn ai"),
    )
    .unwrap();

    assert!(result.changed);
    assert_eq!(result.installed_hooks, 3);
    assert!(settings.is_file());
    assert!(!settings.with_file_name("settings.json.svn-ai.bak").exists());
}
