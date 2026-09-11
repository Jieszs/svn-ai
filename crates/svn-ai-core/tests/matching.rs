use svn_ai_core::{AttributionMetrics, match_attribution};
use svn_ai_protocol::{
    AttributionEvent, AttributionHunk, ChangeKind, Digest, RepoPath, RevisionEvent,
    RevisionFileChange, ToolKind,
};

fn digest(value: u8) -> Digest {
    Digest::new([value; 32])
}

fn hunk(lines: &[u8], contexts: &[u8]) -> AttributionHunk {
    AttributionHunk {
        old_start: 0,
        old_len: 0,
        new_start: 0,
        new_line_digests: lines.iter().copied().map(digest).collect(),
        new_context_digests: contexts.iter().copied().map(digest).collect(),
    }
}

fn event(
    event_id: &str,
    repository_uuid: &str,
    user: &str,
    path: &str,
    base_revision: i64,
    hunks: Vec<AttributionHunk>,
) -> AttributionEvent {
    AttributionEvent {
        schema_version: "1.0.0".to_owned(),
        event_id: event_id.to_owned(),
        device_id: "device-1".to_owned(),
        client_version: "0.1.0".to_owned(),
        svn_username: user.to_owned(),
        repository_uuid: repository_uuid.to_owned(),
        repository_root_digest: digest(240),
        base_revision,
        session_id_digest: digest(241),
        tool_use_id_digest: digest(242),
        tool: ToolKind::Edit,
        occurred_at: "2026-09-11T08:00:00+08:00".to_owned(),
        model_id: Some("claude".to_owned()),
        path: RepoPath::try_from(path).unwrap(),
        hunks,
    }
}

fn revision(
    repository_uuid: &str,
    author: &str,
    revision: i64,
    path: &str,
    hunks: Vec<AttributionHunk>,
) -> RevisionEvent {
    RevisionEvent {
        schema_version: "1.0.0".to_owned(),
        event_id: format!("revision-{revision}"),
        agent_version: "0.1.0".to_owned(),
        repository_uuid: repository_uuid.to_owned(),
        revision,
        author: author.to_owned(),
        committed_at: "2026-09-11T08:05:00+08:00".to_owned(),
        changes: vec![RevisionFileChange {
            path: RepoPath::try_from(path).unwrap(),
            kind: ChangeKind::Modify,
            copy_from: None,
            excluded: false,
            binary: false,
            hunks,
        }],
    }
}

#[test]
fn matches_all_ten_ai_lines() {
    let ai = event(
        "ai-1",
        "repo-1",
        "zhengjie",
        "trunk/code.rs",
        1,
        vec![hunk(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            &[11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        )],
    );
    let committed = revision(
        "repo-1",
        "zhengjie",
        2,
        "trunk/code.rs",
        vec![hunk(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            &[11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        )],
    );

    let metrics = match_attribution(&committed, &[ai]);

    assert_eq!(
        metrics,
        AttributionMetrics {
            svn_additions: 10,
            ai_additions: 10,
            non_ai_additions: 0,
            ambiguous_additions: 0,
        }
    );
}

#[test]
fn counts_two_manually_rewritten_lines_as_non_ai() {
    let ai = event(
        "ai-1",
        "repo-1",
        "zhengjie",
        "trunk/code.rs",
        1,
        vec![hunk(
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            &[11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        )],
    );
    let committed = revision(
        "repo-1",
        "zhengjie",
        2,
        "trunk/code.rs",
        vec![hunk(
            &[1, 2, 3, 44, 5, 6, 7, 88, 9, 10],
            &[11, 12, 99, 44, 99, 16, 99, 88, 99, 20],
        )],
    );

    let metrics = match_attribution(&committed, &[ai]);

    assert_eq!(metrics.svn_additions, 10);
    assert_eq!(metrics.ai_additions, 8);
    assert_eq!(metrics.non_ai_additions, 2);
    assert_eq!(metrics.ambiguous_additions, 0);
}

#[test]
fn rejects_events_from_other_identity_or_future_base() {
    let committed = revision(
        "repo-1",
        "zhengjie",
        2,
        "trunk/code.rs",
        vec![hunk(&[1], &[11])],
    );
    let events = vec![
        event(
            "wrong-repo",
            "repo-2",
            "zhengjie",
            "trunk/code.rs",
            1,
            vec![hunk(&[1], &[11])],
        ),
        event(
            "wrong-user",
            "repo-1",
            "other",
            "trunk/code.rs",
            1,
            vec![hunk(&[1], &[11])],
        ),
        event(
            "wrong-path",
            "repo-1",
            "zhengjie",
            "branches/code.rs",
            1,
            vec![hunk(&[1], &[11])],
        ),
        event(
            "future-base",
            "repo-1",
            "zhengjie",
            "trunk/code.rs",
            2,
            vec![hunk(&[1], &[11])],
        ),
    ];

    let metrics = match_attribution(&committed, &events);

    assert_eq!(metrics.ai_additions, 0);
    assert_eq!(metrics.non_ai_additions, 1);
}

#[test]
fn marks_an_unresolved_duplicate_as_ambiguous() {
    let ai = event(
        "ai-1",
        "repo-1",
        "zhengjie",
        "trunk/code.rs",
        1,
        vec![hunk(&[1, 1], &[11, 11])],
    );
    let committed = revision(
        "repo-1",
        "zhengjie",
        2,
        "trunk/code.rs",
        vec![hunk(&[1], &[11])],
    );

    let metrics = match_attribution(&committed, &[ai]);

    assert_eq!(metrics.ai_additions, 0);
    assert_eq!(metrics.non_ai_additions, 0);
    assert_eq!(metrics.ambiguous_additions, 1);
}

#[test]
fn consumes_each_recorded_ai_line_only_once() {
    let ai = event(
        "ai-1",
        "repo-1",
        "zhengjie",
        "trunk/code.rs",
        1,
        vec![hunk(&[1], &[11])],
    );
    let committed = revision(
        "repo-1",
        "zhengjie",
        2,
        "trunk/code.rs",
        vec![hunk(&[1, 1], &[11, 12])],
    );

    let metrics = match_attribution(&committed, &[ai]);

    assert_eq!(metrics.ai_additions, 1);
    assert_eq!(metrics.non_ai_additions, 1);
    assert_eq!(metrics.ambiguous_additions, 0);
}

#[test]
fn excludes_binary_ignored_and_pure_copy_changes_from_line_totals() {
    let mut committed = revision(
        "repo-1",
        "zhengjie",
        2,
        "trunk/code.rs",
        vec![hunk(&[1], &[11])],
    );
    committed.changes[0].excluded = true;
    committed.changes.push(RevisionFileChange {
        path: RepoPath::try_from("trunk/image.bin").unwrap(),
        kind: ChangeKind::Add,
        copy_from: None,
        excluded: false,
        binary: true,
        hunks: vec![hunk(&[2], &[12])],
    });
    committed.changes.push(RevisionFileChange {
        path: RepoPath::try_from("branches/code.rs").unwrap(),
        kind: ChangeKind::Copy,
        copy_from: None,
        excluded: false,
        binary: false,
        hunks: Vec::new(),
    });

    let metrics = match_attribution(&committed, &[]);

    assert_eq!(metrics.svn_additions, 0);
}
