//! Provider publishing `provider.mock.tool_request` citing an id
//! never observed in either set is rejected as
//! `InvalidInReplyTo { StaleRequestId }` (scope §B6 step 7, c10).

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::InReplyToReason;
use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn tool_request_stale_id_rejected() {
    let (broker, canonical) = provider_broker();
    let topic = "provider.mock.tool_request";
    let unknown = JsonRpcId::from("never-seen");
    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [unknown.clone()],
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
            } if id == &unknown
        ),
        "expected InvalidInReplyTo{{StaleRequestId}}, got {err:?}"
    );
}
