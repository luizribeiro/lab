//! Provider publishing `provider.mock.tool_request` without an
//! `in_reply_to` field is rejected as
//! `InvalidInReplyTo { Missing }` (scope §B6 step 7, c10).

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::InReplyToReason;
use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn tool_request_missing_in_reply_to_rejected() {
    let (broker, canonical) = provider_broker();
    let topic = "provider.mock.tool_request";
    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "request_id": JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                reason: InReplyToReason::Missing,
                ref topic,
                ..
            } if topic == "provider.mock.tool_request"
        ),
        "expected InvalidInReplyTo{{Missing}}, got {err:?}"
    );
}
