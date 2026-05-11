//! Scope §A4 / pi-6 owner-judgment item 4: a default-constructed
//! `ReemitRouter` exposes a `TaintMatchMap` with a 5-minute TTL.
//!
//! Observed indirectly through the substring-arm retain pass in
//! `TaintMatchMap::lookup`: a recorded substring entry must still match
//! ~4 minutes later, and the same entry must be evicted just past the
//! 5-minute mark. The router-owned `Arc<TaintMatchMap>` is reached via
//! the test-only `taint_match_for_test` seam.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

#[tokio::test(start_paused = true)]
async fn taint_match_map_default_ttl_five_minutes() {
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
    let map = router.taint_match_for_test();

    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    let payload = serde_json::json!({"q": "abcdefghijklmnop"});
    map.record(&payload, &taint);

    tokio::time::advance(std::time::Duration::from_secs(240)).await;
    let hits = map.lookup(&payload);
    assert_eq!(hits, taint, "entry alive at t+4min (TTL >= 5min)");

    tokio::time::advance(std::time::Duration::from_secs(61)).await;
    let hits = map.lookup(&payload);
    assert!(
        hits.is_empty(),
        "entry expired by t+5min1s (TTL == 5min); got {hits:?}"
    );
}
