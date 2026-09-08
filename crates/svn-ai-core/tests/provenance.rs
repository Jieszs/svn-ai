use svn_ai_core::{FileProvenance, FingerprintKey, HunkAttribution, diff_files};

#[test]
fn ai_insert_then_non_ai_rewrite_reduces_ai_stock() {
    let key = FingerprintKey::new([6; 32]);
    let mut state = FileProvenance::legacy(b"base", &key);

    let ai_edit = diff_files(b"base", b"base\nai one\nai two", &key);
    state
        .apply(&ai_edit, &[HunkAttribution::ai(0, "event-1")])
        .unwrap();
    assert_eq!(state.metrics().ai_lines, 2);
    assert_eq!(state.metrics().legacy_lines, 1);

    let non_ai_edit = diff_files(b"base\nai one\nai two", b"base\nhuman\nai two", &key);
    state
        .apply(&non_ai_edit, &[HunkAttribution::non_ai(0)])
        .unwrap();
    assert_eq!(state.metrics().ai_lines, 1);
    assert_eq!(state.metrics().non_ai_lines, 1);
    assert_eq!(state.metrics().legacy_lines, 1);
}

#[test]
fn missing_hunk_attribution_is_an_error() {
    let key = FingerprintKey::new([8; 32]);
    let mut state = FileProvenance::legacy(b"a", &key);
    let edit = diff_files(b"a", b"a\nb", &key);

    assert!(state.apply(&edit, &[]).is_err());
}

#[test]
fn duplicate_hunk_attribution_is_an_error() {
    let key = FingerprintKey::new([8; 32]);
    let mut state = FileProvenance::legacy(b"a", &key);
    let edit = diff_files(b"a", b"a\nb", &key);

    let attributions = [
        HunkAttribution::ai(0, "event-1"),
        HunkAttribution::non_ai(0),
    ];
    assert!(state.apply(&edit, &attributions).is_err());
}

#[test]
fn copied_state_preserves_origins_without_counting_new_ai() {
    let key = FingerprintKey::new([6; 32]);
    let mut source = FileProvenance::legacy(b"base", &key);
    let edit = diff_files(b"base", b"base\nai", &key);
    source
        .apply(&edit, &[HunkAttribution::ai(0, "event-2")])
        .unwrap();

    let copy = source.clone_for_svn_copy();

    assert_eq!(copy.metrics(), source.metrics());
    assert_eq!(copy.generated_ai_lines(), 0);
    assert_eq!(source.generated_ai_lines(), 1);
}

#[test]
fn deletion_requires_no_origin_and_removes_the_line() {
    let key = FingerprintKey::new([6; 32]);
    let mut state = FileProvenance::legacy(b"base", &key);
    let insert = diff_files(b"base", b"base\nai", &key);
    state
        .apply(&insert, &[HunkAttribution::ai(0, "event-3")])
        .unwrap();

    let delete = diff_files(b"base\nai", b"base", &key);
    state.apply(&delete, &[]).unwrap();

    assert_eq!(state.metrics().ai_lines, 0);
    assert_eq!(state.metrics().total_lines, 1);
}

#[test]
fn edit_script_for_a_different_base_is_rejected() {
    let key = FingerprintKey::new([6; 32]);
    let mut state = FileProvenance::legacy(b"one\ntwo", &key);
    let edit = diff_files(b"one", b"one\nnew", &key);

    assert!(
        state
            .apply(&edit, &[HunkAttribution::unattributed(0)])
            .is_err()
    );
}

#[test]
fn inconsistent_new_line_position_is_rejected() {
    let key = FingerprintKey::new([6; 32]);
    let mut state = FileProvenance::legacy(b"one", &key);
    let mut edit = diff_files(b"one", b"one\nnew", &key);
    edit.hunks[0].new_start = 99;

    assert!(
        state
            .apply(&edit, &[HunkAttribution::unattributed(0)])
            .is_err()
    );
}
