//! Provider publishing `provider.mock.tool_request` with
//! `request_id: None` is rejected as `MissingRequestId` — scope §B0
//! table-of-truth enforcement (c10).

use rafaello_core::error::Publisher;
use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn missing_request_id_rejected() {
    let (broker, canonical) = provider_broker();
    let topic = "provider.mock.tool_request";
    let params = serde_json::json!({"topic": topic, "payload": {}});
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::MissingRequestId {
                publisher: Publisher::Provider { .. },
                ref topic,
            } if topic == "provider.mock.tool_request"
        ),
        "expected MissingRequestId{{Provider}}, got {err:?}"
    );
}
