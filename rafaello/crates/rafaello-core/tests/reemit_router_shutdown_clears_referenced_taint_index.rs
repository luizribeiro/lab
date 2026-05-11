//! pi-5 M-3 / scope §TR4a: when a `ReemitRouter` shuts down via its
//! `watch::Receiver`, the spawned task calls
//! `ReferencedTaintIndex::clear()` on the router-owned cache. Two
//! routers share the same `Arc<ReferencedTaintIndex>` via
//! `with_referenced_taint_index`; entries are recorded; router A is
//! shut down; the cache is then observed empty through router B.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
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
async fn reemit_router_shutdown_clears_referenced_taint_index() {
    let provider = CanonicalId::parse("local/test:mock@0.1.0").expect("canonical");
    let acl = make_acl(&provider);
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let shared = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));

    let (shutdown_a_tx, shutdown_a_rx) = watch::channel(false);
    let router_a = ReemitRouter::new(broker.clone(), acl.clone(), provider.clone(), shutdown_a_rx)
        .with_referenced_taint_index(shared.clone());

    let (_shutdown_b_tx, shutdown_b_rx) = watch::channel(false);
    let router_b = ReemitRouter::new(broker, acl, provider, shutdown_b_rx)
        .with_referenced_taint_index(shared.clone());
    let observed_via_b = router_b.referenced_taint_index_for_test();
    assert!(
        Arc::ptr_eq(&observed_via_b, &shared),
        "router B is wired to the shared cache",
    );

    let req_id = JsonRpcId::String("req-1".to_string());
    let res_id = JsonRpcId::String("res-1".to_string());
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    shared.record_request(&req_id, &taint);
    shared.record_result(&res_id, &taint);
    assert_eq!(shared.lookup_request(&req_id), Some(taint.clone()));
    assert_eq!(shared.lookup_result(&res_id), Some(taint.clone()));

    let join_a = router_a.start();
    shutdown_a_tx.send(true).expect("shutdown rx alive");
    tokio::time::timeout(Duration::from_secs(2), join_a)
        .await
        .expect("router A task exits within 2s")
        .expect("router A task did not panic");

    assert_eq!(
        observed_via_b.lookup_request(&req_id),
        None,
        "shared cache request arm must be cleared on router A shutdown",
    );
    assert_eq!(
        observed_via_b.lookup_result(&res_id),
        None,
        "shared cache result arm must be cleared on router A shutdown",
    );
}
