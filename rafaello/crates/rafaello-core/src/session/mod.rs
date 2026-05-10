//! Session storage: per-session SQLite store with a Flock-protected lockfile
//! (scope §S1, §S2, §S3, §S5).
//!
//! `SessionStore::open` enforces lock-first ordering: the directory is
//! created, the lockfile opened with `O_CLOEXEC`, an exclusive `flock` is
//! acquired, the holder pid is written, and only then is the SQLite file
//! opened and the schema initialized.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use nix::fcntl::{Flock, FlockArg};
use parking_lot::Mutex;
use rusqlite::Connection;
use thiserror::Error;
use ulid::Ulid;

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
}

pub struct SessionStore {
    conn: Mutex<Connection>,
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
            "#,
        )?;

        let session_id = init_or_verify_meta(&conn)?;

        Ok(SessionStore {
            conn: Mutex::new(conn),
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
