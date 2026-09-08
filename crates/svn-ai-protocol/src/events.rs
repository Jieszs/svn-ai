use serde::{Deserialize, Serialize};

use crate::{Digest, RepoPath};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionEvent {
    pub schema_version: String,
    pub event_id: String,
    pub device_id: String,
    pub client_version: String,
    pub svn_username: String,
    pub repository_uuid: String,
    pub repository_root_digest: Digest,
    pub base_revision: i64,
    pub session_id_digest: Digest,
    pub tool_use_id_digest: Digest,
    pub tool: ToolKind,
    pub occurred_at: String,
    pub model_id: Option<String>,
    pub path: RepoPath,
    pub hunks: Vec<AttributionHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionHunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_line_digests: Vec<Digest>,
    pub new_context_digests: Vec<Digest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Edit,
    Write,
    Bash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedLineState {
    Pending,
    Accepted,
    Discarded,
    Expired,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionEvent {
    pub schema_version: String,
    pub event_id: String,
    pub agent_version: String,
    pub repository_uuid: String,
    pub revision: i64,
    pub author: String,
    pub committed_at: String,
    pub changes: Vec<RevisionFileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionFileChange {
    pub path: RepoPath,
    pub kind: ChangeKind,
    pub copy_from: Option<CopyFrom>,
    pub excluded: bool,
    pub binary: bool,
    pub hunks: Vec<AttributionHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyFrom {
    pub path: RepoPath,
    pub revision: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Add,
    Modify,
    Delete,
    Copy,
}
