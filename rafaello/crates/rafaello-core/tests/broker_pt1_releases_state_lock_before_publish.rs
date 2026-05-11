//! c14 / §PT1 / pi-3 M-2 — the §PT1 violation path drops the `state`
//! lock before performing the synthetic-deny publish or the lifecycle
//! publish. Were the lock still held, `fan_out`'s recipient-collection
//! lock acquisition would deadlock when the publish path is observed
//! by a subscriber that re-enters the broker.
//!
//! We exercise this by installing an internal subscriber on
//! `core.session.tool_result` whose handler synchronously inspects
//! `outstanding_dispatched_count` on the broker (which itself acquires
//! the `state` lock). The whole `handle_plugin_publish` call is run
//! under a 2-second timeout: a regression that holds the lock across
//! the synthetic publish would block forever inside the re-entrant
//! lookup.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn pt1_releases_state_lock_before_publish() {
    let canonical = CanonicalId::parse("local/test:plug@0.1.0").expect("canonical");
    let topic_id = "plug_local_test";
    let topic = format!("plugin.{topic_id}.tool_result");
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![topic.clone()],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id}.tool_request")],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");

    let observed = Arc::new(AtomicBool::new(false));
    let (mut synthetic_rx, _sub) =
        broker.subscribe_internal(vec!["core.session.tool_result".to_string()], 4);
    let probe_broker = broker.clone();
    let probe_canonical = canonical.clone();
    let observed_handle = observed.clone();
    let probe = thread::spawn(move || {
        if synthetic_rx.blocking_recv().is_some() {
            // Re-enter the broker's `state` lock — this would block
            // indefinitely if the §PT1 publish path held it.
            let _ = probe_broker.outstanding_dispatched_count(&probe_canonical);
            observed_handle.store(true, Ordering::SeqCst);
        }
    });

    let id = JsonRpcId::from("req-c14f");
    broker
        .publish_for_tool_dispatch(
            &canonical,
            serde_json::json!({}),
            id.clone(),
            None,
            None,
            vec![TaintEntry {
                source: "tool".to_string(),
                detail: Some("rafaello-fetch".to_string()),
            }],
        )
        .expect("dispatch ok");

    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": ""},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-c14f"),
        "taint": [{"source": "user", "detail": null}],
    });

    let done = Arc::new(AtomicBool::new(false));
    let done_check = done.clone();
    let watchdog = thread::spawn(move || {
        let start = std::time::Instant::now();
        while !done_check.load(Ordering::SeqCst) {
            if start.elapsed() > Duration::from_secs(2) {
                panic!("§PT1 publish deadlocked — lock not released");
            }
            thread::sleep(Duration::from_millis(20));
        }
    });

    let _ = broker.handle_plugin_publish(&canonical, &params);
    done.store(true, Ordering::SeqCst);
    watchdog.join().expect("watchdog joins");

    let _ = probe.join();
    assert!(
        observed.load(Ordering::SeqCst),
        "re-entrant probe must have observed the synthetic publish and re-acquired the state lock"
    );
}
