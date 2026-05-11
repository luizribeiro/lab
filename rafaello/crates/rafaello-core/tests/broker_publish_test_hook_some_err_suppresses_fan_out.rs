//! `install_publish_test_hook` returning `Some(err)` short-circuits a
//! `publish_core_with_taint` call: `fan_out` does not run (no internal
//! subscriber observes the event) and the hook's error propagates to
//! the caller. Scope §TM4 / pi-6 M-1 ordering rule.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::Arc;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::error::BrokerError;
use rafaello_core::lock::CanonicalId;

#[test]
fn some_err_suppresses_fan_out() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (mut rx, _isub) = broker.subscribe_internal(vec!["core.**".to_string()], 8);

    let sentinel = CanonicalId::parse("local/test:sentinel@0.1.0").expect("canonical");
    let sentinel_for_hook = sentinel.clone();
    broker.install_publish_test_hook(Arc::new(move |_event| {
        Some(BrokerError::NotInAcl(sentinel_for_hook.clone()))
    }));

    let err = broker
        .publish_core("core.lifecycle.boot", serde_json::json!({}))
        .expect_err("hook must short-circuit publish");
    assert!(
        matches!(err, BrokerError::NotInAcl(ref c) if c == &sentinel),
        "expected NotInAcl({sentinel}), got {err:?}",
    );

    assert!(
        rx.try_recv().is_err(),
        "internal subscriber must not observe an event when hook short-circuits fan_out"
    );
}
