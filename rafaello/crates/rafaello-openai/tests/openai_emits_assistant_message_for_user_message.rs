//! c33 / scope §OP3 — a `stop` response yields one
//! `provider.openai.assistant_message` with the model's text.

mod common;

use common::openai_provider_handle::{
    payload_text, start_http_stub, topic_for, OpenAiProviderHandle,
};

const BODY: &str = r#"{
  "id": "cmpl-1",
  "choices": [
    { "index": 0,
      "message": { "role": "assistant", "content": "Hello there!" },
      "finish_reason": "stop" }
  ]
}"#;

#[tokio::test]
async fn emits_assistant_message_for_user_message() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;
    handle.publish_user_message("hello");
    let event = handle.recv_event().await;
    assert_eq!(event.topic, topic_for("assistant_message"));
    assert_eq!(payload_text(&event), "Hello there!");
    assert!(event.request_id.is_some(), "request_id must be set");
}
