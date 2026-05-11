#![allow(dead_code)]
//! Shared test helpers for the c18 re-emit-router tests (pi-1 M-3).
//!
//! Each c18 test imports `assert_origin_taint` for canonical-taint
//! assertions and `subscribe_router_test_receiver` for a tight wrapper
//! around `broker.subscribe_internal` pre-configured with the four
//! router patterns. Builder helpers below construct a broker + ACL
//! with the mock provider already registered, optionally with a tool
//! plugin and tool-route map.

use std::collections::{BTreeMap, BTreeSet};

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, InternalSubscription};
use rafaello_core::lock::CanonicalId;
use tokio::sync::mpsc;

use super::peer_test_kit::fresh_peer;

pub const MOCK_PROVIDER_ID: &str = "mock";
pub const MOCK_TOPIC_ID: &str = "mockprov_local_test";
pub const MOCK_CANONICAL: &str = "local/test:mockprov@0.1.0";
pub const READFILE_TOPIC_ID: &str = "readfile_local_test";
pub const READFILE_CANONICAL: &str = "local/test:readfile@0.1.0";
pub const TUI_ATTACH_ID: &str = "tui";

pub fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

pub fn mock_provider_acl() -> PluginAcl {
    PluginAcl {
        topic_id: MOCK_TOPIC_ID.to_string(),
        publish_topics: vec![
            format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
            format!("provider.{MOCK_PROVIDER_ID}.assistant_message"),
        ],
        subscribe_patterns: vec![],
        auto_subscribes: vec![],
        provider_id: Some(MOCK_PROVIDER_ID.to_string()),
    }
}

pub fn readfile_plugin_acl() -> PluginAcl {
    PluginAcl {
        topic_id: READFILE_TOPIC_ID.to_string(),
        publish_topics: vec![format!("plugin.{READFILE_TOPIC_ID}.tool_result")],
        subscribe_patterns: vec![],
        auto_subscribes: vec![format!("plugin.{READFILE_TOPIC_ID}.tool_request")],
        provider_id: None,
    }
}

pub fn tui_frontend_acl(publish_user_message: bool) -> FrontendAcl {
    let mut publish_topics = BTreeSet::new();
    if publish_user_message {
        publish_topics.insert(format!("frontend.{TUI_ATTACH_ID}.user_message"));
    }
    FrontendAcl {
        subscribe_patterns: BTreeSet::new(),
        auto_subscribes: BTreeSet::new(),
        publish_topics,
    }
}

#[derive(Default)]
pub struct RigOpts {
    pub include_readfile_plugin: bool,
    pub tool_routes: Vec<(&'static str, &'static str)>,
    pub include_tui_frontend: bool,
    pub extra_plugins: Vec<(CanonicalId, PluginAcl)>,
}

pub struct ReemitRig {
    pub broker: Broker,
    pub acl: BrokerAcl,
    pub provider_canonical: CanonicalId,
    pub frontend_attach: Option<AttachId>,
    pub readfile_canonical: Option<CanonicalId>,
}

pub fn build_rig(opts: RigOpts) -> ReemitRig {
    let provider_canonical = cid(MOCK_CANONICAL);
    let mut plugins: BTreeMap<CanonicalId, PluginAcl> = BTreeMap::new();
    plugins.insert(provider_canonical.clone(), mock_provider_acl());

    let readfile_canonical = if opts.include_readfile_plugin {
        let c = cid(READFILE_CANONICAL);
        plugins.insert(c.clone(), readfile_plugin_acl());
        Some(c)
    } else {
        None
    };

    for (c, a) in opts.extra_plugins {
        plugins.insert(c, a);
    }

    let mut tool_routes = BTreeMap::new();
    for (tool, owner) in &opts.tool_routes {
        tool_routes.insert((*tool).to_string(), cid(owner));
    }

    let frontend_attach = if opts.include_tui_frontend {
        Some(AttachId::new(TUI_ATTACH_ID).expect("attach id"))
    } else {
        None
    };
    let mut frontends = BTreeMap::new();
    if let Some(aid) = &frontend_attach {
        frontends.insert(aid.clone(), tui_frontend_acl(true));
    }

    let acl = BrokerAcl {
        plugins,
        tool_routes,
        frontends,
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_provider(provider_canonical.clone(), peer)
        .expect("provider registers");
    Box::leak(Box::new(guard));

    if let Some(c) = &readfile_canonical {
        let (peer, _rx) = fresh_peer();
        let guard = broker
            .register_plugin(c.clone(), peer)
            .expect("readfile plugin registers");
        Box::leak(Box::new(guard));
    }

    if let Some(aid) = &frontend_attach {
        let (peer, _rx) = fresh_peer();
        let guard = broker
            .register_frontend(aid.clone(), peer)
            .expect("tui frontend registers");
        Box::leak(Box::new(guard));
    }

    ReemitRig {
        broker,
        acl,
        provider_canonical,
        frontend_attach,
        readfile_canonical,
    }
}

/// A tight wrapper around `broker.subscribe_internal` pre-configured
/// with the four router patterns. Tests use it to observe inbound
/// events that the router would see.
pub fn subscribe_router_test_receiver(
    broker: &Broker,
) -> (mpsc::Receiver<BusEvent>, InternalSubscription) {
    broker.subscribe_internal(
        vec![
            "frontend.tui.user_message".to_string(),
            format!("provider.{MOCK_PROVIDER_ID}.**"),
            "plugin.*.tool_result".to_string(),
        ],
        64,
    )
}

/// Subscribe to canonical `core.session.**` + `core.lifecycle.**`
/// fan-out events emitted by the router.
pub fn subscribe_core_test_receiver(
    broker: &Broker,
) -> (mpsc::Receiver<BusEvent>, InternalSubscription) {
    broker.subscribe_internal(
        vec![
            "core.session.**".to_string(),
            "core.lifecycle.**".to_string(),
        ],
        64,
    )
}

/// Drain `rx` until an event with `topic` arrives or 2 seconds pass.
/// Other events seen along the way are pushed into `seen` so callers
/// can assert negative-set properties (e.g. "no canonical tool_request
/// emitted").
pub async fn await_topic(
    rx: &mut mpsc::Receiver<BusEvent>,
    topic: &str,
    seen: &mut Vec<BusEvent>,
) -> BusEvent {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {topic}; seen: {seen:?}"))
            .unwrap_or_else(|| panic!("channel closed before {topic}; seen: {seen:?}"));
        if event.topic == topic {
            return event;
        }
        seen.push(event);
    }
}

/// Drain the receiver for `duration` and return everything observed.
/// Used by negative tests to confirm an event was NOT emitted.
pub async fn drain_for(
    rx: &mut mpsc::Receiver<BusEvent>,
    duration: std::time::Duration,
) -> Vec<BusEvent> {
    let mut out = Vec::new();
    let deadline = tokio::time::Instant::now() + duration;
    loop {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(event)) => out.push(event),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    out
}

/// Assert the event carries exactly one taint entry with the given
/// source and (optional) detail.
pub fn assert_origin_taint(event: &BusEvent, source: &str, detail: Option<&str>) {
    let taint = event
        .taint
        .as_ref()
        .unwrap_or_else(|| panic!("event {} missing taint", event.topic));
    assert_eq!(
        taint.len(),
        1,
        "expected exactly one taint entry on {}, got {:?}",
        event.topic,
        taint
    );
    let entry = &taint[0];
    assert_eq!(entry.source, source, "taint source on {}", event.topic);
    assert_eq!(
        entry.detail.as_deref(),
        detail,
        "taint detail on {}",
        event.topic
    );
}
