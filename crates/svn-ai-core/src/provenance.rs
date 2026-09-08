use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use svn_ai_protocol::Digest;
use thiserror::Error;

use crate::{EditScript, FingerprintKey, fingerprint_lines};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Ai { event_id: String },
    NonAi,
    Unattributed,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineProvenance {
    pub line_digest: Digest,
    pub context_digest: Digest,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileProvenance {
    lines: Vec<LineProvenance>,
    generated_ai_lines: u64,
}

impl FileProvenance {
    pub fn legacy(content: &[u8], key: &FingerprintKey) -> Self {
        let file = fingerprint_lines(content, key);
        let lines = file
            .line_digests
            .into_iter()
            .zip(file.context_digests)
            .map(|(line_digest, context_digest)| LineProvenance {
                line_digest,
                context_digest,
                origin: Origin::Legacy,
            })
            .collect();

        Self {
            lines,
            generated_ai_lines: 0,
        }
    }

    pub fn apply(
        &mut self,
        script: &EditScript,
        attributions: &[HunkAttribution],
    ) -> Result<(), ProvenanceError> {
        if self.lines.len() != script.before_line_count as usize {
            return Err(ProvenanceError::BaseLineCount {
                expected: self.lines.len(),
                actual: script.before_line_count,
            });
        }

        let origins = validate_attributions(script, attributions)?;
        validate_hunks(script, self.lines.len())?;

        let mut next = Vec::with_capacity(script.after_line_count as usize);
        let mut old_cursor = 0_usize;
        let mut generated_ai_lines = 0_u64;

        for (index, hunk) in script.hunks.iter().enumerate() {
            let old_start = hunk.old_start as usize;
            let old_end = old_start + hunk.old_len as usize;
            next.extend_from_slice(&self.lines[old_cursor..old_start]);

            if !hunk.new_line_digests.is_empty() {
                let origin = origins
                    .get(&index)
                    .expect("validated insertion hunk must have an origin");
                if matches!(origin, Origin::Ai { .. }) {
                    generated_ai_lines += hunk.new_line_digests.len() as u64;
                }
                next.extend(
                    hunk.new_line_digests
                        .iter()
                        .copied()
                        .zip(hunk.new_context_digests.iter().copied())
                        .map(|(line_digest, context_digest)| LineProvenance {
                            line_digest,
                            context_digest,
                            origin: origin.clone(),
                        }),
                );
            }
            old_cursor = old_end;
        }

        next.extend_from_slice(&self.lines[old_cursor..]);
        if next.len() != script.after_line_count as usize {
            return Err(ProvenanceError::AfterLineCount {
                expected: script.after_line_count,
                actual: next.len(),
            });
        }

        self.lines = next;
        self.generated_ai_lines += generated_ai_lines;
        Ok(())
    }

    pub fn metrics(&self) -> StockMetrics {
        let mut metrics = StockMetrics {
            total_lines: self.lines.len() as u64,
            ..StockMetrics::default()
        };
        for line in &self.lines {
            match line.origin {
                Origin::Ai { .. } => metrics.ai_lines += 1,
                Origin::NonAi => metrics.non_ai_lines += 1,
                Origin::Unattributed => metrics.unattributed_lines += 1,
                Origin::Legacy => metrics.legacy_lines += 1,
            }
        }
        metrics
    }

    pub fn generated_ai_lines(&self) -> u64 {
        self.generated_ai_lines
    }

    pub fn lines(&self) -> &[LineProvenance] {
        &self.lines
    }

    pub fn clone_for_svn_copy(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            generated_ai_lines: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkAttribution {
    pub hunk_index: usize,
    pub origin: Origin,
}

impl HunkAttribution {
    pub fn ai(hunk_index: usize, event_id: impl Into<String>) -> Self {
        Self {
            hunk_index,
            origin: Origin::Ai {
                event_id: event_id.into(),
            },
        }
    }

    pub const fn non_ai(hunk_index: usize) -> Self {
        Self {
            hunk_index,
            origin: Origin::NonAi,
        }
    }

    pub const fn unattributed(hunk_index: usize) -> Self {
        Self {
            hunk_index,
            origin: Origin::Unattributed,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockMetrics {
    pub total_lines: u64,
    pub ai_lines: u64,
    pub non_ai_lines: u64,
    pub unattributed_lines: u64,
    pub legacy_lines: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProvenanceError {
    #[error("edit script expects {actual} source lines but state contains {expected}")]
    BaseLineCount { expected: usize, actual: u32 },
    #[error("edit hunk {hunk_index} is outside or overlaps the source file")]
    InvalidHunkRange { hunk_index: usize },
    #[error("edit hunk {hunk_index} has an inconsistent result position")]
    InvalidNewRange { hunk_index: usize },
    #[error("edit hunk {hunk_index} has mismatched line and context digest counts")]
    DigestCountMismatch { hunk_index: usize },
    #[error("edit hunk attribution {hunk_index} does not identify an insertion hunk")]
    UnexpectedAttribution { hunk_index: usize },
    #[error("edit hunk {hunk_index} has more than one attribution")]
    DuplicateAttribution { hunk_index: usize },
    #[error("edit hunk {hunk_index} inserts lines but has no attribution")]
    MissingAttribution { hunk_index: usize },
    #[error("edit script claims {expected} result lines but applying it produced {actual}")]
    AfterLineCount { expected: u32, actual: usize },
}

fn validate_attributions(
    script: &EditScript,
    attributions: &[HunkAttribution],
) -> Result<BTreeMap<usize, Origin>, ProvenanceError> {
    let mut origins = BTreeMap::new();
    for attribution in attributions {
        let Some(hunk) = script.hunks.get(attribution.hunk_index) else {
            return Err(ProvenanceError::UnexpectedAttribution {
                hunk_index: attribution.hunk_index,
            });
        };
        if hunk.new_line_digests.is_empty() {
            return Err(ProvenanceError::UnexpectedAttribution {
                hunk_index: attribution.hunk_index,
            });
        }
        if origins
            .insert(attribution.hunk_index, attribution.origin.clone())
            .is_some()
        {
            return Err(ProvenanceError::DuplicateAttribution {
                hunk_index: attribution.hunk_index,
            });
        }
    }

    for (index, hunk) in script.hunks.iter().enumerate() {
        if !hunk.new_line_digests.is_empty() && !origins.contains_key(&index) {
            return Err(ProvenanceError::MissingAttribution { hunk_index: index });
        }
    }
    Ok(origins)
}

fn validate_hunks(script: &EditScript, source_len: usize) -> Result<(), ProvenanceError> {
    let mut previous_old_end = 0_usize;
    let mut previous_new_end = 0_usize;
    for (index, hunk) in script.hunks.iter().enumerate() {
        if hunk.new_line_digests.len() != hunk.new_context_digests.len() {
            return Err(ProvenanceError::DigestCountMismatch { hunk_index: index });
        }
        let start = hunk.old_start as usize;
        let Some(end) = start.checked_add(hunk.old_len as usize) else {
            return Err(ProvenanceError::InvalidHunkRange { hunk_index: index });
        };
        if start < previous_old_end || end > source_len {
            return Err(ProvenanceError::InvalidHunkRange { hunk_index: index });
        }
        let expected_new_start = previous_new_end + (start - previous_old_end);
        if hunk.new_start as usize != expected_new_start {
            return Err(ProvenanceError::InvalidNewRange { hunk_index: index });
        }
        previous_old_end = end;
        previous_new_end = hunk.new_start as usize + hunk.new_line_digests.len();
    }
    Ok(())
}
