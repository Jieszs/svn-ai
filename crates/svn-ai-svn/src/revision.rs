use std::{ffi::OsString, path::PathBuf};

use svn_ai_core::{FingerprintKey, diff_files};
use svn_ai_protocol::{AttributionHunk, ChangeKind, RevisionEvent, RevisionFileChange};
use thiserror::Error;

use crate::{ChangedParseError, CommandRunner, ProcessRunner, parse_changed};

#[derive(Debug)]
pub struct SvnLook<R = ProcessRunner> {
    executable: PathBuf,
    runner: R,
}

impl SvnLook<ProcessRunner> {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            runner: ProcessRunner,
        }
    }
}

impl<R: CommandRunner> SvnLook<R> {
    pub fn with_runner(executable: PathBuf, runner: R) -> Self {
        Self { executable, runner }
    }

    pub const fn runner(&self) -> &R {
        &self.runner
    }

    pub fn read_revision(
        &self,
        repository: &std::path::Path,
        revision: i64,
        key: &FingerprintKey,
    ) -> Result<RevisionEvent, SvnLookError> {
        if revision <= 0 {
            return Err(SvnLookError::InvalidRevision(revision));
        }

        let repository_uuid = self.text(&["uuid".into(), repository.as_os_str().into()])?;
        let author = self.text(&revision_args("author", revision, repository))?;
        let committed_at = self.text(&revision_args("date", revision, repository))?;
        let changed = self.text(&[
            "changed".into(),
            "--copy-info".into(),
            "-r".into(),
            revision.to_string().into(),
            repository.as_os_str().into(),
        ])?;
        let changed_paths = parse_changed(&changed)?;

        let mut changes = Vec::new();
        for changed_path in changed_paths {
            if changed_path.is_directory {
                continue;
            }

            let (before, after) = if !changed_path.text_changed {
                (Vec::new(), Vec::new())
            } else {
                let before = match changed_path.kind {
                    ChangeKind::Add => Vec::new(),
                    ChangeKind::Copy => {
                        let source = changed_path
                            .copy_from
                            .as_ref()
                            .expect("parsed copy must have source metadata");
                        self.cat(repository, source.revision, source.path.as_str())?
                    }
                    ChangeKind::Modify | ChangeKind::Delete => {
                        self.cat(repository, revision - 1, changed_path.path.as_str())?
                    }
                };
                let after = match changed_path.kind {
                    ChangeKind::Delete => Vec::new(),
                    ChangeKind::Add | ChangeKind::Modify => {
                        self.cat(repository, revision, changed_path.path.as_str())?
                    }
                    ChangeKind::Copy => {
                        self.cat(repository, revision, changed_path.path.as_str())?
                    }
                };
                (before, after)
            };

            let binary = before.contains(&0) || after.contains(&0);
            let hunks = if binary || !changed_path.text_changed {
                Vec::new()
            } else {
                diff_files(&before, &after, key)
                    .hunks
                    .iter()
                    .map(AttributionHunk::from)
                    .collect()
            };

            changes.push(RevisionFileChange {
                path: changed_path.path,
                kind: changed_path.kind,
                copy_from: changed_path.copy_from,
                excluded: false,
                binary,
                hunks,
            });
        }

        Ok(RevisionEvent {
            schema_version: "1.0.0".to_owned(),
            event_id: format!("{repository_uuid}:r{revision}"),
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
            repository_uuid,
            revision,
            author,
            committed_at,
            changes,
        })
    }

    fn cat(
        &self,
        repository: &std::path::Path,
        revision: i64,
        path: &str,
    ) -> Result<Vec<u8>, SvnLookError> {
        self.bytes(&[
            "cat".into(),
            "-r".into(),
            revision.to_string().into(),
            repository.as_os_str().into(),
            path.into(),
        ])
    }

    fn text(&self, args: &[OsString]) -> Result<String, SvnLookError> {
        let bytes = self.bytes(args)?;
        let text = String::from_utf8(bytes).map_err(|_| SvnLookError::NonUtf8Output {
            command: display_command(&self.executable, args),
        })?;
        Ok(text.trim_end_matches(['\r', '\n']).to_owned())
    }

    fn bytes(&self, args: &[OsString]) -> Result<Vec<u8>, SvnLookError> {
        let command = display_command(&self.executable, args);
        let output =
            self.runner
                .run(&self.executable, args)
                .map_err(|error| SvnLookError::CommandIo {
                    command: command.clone(),
                    message: error.to_string(),
                })?;
        if !output.success {
            return Err(SvnLookError::CommandFailed {
                command,
                exit_code: output.exit_code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(output.stdout)
    }
}

fn revision_args(command: &str, revision: i64, repository: &std::path::Path) -> Vec<OsString> {
    vec![
        command.into(),
        "-r".into(),
        revision.to_string().into(),
        repository.as_os_str().into(),
    ]
}

fn display_command(executable: &std::path::Path, args: &[OsString]) -> String {
    std::iter::once(executable.as_os_str())
        .chain(args.iter().map(OsString::as_os_str))
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SvnLookError {
    #[error("SVN revision must be greater than zero, got {0}")]
    InvalidRevision(i64),
    #[error("failed to start `{command}`: {message}")]
    CommandIo { command: String, message: String },
    #[error("`{command}` exited with {exit_code:?}: {stderr}")]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },
    #[error("`{command}` returned text that is not UTF-8")]
    NonUtf8Output { command: String },
    #[error(transparent)]
    Changed(#[from] ChangedParseError),
}
