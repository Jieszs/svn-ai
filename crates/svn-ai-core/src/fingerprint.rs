use serde::{Deserialize, Serialize};
use svn_ai_protocol::Digest;

const LINE_DOMAIN: &[u8] = b"svn-ai/line/v1\0";
const CONTEXT_DOMAIN: &[u8] = b"svn-ai/context/v1\0";

#[derive(Clone)]
pub struct FingerprintKey([u8; 32]);

impl FingerprintKey {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintedFile {
    pub line_digests: Vec<Digest>,
    pub context_digests: Vec<Digest>,
}

pub fn split_lines(content: &[u8]) -> Vec<&[u8]> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in content.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }

        let mut end = index;
        if end > start && content[end - 1] == b'\r' {
            end -= 1;
        }
        lines.push(&content[start..end]);
        start = index + 1;
    }

    if start < content.len() {
        lines.push(&content[start..]);
    }
    lines
}

pub fn fingerprint_lines(content: &[u8], key: &FingerprintKey) -> FingerprintedFile {
    let line_digests: Vec<_> = split_lines(content)
        .into_iter()
        .map(|line| fingerprint_line(line, key))
        .collect();

    let context_digests = (0..line_digests.len())
        .map(|index| fingerprint_context(&line_digests, index, key))
        .collect();

    FingerprintedFile {
        line_digests,
        context_digests,
    }
}

fn fingerprint_line(line: &[u8], key: &FingerprintKey) -> Digest {
    let mut hasher = blake3::Hasher::new_keyed(key.as_bytes());
    hasher.update(LINE_DOMAIN);
    hasher.update(&(line.len() as u64).to_le_bytes());
    hasher.update(line);
    Digest::new(*hasher.finalize().as_bytes())
}

fn fingerprint_context(lines: &[Digest], index: usize, key: &FingerprintKey) -> Digest {
    let mut hasher = blake3::Hasher::new_keyed(key.as_bytes());
    hasher.update(CONTEXT_DOMAIN);
    update_optional_digest(&mut hasher, index.checked_sub(1).and_then(|i| lines.get(i)));
    update_optional_digest(&mut hasher, lines.get(index));
    update_optional_digest(&mut hasher, lines.get(index + 1));
    Digest::new(*hasher.finalize().as_bytes())
}

fn update_optional_digest(hasher: &mut blake3::Hasher, digest: Option<&Digest>) {
    match digest {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(digest.as_bytes());
        }
        None => {
            hasher.update(&[0]);
            hasher.update(&[0; 32]);
        }
    }
}
