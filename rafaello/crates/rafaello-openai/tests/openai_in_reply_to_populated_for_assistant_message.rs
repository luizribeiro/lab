//! c33 / scope §OP3 + security RFC §7.2.6 row 2 —
//! `assistant_message.in_reply_to` cites the observed user_message id.

mod common;

use common::openai_provider_handle::{start_http_stub, topic_for, OpenAiProviderHandle};

const BODY: &str = r#"{
  "id": "cmpl-1",
  "choices": [
    { "index": 0,
      "message": { "role": "assistant", "content": "Hello!" },
      "finish_reason": "stop" }
  ]
}"#;

#[tokio::test]
async fn in_reply_to_populated_for_assistant_message() {
    let stub = start_http_stub(vec![(200, BODY.to_string())]).await;
    let mut handle = OpenAiProviderHandle::launch(stub).await;
    let user_id = handle.publish_user_message("hi");
    let event = handle.recv_event().await;
    assert_eq!(event.topic, topic_for("assistant_message"));
    let cited = event
        .in_reply_to
        .as_ref()
        .expect("assistant_message must carry in_reply_to");
    assert!(
        cited.contains(&user_id),
        "assistant_message.in_reply_to must cite user_message id; got {cited:?}"
    );
}
