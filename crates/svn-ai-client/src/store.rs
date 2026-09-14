use std::{cell::RefCell, path::PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use svn_ai_protocol::{AttributionEvent, Digest, RepoPath, ToolKind};
use thiserror::Error;

const DATABASE_FILE: &str = "events.sqlite3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRecord {
    pub session_digest: Digest,
    pub tool_digest: Digest,
    pub before_content: Vec<u8>,
    pub repository_uuid: String,
    pub repository_root_digest: Digest,
    pub base_revision: i64,
    pub path: RepoPath,
    pub file_path: PathBuf,
    pub tool: ToolKind,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoreStatus {
    pub pending_transactions: u64,
    pub attribution_events: u64,
}

pub struct EventStore {
    connection: RefCell<Connection>,
}

impl EventStore {
    pub fn open(home: &std::path::Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(home).map_err(|error| StoreError::Io(error.to_string()))?;
        let connection = Connection::open(home.join(DATABASE_FILE))?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS transactions (
                 session_digest TEXT NOT NULL,
                 tool_digest TEXT NOT NULL,
                 before_content BLOB NOT NULL,
                 repository_uuid TEXT NOT NULL,
                 repository_root_digest TEXT NOT NULL,
                 base_revision INTEGER NOT NULL,
                 repo_path TEXT NOT NULL,
                 file_path TEXT NOT NULL,
                 tool_json TEXT NOT NULL,
                 occurred_at TEXT NOT NULL,
                 PRIMARY KEY (session_digest, tool_digest)
             );
             CREATE TABLE IF NOT EXISTS attribution_events (
                 event_id TEXT PRIMARY KEY,
                 event_json TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );",
        )?;
        Ok(Self {
            connection: RefCell::new(connection),
        })
    }

    pub fn begin_transaction(&self, record: &TransactionRecord) -> Result<(), StoreError> {
        let tool_json = serde_json::to_string(&record.tool)?;
        self.connection.borrow().execute(
            "INSERT OR REPLACE INTO transactions (
                session_digest, tool_digest, before_content, repository_uuid,
                repository_root_digest, base_revision, repo_path, file_path,
                tool_json, occurred_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.session_digest.to_string(),
                record.tool_digest.to_string(),
                record.before_content,
                record.repository_uuid,
                record.repository_root_digest.to_string(),
                record.base_revision,
                record.path.as_str(),
                record.file_path.to_string_lossy(),
                tool_json,
                record.occurred_at,
            ],
        )?;
        Ok(())
    }

    pub fn load_transaction(
        &self,
        session_digest: &Digest,
        tool_digest: &Digest,
    ) -> Result<Option<TransactionRecord>, StoreError> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare(
            "SELECT before_content, repository_uuid, repository_root_digest,
                    base_revision, repo_path, file_path, tool_json, occurred_at
             FROM transactions
             WHERE session_digest = ?1 AND tool_digest = ?2",
        )?;
        let row = statement
            .query_row(
                params![session_digest.to_string(), tool_digest.to_string()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        row.map(
            |(
                before_content,
                repository_uuid,
                repository_root_digest,
                base_revision,
                repo_path,
                file_path,
                tool_json,
                occurred_at,
            )| {
                Ok(TransactionRecord {
                    session_digest: *session_digest,
                    tool_digest: *tool_digest,
                    before_content,
                    repository_uuid,
                    repository_root_digest: Digest::from_hex(&repository_root_digest)
                        .map_err(|error| StoreError::Data(error.to_string()))?,
                    base_revision,
                    path: RepoPath::try_from(repo_path)
                        .map_err(|error| StoreError::Data(error.to_string()))?,
                    file_path: PathBuf::from(file_path),
                    tool: serde_json::from_str(&tool_json)?,
                    occurred_at,
                })
            },
        )
        .transpose()
    }

    pub fn finish_transaction(
        &self,
        session_digest: &Digest,
        tool_digest: &Digest,
        event: &AttributionEvent,
    ) -> Result<(), StoreError> {
        let event_json = serde_json::to_string(event)?;
        let mut connection = self.connection.borrow_mut();
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT OR IGNORE INTO attribution_events (event_id, event_json, created_at)
             VALUES (?1, ?2, ?3)",
            params![event.event_id, event_json, event.occurred_at],
        )?;
        transaction.execute(
            "DELETE FROM transactions WHERE session_digest = ?1 AND tool_digest = ?2",
            params![session_digest.to_string(), tool_digest.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn cancel_transaction(
        &self,
        session_digest: &Digest,
        tool_digest: &Digest,
    ) -> Result<(), StoreError> {
        self.connection.borrow().execute(
            "DELETE FROM transactions WHERE session_digest = ?1 AND tool_digest = ?2",
            params![session_digest.to_string(), tool_digest.to_string()],
        )?;
        Ok(())
    }

    pub fn list_events(&self) -> Result<Vec<AttributionEvent>, StoreError> {
        let connection = self.connection.borrow();
        let mut statement =
            connection.prepare("SELECT event_json FROM attribution_events ORDER BY rowid ASC")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            let json = row?;
            serde_json::from_str(&json).map_err(StoreError::from)
        })
        .collect()
    }

    pub fn status(&self) -> Result<StoreStatus, StoreError> {
        let connection = self.connection.borrow();
        let pending_transactions =
            connection.query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?;
        let attribution_events =
            connection.query_row("SELECT COUNT(*) FROM attribution_events", [], |row| {
                row.get(0)
            })?;
        Ok(StoreStatus {
            pending_transactions,
            attribution_events,
        })
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("event store I/O failed: {0}")]
    Io(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("event store contains invalid data: {0}")]
    Data(String),
}
