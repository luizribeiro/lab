//! Two consecutive `install_publish_test_hook` calls: only the
//! second hook fires on a subsequent publish (last-writer-wins per
//! scope §TM4 / pi-6 N-5 overwrite semantics — no explicit clear
//! method).

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[test]
fn second_install_replaces_first() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let a_fires = Arc::new(AtomicUsize::new(0));
    let b_fires = Arc::new(AtomicUsize::new(0));

    let a_for_hook = a_fires.clone();
    broker.install_publish_test_hook(Arc::new(move |_event| {
        a_for_hook.fetch_add(1, Ordering::SeqCst);
        None
    }));

    let b_for_hook = b_fires.clone();
    broker.install_publish_test_hook(Arc::new(move |_event| {
        b_for_hook.fetch_add(1, Ordering::SeqCst);
        None
    }));

    broker
        .publish_core("core.lifecycle.boot", serde_json::json!({}))
        .expect("publish accepted");

    assert_eq!(
        a_fires.load(Ordering::SeqCst),
        0,
        "first-installed hook must not fire after a second install replaces it"
    );
    assert_eq!(
        b_fires.load(Ordering::SeqCst),
        1,
        "second-installed hook must fire exactly once on publish"
    );
}
