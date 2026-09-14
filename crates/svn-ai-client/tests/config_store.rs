use std::path::PathBuf;

use svn_ai::{ClientConfig, ConfigError, EventStore, FingerprintSecret, TransactionRecord};
use svn_ai_protocol::{AttributionEvent, AttributionHunk, Digest, RepoPath, ToolKind};
use tempfile::tempdir;

fn digest(value: u8) -> Digest {
    Digest::new([value; 32])
}

fn transaction() -> TransactionRecord {
    TransactionRecord {
        session_digest: digest(1),
        tool_digest: digest(2),
        before_content: b"secret source before edit\n".to_vec(),
        repository_uuid: "repo-uuid".to_owned(),
        repository_root_digest: digest(3),
        base_revision: 7,
        path: RepoPath::try_from("trunk/src/main.rs").unwrap(),
        file_path: PathBuf::from("C:/work/project/src/main.rs"),
        tool: ToolKind::Edit,
        occurred_at: "2026-09-14T09:00:00+08:00".to_owned(),
    }
}

fn event(event_id: &str) -> AttributionEvent {
    AttributionEvent {
        schema_version: "1.0.0".to_owned(),
        event_id: event_id.to_owned(),
        device_id: "device-1".to_owned(),
        client_version: "0.1.0".to_owned(),
        svn_username: "zhengjie".to_owned(),
        repository_uuid: "repo-uuid".to_owned(),
        repository_root_digest: digest(3),
        base_revision: 7,
        session_id_digest: digest(1),
        tool_use_id_digest: digest(2),
        tool: ToolKind::Edit,
        occurred_at: "2026-09-14T09:00:01+08:00".to_owned(),
        model_id: None,
        path: RepoPath::try_from("trunk/src/main.rs").unwrap(),
        hunks: vec![AttributionHunk {
            old_start: 0,
            old_len: 0,
            new_start: 0,
            new_line_digests: vec![digest(4)],
            new_context_digests: vec![digest(5)],
        }],
    }
}

#[test]
fn configuration_round_trips_without_changing_the_key() {
    let home = tempdir().unwrap();
    let config = ClientConfig {
        device_id: "device-1".to_owned(),
        svn_username: "zhengjie".to_owned(),
        fingerprint_secret: FingerprintSecret::parse(
            "0707070707070707070707070707070707070707070707070707070707070707",
        )
        .unwrap(),
        svn_executable: PathBuf::from("C:/Program Files/Subversion/bin/svn.exe"),
    };

    config.save(home.path()).unwrap();
    let loaded = ClientConfig::load(home.path()).unwrap();

    assert_eq!(loaded, config);
    assert_eq!(
        loaded.fingerprint_secret.to_hex(),
        "0707070707070707070707070707070707070707070707070707070707070707"
    );
}

#[test]
fn invalid_fingerprint_keys_are_rejected() {
    assert_eq!(
        FingerprintSecret::parse("abcd").unwrap_err(),
        ConfigError::InvalidFingerprintSecret
    );
    assert_eq!(
        FingerprintSecret::parse(
            "zz07070707070707070707070707070707070707070707070707070707070707",
        )
        .unwrap_err(),
        ConfigError::InvalidFingerprintSecret
    );
}

#[test]
fn pending_transactions_survive_reopening_the_database() {
    let home = tempdir().unwrap();
    let record = transaction();
    EventStore::open(home.path())
        .unwrap()
        .begin_transaction(&record)
        .unwrap();

    let reopened = EventStore::open(home.path()).unwrap();

    assert_eq!(
        reopened
            .load_transaction(&record.session_digest, &record.tool_digest)
            .unwrap(),
        Some(record)
    );
    assert_eq!(reopened.status().unwrap().pending_transactions, 1);
}

#[test]
fn finishing_atomically_removes_source_snapshot_and_inserts_event() {
    let home = tempdir().unwrap();
    let store = EventStore::open(home.path()).unwrap();
    let record = transaction();
    let attribution = event("event-1");
    store.begin_transaction(&record).unwrap();

    store
        .finish_transaction(&record.session_digest, &record.tool_digest, &attribution)
        .unwrap();

    assert_eq!(
        store
            .load_transaction(&record.session_digest, &record.tool_digest)
            .unwrap(),
        None
    );
    assert_eq!(store.list_events().unwrap(), vec![attribution]);
    let status = store.status().unwrap();
    assert_eq!(status.pending_transactions, 0);
    assert_eq!(status.attribution_events, 1);
}

#[test]
fn duplicate_event_ids_are_idempotent() {
    let home = tempdir().unwrap();
    let store = EventStore::open(home.path()).unwrap();
    let first = transaction();
    let mut second = transaction();
    second.tool_digest = digest(9);
    let attribution = event("same-event");
    store.begin_transaction(&first).unwrap();
    store.begin_transaction(&second).unwrap();

    store
        .finish_transaction(&first.session_digest, &first.tool_digest, &attribution)
        .unwrap();
    store
        .finish_transaction(&second.session_digest, &second.tool_digest, &attribution)
        .unwrap();

    assert_eq!(store.list_events().unwrap().len(), 1);
    assert_eq!(store.status().unwrap().pending_transactions, 0);
}

#[test]
fn cancelling_a_transaction_deletes_its_source_snapshot() {
    let home = tempdir().unwrap();
    let store = EventStore::open(home.path()).unwrap();
    let record = transaction();
    store.begin_transaction(&record).unwrap();

    store
        .cancel_transaction(&record.session_digest, &record.tool_digest)
        .unwrap();

    assert_eq!(store.status().unwrap().pending_transactions, 0);
    assert!(store.list_events().unwrap().is_empty());
}
