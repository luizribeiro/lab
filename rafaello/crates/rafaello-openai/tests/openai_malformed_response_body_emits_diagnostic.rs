//! Scope §OP1: malformed JSON → `"openai: malformed response: <serde error>"`,
//! plus full body logged for `manual-validation.md` capture.

mod common;

use rafaello_openai::{map_to_assistant, OpenaiError, WireClient};

#[tokio::test]
async fn malformed_response_body_maps_to_diagnostic() {
    let stub = common::start(200, "this is not json {{").await;
    let client = WireClient::new(stub.endpoint, "sk-test".to_string());
    let err = client
        .chat(&common::sample_request())
        .await
        .expect_err("malformed body must surface as error");
    assert!(
        matches!(err, OpenaiError::Malformed(_)),
        "expected Malformed, got {err:?}"
    );
    let text = map_to_assistant(&err);
    assert!(
        text.starts_with("openai: malformed response: "),
        "got {text:?}"
    );
}
