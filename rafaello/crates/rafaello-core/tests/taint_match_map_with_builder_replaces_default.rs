//! Scope §TM3 — `ReemitRouter::with_taint_match_map` swaps the
//! default `Arc<TaintMatchMap>` for a caller-supplied one. Asserted by
//! `Arc::ptr_eq` between the input and the value observed through the
//! `taint_match_for_test` seam.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

#[tokio::test]
async fn taint_match_map_with_builder_replaces_default() {
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

    let custom = Arc::new(TaintMatchMap::new(Duration::from_secs(7), 4));
    let router =
        ReemitRouter::new(broker, acl, provider, shutdown_rx).with_taint_match_map(custom.clone());

    let observed = router.taint_match_for_test();
    assert!(
        Arc::ptr_eq(&observed, &custom),
        "builder must install the caller-supplied Arc<TaintMatchMap>",
    );
}
