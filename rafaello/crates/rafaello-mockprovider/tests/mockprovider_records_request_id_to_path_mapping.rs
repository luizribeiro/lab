//! c21 / pi-1 H-4 — `outstanding` records every emitted tool_request id;
//! a `tool_result` cites the second to resolve back to b.txt.

mod common;

use common::mock_provider_handle::{payload_path, payload_text, MockProviderHandle, PROVIDER_ID};

#[tokio::test]
async fn records_request_id_to_path_mapping() {
    let mut handle = MockProviderHandle::launch().await;

    handle.publish_user_message("what's in a.txt");
    let req_a = handle.recv_event().await;
    assert_eq!(payload_path(&req_a), "a.txt");
    let req_a_id = req_a.request_id.expect("request_id on tool_request");

    handle.publish_user_message("what's in b.txt");
    let req_b = handle.recv_event().await;
    assert_eq!(payload_path(&req_b), "b.txt");
    let req_b_id = req_b.request_id.expect("request_id on tool_request");

    assert_ne!(req_a_id, req_b_id);

    handle.inject_tool_result(req_b_id.clone(), "B contents");
    let asst = handle.recv_event().await;
    assert_eq!(
        asst.topic,
        format!("provider.{PROVIDER_ID}.assistant_message")
    );
    let text = payload_text(&asst);
    assert!(
        text.contains("b.txt"),
        "assistant text should mention b.txt; got {text:?}"
    );
    assert!(
        !text.contains("a.txt"),
        "assistant text must not mention a.txt; got {text:?}"
    );
}
