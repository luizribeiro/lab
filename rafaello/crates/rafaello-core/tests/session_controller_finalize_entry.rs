//! c23 — `SessionController::finalize_entry` appends the entry to the
//! SQLite store and publishes one `core.session.entry.finalized`
//! event with `replay: false` (scope §S1).

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::renderer::{Capabilities, RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};

mod common;
use common::session_test_kit::in_memory_broker_with_tui_and_observer_acl;

#[tokio::test(flavor = "multi_thread")]
async fn finalize_entry_persists_and_publishes_one_event() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let mut kit = in_memory_broker_with_tui_and_observer_acl();
    let controller = SessionController::new(store, pipeline, kit.broker.clone());

    let entry = Entry::new_text("hello");
    let entry_id = entry.id;
    let caps = Capabilities::tui_default();
    controller
        .finalize_entry(entry, &caps)
        .await
        .expect("finalize_entry succeeds");

    let stored = controller.store().load_entries().expect("load_entries");
    assert_eq!(stored.len(), 1, "exactly one row persisted");
    assert_eq!(stored[0].seq, 0);
    assert_eq!(stored[0].entry.id, entry_id);

    let notification = kit
        .observer_rx
        .try_recv()
        .expect("observer receives one bus.event");
    assert_eq!(notification.method, "bus.event");
    let event = &notification.params;
    assert_eq!(event["topic"], "core.session.entry.finalized");
    assert_eq!(event["payload"]["seq"], 0);
    assert_eq!(event["payload"]["replay"], false);
    assert_eq!(event["payload"]["entry"]["id"], entry_id.to_string());

    assert!(
        kit.observer_rx.try_recv().is_err(),
        "exactly one event from a single finalize_entry"
    );
}
