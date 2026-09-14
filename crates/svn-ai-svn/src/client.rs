use std::path::PathBuf;

use percent_encoding::percent_decode_str;
use serde::Deserialize;
use svn_ai_protocol::RepoPath;
use thiserror::Error;

use crate::{CommandRunner, ProcessRunner};

#[derive(Debug)]
pub struct SvnClient<R = ProcessRunner> {
    executable: PathBuf,
    runner: R,
}

impl SvnClient<ProcessRunner> {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            runner: ProcessRunner,
        }
    }
}

impl<R: CommandRunner> SvnClient<R> {
    pub fn with_runner(executable: PathBuf, runner: R) -> Self {
        Self { executable, runner }
    }

    pub const fn runner(&self) -> &R {
        &self.runner
    }

    pub fn discover(&self, target: &std::path::Path) -> Result<WorkingCopyInfo, SvnClientError> {
        if !target.is_absolute() {
            return Err(SvnClientError::InvalidTarget(target.to_path_buf()));
        }
        let root_path =
            find_working_copy_root(target).ok_or_else(|| SvnClientError::NotWorkingCopy {
                target: target.to_path_buf(),
            })?;
        let args = vec![
            "info".into(),
            "--xml".into(),
            "--depth".into(),
            "empty".into(),
            root_path.as_os_str().into(),
        ];
        let output = self
            .runner
            .run(&self.executable, &args)
            .map_err(|error| SvnClientError::CommandIo(error.to_string()))?;
        if !output.success {
            return Err(SvnClientError::CommandFailed {
                exit_code: output.exit_code,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        let xml = std::str::from_utf8(&output.stdout)
            .map_err(|error| SvnClientError::InvalidXml(error.to_string()))?;
        let parsed: XmlInfo = quick_xml::de::from_str(xml)
            .map_err(|error| SvnClientError::InvalidXml(error.to_string()))?;
        let prefix = repository_prefix(&parsed.entry.url, &parsed.entry.repository.root)?;

        Ok(WorkingCopyInfo {
            root_path,
            repository_uuid: parsed.entry.repository.uuid,
            repository_root_url: parsed.entry.repository.root,
            repository_path_prefix: prefix,
            base_revision: parsed.entry.revision,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingCopyInfo {
    pub root_path: PathBuf,
    pub repository_uuid: String,
    pub repository_root_url: String,
    pub repository_path_prefix: String,
    pub base_revision: i64,
}

impl WorkingCopyInfo {
    pub fn repo_path(&self, target: &std::path::Path) -> Result<RepoPath, SvnClientError> {
        let relative = target
            .strip_prefix(&self.root_path)
            .map_err(|_| SvnClientError::InvalidTarget(target.to_path_buf()))?;
        let relative_parts = relative
            .components()
            .map(|component| match component {
                std::path::Component::Normal(value) => value
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| SvnClientError::InvalidTarget(target.to_path_buf())),
                _ => Err(SvnClientError::InvalidTarget(target.to_path_buf())),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let relative_path = relative_parts.join("/");
        let path = match (
            self.repository_path_prefix.is_empty(),
            relative_path.is_empty(),
        ) {
            (false, false) => format!("{}/{}", self.repository_path_prefix, relative_path),
            (false, true) => self.repository_path_prefix.clone(),
            (true, false) => relative_path,
            (true, true) => return Err(SvnClientError::InvalidTarget(target.to_path_buf())),
        };
        RepoPath::try_from(path).map_err(|_| SvnClientError::InvalidTarget(target.to_path_buf()))
    }
}

fn find_working_copy_root(target: &std::path::Path) -> Option<PathBuf> {
    target
        .ancestors()
        .find(|candidate| candidate.join(".svn").is_dir())
        .map(std::path::Path::to_path_buf)
}

fn repository_prefix(url: &str, root: &str) -> Result<String, SvnClientError> {
    let suffix = url
        .strip_prefix(root)
        .ok_or_else(|| SvnClientError::RepositoryUrlMismatch {
            url: url.to_owned(),
            root: root.to_owned(),
        })?;
    if !suffix.is_empty() && !suffix.starts_with('/') {
        return Err(SvnClientError::RepositoryUrlMismatch {
            url: url.to_owned(),
            root: root.to_owned(),
        });
    }
    percent_decode_str(suffix.trim_start_matches('/'))
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|error| SvnClientError::InvalidXml(error.to_string()))
}

#[derive(Debug, Deserialize)]
struct XmlInfo {
    entry: XmlEntry,
}

#[derive(Debug, Deserialize)]
struct XmlEntry {
    #[serde(rename = "@revision")]
    revision: i64,
    url: String,
    repository: XmlRepository,
}

#[derive(Debug, Deserialize)]
struct XmlRepository {
    root: String,
    uuid: String,
}

#[derive(Debug, Error)]
pub enum SvnClientError {
    #[error("target is not a valid absolute working-copy path: {0}")]
    InvalidTarget(PathBuf),
    #[error("target is not inside an SVN working copy: {target}")]
    NotWorkingCopy { target: PathBuf },
    #[error("failed to run svn info: {0}")]
    CommandIo(String),
    #[error("svn info exited with {exit_code:?}: {stderr}")]
    CommandFailed {
        exit_code: Option<i32>,
        stderr: String,
    },
    #[error("svn info returned invalid XML: {0}")]
    InvalidXml(String),
    #[error("working-copy URL `{url}` is not below repository root `{root}`")]
    RepositoryUrlMismatch { url: String, root: String },
}
