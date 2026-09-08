use svn_ai_core::{FingerprintKey, diff_files};

#[test]
fn edit_script_reports_an_insert_range() {
    let key = FingerprintKey::new([4; 32]);
    let script = diff_files(b"a\nc", b"a\nb\nc", &key);

    assert_eq!(script.hunks.len(), 1);
    assert_eq!((script.hunks[0].old_start, script.hunks[0].old_len), (1, 0));
    assert_eq!(
        (script.hunks[0].new_start, script.hunks[0].new_len()),
        (1, 1)
    );
}

#[test]
fn edit_script_reports_a_delete_range() {
    let key = FingerprintKey::new([4; 32]);
    let script = diff_files(b"a\nb\nc", b"a\nc", &key);

    assert_eq!(script.hunks.len(), 1);
    assert_eq!((script.hunks[0].old_start, script.hunks[0].old_len), (1, 1));
    assert_eq!(script.hunks[0].new_len(), 0);
}

#[test]
fn edit_script_reports_a_replace_range() {
    let key = FingerprintKey::new([4; 32]);
    let script = diff_files(b"a\nb\nc", b"a\nx\nc", &key);

    assert_eq!(script.hunks.len(), 1);
    assert_eq!((script.hunks[0].old_start, script.hunks[0].old_len), (1, 1));
    assert_eq!(
        (script.hunks[0].new_start, script.hunks[0].new_len()),
        (1, 1)
    );
}

#[test]
fn repeated_lines_produce_stable_edit_scripts() {
    let key = FingerprintKey::new([5; 32]);
    let before = b"same\nleft\nsame\nright";
    let after = b"same\nchanged\nsame\nright";

    let first = diff_files(before, after, &key);
    let second = diff_files(before, after, &key);

    assert_eq!(first, second);
    assert_eq!(first.hunks.len(), 1);
    assert_eq!((first.hunks[0].old_start, first.hunks[0].old_len), (1, 1));
}

#[test]
fn newline_style_only_change_is_not_a_code_edit() {
    let key = FingerprintKey::new([6; 32]);
    let script = diff_files(b"a\r\nb\r\n", b"a\nb\n", &key);

    assert!(script.hunks.is_empty());
    assert_eq!(script.before_line_count, 2);
    assert_eq!(script.after_line_count, 2);
}
