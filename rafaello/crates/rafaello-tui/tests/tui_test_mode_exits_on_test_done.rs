//! c25 — receipt of `core.lifecycle.test_done` ends the headless TUI with
//! exit code 0, ahead of the self-timeout deadline.

mod common;

use std::time::Duration;

use common::{
    expect_clean_exit, publish_bus_event, spawn_tui, wait_for_method, wait_for_stderr_line,
    RecordingService, SpawnOpts,
};

#[tokio::test(flavor = "multi_thread")]
async fn test_done_event_triggers_immediate_clean_exit() {
    let (svc, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(60),
            ready_delay_ms: None,
            test_message: None,
            test_confirm_answer: None,
            test_confirm_answers: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        svc,
    );

    wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;

    publish_bus_event(&h.parent_peer, "core.lifecycle.test_done");

    wait_for_stderr_line(&mut h.stderr_lines, "test-done", Duration::from_secs(2)).await;
    expect_clean_exit(&mut h.child, Duration::from_secs(2)).await;
}
