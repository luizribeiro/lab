//! Audit log: passive SQLite sink (scope §AL1-§AL3, decision A6).
//!
//! The `audit_events` table is created at session-store open and written
//! to via a single in-process `AuditWriter` sharing m3's session-store
//! SQLite connection. No bus topic — readers (m6's `rfl audit`) go
//! straight to SQLite.

use std::path::Path;
use std::sync::Arc;

use fittings_core::message::JsonRpcId;
use parking_lot::Mutex;
use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AuditError {
    #[error("audit sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("audit serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("audit io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditKind {
    GatePassthrough,
    GateGrantMatch,
    GateGrantMatchShortCircuit,
    ConfirmRequest,
    /// Emitted by c17 when a confirm-request is dispatched with a
    /// non-empty taint set attached (m5b taint-exfil milestone).
    ConfirmRequestTaintAttached,
    ConfirmAllowed,
    ConfirmDenied,
    ConfirmAllowedWithSessionGrant,
    ConfirmTimeout,
    ConfirmLate,
    ConfirmDuplicate,
    ConfirmUnknown,
    ConfirmMalformed,
    ConfirmResolvedAfterTimeout,
    GrantAdded,
    GrantRevoked,
    GrantList,
    SlashUnknown,
    InstallRefused,
    InstallAccepted,
    /// Emitted by c14 when a plugin publish is rejected because the
    /// outgoing taint set is not a superset of the union of incoming
    /// taints (m5b taint-exfil milestone, §TR4b).
    PluginPublishRejectedTaintSuperset,
    TrifectaOverridden,
    CredentialPathsOverridden,
    /// Emitted by c12 when a tool-request's taint set is constructed
    /// by unioning the taints of the messages referenced via
    /// `in_reply_to` (m5b taint-exfil milestone).
    ToolRequestTaintUnionedFromInReplyTo,
}

impl AuditKind {
    pub fn as_str(&self) -> &'static str {
        use AuditKind::*;
        match self {
            GatePassthrough => "gate_passthrough",
            GateGrantMatch => "gate_grant_match",
            GateGrantMatchShortCircuit => "gate_grant_match_short_circuit",
            ConfirmRequest => "confirm_request",
            ConfirmRequestTaintAttached => "confirm_request_taint_attached",
            ConfirmAllowed => "confirm_allowed",
            ConfirmDenied => "confirm_denied",
            ConfirmAllowedWithSessionGrant => "confirm_allowed_with_session_grant",
            ConfirmTimeout => "confirm_timeout",
            ConfirmLate => "confirm_late",
            ConfirmDuplicate => "confirm_duplicate",
            ConfirmUnknown => "confirm_unknown",
            ConfirmMalformed => "confirm_malformed",
            ConfirmResolvedAfterTimeout => "confirm_resolved_after_timeout",
            GrantAdded => "grant_added",
            GrantRevoked => "grant_revoked",
            GrantList => "grant_list",
            SlashUnknown => "slash_unknown",
            InstallRefused => "install_refused",
            InstallAccepted => "install_accepted",
            PluginPublishRejectedTaintSuperset => "plugin_publish_rejected_taint_superset",
            TrifectaOverridden => "trifecta_overridden",
            CredentialPathsOverridden => "credential_paths_overridden",
            ToolRequestTaintUnionedFromInReplyTo => "tool_request_taint_unioned_from_in_reply_to",
        }
    }
}

pub struct AuditWriter {
    conn: Arc<Mutex<Connection>>,
}

impl AuditWriter {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Install-time constructor: opens (and creates if needed)
    /// `${project_root}/.rafaello/state/session.sqlite` directly,
    /// runs the `audit_events` migration (idempotent), and returns
    /// an `Arc<AuditWriter>`. Used by `rfl install`, which runs
    /// without a `SessionController` (pi-1 M-3, pi-2 M-2).
    pub fn open_for_install(project_root: &Path) -> Result<Arc<Self>, AuditError> {
        let state_dir = project_root.join(".rafaello").join("state");
        std::fs::create_dir_all(&state_dir)?;
        let db_path = state_dir.join("session.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS audit_events (
                seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                at         TEXT NOT NULL,
                kind       TEXT NOT NULL,
                request_id TEXT,
                payload    TEXT NOT NULL
            );
            "#,
        )?;
        Ok(Arc::new(Self::new(Arc::new(Mutex::new(conn)))))
    }

    pub fn record(
        &self,
        kind: AuditKind,
        request_id: Option<&JsonRpcId>,
        payload: &serde_json::Value,
    ) -> Result<i64, AuditError> {
        let at = chrono::Utc::now().to_rfc3339();
        let request_id = request_id.map(|id| id.to_string());
        let payload_str = serde_json::to_string(payload)?;
        let conn = self.conn.lock();
        let seq: i64 = conn.query_row(
            "INSERT INTO audit_events (at, kind, request_id, payload) \
             VALUES (?1, ?2, ?3, ?4) RETURNING seq",
            rusqlite::params![at, kind.as_str(), request_id, payload_str],
            |row| row.get(0),
        )?;
        Ok(seq)
    }
}
