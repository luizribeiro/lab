//! Audit log: passive SQLite sink (scope §AL1-§AL3, decision A6).
//!
//! The `audit_events` table is created at session-store open and written
//! to via a single in-process `AuditWriter` sharing m3's session-store
//! SQLite connection. No bus topic — readers (m6's `rfl audit`) go
//! straight to SQLite.

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditKind {
    GatePassthrough,
    GateGrantMatch,
    GateGrantMatchShortCircuit,
    ConfirmRequest,
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
    TrifectaOverridden,
    CredentialPathsOverridden,
}

impl AuditKind {
    pub fn as_str(&self) -> &'static str {
        use AuditKind::*;
        match self {
            GatePassthrough => "gate_passthrough",
            GateGrantMatch => "gate_grant_match",
            GateGrantMatchShortCircuit => "gate_grant_match_short_circuit",
            ConfirmRequest => "confirm_request",
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
            TrifectaOverridden => "trifecta_overridden",
            CredentialPathsOverridden => "credential_paths_overridden",
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
