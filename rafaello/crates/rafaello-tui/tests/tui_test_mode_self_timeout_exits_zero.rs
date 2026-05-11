//! c25 — without any inbound `bus.event`, the headless TUI honours
//! `RFL_TUI_MAX_LIFETIME` and exits 0 on its own.

mod common;

use std::time::Duration;

use common::{expect_clean_exit, spawn_tui, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn self_timeout_exits_zero() {
    let (svc, _events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(1),
            ready_delay_ms: None,
            test_message: None,
            test_confirm_answer: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        svc,
    );

    expect_clean_exit(&mut h.child, Duration::from_millis(2500)).await;
}
