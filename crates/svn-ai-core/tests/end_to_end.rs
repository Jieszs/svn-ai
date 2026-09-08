use svn_ai_core::{EditHunk, FileProvenance, FingerprintKey, HunkAttribution, diff_files};
use svn_ai_protocol::{
    AttributionEvent, AttributionHunk, Digest, RepoPath, SCHEMA_VERSION, ToolKind,
};

#[test]
fn protocol_round_trip_preserves_an_ai_attribution_lifecycle() {
    let key = FingerprintKey::new([12; 32]);
    let before = b"base";
    let after_ai = b"base\nai one\nai two";
    let client_script = diff_files(before, after_ai, &key);
    let protocol_hunk = AttributionHunk::from(&client_script.hunks[0]);
    let event = AttributionEvent {
        schema_version: SCHEMA_VERSION.to_owned(),
        event_id: "event-e2e-1".to_owned(),
        device_id: "device-1".to_owned(),
        client_version: "0.1.0".to_owned(),
        svn_username: "zhengjie".to_owned(),
        repository_uuid: "repo-uuid".to_owned(),
        repository_root_digest: Digest::new([1; 32]),
        base_revision: 42,
        session_id_digest: Digest::new([2; 32]),
        tool_use_id_digest: Digest::new([3; 32]),
        tool: ToolKind::Edit,
        occurred_at: "2026-09-08T06:00:00Z".to_owned(),
        model_id: None,
        path: RepoPath::try_from("src/main.rs").unwrap(),
        hunks: vec![protocol_hunk],
    };

    let json = serde_json::to_string(&event).unwrap();
    let decoded: AttributionEvent = serde_json::from_str(&json).unwrap();
    let server_script = diff_files(before, after_ai, &key);
    assert_eq!(
        decoded.hunks[0].new_line_digests,
        server_script.hunks[0].new_line_digests
    );

    let mut state = FileProvenance::legacy(before, &key);
    state
        .apply(&server_script, &[HunkAttribution::ai(0, decoded.event_id)])
        .unwrap();
    assert_eq!(state.metrics().ai_lines, 2);
    assert_eq!(state.metrics().legacy_lines, 1);

    let after_human = b"base\nhuman\nai two";
    let human_script = diff_files(after_ai, after_human, &key);
    state
        .apply(&human_script, &[HunkAttribution::non_ai(0)])
        .unwrap();
    assert_eq!(state.metrics().ai_lines, 1);
    assert_eq!(state.metrics().non_ai_lines, 1);
}

#[test]
fn malformed_protocol_hunk_is_rejected() {
    let protocol_hunk = AttributionHunk {
        old_start: 0,
        old_len: 0,
        new_start: 0,
        new_line_digests: vec![Digest::new([1; 32])],
        new_context_digests: vec![],
    };

    assert!(EditHunk::try_from(&protocol_hunk).is_err());
}
