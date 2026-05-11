//! c25 — every received `bus.event` produces a `bus.event topic=... seq=N`
//! sentinel line on the headless TUI's stderr.

mod common;

use std::time::Duration;

use common::{
    publish_bus_event, spawn_tui, wait_for_method, wait_for_stderr_line, RecordingService,
    SpawnOpts,
};

#[tokio::test(flavor = "multi_thread")]
async fn bus_event_to_child_appears_on_stderr() {
    let (svc, mut events) = RecordingService::new();
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

    wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;

    publish_bus_event(&h.parent_peer, "core.lifecycle.demo");

    let line = wait_for_stderr_line(
        &mut h.stderr_lines,
        "bus.event topic=core.lifecycle.demo seq=1",
        Duration::from_secs(3),
    )
    .await;
    assert!(line.contains("seq=1"), "unexpected line: {line}");
}
