use serde::{Deserialize, Serialize};
use svn_ai_protocol::{AttributionEvent, Digest, RevisionEvent};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionMetrics {
    pub svn_additions: u64,
    pub ai_additions: u64,
    pub non_ai_additions: u64,
    pub ambiguous_additions: u64,
}

pub fn match_attribution(
    revision: &RevisionEvent,
    events: &[AttributionEvent],
) -> AttributionMetrics {
    let mut metrics = AttributionMetrics::default();

    for change in revision
        .changes
        .iter()
        .filter(|change| !change.excluded && !change.binary)
    {
        let mut candidates = events
            .iter()
            .filter(|event| {
                event.repository_uuid == revision.repository_uuid
                    && event.svn_username == revision.author
                    && event.path == change.path
                    && event.base_revision < revision.revision
            })
            .flat_map(|event| {
                event.hunks.iter().flat_map(move |hunk| {
                    hunk.new_line_digests
                        .iter()
                        .copied()
                        .zip(hunk.new_context_digests.iter().copied())
                        .map(move |(line, context)| Candidate {
                            line,
                            context,
                            consumed: false,
                        })
                })
            })
            .collect::<Vec<_>>();

        for hunk in &change.hunks {
            for (line, context) in hunk
                .new_line_digests
                .iter()
                .copied()
                .zip(hunk.new_context_digests.iter().copied())
            {
                metrics.svn_additions += 1;
                match select_candidate(&candidates, line, context) {
                    CandidateSelection::Matched(index) => {
                        candidates[index].consumed = true;
                        metrics.ai_additions += 1;
                    }
                    CandidateSelection::Missing => metrics.non_ai_additions += 1,
                    CandidateSelection::Ambiguous => metrics.ambiguous_additions += 1,
                }
            }
        }
    }

    metrics
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    line: Digest,
    context: Digest,
    consumed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateSelection {
    Matched(usize),
    Missing,
    Ambiguous,
}

fn select_candidate(candidates: &[Candidate], line: Digest, context: Digest) -> CandidateSelection {
    let exact = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            !candidate.consumed && candidate.line == line && candidate.context == context
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match exact.as_slice() {
        [index] => return CandidateSelection::Matched(*index),
        [_, _, ..] => return CandidateSelection::Ambiguous,
        [] => {}
    }

    let matching_line = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| !candidate.consumed && candidate.line == line)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matching_line.as_slice() {
        [] => CandidateSelection::Missing,
        [index] => CandidateSelection::Matched(*index),
        [_, _, ..] => CandidateSelection::Ambiguous,
    }
}
