//! Scope §OP1: 5xx → `"openai: server error <status>"`.

mod common;

use rafaello_openai::{map_to_assistant, OpenaiError, WireClient};

#[tokio::test]
async fn http_500_maps_to_server_error_assistant_message() {
    let stub = common::start(500, r#"{"error":"boom"}"#).await;
    let client = WireClient::new(stub.endpoint, "sk-test".to_string());
    let err = client
        .chat(&common::sample_request())
        .await
        .expect_err("500 must surface as error");
    assert!(
        matches!(err, OpenaiError::ServerError { status: 500 }),
        "expected ServerError{{status:500}}, got {err:?}"
    );
    assert_eq!(map_to_assistant(&err), "openai: server error 500");
}
