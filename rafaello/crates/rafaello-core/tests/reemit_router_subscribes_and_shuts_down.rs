//! `ReemitRouter::start` spawns a tokio task that listens on the
//! shutdown `watch::Receiver` and exits cleanly when signalled (scope
//! §CR1 + §CR6). Construction also exercises the §CR1 active-provider
//! `provider_id` lookup: the ACL contains exactly one provider plugin
//! with a `provider_id` set, and the router resolves the public
//! `provider.<id>.**` subscribe pattern from that field.

use std::collections::BTreeMap;
use std::time::Duration;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

#[tokio::test]
async fn reemit_router_subscribes_and_shuts_down() {
    let provider = CanonicalId::parse("local/test:mock@0.1.0").expect("canonical");
    let provider_acl = PluginAcl {
        topic_id: "mock_local_test".to_string(),
        publish_topics: vec!["provider.mock.tool_request".to_string()],
        subscribe_patterns: vec!["core.session.tool_request".to_string()],
        auto_subscribes: vec![],
        provider_id: Some("mock".to_string()),
    };
    let mut plugins = BTreeMap::new();
    plugins.insert(provider.clone(), provider_acl);
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(broker, acl, provider, shutdown_rx);
    let join = router.start();

    shutdown_tx.send(true).expect("shutdown receiver alive");

    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router task exits within 2s")
        .expect("router task did not panic");
}
