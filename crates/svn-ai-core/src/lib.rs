mod edit;
mod fingerprint;
mod matching;
mod provenance;

pub use edit::{EditHunk, EditScript, HunkConversionError, diff_files};
pub use fingerprint::{FingerprintKey, FingerprintedFile, fingerprint_lines, split_lines};
pub use matching::{AttributionMetrics, match_attribution};
pub use provenance::{
    FileProvenance, HunkAttribution, LineProvenance, Origin, ProvenanceError, StockMetrics,
};
