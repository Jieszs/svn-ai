use svn_ai_core::{FingerprintKey, fingerprint_lines, split_lines};

#[test]
fn crlf_and_lf_produce_identical_line_digests() {
    let key = FingerprintKey::new([7; 32]);
    let lf = fingerprint_lines(b"alpha\nbeta\n", &key);
    let crlf = fingerprint_lines(b"alpha\r\nbeta\r\n", &key);

    assert_eq!(lf.line_digests, crlf.line_digests);
    assert_eq!(lf.context_digests, crlf.context_digests);
}

#[test]
fn indentation_and_key_changes_are_significant() {
    let key_a = FingerprintKey::new([1; 32]);
    let key_b = FingerprintKey::new([2; 32]);

    assert_ne!(
        fingerprint_lines(b"x", &key_a).line_digests,
        fingerprint_lines(b" x", &key_a).line_digests
    );
    assert_ne!(
        fingerprint_lines(b"x", &key_a).line_digests,
        fingerprint_lines(b"x", &key_b).line_digests
    );
}

#[test]
fn arbitrary_bytes_are_supported_without_serializing_source_text() {
    let key = FingerprintKey::new([9; 32]);
    let file = fingerprint_lines(&[0xff, b'\n', 0xfe], &key);

    assert_eq!(file.line_digests.len(), 2);
    let serialized = serde_json::to_string(&file).unwrap();
    assert!(!serialized.contains('\u{fffd}'));
}

#[test]
fn context_disambiguates_identical_lines() {
    let key = FingerprintKey::new([3; 32]);
    let file = fingerprint_lines(b"before\nsame\nafter\nsame\nend", &key);

    assert_eq!(file.line_digests[1], file.line_digests[3]);
    assert_ne!(file.context_digests[1], file.context_digests[3]);
}

#[test]
fn splitting_does_not_create_a_phantom_line_after_a_trailing_newline() {
    assert_eq!(split_lines(b""), Vec::<&[u8]>::new());
    assert_eq!(split_lines(b"alpha\n"), vec![b"alpha".as_slice()]);
    assert_eq!(
        split_lines(b"alpha\nbeta"),
        vec![b"alpha".as_slice(), b"beta".as_slice()]
    );
}

#[test]
fn serialized_fingerprints_do_not_contain_source_lines() {
    let key = FingerprintKey::new([10; 32]);
    let file = fingerprint_lines(b"bank_secret_literal", &key);

    let serialized = serde_json::to_string(&file).unwrap();
    assert!(!serialized.contains("bank_secret_literal"));
}
