//! When an internal subscriber's bounded channel is full, the broker
//! logs a `tracing::warn!` and continues. Scope §B7 + pi-2 M-1.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[tracing_test::traced_test]
#[test]
fn drops_event_when_full() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (_rx, _guard) = broker.subscribe_internal(vec!["core.**".to_string()], 1);

    broker
        .publish_core("core.lifecycle.boot", serde_json::json!({"n": 1}))
        .expect("first publish accepted");
    broker
        .publish_core("core.lifecycle.boot", serde_json::json!({"n": 2}))
        .expect("second publish accepted");

    assert!(logs_contain(
        "internal subscriber dropped event — channel full"
    ));
}
