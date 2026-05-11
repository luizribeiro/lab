//! pi-5 M-3 ripple / scope §TM1: when a `ReemitRouter` shuts down via
//! its `watch::Receiver`, the spawned task calls
//! `TaintMatchMap::clear()` on the router-owned map. Two routers share
//! the same `Arc<TaintMatchMap>` via `with_taint_match_map`; an entry
//! is recorded; router A is shut down; the map is then observed empty
//! through router B.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

fn make_acl(provider: &CanonicalId) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        provider.clone(),
        PluginAcl {
            topic_id: "mock_local_test".to_string(),
            publish_topics: vec!["provider.mock.tool_request".to_string()],
            subscribe_patterns: vec!["core.session.tool_request".to_string()],
            auto_subscribes: vec![],
            provider_id: Some("mock".to_string()),
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    }
}

#[tokio::test]
async fn reemit_router_shutdown_clears_taint_match_map() {
    let provider = CanonicalId::parse("local/test:mock@0.1.0").expect("canonical");
    let acl = make_acl(&provider);
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let shared = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));

    let (shutdown_a_tx, shutdown_a_rx) = watch::channel(false);
    let router_a = ReemitRouter::new(broker.clone(), acl.clone(), provider.clone(), shutdown_a_rx)
        .with_taint_match_map(shared.clone());

    let (_shutdown_b_tx, shutdown_b_rx) = watch::channel(false);
    let router_b = ReemitRouter::new(broker, acl, provider, shutdown_b_rx)
        .with_taint_match_map(shared.clone());
    let observed_via_b = router_b.taint_match_for_test();
    assert!(
        Arc::ptr_eq(&observed_via_b, &shared),
        "router B is wired to the shared map",
    );

    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    shared.record(&serde_json::json!({"q": "abcdefghijklmnop"}), &taint);
    assert_eq!(
        shared.lookup(&serde_json::json!({"q": "abcdefghijklmnop"})),
        taint,
        "entry visible before shutdown",
    );

    let join_a = router_a.start();
    shutdown_a_tx.send(true).expect("shutdown rx alive");
    tokio::time::timeout(Duration::from_secs(2), join_a)
        .await
        .expect("router A task exits within 2s")
        .expect("router A task did not panic");

    let hits = observed_via_b.lookup(&serde_json::json!({"q": "abcdefghijklmnop"}));
    assert!(
        hits.is_empty(),
        "shared TaintMatchMap must be cleared on router A shutdown; got {hits:?}",
    );
}
