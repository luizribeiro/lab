//! Provider publishing on a top-level namespace outside
//! `{core, provider, plugin, frontend}` is rejected as
//! `UnknownNamespace` (scope §B6 step 4, c10).

use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn unknown_namespace_rejected() {
    let (broker, canonical) = provider_broker();
    let bad = "evil.foo";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(err, BrokerError::UnknownNamespace { ref topic, .. } if topic == bad),
        "expected UnknownNamespace, got {err:?}"
    );
}
