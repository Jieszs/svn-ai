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
    success_bytes(stdout.as_bytes())
}

fn success_bytes(stdout: &[u8]) -> CommandOutput {
    CommandOutput {
        success: true,
        exit_code: Some(0),
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

#[cfg(windows)]
#[test]
fn decodes_windows_console_encoded_metadata_and_paths() {
    use windows_sys::Win32::{Globalization::GetACP, System::Console::GetConsoleOutputCP};

    let console_code_page = unsafe { GetConsoleOutputCP() };
    let ansi_code_page = unsafe { GetACP() };
    if console_code_page != 936 && ansi_code_page != 936 {
        return;
    }

    let repository = "D:/svn/repository";
    let runner = FixtureRunner::new(vec![
        (&["uuid", repository], success("repo-uuid\n")),
        (
            &["author", "-r", "2", repository],
            success_bytes(&[0xD5, 0xC5, 0xC8, 0xFD, 0x0A]),
        ),
        (
            &["date", "-r", "2", repository],
            success_bytes(&[
                0x32, 0x30, 0x32, 0x36, 0x2D, 0x30, 0x39, 0x2D, 0x31, 0x35, 0x20, 0x30, 0x39, 0x3A,
                0x33, 0x33, 0x3A, 0x32, 0x31, 0x20, 0x2B, 0x30, 0x38, 0x30, 0x30, 0x20, 0x28, 0xD6,
                0xDC, 0xB6, 0xFE, 0x2C, 0x20, 0x31, 0x35, 0x20, 0x39, 0xD4, 0xC2, 0x20, 0x32, 0x30,
                0x32, 0x36, 0x29, 0x0A,
            ]),
        ),
        (
            &["changed", "--copy-info", "-r", "2", repository],
            success_bytes(&[
                0x41, 0x20, 0x20, 0x20, 0x74, 0x72, 0x75, 0x6E, 0x6B, 0x2F, 0xBC, 0xC6, 0xCB, 0xE3,
                0xC6, 0xF7, 0x2E, 0x70, 0x79, 0x0A,
            ]),
        ),
        (
            &["cat", "-r", "2", repository, "trunk/计算器.py"],
            success("print('ok')\n"),
        ),
    ]);
    let svnlook = SvnLook::with_runner(PathBuf::from("fixture-svnlook"), runner);

    let event = svnlook
        .read_revision(Path::new(repository), 2, &FingerprintKey::new([7; 32]))
        .expect("Windows console text should decode without losing Chinese metadata");

    assert_eq!(event.author, "张三");
    assert_eq!(
        event.committed_at,
        "2026-09-15 09:33:21 +0800 (周二, 15 9月 2026)"
    );
    assert_eq!(event.changes[0].path.as_str(), "trunk/计算器.py");
    svnlook.runner().assert_exhausted();
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
