//! c36 §OP2 item 6 — after the `core.tools_list` cache populates,
//! every `ChatCompletion` request carries the schemas in the
//! `tools` field, wrapped in OpenAI's `{type: "function",
//! function: {...}}` envelope.

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
async fn request_carries_tool_schemas() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let tools = json!([{
        "name": "send-mail",
        "description": "Send an email",
        "parameters_schema": {
            "type": "object",
            "properties": {"to": {"type": "string"}},
            "required": ["to"]
        }
    }]);
    let mut handle = OpenAiProviderHandle::launch_with_tools(stub, Some(tools)).await;

    handle.publish_user_message("hi");
    let _event = handle.recv_event().await;

    let bodies = handle.http.captured_bodies().await;
    assert_eq!(bodies.len(), 1);
    let parsed: Value = serde_json::from_str(&bodies[0]).expect("body json");
    let tools = parsed
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(Value::as_str),
        Some("function")
    );
    let func = tools[0].get("function").expect("function");
    assert_eq!(func.get("name").and_then(Value::as_str), Some("send-mail"));
    assert_eq!(
        func.get("description").and_then(Value::as_str),
        Some("Send an email")
    );
    let params = func.get("parameters").expect("parameters");
    assert_eq!(
        params
            .get("required")
            .and_then(Value::as_array)
            .map(|a| a.len()),
        Some(1)
    );
}
