//! c37 / scope §CHAT3: `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE` JSON
//! `{tool, args_subset}` causes the TUI, on startup, to publish a synthetic
//! `frontend.tui.slash_command` carrying a `/grant` payload BEFORE the
//! `RFL_TUI_TEST_MESSAGE` user_message publish.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn grant_published_before_user_message() {
    let (svc, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: Some("please email alice".to_string()),
            test_confirm_answer: None,
            test_confirm_answers: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: Some(
                r#"{"tool":"send-mail","args_subset":{"to":"alice@example.com"}}"#.to_string(),
            ),
        },
        svc,
    );

    wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;

    let first = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert_eq!(
        first.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.slash_command")
    );
    let payload = first.params.get("payload").expect("payload");
    assert_eq!(payload["command"], "grant");
    assert_eq!(payload["args"]["tool"], "send-mail");
    assert_eq!(payload["args"]["template"]["to"], "alice@example.com");

    let second = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;
    assert_eq!(
        second.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.user_message")
    );
    assert_eq!(
        second
            .params
            .get("payload")
            .and_then(|p| p.get("text"))
            .and_then(|v| v.as_str()),
        Some("please email alice")
    );

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
