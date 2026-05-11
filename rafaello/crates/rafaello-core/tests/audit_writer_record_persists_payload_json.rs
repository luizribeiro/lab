//! c08 — `AuditWriter::record` persists the JSON payload verbatim;
//! a raw SQLite read round-trips the value.

use std::sync::Arc;

use rafaello_core::audit::AuditKind;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};

mod common;
use common::session_test_kit::in_memory_broker_with_tui_and_observer_acl;

#[test]
fn audit_writer_record_persists_payload_json() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let kit = in_memory_broker_with_tui_and_observer_acl();
    let controller = SessionController::new(store, pipeline, kit.broker.clone());
    let writer = controller.audit_writer();

    let payload = serde_json::json!({"foo": "bar"});
    let seq = writer
        .record(AuditKind::GrantAdded, None, &payload)
        .expect("record");

    let conn =
        rusqlite::Connection::open(tmp.path().join("session.sqlite")).expect("readback connection");
    let stored: String = conn
        .query_row(
            "SELECT payload FROM audit_events WHERE seq = ?1",
            [seq],
            |row| row.get(0),
        )
        .expect("payload row");
    let round_tripped: serde_json::Value = serde_json::from_str(&stored).expect("payload parses");
    assert_eq!(round_tripped, payload);
}
