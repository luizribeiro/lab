//! c33 / scope §OP3 — multi-turn: turn-1 emits a tool_request,
//! turn-2 (triggered by a second user_message that follows a
//! tool_result injection) emits a final assistant_message. The
//! adapter must carry the tool_result back into its conversation
//! history (`role: "tool"`) and resume.

mod common;

use common::openai_provider_handle::{
    payload_text, start_http_stub, topic_for, OpenAiProviderHandle,
};

const TURN1: &str = r#"{
  "id": "cmpl-1",
  "choices": [
    { "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          { "id": "call_abc",
            "type": "function",
            "function": { "name": "read-file", "arguments": "{\"path\":\"a.txt\"}" } }
        ]
      },
      "finish_reason": "tool_calls" }
  ]
}"#;

const TURN2: &str = r#"{
  "id": "cmpl-2",
  "choices": [
    { "index": 0,
      "message": { "role": "assistant", "content": "Here are the contents." },
      "finish_reason": "stop" }
  ]
}"#;

#[tokio::test]
async fn handles_tool_call_followed_by_assistant_message() {
    let stub = start_http_stub(vec![(200, TURN1.to_string()), (200, TURN2.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;

    handle.publish_user_message("read a");
    let req1 = handle.recv_event().await;
    assert_eq!(req1.topic, topic_for("tool_request"));
    let req1_id = req1.request_id.expect("turn-1 request_id");

    handle.inject_tool_result(req1_id, "A contents");

    handle.publish_user_message("ok, summarise");
    let asst = handle.recv_event().await;
    assert_eq!(asst.topic, topic_for("assistant_message"));
    assert_eq!(payload_text(&asst), "Here are the contents.");
}
