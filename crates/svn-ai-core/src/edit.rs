use imara_diff::{Algorithm, Diff, Interner};
use serde::{Deserialize, Serialize};
use svn_ai_protocol::Digest;

use crate::{FingerprintKey, fingerprint_lines};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditScript {
    pub before_line_count: u32,
    pub after_line_count: u32,
    pub hunks: Vec<EditHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditHunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_line_digests: Vec<Digest>,
    pub new_context_digests: Vec<Digest>,
}

impl EditHunk {
    pub fn new_len(&self) -> u32 {
        self.new_line_digests
            .len()
            .try_into()
            .expect("imara-diff limits inputs to fewer than u32::MAX lines")
    }
}

pub fn diff_files(before: &[u8], after: &[u8], key: &FingerprintKey) -> EditScript {
    let before_file = fingerprint_lines(before, key);
    let after_file = fingerprint_lines(after, key);

    let mut interner =
        Interner::new(before_file.line_digests.len() + after_file.line_digests.len());
    let before_tokens: Vec<_> = before_file
        .line_digests
        .iter()
        .copied()
        .map(|digest| interner.intern(digest))
        .collect();
    let after_tokens: Vec<_> = after_file
        .line_digests
        .iter()
        .copied()
        .map(|digest| interner.intern(digest))
        .collect();

    let mut diff = Diff::default();
    diff.compute_with(
        Algorithm::Myers,
        &before_tokens,
        &after_tokens,
        interner.num_tokens(),
    );

    let hunks = diff
        .hunks()
        .map(|hunk| EditHunk {
            old_start: hunk.before.start,
            old_len: hunk.before.end - hunk.before.start,
            new_start: hunk.after.start,
            new_line_digests: after_file.line_digests
                [hunk.after.start as usize..hunk.after.end as usize]
                .to_vec(),
            new_context_digests: after_file.context_digests
                [hunk.after.start as usize..hunk.after.end as usize]
                .to_vec(),
        })
        .collect();

    EditScript {
        before_line_count: before_file
            .line_digests
            .len()
            .try_into()
            .expect("imara-diff limits inputs to fewer than u32::MAX lines"),
        after_line_count: after_file
            .line_digests
            .len()
            .try_into()
            .expect("imara-diff limits inputs to fewer than u32::MAX lines"),
        hunks,
    }
}
