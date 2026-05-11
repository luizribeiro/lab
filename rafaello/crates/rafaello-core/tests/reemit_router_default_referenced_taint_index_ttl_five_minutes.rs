//! Scope §TR4a / pi-2 B-1: a default-constructed `ReemitRouter` exposes
//! a `ReferencedTaintIndex` with a 5-minute TTL. Observed indirectly
//! through the lazy-expiry pass in `lookup_request`: a recorded entry
//! must still resolve at ~4 minutes and must be evicted just past the
//! 5-minute mark.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

#[tokio::test(start_paused = true)]
async fn reemit_router_default_referenced_taint_index_ttl_five_minutes() {
    let provider = CanonicalId::parse("local/test:mock@0.1.0").expect("canonical");
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
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(broker, acl, provider, shutdown_rx);
    let idx = router.referenced_taint_index_for_test();

    let id = JsonRpcId::String("req-1".to_string());
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    idx.record_request(&id, &taint);

    tokio::time::advance(std::time::Duration::from_secs(240)).await;
    assert_eq!(
        idx.lookup_request(&id),
        Some(taint.clone()),
        "entry alive at t+4min (TTL >= 5min)",
    );

    tokio::time::advance(std::time::Duration::from_secs(61)).await;
    assert_eq!(
        idx.lookup_request(&id),
        None,
        "entry expired by t+5min1s (TTL == 5min)",
    );
}
