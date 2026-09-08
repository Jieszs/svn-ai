mod edit;
mod fingerprint;
mod provenance;

pub use edit::{EditHunk, EditScript, diff_files};
pub use fingerprint::{FingerprintKey, FingerprintedFile, fingerprint_lines, split_lines};
pub use provenance::{
    FileProvenance, HunkAttribution, LineProvenance, Origin, ProvenanceError, StockMetrics,
};
