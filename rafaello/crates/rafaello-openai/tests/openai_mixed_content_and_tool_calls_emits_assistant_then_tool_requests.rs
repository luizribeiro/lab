//! c33 / scope §OP1 mixed-content row + §OP3 — when the response
//! carries both `content` and `tool_calls`, emit the
//! assistant_message first, then one tool_request per `tool_calls[i]`
//! in array order.

mod common;

use common::openai_provider_handle::{
    payload_args, payload_text, payload_tool, start_http_stub, topic_for, OpenAiProviderHandle,
};

const BODY: &str = r#"{
  "id": "cmpl-1",
  "choices": [
    { "index": 0,
      "message": {
        "role": "assistant",
        "content": "On it.",
        "tool_calls": [
          { "id": "call_a",
            "type": "function",
            "function": { "name": "read-file", "arguments": "{\"path\":\"a.txt\"}" } },
          { "id": "call_b",
            "type": "function",
            "function": { "name": "read-file", "arguments": "{\"path\":\"b.txt\"}" } }
        ]
      },
      "finish_reason": "tool_calls" }
  ]
}"#;

#[tokio::test]
async fn mixed_content_and_tool_calls_emits_assistant_then_tool_requests() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;
    handle.publish_user_message("read both");

    let asst = handle.recv_event().await;
    assert_eq!(asst.topic, topic_for("assistant_message"));
    assert_eq!(payload_text(&asst), "On it.");

    let req_a = handle.recv_event().await;
    assert_eq!(req_a.topic, topic_for("tool_request"));
    assert_eq!(payload_tool(&req_a), "read-file");
    assert_eq!(
        payload_args(&req_a).get("path").and_then(|v| v.as_str()),
        Some("a.txt")
    );

    let req_b = handle.recv_event().await;
    assert_eq!(req_b.topic, topic_for("tool_request"));
    assert_eq!(
        payload_args(&req_b).get("path").and_then(|v| v.as_str()),
        Some("b.txt")
    );
}
