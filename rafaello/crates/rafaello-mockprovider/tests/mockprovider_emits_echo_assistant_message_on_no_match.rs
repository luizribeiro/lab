//! c21 / scope §PR2 — non-matching user_message yields echo
//! assistant_message citing the user_message id.

mod common;

use common::mock_provider_handle::{payload_text, MockProviderHandle, PROVIDER_ID};

#[tokio::test]
async fn emits_echo_assistant_message_on_no_match() {
    let mut handle = MockProviderHandle::launch().await;
    let user_id = handle.publish_user_message("hello");

    let event = handle.recv_event().await;
    assert_eq!(
        event.topic,
        format!("provider.{PROVIDER_ID}.assistant_message")
    );
    assert_eq!(payload_text(&event), "echo: hello");
    assert_eq!(event.in_reply_to.as_deref(), Some(&[user_id][..]));
}
