//! c16 — `RFL_TUI_TEST_MESSAGE` env hook: after `frontend.ready` resolves,
//! the TUI publishes `frontend.tui.user_message` with the env value and a
//! fresh `JsonRpcId::String` `request_id`.

mod common;

use std::time::Duration;

use fittings_core::message::JsonRpcId;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn tui_publishes_test_message_after_frontend_ready() {
    let (recorder, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: Some("what's in README.md".to_string()),
            test_confirm_answer: None,
            test_confirm_answers: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        recorder,
    );

    let ready = wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;
    assert!(!ready.is_notification, "frontend.ready must be a call");

    let publish = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert!(
        publish.is_notification,
        "bus.publish must be a notification"
    );

    let topic = publish
        .params
        .get("topic")
        .and_then(|v| v.as_str())
        .expect("bus.publish topic");
    assert_eq!(topic, "frontend.tui.user_message");

    let text = publish
        .params
        .get("payload")
        .and_then(|p| p.get("text"))
        .and_then(|v| v.as_str())
        .expect("payload.text");
    assert_eq!(text, "what's in README.md");

    let request_id: JsonRpcId = serde_json::from_value(
        publish
            .params
            .get("request_id")
            .cloned()
            .expect("request_id present"),
    )
    .expect("request_id parses as JsonRpcId");
    match request_id {
        JsonRpcId::String(s) => assert!(!s.is_empty(), "request_id String must be non-empty"),
        other => panic!("expected JsonRpcId::String, got {other:?}"),
    }

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
