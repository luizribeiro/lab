//! c37 / scope §CHAT3: `RFL_TUI_TEST_CONFIRM_ANSWER=timeout` causes the TUI to
//! suppress publishing any `frontend.tui.confirm_answer` even when a
//! `core.session.confirm_request` is observed.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn timeout_answer_suppresses_publish() {
    let (svc, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(3),
            ready_delay_ms: None,
            test_message: None,
            test_confirm_answer: Some("timeout".to_string()),
            test_confirm_delay_ms: Some(0),
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
                    "request_id": "01HZ_TIMEOUT",
                    "summary": "",
                    "details": {},
                    "ttl_seconds": 60_u64,
                },
                "publisher": { "kind": "core" },
            }),
        )
        .expect("publish confirm_request bus.event");

    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, events.recv()).await {
            Ok(Some(rec)) => {
                if rec.method == "bus.publish" {
                    let topic = rec
                        .params
                        .get("topic")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    assert_ne!(
                        topic, "frontend.tui.confirm_answer",
                        "TUI must not publish confirm_answer under `timeout`"
                    );
                }
            }
            _ => break,
        }
    }

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
