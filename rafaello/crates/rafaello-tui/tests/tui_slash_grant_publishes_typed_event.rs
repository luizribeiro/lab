//! c17 / scope §SL5: `/grant <tool>` published as a typed
//! `frontend.tui.slash_command` event with mandatory envelope
//! `request_id` (no envelope `in_reply_to` — root event per §SL0).

mod common;

use std::time::Duration;

use fittings_core::message::JsonRpcId;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn slash_grant_publishes_typed_event() {
    let (recorder, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: Some("/grant tool_a".to_string()),
            test_confirm_answer: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        recorder,
    );

    let _ready = wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;
    let publish = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert!(publish.is_notification);

    assert_eq!(
        publish.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.slash_command")
    );

    let payload = publish.params.get("payload").expect("payload");
    assert_eq!(
        payload.get("command").and_then(|v| v.as_str()),
        Some("grant")
    );
    let args = payload.get("args").expect("args");
    assert_eq!(args.get("tool").and_then(|v| v.as_str()), Some("tool_a"));
    assert!(args.get("template").expect("template").is_object());

    let request_id: JsonRpcId = serde_json::from_value(
        publish
            .params
            .get("request_id")
            .cloned()
            .expect("request_id"),
    )
    .expect("request_id parses");
    match request_id {
        JsonRpcId::String(s) => assert!(!s.is_empty()),
        other => panic!("expected JsonRpcId::String, got {other:?}"),
    }

    assert!(
        publish.params.get("in_reply_to").is_none(),
        "slash_command is a root event — no envelope in_reply_to"
    );

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
