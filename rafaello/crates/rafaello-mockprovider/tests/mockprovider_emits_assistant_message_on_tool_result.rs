//! c21 / scope §PR2 — `tool_result` triggers assistant_message citing
//! the tool_result's request_id.

mod common;

use common::mock_provider_handle::{payload_text, MockProviderHandle, PROVIDER_ID};

#[tokio::test]
async fn emits_assistant_message_on_tool_result() {
    let mut handle = MockProviderHandle::launch().await;
    handle.publish_user_message("what's in README.md");
    let tool_req = handle.recv_event().await;
    let tool_req_id = tool_req.request_id.expect("tool_request request_id");

    let result_id = handle.inject_tool_result(tool_req_id, "Hello!");

    let asst = handle.recv_event().await;
    assert_eq!(
        asst.topic,
        format!("provider.{PROVIDER_ID}.assistant_message")
    );
    let text = payload_text(&asst);
    assert!(
        text.starts_with("Here's what's in"),
        "assistant text should start with the canonical prefix; got {text:?}"
    );
    assert!(text.contains("Hello!"), "assistant text must echo content");
    assert_eq!(asst.in_reply_to.as_deref(), Some(&[result_id][..]));
}
