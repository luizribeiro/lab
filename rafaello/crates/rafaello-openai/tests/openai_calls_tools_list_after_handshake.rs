//! c36 §OP2 item 6 — after the fittings handshake completes (and
//! before processing `core.session.user_message`), the bin calls
//! `core.tools_list`. The response is cached and used on the next
//! chat-completions request, proving the call landed before any
//! HTTP POST went out.

mod common;

use common::openai_provider_handle::{start_http_stub, OpenAiProviderHandle};
use serde_json::{json, Value};

const BODY: &str = r#"{
  "id": "cmpl-1",
  "choices": [
    { "index": 0,
      "message": { "role": "assistant", "content": "ok" },
      "finish_reason": "stop" }
  ]
}"#;

#[tokio::test]
async fn calls_tools_list_after_handshake() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let tools = json!([{
        "name": "do-thing",
        "description": "do a thing",
        "parameters_schema": {"type": "object", "properties": {}, "required": []}
    }]);
    let mut handle = OpenAiProviderHandle::launch_with_tools(stub, Some(tools)).await;

    assert!(
        handle.http.captured_bodies().await.is_empty(),
        "no chat_completions POST should occur before user_message"
    );

    handle.publish_user_message("hi");
    let _event = handle.recv_event().await;

    let bodies = handle.http.captured_bodies().await;
    assert_eq!(bodies.len(), 1, "exactly one POST after user_message");
    let parsed: Value = serde_json::from_str(&bodies[0]).expect("body json");
    assert!(
        parsed.get("tools").is_some(),
        "request must carry cached tools after handshake call"
    );
}
