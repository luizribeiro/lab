//! Dropping the [`InternalSubscription`] guard removes the slot from
//! `BrokerInner.internal_subscribers`; subsequent publishes no longer
//! land in the (already-closed) receiver. Scope §B7 + pi-2 M-1.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[test]
fn unregister_on_drop() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (mut rx, guard) = broker.subscribe_internal(vec!["core.**".to_string()], 8);

    broker
        .publish_core("core.lifecycle.boot", serde_json::json!({}))
        .expect("publish accepted");
    let _evt = rx.try_recv().expect("event delivered while subscribed");

    drop(guard);

    broker
        .publish_core("core.lifecycle.boot", serde_json::json!({}))
        .expect("publish accepted");
    assert!(
        rx.try_recv().is_err(),
        "no events delivered after subscription dropped"
    );
}
