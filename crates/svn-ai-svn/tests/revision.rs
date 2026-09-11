use std::{
    collections::VecDeque,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Mutex,
};

use svn_ai_core::FingerprintKey;
use svn_ai_protocol::ChangeKind;
use svn_ai_svn::{CommandOutput, CommandRunner, SvnLook, SvnLookError};

struct ExpectedCall {
    args: Vec<OsString>,
    output: CommandOutput,
}

struct FixtureRunner {
    calls: Mutex<VecDeque<ExpectedCall>>,
}

impl FixtureRunner {
    fn new(calls: Vec<(&[&str], CommandOutput)>) -> Self {
        let calls = calls
            .into_iter()
            .map(|(args, output)| ExpectedCall {
                args: args.iter().map(OsString::from).collect(),
                output,
            })
            .collect();
        Self {
            calls: Mutex::new(calls),
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
        assert_eq!(executable, Path::new("fixture-svnlook"));
        let expected = self
            .calls
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected svnlook call");
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

#[test]
fn reads_add_modify_delete_and_utf8_paths() {
    let repository = "C:/svn/repositories/demo";
    let runner = FixtureRunner::new(vec![
        (&["uuid", repository], success("repo-uuid\n")),
        (&["author", "-r", "2", repository], success("zhengjie\n")),
        (
            &["date", "-r", "2", repository],
            success("2026-09-11 08:10:00 +0800 (Fri, 11 Sep 2026)\n"),
        ),
        (
            &["changed", "--copy-info", "-r", "2", repository],
            success(concat!(
                "A   trunk/新增.rs\n",
                "U   trunk/existing.rs\n",
                "D   trunk/deleted.rs\n",
                "_U  trunk/properties-only.rs\n",
            )),
        ),
        (
            &["cat", "-r", "2", repository, "trunk/新增.rs"],
            success("new one\nnew two\n"),
        ),
        (
            &["cat", "-r", "1", repository, "trunk/existing.rs"],
            success("old\n"),
        ),
        (
            &["cat", "-r", "2", repository, "trunk/existing.rs"],
            success("changed\n"),
        ),
        (
            &["cat", "-r", "1", repository, "trunk/deleted.rs"],
            success("deleted\n"),
        ),
    ]);
    let svnlook = SvnLook::with_runner(PathBuf::from("fixture-svnlook"), runner);

    let event = svnlook
        .read_revision(Path::new(repository), 2, &FingerprintKey::new([7; 32]))
        .expect("fixture revision");

    assert_eq!(event.repository_uuid, "repo-uuid");
    assert_eq!(event.revision, 2);
    assert_eq!(event.author, "zhengjie");
    assert_eq!(
        event.committed_at,
        "2026-09-11 08:10:00 +0800 (Fri, 11 Sep 2026)"
    );
    assert_eq!(event.changes.len(), 4);

    assert_eq!(event.changes[0].path.as_str(), "trunk/新增.rs");
    assert_eq!(event.changes[0].kind, ChangeKind::Add);
    assert_eq!(event.changes[0].hunks[0].new_line_digests.len(), 2);

    assert_eq!(event.changes[1].kind, ChangeKind::Modify);
    assert_eq!(event.changes[1].hunks[0].old_len, 1);
    assert_eq!(event.changes[1].hunks[0].new_line_digests.len(), 1);

    assert_eq!(event.changes[2].kind, ChangeKind::Delete);
    assert_eq!(event.changes[2].hunks[0].old_len, 1);
    assert!(event.changes[2].hunks[0].new_line_digests.is_empty());

    assert_eq!(event.changes[3].kind, ChangeKind::Modify);
    assert!(event.changes[3].hunks.is_empty());
    svnlook.runner().assert_exhausted();
}

#[test]
fn reports_non_zero_svnlook_exit_with_stderr() {
    let runner = FixtureRunner::new(vec![(
        &["uuid", "C:/missing"],
        CommandOutput {
            success: false,
            exit_code: Some(1),
            stdout: Vec::new(),
            stderr: b"svnlook: E000002: repository not found".to_vec(),
        },
    )]);
    let svnlook = SvnLook::with_runner(PathBuf::from("fixture-svnlook"), runner);

    let error = svnlook
        .read_revision(Path::new("C:/missing"), 2, &FingerprintKey::new([7; 32]))
        .expect_err("command should fail");

    assert!(matches!(
        error,
        SvnLookError::CommandFailed {
            exit_code: Some(1),
            ..
        }
    ));
    assert!(error.to_string().contains("repository not found"));
}

#[test]
fn reads_copy_source_and_only_reports_edits_after_the_copy() {
    let repository = "C:/svn/repositories/demo";
    let runner = FixtureRunner::new(vec![
        (&["uuid", repository], success("repo-uuid\n")),
        (&["author", "-r", "2", repository], success("zhengjie\n")),
        (
            &["date", "-r", "2", repository],
            success("2026-09-11 08:10:00 +0800\n"),
        ),
        (
            &["changed", "--copy-info", "-r", "2", repository],
            success(concat!(
                "A + branches/demo/code.rs\n",
                "    (from trunk/code.rs:r1)\n",
            )),
        ),
        (
            &["cat", "-r", "1", repository, "trunk/code.rs"],
            success("same line\n"),
        ),
        (
            &["cat", "-r", "2", repository, "branches/demo/code.rs"],
            success("same line\n"),
        ),
    ]);
    let svnlook = SvnLook::with_runner(PathBuf::from("fixture-svnlook"), runner);

    let event = svnlook
        .read_revision(Path::new(repository), 2, &FingerprintKey::new([7; 32]))
        .expect("copy revision");

    assert_eq!(event.changes.len(), 1);
    assert_eq!(event.changes[0].kind, ChangeKind::Copy);
    assert!(event.changes[0].hunks.is_empty());
    let source = event.changes[0].copy_from.as_ref().expect("copy source");
    assert_eq!(source.path.as_str(), "trunk/code.rs");
    assert_eq!(source.revision, 1);
    svnlook.runner().assert_exhausted();
}

#[test]
fn rejects_revision_zero_before_running_commands() {
    let runner = FixtureRunner::new(Vec::new());
    let svnlook = SvnLook::with_runner(PathBuf::from("fixture-svnlook"), runner);

    let error = svnlook
        .read_revision(Path::new("C:/repo"), 0, &FingerprintKey::new([7; 32]))
        .expect_err("revision zero has no commit author");

    assert_eq!(error, SvnLookError::InvalidRevision(0));
    svnlook.runner().assert_exhausted();
}
