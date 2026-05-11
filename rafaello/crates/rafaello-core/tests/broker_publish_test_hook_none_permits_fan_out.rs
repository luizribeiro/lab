//! `install_publish_test_hook` returning `None` leaves the publish
//! path untouched: `fan_out` runs, internal subscribers observe the
//! event, and the call returns `Ok`. Scope §TM4 / pi-6 M-1.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[test]
fn none_permits_fan_out() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (mut rx, _isub) = broker.subscribe_internal(vec!["core.**".to_string()], 8);

    let hook_fires = Arc::new(AtomicUsize::new(0));
    let hook_fires_for_hook = hook_fires.clone();
    broker.install_publish_test_hook(Arc::new(move |_event| {
        hook_fires_for_hook.fetch_add(1, Ordering::SeqCst);
        None
    }));

    broker
        .publish_core("core.lifecycle.boot", serde_json::json!({}))
        .expect("publish accepted");

    assert_eq!(
        hook_fires.load(Ordering::SeqCst),
        1,
        "hook must be consulted exactly once per publish"
    );

    let event = rx
        .try_recv()
        .expect("internal subscriber must observe the event when hook returns None");
    assert_eq!(event.topic, "core.lifecycle.boot");
}
