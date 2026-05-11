//! c08 — `AuditWriter::record` persists `request_id` for both
//! `Some(id)` and `None` (scope §AL1 — `request_id` nullable).

use std::sync::Arc;

use fittings_core::message::JsonRpcId;
use rafaello_core::audit::AuditKind;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};

mod common;
use common::session_test_kit::in_memory_broker_with_tui_and_observer_acl;

#[test]
fn audit_writer_record_persists_request_id_optional() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let kit = in_memory_broker_with_tui_and_observer_acl();
    let controller = SessionController::new(store, pipeline, kit.broker.clone());
    let writer = controller.audit_writer();

    let id = JsonRpcId::String("req-42".to_string());
    let seq_some = writer
        .record(AuditKind::ConfirmRequest, Some(&id), &serde_json::json!({}))
        .expect("record some");
    let seq_none = writer
        .record(AuditKind::ConfirmTimeout, None, &serde_json::json!({}))
        .expect("record none");

    let conn =
        rusqlite::Connection::open(tmp.path().join("session.sqlite")).expect("readback connection");
    let some_val: Option<String> = conn
        .query_row(
            "SELECT request_id FROM audit_events WHERE seq = ?1",
            [seq_some],
            |row| row.get(0),
        )
        .expect("some row");
    let none_val: Option<String> = conn
        .query_row(
            "SELECT request_id FROM audit_events WHERE seq = ?1",
            [seq_none],
            |row| row.get(0),
        )
        .expect("none row");

    assert_eq!(some_val.as_deref(), Some("req-42"));
    assert_eq!(none_val, None);
}
