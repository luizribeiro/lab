//! c17 / scope §SL5: a submitted line starting with `/` MUST NOT produce
//! a `frontend.tui.user_message` publish — it routes to
//! `frontend.tui.slash_command` instead.

mod common;

use std::time::Duration;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn user_message_starting_with_slash_not_published() {
    let (recorder, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: Some("/foo".to_string()),
            test_confirm_answer: None,
            test_confirm_answers: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        recorder,
    );

    let _ready = wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;
    let publish = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;

    let topic = publish
        .params
        .get("topic")
        .and_then(|v| v.as_str())
        .expect("topic");
    assert_eq!(topic, "frontend.tui.slash_command");
    assert_ne!(
        topic, "frontend.tui.user_message",
        "slash-prefixed input must not go through the user_message path"
    );

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
