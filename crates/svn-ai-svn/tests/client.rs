use std::{
    collections::VecDeque,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Mutex,
};

use svn_ai_svn::{CommandOutput, CommandRunner, SvnClient, SvnClientError};
use tempfile::tempdir;

struct ExpectedCall {
    args: Vec<OsString>,
    output: CommandOutput,
}

struct FixtureRunner {
    calls: Mutex<VecDeque<ExpectedCall>>,
}

impl FixtureRunner {
    fn new(calls: Vec<(Vec<OsString>, CommandOutput)>) -> Self {
        Self {
            calls: Mutex::new(
                calls
                    .into_iter()
                    .map(|(args, output)| ExpectedCall { args, output })
                    .collect(),
            ),
        }
    }

    fn assert_exhausted(&self) {
        assert!(
            self.calls.lock().unwrap().is_empty(),
            "unused fixture calls"
        );
    }
}

impl CommandRunner for FixtureRunner {
    fn run(&self, executable: &Path, args: &[OsString]) -> std::io::Result<CommandOutput> {
        assert_eq!(executable, Path::new("fixture-svn"));
        let expected = self
            .calls
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected svn call");
        assert_eq!(args, expected.args);
        Ok(expected.output)
    }
}

fn success(stdout: &str) -> CommandOutput {
    CommandOutput {
        success: true,
        exit_code: Some(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn svn17_info_xml(url: &str, root: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<info>
<entry kind="dir" path="." revision="7">
<url>{url}</url>
<repository>
<root>{root}</root>
<uuid>5e7d134a-54fb-0310-bd04-b611643e5c25</uuid>
</repository>
<wc-info><schedule>normal</schedule><depth>infinity</depth></wc-info>
<commit revision="7"><author>sally</author><date>2026-09-14T01:00:00Z</date></commit>
</entry>
</info>"#
    )
}

fn fixture_client(root: &Path, xml: &str) -> SvnClient<FixtureRunner> {
    let args = vec![
        "info".into(),
        "--xml".into(),
        "--depth".into(),
        "empty".into(),
        root.as_os_str().into(),
    ];
    SvnClient::with_runner(
        PathBuf::from("fixture-svn"),
        FixtureRunner::new(vec![(args, success(xml))]),
    )
}

#[test]
fn discovers_nested_file_and_builds_repository_relative_path() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("working copy");
    let target = root.join("src").join("中文.rs");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, "fn main() {}\n").unwrap();
    let client = fixture_client(
        &root,
        &svn17_info_xml(
            "https://svn.example.test/repos/demo/trunk%20space",
            "https://svn.example.test/repos/demo",
        ),
    );

    let info = client.discover(&target).unwrap();

    assert_eq!(info.root_path, root);
    assert_eq!(info.repository_uuid, "5e7d134a-54fb-0310-bd04-b611643e5c25");
    assert_eq!(
        info.repository_root_url,
        "https://svn.example.test/repos/demo"
    );
    assert_eq!(info.repository_path_prefix, "trunk space");
    assert_eq!(info.base_revision, 7);
    assert_eq!(
        info.repo_path(&target).unwrap().as_str(),
        "trunk space/src/中文.rs"
    );
    client.runner().assert_exhausted();
}

#[test]
fn discovers_a_file_that_write_has_not_created_yet() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("wc");
    let target = root.join("new").join("generated.rs");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    let client = fixture_client(
        &root,
        &svn17_info_xml(
            "https://svn.example.test/repos/demo/trunk",
            "https://svn.example.test/repos/demo",
        ),
    );

    let info = client.discover(&target).unwrap();

    assert_eq!(
        info.repo_path(&target).unwrap().as_str(),
        "trunk/new/generated.rs"
    );
    client.runner().assert_exhausted();
}

#[test]
fn decodes_percent_encoded_utf8_repository_prefix() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("wc");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    let client = fixture_client(
        &root,
        &svn17_info_xml(
            "https://svn.example.test/repos/demo/%E4%B8%AD%E6%96%87%20trunk",
            "https://svn.example.test/repos/demo",
        ),
    );

    let info = client.discover(&root.join("code.rs")).unwrap();

    assert_eq!(info.repository_path_prefix, "中文 trunk");
}

#[test]
fn rejects_targets_outside_a_working_copy_without_running_svn() {
    let temp = tempdir().unwrap();
    let runner = FixtureRunner::new(Vec::new());
    let client = SvnClient::with_runner(PathBuf::from("fixture-svn"), runner);

    let error = client.discover(&temp.path().join("code.rs")).unwrap_err();

    assert!(matches!(error, SvnClientError::NotWorkingCopy { .. }));
    client.runner().assert_exhausted();
}

#[test]
fn reports_svn_info_command_failures() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("wc");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    let args = vec![
        "info".into(),
        "--xml".into(),
        "--depth".into(),
        "empty".into(),
        root.as_os_str().into(),
    ];
    let client = SvnClient::with_runner(
        PathBuf::from("fixture-svn"),
        FixtureRunner::new(vec![(
            args,
            CommandOutput {
                success: false,
                exit_code: Some(1),
                stdout: Vec::new(),
                stderr: b"svn: E155007: is not a working copy".to_vec(),
            },
        )]),
    );

    let error = client.discover(&root.join("code.rs")).unwrap_err();

    assert!(matches!(
        error,
        SvnClientError::CommandFailed {
            exit_code: Some(1),
            ..
        }
    ));
    assert!(error.to_string().contains("E155007"));
}

#[test]
fn rejects_malformed_svn_info_xml() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("wc");
    std::fs::create_dir_all(root.join(".svn")).unwrap();
    let client = fixture_client(&root, "<info><entry>");

    let error = client.discover(&root.join("code.rs")).unwrap_err();

    assert!(matches!(error, SvnClientError::InvalidXml(_)));
}
