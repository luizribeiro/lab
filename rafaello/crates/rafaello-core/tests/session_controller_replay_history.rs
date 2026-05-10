//! c23 — `SessionController::replay_history` iterates `load_entries`,
//! renders each, and publishes one `core.session.entry.finalized`
//! event per entry with `replay: true` in seq order (scope §S1).

use std::sync::Arc;

use rafaello_core::entry::Entry;
use rafaello_core::renderer::{Capabilities, RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};

mod common;
use common::session_test_kit::in_memory_broker_with_tui_and_observer_acl;

#[tokio::test(flavor = "multi_thread")]
async fn replay_history_emits_one_event_per_stored_entry_in_seq_order() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");

    let entries = [
        Entry::new_text("first"),
        Entry::new_heading(1, "second"),
        Entry::new_text("third"),
    ];
    let ids: Vec<_> = entries.iter().map(|e| e.id).collect();
    for e in &entries {
        store.append_entry(e).expect("seed append");
    }

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let mut kit = in_memory_broker_with_tui_and_observer_acl();
    let controller = SessionController::new(store, pipeline, kit.broker.clone());

    let caps = Capabilities::tui_default();
    controller
        .replay_history(&caps)
        .await
        .expect("replay_history succeeds");

    for (expected_seq, expected_id) in ids.iter().enumerate() {
        let notification = kit
            .observer_rx
            .try_recv()
            .unwrap_or_else(|e| panic!("expected event #{expected_seq}, got {e:?}"));
        assert_eq!(notification.method, "bus.event");
        let event = &notification.params;
        assert_eq!(event["topic"], "core.session.entry.finalized");
        assert_eq!(event["payload"]["seq"], expected_seq as u64);
        assert_eq!(event["payload"]["replay"], true);
        assert_eq!(event["payload"]["entry"]["id"], expected_id.to_string());
    }

    assert!(
        kit.observer_rx.try_recv().is_err(),
        "exactly three events emitted"
    );
}
