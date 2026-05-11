//! Provider publishing on `provider.mock.confidential` (a topic in
//! its own namespace but not in `publish_topics`) is rejected as
//! `PublishOutsideGrant` (scope §B6 step 5, c10).

use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn outside_grant_rejected() {
    let (broker, canonical) = provider_broker();
    let bad = "provider.mock.confidential";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(err, BrokerError::PublishOutsideGrant { ref topic, .. } if topic == bad),
        "expected PublishOutsideGrant, got {err:?}"
    );
}
