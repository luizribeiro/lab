//! Scope §A3 / pi-6 owner-judgment item 5 (N-1): a default-constructed
//! `ReemitRouter` exposes a `TaintMatchMap` whose substring arm fires
//! at exactly 16 bytes, observed indirectly: a 15-byte recorded string
//! does NOT yield a substring hit on a 17-byte arg superstring, but a
//! 16-byte recorded string DOES.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

#[tokio::test]
async fn taint_match_map_default_substring_threshold_sixteen() {
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
        source: "tool".to_string(),
        detail: Some("local/test:mock@0.1.0".to_string()),
    }];

    let s15 = "AAAAAAAAAAAAAAA";
    assert_eq!(s15.len(), 15);
    map.record(&serde_json::Value::String(s15.to_string()), &taint);
    let superstring_17 = "xAAAAAAAAAAAAAAAy";
    assert_eq!(superstring_17.len(), 17);
    let hits = map.lookup(&serde_json::Value::String(superstring_17.to_string()));
    assert!(
        hits.is_empty(),
        "15-byte record must not enter the substring arm (threshold == 16); got {hits:?}"
    );

    let s16 = "BBBBBBBBBBBBBBBB";
    assert_eq!(s16.len(), 16);
    map.record(&serde_json::Value::String(s16.to_string()), &taint);
    let superstring_18 = "xBBBBBBBBBBBBBBBBy";
    assert_eq!(superstring_18.len(), 18);
    let hits = map.lookup(&serde_json::Value::String(superstring_18.to_string()));
    assert_eq!(
        hits, taint,
        "16-byte record must hit on 18-byte superstring"
    );
}
