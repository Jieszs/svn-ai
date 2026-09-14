use std::{fs, path::PathBuf};

use chrono::Utc;
use serde::Deserialize;
use svn_ai_core::{FingerprintKey, diff_files};
use svn_ai_protocol::{AttributionEvent, AttributionHunk, Digest, ToolKind};
use svn_ai_svn::{CommandRunner, SvnClient, SvnClientError};
use thiserror::Error;
use uuid::Uuid;

use crate::{ClientConfig, EventStore, StoreError, TransactionRecord};

#[derive(Debug, Clone, Deserialize)]
pub struct HookInput {
    session_id: String,
    cwd: PathBuf,
    hook_event_name: String,
    tool_name: String,
    tool_input: serde_json::Value,
    tool_use_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookOutcome {
    Captured,
    Finalized,
    Cancelled,
    Ignored,
}

pub struct HookProcessor<'a, R> {
    config: &'a ClientConfig,
    store: &'a EventStore,
    svn: &'a SvnClient<R>,
}

impl<'a, R: CommandRunner> HookProcessor<'a, R> {
    pub const fn new(
        config: &'a ClientConfig,
        store: &'a EventStore,
        svn: &'a SvnClient<R>,
    ) -> Self {
        Self { config, store, svn }
    }

    pub fn process(&self, input: HookInput) -> Result<HookOutcome, HookError> {
        let Some(tool) = supported_tool(&input.tool_name) else {
            return Ok(HookOutcome::Ignored);
        };
        let session_digest = private_digest(
            self.config.fingerprint_secret.as_bytes(),
            b"svn-ai/session/v1\0",
            input.session_id.as_bytes(),
        );
        let tool_digest = private_digest(
            self.config.fingerprint_secret.as_bytes(),
            b"svn-ai/tool-use/v1\0",
            input.tool_use_id.as_bytes(),
        );

        match input.hook_event_name.as_str() {
            "PreToolUse" => self.capture_before(input, tool, session_digest, tool_digest),
            "PostToolUse" => self.finalize(session_digest, tool_digest),
            "PostToolUseFailure" => {
                self.store
                    .cancel_transaction(&session_digest, &tool_digest)?;
                Ok(HookOutcome::Cancelled)
            }
            _ => Ok(HookOutcome::Ignored),
        }
    }

    fn capture_before(
        &self,
        input: HookInput,
        tool: ToolKind,
        session_digest: Digest,
        tool_digest: Digest,
    ) -> Result<HookOutcome, HookError> {
        let target = target_path(&input)?;
        let working_copy = match self.svn.discover(&target) {
            Ok(info) => info,
            Err(SvnClientError::NotWorkingCopy { .. }) => return Ok(HookOutcome::Ignored),
            Err(error) => return Err(error.into()),
        };
        let path = working_copy.repo_path(&target)?;
        let before_content = read_or_empty(&target)?;
        let repository_root_digest = private_digest(
            self.config.fingerprint_secret.as_bytes(),
            b"svn-ai/repository-root/v1\0",
            working_copy.repository_root_url.as_bytes(),
        );
        self.store.begin_transaction(&TransactionRecord {
            session_digest,
            tool_digest,
            before_content,
            repository_uuid: working_copy.repository_uuid,
            repository_root_digest,
            base_revision: working_copy.base_revision,
            path,
            file_path: target,
            tool,
            occurred_at: Utc::now().to_rfc3339(),
        })?;
        Ok(HookOutcome::Captured)
    }

    fn finalize(
        &self,
        session_digest: Digest,
        tool_digest: Digest,
    ) -> Result<HookOutcome, HookError> {
        let Some(transaction) = self.store.load_transaction(&session_digest, &tool_digest)? else {
            return Ok(HookOutcome::Ignored);
        };
        let after_content = read_or_empty(&transaction.file_path)?;
        let key = FingerprintKey::new(*self.config.fingerprint_secret.as_bytes());
        let script = diff_files(&transaction.before_content, &after_content, &key);
        if script.hunks.is_empty() {
            self.store
                .cancel_transaction(&session_digest, &tool_digest)?;
            return Ok(HookOutcome::Ignored);
        }
        let event = AttributionEvent {
            schema_version: "1.0.0".to_owned(),
            event_id: Uuid::now_v7().to_string(),
            device_id: self.config.device_id.clone(),
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
            svn_username: self.config.svn_username.clone(),
            repository_uuid: transaction.repository_uuid,
            repository_root_digest: transaction.repository_root_digest,
            base_revision: transaction.base_revision,
            session_id_digest: session_digest,
            tool_use_id_digest: tool_digest,
            tool: transaction.tool,
            occurred_at: transaction.occurred_at,
            model_id: None,
            path: transaction.path,
            hunks: script.hunks.iter().map(AttributionHunk::from).collect(),
        };
        self.store
            .finish_transaction(&session_digest, &tool_digest, &event)?;
        Ok(HookOutcome::Finalized)
    }
}

fn supported_tool(name: &str) -> Option<ToolKind> {
    match name {
        "Edit" => Some(ToolKind::Edit),
        "Write" => Some(ToolKind::Write),
        _ => None,
    }
}

fn target_path(input: &HookInput) -> Result<PathBuf, HookError> {
    let value = input
        .tool_input
        .get("file_path")
        .and_then(serde_json::Value::as_str)
        .ok_or(HookError::MissingFilePath)?;
    let path = PathBuf::from(value);
    Ok(if path.is_absolute() {
        path
    } else {
        input.cwd.join(path)
    })
}

fn read_or_empty(path: &std::path::Path) -> Result<Vec<u8>, HookError> {
    match fs::read(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(HookError::FileIo {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

fn private_digest(key: &[u8; 32], domain: &[u8], value: &[u8]) -> Digest {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(domain);
    hasher.update(value);
    Digest::new(*hasher.finalize().as_bytes())
}

#[derive(Debug, Error)]
pub enum HookError {
    #[error("Claude hook input has no string tool_input.file_path")]
    MissingFilePath,
    #[error("failed to read `{path}`: {message}")]
    FileIo { path: PathBuf, message: String },
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Svn(#[from] SvnClientError),
}
