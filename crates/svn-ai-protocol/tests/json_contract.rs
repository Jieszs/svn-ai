use svn_ai_protocol::{AttributionEvent, RepoPath, SCHEMA_VERSION};

#[test]
fn attribution_event_matches_the_golden_contract() {
    let json = include_str!("golden/ai-attribution-event.json");
    let event: AttributionEvent = serde_json::from_str(json).unwrap();

    assert_eq!(SCHEMA_VERSION, "svn-ai/1.0.0");
    assert_eq!(event.schema_version, SCHEMA_VERSION);
    assert_eq!(event.svn_username, "zhengjie");
    assert_eq!(event.hunks[0].new_line_digests.len(), 2);
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        serde_json::from_str::<serde_json::Value>(json).unwrap()
    );
}

#[test]
fn repository_path_rejects_absolute_and_parent_paths() {
    for invalid in [r"C:\work\a.rs", "/srv/a.rs", "../secret.rs", "a/../../b.rs"] {
        assert!(RepoPath::try_from(invalid).is_err(), "accepted {invalid}");
    }
}

#[test]
fn repository_path_accepts_normalized_relative_paths() {
    let path = RepoPath::try_from("src/main.rs").unwrap();
    assert_eq!(path.as_str(), "src/main.rs");
}
