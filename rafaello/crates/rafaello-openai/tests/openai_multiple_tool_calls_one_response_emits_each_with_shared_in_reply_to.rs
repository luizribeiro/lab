//! c33 / scope §OP1 multiple-tool-calls row + §OP3 — a single
//! response carrying N `tool_calls` yields N
//! `provider.openai.tool_request` events in array order, each with
//! a fresh `request_id`, all sharing the same `in_reply_to`.

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
async fn multiple_tool_calls_one_response_emits_each_with_shared_in_reply_to() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;
    handle.publish_user_message("read both");

    let req_a = handle.recv_event().await;
    let req_b = handle.recv_event().await;

    assert_eq!(req_a.topic, topic_for("tool_request"));
    assert_eq!(req_b.topic, topic_for("tool_request"));
    assert_eq!(payload_tool(&req_a), "read-file");
    assert_eq!(payload_tool(&req_b), "read-file");
    assert_eq!(
        payload_args(&req_a).get("path").and_then(|v| v.as_str()),
        Some("a.txt")
    );
    assert_eq!(
        payload_args(&req_b).get("path").and_then(|v| v.as_str()),
        Some("b.txt")
    );
    assert_ne!(
        req_a.request_id, req_b.request_id,
        "each tool_request must carry a fresh request_id"
    );
    assert_eq!(
        req_a.in_reply_to, req_b.in_reply_to,
        "tool_requests emitted from one response share in_reply_to"
    );
}
