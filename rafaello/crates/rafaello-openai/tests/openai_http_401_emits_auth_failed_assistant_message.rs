//! Scope §OP1a: 401/403 → `"openai: auth failed (<status>); check API key env var"`.

mod common;

use rafaello_openai::{map_to_assistant, OpenaiError, WireClient};

#[tokio::test]
async fn http_401_maps_to_auth_failed_assistant_message() {
    let stub = common::start(401, r#"{"error":"unauthorized"}"#).await;
    let client = WireClient::new(stub.endpoint, "sk-bad".to_string());
    let err = client
        .chat(&common::sample_request())
        .await
        .expect_err("401 must surface as error");
    assert!(
        matches!(err, OpenaiError::AuthFailed { status: 401 }),
        "expected AuthFailed{{status:401}}, got {err:?}"
    );
    assert_eq!(
        map_to_assistant(&err),
        "openai: auth failed (401); check API key env var"
    );
}
