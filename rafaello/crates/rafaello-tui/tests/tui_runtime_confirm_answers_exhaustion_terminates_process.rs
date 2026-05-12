//! §TUI-MA1: scripting fewer answers than modals terminates the TUI process
//! with a non-zero exit and a pinned exhaustion message on stderr.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

#[tokio::test(flavor = "multi_thread")]
async fn third_modal_with_two_scripted_answers_exits_non_zero_with_pinned_stderr() {
    let (svc, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(10),
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

    for confirm_id in ["01HZ_ID_A", "01HZ_ID_B", "01HZ_ID_C"] {
        h.parent_peer
            .notify(
                "bus.event",
                json!({
                    "topic": "core.session.confirm_request",
                    "payload": {
                        "request_id": confirm_id,
                        "summary": "modal",
                        "details": { "tool": "send-mail" },
                        "ttl_seconds": 60_u64,
                    },
                    "publisher": { "kind": "core" },
                }),
            )
            .expect("publish confirm_request bus.event");
    }

    let status = tokio::time::timeout(Duration::from_secs(5), h.child.wait())
        .await
        .expect("child must exit after exhaustion")
        .expect("child wait failed");
    assert_ne!(status.code(), Some(0), "expected non-zero exit");

    let needle = "TestConfirmAnswers queue exhausted; modal #3 had no scripted answer";
    let found = drain_stderr_for(&mut h.stderr_lines, needle, Duration::from_secs(2)).await;
    assert!(
        found,
        "stderr should contain pinned exhaustion message `{needle}`"
    );
}

async fn drain_stderr_for(
    rx: &mut UnboundedReceiver<String>,
    needle: &str,
    timeout: Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return false;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(line)) => {
                if line.contains(needle) {
                    return true;
                }
            }
            Ok(None) => return false,
            Err(_) => return false,
        }
    }
}
