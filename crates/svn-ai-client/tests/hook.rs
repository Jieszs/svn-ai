use std::{
    collections::VecDeque,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Mutex,
};

use svn_ai::{ClientConfig, EventStore, FingerprintSecret, HookInput, HookOutcome, HookProcessor};
use svn_ai_svn::{CommandOutput, CommandRunner, SvnClient};
use tempfile::tempdir;

struct FixtureRunner {
    calls: Mutex<VecDeque<(Vec<OsString>, CommandOutput)>>,
}

impl FixtureRunner {
    fn for_working_copy(root: &Path) -> Self {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<info><entry kind="dir" path="." revision="1">
<url>file:///repository/trunk</url>
<repository><root>file:///repository</root><uuid>repo-uuid</uuid></repository>
<wc-info><schedule>normal</schedule><depth>infinity</depth></wc-info>
</entry></info>"#
            .to_owned();
        Self {
            calls: Mutex::new(VecDeque::from([(
                vec![
                    "info".into(),
                    "--xml".into(),
                    "--depth".into(),
                    "empty".into(),
                    root.as_os_str().into(),
                ],
                CommandOutput {
                    success: true,
                    exit_code: Some(0),
                    stdout: xml.into_bytes(),
                    stderr: Vec::new(),
                },
            )])),
        }
    }

    fn empty() -> Self {
        Self {
            calls: Mutex::new(VecDeque::new()),
        }
    }

    fn assert_exhausted(&self) {
        assert!(self.calls.lock().unwrap().is_empty(), "unused svn calls");
    }
}

impl CommandRunner for FixtureRunner {
    fn run(&self, executable: &Path, args: &[OsString]) -> std::io::Result<CommandOutput> {
        assert_eq!(executable, Path::new("fixture-svn"));
        let (expected_args, output) = self
            .calls
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected svn call");
        assert_eq!(args, expected_args);
        Ok(output)
    }
}

fn config() -> ClientConfig {
    ClientConfig {
        device_id: "device-1".to_owned(),
        svn_username: "zhengjie".to_owned(),
        fingerprint_secret: FingerprintSecret::parse(
            "0707070707070707070707070707070707070707070707070707070707070707",
        )
        .unwrap(),
        svn_executable: PathBuf::from("fixture-svn"),
    }
}

fn hook_json(event: &str, tool: &str, cwd: &Path, file: Option<&Path>) -> HookInput {
    let mut tool_input = serde_json::json!({});
    if let Some(file) = file {
        tool_input["file_path"] = serde_json::Value::String(file.display().to_string());
        if tool == "Edit" {
            tool_input["old_string"] = serde_json::Value::String("old".to_owned());
            tool_input["new_string"] = serde_json::Value::String("new".to_owned());
        } else if tool == "Write" {
            tool_input["content"] = serde_json::Value::String("not persisted".to_owned());
        }
    }
    serde_json::from_value(serde_json::json!({
        "session_id": "session-abc",
        "transcript_path": "/private/transcript.jsonl",
        "cwd": cwd,
        "permission_mode": "default",
        "hook_event_name": event,
        "tool_name": tool,
        "tool_input": tool_input,
        "tool_response": {"ok": true},
        "tool_use_id": "toolu_01ABC123",
        "duration_ms": 15
    }))
    .unwrap()
}

#[test]
fn pre_and_post_capture_ten_ai_lines_without_persisting_source_text() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let root = temp.path().join("wc");
    let file = root.join("code.rs");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    std::fs::write(&file, "legacy\n").unwrap();
    let store = EventStore::open(&home).unwrap();
    let svn = SvnClient::with_runner(
        PathBuf::from("fixture-svn"),
        FixtureRunner::for_working_copy(&root),
    );
    let config = config();
    let processor = HookProcessor::new(&config, &store, &svn);

    let pre = hook_json("PreToolUse", "Edit", &root, Some(&file));
    assert_eq!(processor.process(pre).unwrap(), HookOutcome::Captured);
    assert_eq!(store.status().unwrap().pending_transactions, 1);

    let mut content = String::from("legacy\n");
    for number in 1..=10 {
        content.push_str(&format!("ai-line-{number:02}\n"));
    }
    std::fs::write(&file, &content).unwrap();
    let post = hook_json("PostToolUse", "Edit", &root, Some(&file));
    assert_eq!(processor.process(post).unwrap(), HookOutcome::Finalized);

    let events = store.list_events().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].device_id, "device-1");
    assert_eq!(events[0].svn_username, "zhengjie");
    assert_eq!(events[0].repository_uuid, "repo-uuid");
    assert_eq!(events[0].base_revision, 1);
    assert_eq!(events[0].path.as_str(), "trunk/code.rs");
    assert_eq!(events[0].hunks[0].new_line_digests.len(), 10);
    assert_eq!(store.status().unwrap().pending_transactions, 0);
    let json = serde_json::to_string(&events[0]).unwrap();
    assert!(!json.contains("legacy"));
    assert!(!json.contains("ai-line"));
    assert!(!json.contains(&file.display().to_string()));
    assert!(!json.contains("transcript"));
    svn.runner().assert_exhausted();
}

#[test]
fn write_captures_a_file_that_did_not_exist_before_the_tool() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("wc");
    let file = root.join("generated.rs");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    let store = EventStore::open(&temp.path().join("home")).unwrap();
    let svn = SvnClient::with_runner(
        PathBuf::from("fixture-svn"),
        FixtureRunner::for_working_copy(&root),
    );
    let config = config();
    let processor = HookProcessor::new(&config, &store, &svn);

    assert_eq!(
        processor
            .process(hook_json("PreToolUse", "Write", &root, Some(&file)))
            .unwrap(),
        HookOutcome::Captured
    );
    std::fs::write(&file, "generated\n").unwrap();
    assert_eq!(
        processor
            .process(hook_json("PostToolUse", "Write", &root, Some(&file)))
            .unwrap(),
        HookOutcome::Finalized
    );

    let events = store.list_events().unwrap();
    assert_eq!(events[0].path.as_str(), "trunk/generated.rs");
    assert_eq!(events[0].hunks[0].new_line_digests.len(), 1);
}

#[test]
fn failed_tool_cancels_and_deletes_the_before_snapshot() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("wc");
    let file = root.join("code.rs");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    std::fs::write(&file, "secret before\n").unwrap();
    let store = EventStore::open(&temp.path().join("home")).unwrap();
    let svn = SvnClient::with_runner(
        PathBuf::from("fixture-svn"),
        FixtureRunner::for_working_copy(&root),
    );
    let config = config();
    let processor = HookProcessor::new(&config, &store, &svn);
    processor
        .process(hook_json("PreToolUse", "Edit", &root, Some(&file)))
        .unwrap();

    let failure = hook_json("PostToolUseFailure", "Edit", &root, Some(&file));
    assert_eq!(processor.process(failure).unwrap(), HookOutcome::Cancelled);

    assert_eq!(store.status().unwrap().pending_transactions, 0);
    assert!(store.list_events().unwrap().is_empty());
}

#[test]
fn unsupported_tools_are_ignored_without_running_svn() {
    let temp = tempdir().unwrap();
    let store = EventStore::open(&temp.path().join("home")).unwrap();
    let svn = SvnClient::with_runner(PathBuf::from("fixture-svn"), FixtureRunner::empty());
    let config = config();
    let processor = HookProcessor::new(&config, &store, &svn);

    let outcome = processor
        .process(hook_json("PreToolUse", "Read", temp.path(), None))
        .unwrap();

    assert_eq!(outcome, HookOutcome::Ignored);
    svn.runner().assert_exhausted();
}

#[test]
fn files_outside_svn_are_ignored() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("code.rs");
    std::fs::write(&file, "code\n").unwrap();
    let store = EventStore::open(&temp.path().join("home")).unwrap();
    let svn = SvnClient::with_runner(PathBuf::from("fixture-svn"), FixtureRunner::empty());
    let config = config();
    let processor = HookProcessor::new(&config, &store, &svn);

    let outcome = processor
        .process(hook_json("PreToolUse", "Edit", temp.path(), Some(&file)))
        .unwrap();

    assert_eq!(outcome, HookOutcome::Ignored);
    assert_eq!(store.status().unwrap().pending_transactions, 0);
    svn.runner().assert_exhausted();
}
