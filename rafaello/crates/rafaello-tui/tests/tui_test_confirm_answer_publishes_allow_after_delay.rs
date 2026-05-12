//! c37 / scope §CHAT3: `RFL_TUI_TEST_CONFIRM_ANSWER=allow` causes the TUI to
//! auto-publish a `frontend.tui.confirm_answer` on `bus.publish` in response
//! to an observed `core.session.confirm_request`, after the configured delay.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};
use fittings_core::message::JsonRpcId;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn confirm_answer_allow_published_after_delay() {
    let (svc, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: None,
            test_confirm_answer: Some("allow".to_string()),
            test_confirm_answers: None,
            test_confirm_delay_ms: Some(10),
            test_grant_before_message: None,
        },
        svc,
    );

    wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;

    h.parent_peer
        .notify(
            "bus.event",
            json!({
                "topic": "core.session.confirm_request",
                "payload": {
                    "request_id": "01HZ_CONFIRM_ID",
                    "summary": "send-mail via mailcat",
                    "details": { "tool": "send-mail" },
                    "ttl_seconds": 60_u64,
                },
                "publisher": { "kind": "core" },
            }),
        )
        .expect("publish confirm_request bus.event");

    let publish = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert!(publish.is_notification, "bus.publish must be notification");
    let topic = publish
        .params
        .get("topic")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(topic, "frontend.tui.confirm_answer");
    let payload = publish.params.get("payload").expect("payload");
    assert_eq!(payload["request_id"], "01HZ_CONFIRM_ID");
    assert_eq!(payload["answer"], "allow");
    let in_reply_to: Vec<JsonRpcId> = serde_json::from_value(
        publish
            .params
            .get("in_reply_to")
            .cloned()
            .expect("in_reply_to"),
    )
    .expect("in_reply_to parses");
    assert_eq!(
        in_reply_to,
        vec![JsonRpcId::String("01HZ_CONFIRM_ID".to_string())]
    );

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
