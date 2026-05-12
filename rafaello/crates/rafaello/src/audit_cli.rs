//! `rfl audit` subcommand (scope §D1).
//!
//! Reads `${PROJECT_ROOT}/.rafaello/state/session.sqlite` in read-only
//! mode and renders the `audit_events` table one row per line. Filter
//! flags (`--kind`, `--request-id`, …) land in c12.

use std::io::Write;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};

#[derive(Debug, clap::Args)]
pub struct AuditArgs {
    #[arg(long)]
    pub project_root: Option<PathBuf>,
    // Filter flags land in c12.
}

#[derive(Debug, thiserror::Error)]
pub enum AuditCliError {
    #[error("io error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sqlite error on {path:?}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
}

const PAYLOAD_SUMMARY_CHARS: usize = 80;

pub fn run(args: AuditArgs) -> Result<(), AuditCliError> {
    let project_root = match args.project_root {
        Some(p) => p,
        None => std::env::current_dir().map_err(|source| AuditCliError::Io {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let db_path = project_root
        .join(".rafaello")
        .join("state")
        .join("session.sqlite");

    if !db_path.exists() {
        emit_empty_banner();
        return Ok(());
    }

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
        |source| AuditCliError::Sqlite {
            path: db_path.clone(),
            source,
        },
    )?;
    let mut stmt = conn
        .prepare(
            "SELECT seq, at, kind, request_id, payload \
             FROM audit_events ORDER BY seq ASC",
        )
        .map_err(|source| AuditCliError::Sqlite {
            path: db_path.clone(),
            source,
        })?;
    let mut rows = stmt.query([]).map_err(|source| AuditCliError::Sqlite {
        path: db_path.clone(),
        source,
    })?;

    let mut count = 0usize;
    let mut stdout = std::io::stdout().lock();
    while let Some(row) = rows.next().map_err(|source| AuditCliError::Sqlite {
        path: db_path.clone(),
        source,
    })? {
        let seq: i64 = row_get(row, 0, &db_path)?;
        let at: String = row_get(row, 1, &db_path)?;
        let kind: String = row_get(row, 2, &db_path)?;
        let request_id: Option<String> = row_get(row, 3, &db_path)?;
        let payload: String = row_get(row, 4, &db_path)?;
        let summary = truncate_chars(&payload, PAYLOAD_SUMMARY_CHARS);
        let rid = request_id.as_deref().unwrap_or("-");
        let _ = writeln!(stdout, "{seq}  {at}  {kind}  [{rid}]  {summary}");
        count += 1;
    }
    let _ = stdout.flush();

    if count == 0 {
        emit_empty_banner();
    }
    Ok(())
}

fn row_get<T: rusqlite::types::FromSql>(
    row: &rusqlite::Row<'_>,
    idx: usize,
    db_path: &std::path::Path,
) -> Result<T, AuditCliError> {
    row.get(idx).map_err(|source| AuditCliError::Sqlite {
        path: db_path.to_path_buf(),
        source,
    })
}

fn emit_empty_banner() {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "no audit events");
    let _ = stderr.flush();
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
