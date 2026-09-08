mod edit;
mod fingerprint;

pub use edit::{EditHunk, EditScript, diff_files};
pub use fingerprint::{FingerprintKey, FingerprintedFile, fingerprint_lines, split_lines};
