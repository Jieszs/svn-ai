use svn_ai_protocol::ChangeKind;
use svn_ai_svn::{ChangedParseError, parse_changed};

#[test]
fn parses_content_and_property_status_columns() {
    let output = concat!(
        "A   trunk/new.rs\n",
        "U   trunk/code with spaces.rs\n",
        "_U  trunk/config.properties\n",
        "UU  trunk/both.rs\n",
        "D   trunk/old.rs\n",
        "A   trunk/new-directory/\n",
    );

    let changes = parse_changed(output).expect("valid svnlook output");

    assert_eq!(changes.len(), 6);
    assert_eq!(changes[0].kind, ChangeKind::Add);
    assert!(changes[0].text_changed);
    assert!(!changes[0].properties_changed);
    assert_eq!(changes[0].path.as_str(), "trunk/new.rs");

    assert_eq!(changes[1].kind, ChangeKind::Modify);
    assert!(changes[1].text_changed);
    assert_eq!(changes[1].path.as_str(), "trunk/code with spaces.rs");

    assert_eq!(changes[2].kind, ChangeKind::Modify);
    assert!(!changes[2].text_changed);
    assert!(changes[2].properties_changed);

    assert_eq!(changes[3].kind, ChangeKind::Modify);
    assert!(changes[3].text_changed);
    assert!(changes[3].properties_changed);

    assert_eq!(changes[4].kind, ChangeKind::Delete);
    assert_eq!(changes[5].path.as_str(), "trunk/new-directory");
    assert!(changes[5].is_directory);
}

#[test]
fn parses_copy_from_records_from_subversion_17() {
    let output = concat!(
        "A + trunk/vendors/baker/toast.txt\n",
        "    (from trunk/vendors/baker/bread.txt:r63)\n",
    );

    let changes = parse_changed(output).expect("valid copy record");

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Copy);
    let source = changes[0].copy_from.as_ref().expect("copy source");
    assert_eq!(source.path.as_str(), "trunk/vendors/baker/bread.txt");
    assert_eq!(source.revision, 63);
}

#[test]
fn rejects_copy_marker_without_copy_metadata() {
    let error = parse_changed("A + trunk/copied.rs\n").expect_err("missing copy source");

    assert_eq!(error, ChangedParseError::MissingCopySource { line: 1 });
}

#[test]
fn rejects_malformed_copy_revision() {
    let output = concat!(
        "A + trunk/copied.rs\n",
        "    (from trunk/original.rs:rnot-a-number)\n",
    );

    let error = parse_changed(output).expect_err("invalid copy revision");

    assert_eq!(error, ChangedParseError::InvalidCopySource { line: 2 });
}

#[test]
fn rejects_unknown_status_columns() {
    let error = parse_changed("X   trunk/code.rs\n").expect_err("unknown status");

    assert_eq!(error, ChangedParseError::InvalidStatus { line: 1 });
}
