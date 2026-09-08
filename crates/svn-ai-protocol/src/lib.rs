mod events;
mod identity;

pub use events::{
    AttributionEvent, AttributionHunk, ChangeKind, CopyFrom, GeneratedLineState, RevisionEvent,
    RevisionFileChange, ToolKind,
};
pub use identity::{Digest, DigestError, RepoPath, RepoPathError};

pub const SCHEMA_VERSION: &str = "svn-ai/1.0.0";
