//! `rfl audit` subcommand (scope §D1 + §D2).
//!
//! Reads `${PROJECT_ROOT}/.rafaello/state/session.sqlite` in read-only
//! mode and renders the `audit_events` table one row per line. Filter
//! flags (`--kind`, `--since`, `--request-id`, `--json`, `--full`) land
//! in c12; the query never joins `entries` (live schema has no
//! `call_id` column — scope §"Out of scope" item 10).

use std::io::Write;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags};

#[derive(Debug, clap::Args)]
pub struct AuditArgs {
    #[arg(long)]
    pub project_root: Option<PathBuf>,
    #[arg(long)]
    pub kind: Vec<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub request_id: Option<String>,
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(long, default_value_t = false)]
    pub full: bool,
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
    #[error("unknown audit kind: {kind}; see AuditKind::as_str table")]
    UnknownKind { kind: String },
    #[error("invalid --since spec: {spec}; expected <number><m|h|d> (e.g. 30m, 1h, 24h, 7d)")]
    InvalidSince { spec: String },
}

const PAYLOAD_SUMMARY_CHARS: usize = 80;

/// Static lookup table of valid `AuditKind::as_str()` values
/// (scope §"Glossary": "static lookup table maintained alongside
/// `as_str`"). Mirrors `rafaello_core::audit::AuditKind` — keep in
/// sync when adding kinds.
pub const KIND_STRINGS: &[&str] = &[
    "gate_passthrough",
    "gate_grant_match",
    "gate_grant_match_short_circuit",
    "confirm_request",
    "confirm_request_taint_attached",
    "confirm_allowed",
    "confirm_denied",
    "confirm_allowed_with_session_grant",
    "confirm_timeout",
    "confirm_late",
    "confirm_duplicate",
    "confirm_unknown",
    "confirm_malformed",
    "confirm_resolved_after_timeout",
    "grant_added",
    "grant_revoked",
    "grant_list",
    "slash_unknown",
    "install_refused",
    "install_accepted",
    "plugin_publish_rejected_taint_superset",
    "trifecta_overridden",
    "credential_paths_overridden",
    "tool_request_taint_unioned_from_in_reply_to",
];

fn validate_kind(value: &str) -> Result<(), AuditCliError> {
    if KIND_STRINGS.contains(&value) {
        Ok(())
    } else {
        Err(AuditCliError::UnknownKind {
            kind: value.to_owned(),
        })
    }
}

pub fn parse_since(spec: &str) -> Result<chrono::DateTime<chrono::Utc>, AuditCliError> {
    if spec.len() < 2 {
        return Err(AuditCliError::InvalidSince {
            spec: spec.to_owned(),
        });
    }
    let (num, suffix) = spec.split_at(spec.len() - 1);
    let n: i64 = num.parse().map_err(|_| AuditCliError::InvalidSince {
        spec: spec.to_owned(),
    })?;
    if n < 0 {
        return Err(AuditCliError::InvalidSince {
            spec: spec.to_owned(),
        });
    }
    let dur = match suffix {
        "m" => chrono::Duration::minutes(n),
        "h" => chrono::Duration::hours(n),
        "d" => chrono::Duration::days(n),
        _ => {
            return Err(AuditCliError::InvalidSince {
                spec: spec.to_owned(),
            })
        }
    };
    Ok(chrono::Utc::now() - dur)
}

pub struct BuiltQuery {
    pub sql: String,
    pub params: Vec<rusqlite::types::Value>,
}

pub fn build_query(
    kinds: &[String],
    request_id: Option<&str>,
    since: Option<&chrono::DateTime<chrono::Utc>>,
) -> BuiltQuery {
    let mut sql = String::from("SELECT seq, at, kind, request_id, payload FROM audit_events");
    let mut params: Vec<rusqlite::types::Value> = Vec::new();
    let mut clauses: Vec<String> = Vec::new();
    if !kinds.is_empty() {
        let placeholders: Vec<&str> = kinds.iter().map(|_| "?").collect();
        clauses.push(format!("kind IN ({})", placeholders.join(", ")));
        for k in kinds {
            params.push(rusqlite::types::Value::Text(k.clone()));
        }
    }
    if let Some(rid) = request_id {
        clauses.push("request_id = ?".to_string());
        params.push(rusqlite::types::Value::Text(rid.to_string()));
    }
    if let Some(ts) = since {
        clauses.push("at >= ?".to_string());
        params.push(rusqlite::types::Value::Text(ts.to_rfc3339()));
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY seq ASC");
    BuiltQuery { sql, params }
}

pub fn run(args: AuditArgs) -> Result<(), AuditCliError> {
    for k in &args.kind {
        validate_kind(k)?;
    }
    let since = args.since.as_deref().map(parse_since).transpose()?;

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
    let built = build_query(&args.kind, args.request_id.as_deref(), since.as_ref());
    let mut stmt = conn
        .prepare(&built.sql)
        .map_err(|source| AuditCliError::Sqlite {
            path: db_path.clone(),
            source,
        })?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(built.params.iter()))
        .map_err(|source| AuditCliError::Sqlite {
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
        if args.json {
            let payload_value: serde_json::Value = serde_json::from_str(&payload)
                .unwrap_or_else(|_| serde_json::Value::String(payload.clone()));
            let obj = serde_json::json!({
                "seq": seq,
                "at": at,
                "kind": kind,
                "request_id": request_id,
                "payload": payload_value,
            });
            let _ = writeln!(
                stdout,
                "{}",
                serde_json::to_string(&obj).expect("serialize audit row json")
            );
        } else {
            let summary = if args.full {
                payload.clone()
            } else {
                truncate_chars(&payload, PAYLOAD_SUMMARY_CHARS)
            };
            let rid = request_id.as_deref().unwrap_or("-");
            let _ = writeln!(stdout, "{seq}  {at}  {kind}  [{rid}]  {summary}");
        }
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
