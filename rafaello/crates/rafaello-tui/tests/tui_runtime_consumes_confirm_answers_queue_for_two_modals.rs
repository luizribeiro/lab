//! §TUI-MA1: runtime dequeues the scripted queue once per modal — two modals
//! consume `allow,deny` in order.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn two_modals_consume_allow_then_deny_in_order() {
    let (svc, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: None,
            test_confirm_answer: None,
            test_confirm_answers: Some("allow,deny".to_string()),
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        svc,
    );

    wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;

    for (i, confirm_id) in ["01HZ_ID_A", "01HZ_ID_B"].iter().enumerate() {
        h.parent_peer
            .notify(
                "bus.event",
                json!({
                    "topic": "core.session.confirm_request",
                    "payload": {
                        "request_id": confirm_id,
                        "summary": format!("call #{i}"),
                        "details": { "tool": "send-mail" },
                        "ttl_seconds": 60_u64,
                    },
                    "publisher": { "kind": "core" },
                }),
            )
            .expect("publish confirm_request bus.event");
    }

    let first = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert_eq!(
        first.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.confirm_answer")
    );
    assert_eq!(first.params["payload"]["request_id"], "01HZ_ID_A");
    assert_eq!(first.params["payload"]["answer"], "allow");

    let second = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert_eq!(
        second.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.confirm_answer")
    );
    assert_eq!(second.params["payload"]["request_id"], "01HZ_ID_B");
    assert_eq!(second.params["payload"]["answer"], "deny");

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
