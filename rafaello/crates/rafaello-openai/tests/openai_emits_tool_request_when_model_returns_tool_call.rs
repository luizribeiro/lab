//! c33 / scope §OP3 — `finish_reason: "tool_calls"` yields one
//! `provider.openai.tool_request` per `tool_calls[i]`.

mod common;

use common::openai_provider_handle::{
    payload_args, payload_tool, start_http_stub, topic_for, OpenAiProviderHandle,
};

const BODY: &str = r#"{
  "id": "cmpl-1",
  "choices": [
    { "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          { "id": "call_abc",
            "type": "function",
            "function": { "name": "read-file", "arguments": "{\"path\":\"README.md\"}" } }
        ]
      },
      "finish_reason": "tool_calls" }
  ]
}"#;

#[tokio::test]
async fn emits_tool_request_when_model_returns_tool_call() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;
    handle.publish_user_message("read README");
    let event = handle.recv_event().await;
    assert_eq!(event.topic, topic_for("tool_request"));
    assert_eq!(payload_tool(&event), "read-file");
    assert_eq!(
        payload_args(&event).get("path").and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert!(event.request_id.is_some(), "request_id must be set");
}
