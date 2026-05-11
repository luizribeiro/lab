//! c21 / scope §PR2 — first-turn `tool_request` emission.

mod common;

use common::mock_provider_handle::{payload_path, MockProviderHandle, PROVIDER_ID};

#[tokio::test]
async fn emits_tool_request_for_read_file_pattern() {
    let mut handle = MockProviderHandle::launch().await;
    let _user_id = handle.publish_user_message("what's in README.md");

    let event = handle.recv_event().await;
    assert_eq!(event.topic, format!("provider.{PROVIDER_ID}.tool_request"));
    assert_eq!(payload_path(&event), "README.md");
    assert_eq!(
        event.payload.get("tool").and_then(|v| v.as_str()),
        Some("read-file")
    );
    assert!(event.request_id.is_some(), "request_id must be set");
    assert_eq!(
        event.in_reply_to.as_deref(),
        Some(&[][..]),
        "first turn carries no prior tool_results"
    );
}
