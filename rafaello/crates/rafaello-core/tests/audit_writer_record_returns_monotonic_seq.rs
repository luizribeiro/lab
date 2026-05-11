//! c08 — `AuditWriter::record` returns 1, 2, 3 on insertion in order
//! (scope §AL4 `seq_monotonic_per_session`).

use std::sync::Arc;

use rafaello_core::audit::AuditKind;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};

mod common;
use common::session_test_kit::in_memory_broker_with_tui_and_observer_acl;

#[test]
fn audit_writer_record_returns_monotonic_seq() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let kit = in_memory_broker_with_tui_and_observer_acl();
    let controller = SessionController::new(store, pipeline, kit.broker.clone());
    let writer = controller.audit_writer();

    let a = writer
        .record(
            AuditKind::ConfirmRequest,
            None,
            &serde_json::json!({"n": 1}),
        )
        .expect("record 1");
    let b = writer
        .record(
            AuditKind::ConfirmAllowed,
            None,
            &serde_json::json!({"n": 2}),
        )
        .expect("record 2");
    let c = writer
        .record(AuditKind::ConfirmDenied, None, &serde_json::json!({"n": 3}))
        .expect("record 3");

    assert_eq!((a, b, c), (1, 2, 3));
}
