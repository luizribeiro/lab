//! c33 / scope §OP3 + security RFC §7.2.6 row 3 —
//! `tool_request.in_reply_to` cites observed prior tool_result ids.
//! On the first turn there are no prior tool_results, so the field
//! is the empty list (broker-required for the envelope to validate).

mod common;

use common::openai_provider_handle::{start_http_stub, topic_for, OpenAiProviderHandle};

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
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          { "id": "call_def",
            "type": "function",
            "function": { "name": "read-file", "arguments": "{\"path\":\"b.txt\"}" } }
        ]
      },
      "finish_reason": "tool_calls" }
  ]
}"#;

#[tokio::test]
async fn in_reply_to_populated_for_tool_request() {
    let stub = start_http_stub(vec![(200, TURN1.to_string()), (200, TURN2.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;

    handle.publish_user_message("read a");
    let req1 = handle.recv_event().await;
    assert_eq!(req1.topic, topic_for("tool_request"));
    assert_eq!(
        req1.in_reply_to.as_deref(),
        Some(&[][..]),
        "first turn carries no prior tool_results"
    );
    let req1_id = req1.request_id.expect("turn-1 request_id");

    let result_id = handle.inject_tool_result(req1_id, "A contents");

    handle.publish_user_message("read b");
    let req2 = handle.recv_event().await;
    assert_eq!(req2.topic, topic_for("tool_request"));
    let cited = req2
        .in_reply_to
        .as_ref()
        .expect("turn-2 tool_request must carry in_reply_to");
    assert!(
        cited.contains(&result_id),
        "turn-2 tool_request.in_reply_to must cite prior tool_result id; got {cited:?}"
    );
}
