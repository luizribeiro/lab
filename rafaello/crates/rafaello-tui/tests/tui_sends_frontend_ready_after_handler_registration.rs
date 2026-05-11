//! c25 — handler-then-ready ordering check.
//!
//! The parent's `FrontendReadyService`-equivalent registers a callback that,
//! the moment `frontend.ready` arrives, immediately publishes a `bus.event`
//! back to the child. If the child's `BusEventHandler` was wired BEFORE the
//! ready RPC was sent, the bus.event will land on the child's handler and
//! show up in the stderr log.

mod common;

use std::time::Duration;

use common::{
    publish_bus_event, spawn_tui, wait_for_stderr_line, OnReadyService, RecordingService, SpawnOpts,
};

#[tokio::test(flavor = "multi_thread")]
async fn child_logs_event_published_during_frontend_ready_callback() {
    let (recorder, _events) = RecordingService::new();
    let svc = OnReadyService::new(recorder, |peer| {
        publish_bus_event(peer, "core.lifecycle.handshake_probe");
    });

    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(3),
            ready_delay_ms: None,
            test_message: None,
            test_confirm_answer: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        svc,
    );

    wait_for_stderr_line(
        &mut h.stderr_lines,
        "bus.event topic=core.lifecycle.handshake_probe seq=1",
        Duration::from_secs(3),
    )
    .await;
}
