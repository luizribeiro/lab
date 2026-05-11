//! c21 / pi-3 H-1 — the canonical multi-turn coverage. Turn-2's
//! `tool_request.in_reply_to` cites the turn-1 `tool_result`'s
//! `request_id`; the broker accepts (retained-context semantics).

mod common;

use common::mock_provider_handle::{payload_path, MockProviderHandle, PROVIDER_ID};

#[tokio::test]
async fn multi_turn_cites_prior_tool_result_id() {
    let mut handle = MockProviderHandle::launch().await;

    handle.publish_user_message("what's in a.txt");
    let req1 = handle.recv_event().await;
    assert_eq!(req1.topic, format!("provider.{PROVIDER_ID}.tool_request"));
    assert_eq!(payload_path(&req1), "a.txt");
    assert_eq!(req1.in_reply_to.as_deref(), Some(&[][..]));
    let req1_id = req1.request_id.expect("turn-1 tool_request id");

    let result1_id = handle.inject_tool_result(req1_id.clone(), "A contents");

    let asst1 = handle.recv_event().await;
    assert_eq!(
        asst1.topic,
        format!("provider.{PROVIDER_ID}.assistant_message")
    );
    assert_eq!(
        asst1.in_reply_to.as_deref(),
        Some(&[result1_id.clone()][..])
    );

    handle.publish_user_message("what's in b.txt");
    let req2 = handle.recv_event().await;
    assert_eq!(req2.topic, format!("provider.{PROVIDER_ID}.tool_request"));
    assert_eq!(payload_path(&req2), "b.txt");
    let req2_in_reply = req2
        .in_reply_to
        .as_ref()
        .expect("turn-2 tool_request carries in_reply_to");
    assert!(
        req2_in_reply.contains(&result1_id),
        "turn-2 in_reply_to must cite turn-1 tool_result id; got {req2_in_reply:?}"
    );
}
