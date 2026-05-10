#![allow(dead_code)]
//! Inline helper for c23: an in-memory `Broker` whose ACL has one
//! `tui` frontend and one synthetic `observer` plugin subscribed to
//! `core.session.**`. The observer plugin is registered with a
//! `PeerHandle` whose `mpsc::Receiver` the test drains to inspect
//! published `bus.event` notifications.
//!
//! The formal m3 harness (with full plugin/fittings wiring) lands in
//! c30; this kit is the minimum surface c23 needs.

use std::collections::BTreeMap;

use fittings_core::context::OutboundNotification;
use tokio::sync::mpsc;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
use rafaello_core::bus::{Broker, RegisteredPlugin};
use rafaello_core::lock::CanonicalId;

use super::peer_test_kit::fresh_peer;

pub struct InMemoryBroker {
    pub broker: Broker,
    pub observer_rx: mpsc::Receiver<OutboundNotification>,
    pub _observer_guard: RegisteredPlugin,
}

pub fn in_memory_broker_with_tui_and_observer_acl() -> InMemoryBroker {
    let observer = CanonicalId::parse("local/test:observer@0.1.0").expect("observer canonical");
    let observer_topic_id = "observer_local_test".to_string();
    let tui = AttachId::new("tui").expect("tui attach id");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        observer.clone(),
        PluginAcl {
            topic_id: observer_topic_id,
            publish_topics: vec![],
            subscribe_patterns: vec!["core.session.**".to_string()],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );

    let mut frontends = BTreeMap::new();
    frontends.insert(tui, FrontendAcl::default());

    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, observer_rx) = fresh_peer();
    let guard = broker
        .register_plugin(observer, peer)
        .expect("observer registration succeeds");

    InMemoryBroker {
        broker,
        observer_rx,
        _observer_guard: guard,
    }
}
