use svn_ai_protocol::{ChangeKind, CopyFrom, RepoPath};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedPath {
    pub path: RepoPath,
    pub kind: ChangeKind,
    pub copy_from: Option<CopyFrom>,
    pub text_changed: bool,
    pub properties_changed: bool,
    pub is_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChangedParseError {
    #[error("line {line} has invalid svnlook status columns")]
    InvalidStatus { line: usize },
    #[error("line {line} has an invalid repository path")]
    InvalidPath { line: usize },
    #[error("line {line} marks a copy but has no copy source")]
    MissingCopySource { line: usize },
    #[error("line {line} has invalid copy source metadata")]
    InvalidCopySource { line: usize },
}

pub fn parse_changed(output: &str) -> Result<Vec<ChangedPath>, ChangedParseError> {
    let lines: Vec<_> = output.lines().collect();
    let mut changes = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line_number = index + 1;
        let line = lines[index];
        let bytes = line.as_bytes();
        if bytes.len() < 5 || !valid_status(bytes[0], bytes[1], bytes[2], bytes[3]) {
            return Err(ChangedParseError::InvalidStatus { line: line_number });
        }

        let text_status = bytes[0];
        let copied = bytes[2] == b'+';
        let raw_path = &line[4..];
        let is_directory = raw_path.ends_with('/');
        let normalized_path = raw_path.strip_suffix('/').unwrap_or(raw_path);
        let path = RepoPath::try_from(normalized_path)
            .map_err(|_| ChangedParseError::InvalidPath { line: line_number })?;

        let copy_from = if copied {
            let copy_line_index = index + 1;
            let Some(copy_line) = lines.get(copy_line_index) else {
                return Err(ChangedParseError::MissingCopySource { line: line_number });
            };
            index += 1;
            Some(parse_copy_source(copy_line, copy_line_index + 1)?)
        } else {
            None
        };

        let kind = match (text_status, copied) {
            (b'A', true) => ChangeKind::Copy,
            (b'A', false) => ChangeKind::Add,
            (b'D', false) => ChangeKind::Delete,
            (b'U' | b'_', false) => ChangeKind::Modify,
            _ => return Err(ChangedParseError::InvalidStatus { line: line_number }),
        };

        changes.push(ChangedPath {
            path,
            kind,
            copy_from,
            text_changed: matches!(text_status, b'A' | b'D' | b'U'),
            properties_changed: bytes[1] == b'U',
            is_directory,
        });
        index += 1;
    }

    Ok(changes)
}

fn valid_status(text: u8, properties: u8, copy: u8, separator: u8) -> bool {
    matches!(text, b'A' | b'D' | b'U' | b'_')
        && matches!(properties, b'U' | b'_' | b' ')
        && matches!(copy, b'+' | b' ')
        && separator == b' '
}

fn parse_copy_source(line: &str, line_number: usize) -> Result<CopyFrom, ChangedParseError> {
    let metadata = line.trim();
    let Some(metadata) = metadata
        .strip_prefix("(from ")
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Err(ChangedParseError::InvalidCopySource { line: line_number });
    };
    let Some((raw_path, raw_revision)) = metadata.rsplit_once(":r") else {
        return Err(ChangedParseError::InvalidCopySource { line: line_number });
    };
    let revision = raw_revision
        .parse()
        .map_err(|_| ChangedParseError::InvalidCopySource { line: line_number })?;
    let normalized_path = raw_path.strip_suffix('/').unwrap_or(raw_path);
    let path = RepoPath::try_from(normalized_path)
        .map_err(|_| ChangedParseError::InvalidCopySource { line: line_number })?;

    Ok(CopyFrom { path, revision })
}
