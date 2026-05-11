//! A `provider.mock.tool_request` may only cite ids previously
//! observed as tool_results (security RFC §7.2.6 row 2). An id seeded
//! into `provider_observed_user_messages` but not into
//! `provider_observed_results` must be rejected as
//! `InvalidInReplyTo { StaleRequestId }` (scope §B6 step 7, c10 —
//! pi-3 B-2, pi-2 B-1 seed seam).

#![cfg(feature = "test-fixture")]

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::InReplyToReason;
use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn tool_request_user_message_id_rejected() {
    let (broker, canonical) = provider_broker();
    let user_msg_id = JsonRpcId::from("user-msg-1");
    broker.seed_provider_observed_user_message_for_test(&canonical, user_msg_id.clone());

    let topic = "provider.mock.tool_request";
    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [user_msg_id.clone()],
        "request_id": JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                reason: InReplyToReason::StaleRequestId { ref id },
                ..
            } if id == &user_msg_id
        ),
        "expected InvalidInReplyTo{{StaleRequestId}}, got {err:?}"
    );
}
