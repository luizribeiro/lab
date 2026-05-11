//! c17 / scope §SL5: an unrecognised slash command publishes
//! `frontend.tui.slash_command` with `command = "unknown"` and the raw
//! input verbatim so core's audit log captures the attempt.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn slash_unknown_command_publishes_unknown_kind() {
    let (recorder, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: Some("/foo bar baz".to_string()),
            test_confirm_answer: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        recorder,
    );

    let _ready = wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;
    let publish = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;

    assert_eq!(
        publish.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.slash_command")
    );
    let payload = publish.params.get("payload").expect("payload");
    assert_eq!(
        payload.get("command").and_then(|v| v.as_str()),
        Some("unknown")
    );
    assert_eq!(
        payload
            .get("args")
            .and_then(|a| a.get("raw"))
            .and_then(|v| v.as_str()),
        Some("/foo bar baz")
    );

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
