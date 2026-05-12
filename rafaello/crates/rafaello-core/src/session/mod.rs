//! Session storage: per-session SQLite store with a Flock-protected lockfile
//! (scope §S1, §S2, §S3, §S5).
//!
//! `SessionStore::open` enforces lock-first ordering: the directory is
//! created, the lockfile opened with `O_CLOEXEC`, an exclusive `flock` is
//! acquired, the holder pid is written, and only then is the SQLite file
//! opened and the schema initialized.

// Module-level result_large_err allow ratified by
// m6 per decisions.md row 67 — boxing the error
// hierarchy is post-v1.
#![allow(clippy::result_large_err)]

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use nix::fcntl::{Flock, FlockArg};
use parking_lot::Mutex;
use rusqlite::Connection;
use thiserror::Error;
use ulid::Ulid;

use crate::audit::AuditWriter;
use crate::bus::Broker;
use crate::entry::Entry;
use crate::error::BrokerError;
use crate::renderer::{Capabilities, RenderPipeline};

const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SessionError {
    #[error("session io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("session serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("session locked by another process (holder pid: {holder_pid:?})")]
    Locked { holder_pid: Option<u32> },
    #[error("session schema mismatch: expected {expected}, found {found}")]
    SchemaMismatch { expected: String, found: String },
    #[error("session publish failed: {source}")]
    Publish {
        #[from]
        source: BrokerError,
    },
}

#[derive(Debug, Clone)]
pub struct StoredEntry {
    pub seq: u64,
    pub entry: Entry,
}

pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
    #[allow(dead_code)]
    lock_guard: Flock<File>,
    session_id: String,
}

impl SessionStore {
    pub fn open(state_dir: &Path) -> Result<Self, SessionError> {
        std::fs::create_dir_all(state_dir)?;

        let lock_path = state_dir.join("session.lock");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .custom_flags(nix::libc::O_CLOEXEC)
            .open(&lock_path)?;

        let mut lock_guard = match Flock::lock(lock_file, FlockArg::LockExclusiveNonblock) {
            Ok(g) => g,
            Err((file, errno)) => {
                if errno == nix::errno::Errno::EWOULDBLOCK {
                    let holder_pid = read_holder_pid(file);
                    return Err(SessionError::Locked { holder_pid });
                }
                return Err(SessionError::Io(std::io::Error::from_raw_os_error(
                    errno as i32,
                )));
            }
        };

        write_holder_pid(&mut lock_guard, std::process::id())?;

        let db_path = state_dir.join("session.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS entries (
                id          TEXT PRIMARY KEY,
                seq         INTEGER NOT NULL UNIQUE,
                parent      TEXT,
                kind        TEXT NOT NULL,
                schema      TEXT,
                payload     TEXT NOT NULL,
                metadata    TEXT NOT NULL,
                fallback    TEXT,
                created_at  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS session_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audit_events (
                seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                at         TEXT NOT NULL,
                kind       TEXT NOT NULL,
                request_id TEXT,
                payload    TEXT NOT NULL
            );
            "#,
        )?;

        let session_id = init_or_verify_meta(&conn)?;

        Ok(SessionStore {
            conn: Arc::new(Mutex::new(conn)),
            lock_guard,
            session_id,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[allow(dead_code)]
    pub(crate) fn conn(&self) -> &Mutex<Connection> {
        &self.conn
    }

    pub(crate) fn conn_arc(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    pub fn append_entry(&self, entry: &Entry) -> Result<u64, SessionError> {
        let id = entry.id.to_string();
        let parent = entry.parent.as_ref().map(|p| p.to_string());
        let schema = entry.schema.clone();
        let payload = serde_json::to_string(&entry.payload)?;
        let metadata = serde_json::to_string(&entry.metadata)?;
        let fallback = entry
            .fallback
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let created_at = entry.metadata.created_at.to_rfc3339();

        let conn = self.conn.lock();
        let seq: i64 = conn.query_row(
            "INSERT INTO entries (id, seq, parent, kind, schema, payload, metadata, fallback, created_at) \
             VALUES (?1, (SELECT COALESCE(MAX(seq), -1) + 1 FROM entries), ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             RETURNING seq",
            rusqlite::params![
                id,
                parent,
                entry.kind,
                schema,
                payload,
                metadata,
                fallback,
                created_at,
            ],
            |row| row.get(0),
        )?;
        Ok(seq as u64)
    }

    pub fn load_entries(&self) -> Result<Vec<StoredEntry>, SessionError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq, id, parent, kind, schema, payload, metadata, fallback \
             FROM entries ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let seq: i64 = row.get(0)?;
            let id: String = row.get(1)?;
            let parent: Option<String> = row.get(2)?;
            let kind: String = row.get(3)?;
            let schema: Option<String> = row.get(4)?;
            let payload: String = row.get(5)?;
            let metadata: String = row.get(6)?;
            let fallback: Option<String> = row.get(7)?;
            Ok((seq, id, parent, kind, schema, payload, metadata, fallback))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (seq, id, parent, kind, schema, payload, metadata, fallback) = row?;
            let id = Ulid::from_string(&id).map_err(|e| {
                SessionError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )),
                ))
            })?;
            let parent = match parent {
                Some(p) => Some(Ulid::from_string(&p).map_err(|e| {
                    SessionError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            e.to_string(),
                        )),
                    ))
                })?),
                None => None,
            };
            let payload: serde_json::Value = serde_json::from_str(&payload)?;
            let metadata: crate::entry::EntryMetadata = serde_json::from_str(&metadata)?;
            let fallback = match fallback {
                Some(s) => Some(serde_json::from_str(&s)?),
                None => None,
            };
            out.push(StoredEntry {
                seq: seq as u64,
                entry: Entry {
                    id,
                    parent,
                    kind,
                    schema,
                    payload,
                    metadata,
                    fallback,
                },
            });
        }
        Ok(out)
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn lock_fd_for_test(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        let file: &File = &self.lock_guard;
        file.as_raw_fd()
    }
}

fn read_holder_pid(mut file: File) -> Option<u32> {
    let mut buf = String::new();
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_to_string(&mut buf).ok()?;
    buf.trim().parse().ok()
}

fn write_holder_pid(guard: &mut Flock<File>, pid: u32) -> std::io::Result<()> {
    let file: &mut File = guard;
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    writeln!(file, "{}", pid)?;
    file.flush()?;
    Ok(())
}

pub struct SessionController {
    store: SessionStore,
    pipeline: RenderPipeline,
    broker: Broker,
    audit_writer: OnceLock<Arc<AuditWriter>>,
}

impl SessionController {
    pub fn new(store: SessionStore, pipeline: RenderPipeline, broker: Broker) -> Self {
        Self {
            store,
            pipeline,
            broker,
            audit_writer: OnceLock::new(),
        }
    }

    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    pub fn audit_writer(&self) -> Arc<AuditWriter> {
        Arc::clone(
            self.audit_writer
                .get_or_init(|| Arc::new(AuditWriter::new(self.store.conn_arc()))),
        )
    }

    pub async fn finalize_entry(
        &self,
        entry: Entry,
        caps: &Capabilities,
    ) -> Result<(), SessionError> {
        let seq = self.store.append_entry(&entry)?;
        let tree = self.pipeline.render(&entry, caps);
        self.broker.publish_core(
            "core.session.entry.finalized",
            serde_json::json!({
                "entry": entry,
                "tree": tree,
                "seq": seq,
                "replay": false,
            }),
        )?;
        Ok(())
    }

    pub async fn replay_history(&self, caps: &Capabilities) -> Result<(), SessionError> {
        for stored in self.store.load_entries()? {
            let tree = self.pipeline.render(&stored.entry, caps);
            self.broker.publish_core(
                "core.session.entry.finalized",
                serde_json::json!({
                    "entry": stored.entry,
                    "tree": tree,
                    "seq": stored.seq,
                    "replay": true,
                }),
            )?;
        }
        Ok(())
    }
}

fn init_or_verify_meta(conn: &Connection) -> Result<String, SessionError> {
    let existing_version: Option<String> = conn
        .query_row(
            "SELECT value FROM session_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(found) = existing_version {
        if found != SCHEMA_VERSION {
            return Err(SessionError::SchemaMismatch {
                expected: SCHEMA_VERSION.to_string(),
                found,
            });
        }
        let session_id: String = conn.query_row(
            "SELECT value FROM session_meta WHERE key = 'session_id'",
            [],
            |row| row.get(0),
        )?;
        Ok(session_id)
    } else {
        let session_id = Ulid::new().to_string();
        conn.execute(
            "INSERT INTO session_meta (key, value) VALUES ('session_id', ?1)",
            [&session_id],
        )?;
        conn.execute(
            "INSERT INTO session_meta (key, value) VALUES ('schema_version', ?1)",
            [SCHEMA_VERSION],
        )?;
        Ok(session_id)
    }
}
