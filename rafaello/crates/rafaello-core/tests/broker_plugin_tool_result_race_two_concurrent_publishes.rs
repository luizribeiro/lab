#![allow(clippy::result_large_err)]
//! Two tasks concurrently publish `tool_result` from the same plugin
//! citing the same id; assert exactly one succeeds and exactly one
//! fails with `StaleRequestId`. The drain happens inside the broker
//! state lock so the intake check is atomic (scope §OM2, commits c10).

use std::collections::BTreeMap;
use std::sync::Arc;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn race_two_concurrent_publishes() {
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
    let broker = Arc::new(Broker::new(acl).expect("acl well-formed"));
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");

    let id = JsonRpcId::from("req-race");
    broker
        .publish_for_tool_dispatch(
            &canonical,
            serde_json::json!({}),
            id.clone(),
            None,
            None,
            Vec::new(),
        )
        .expect("dispatch ok");

    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-1"),
    });

    let b1 = Arc::clone(&broker);
    let c1 = canonical.clone();
    let p1 = params.clone();
    let b2 = Arc::clone(&broker);
    let c2 = canonical.clone();
    let p2 = params.clone();
    let t1 = tokio::task::spawn_blocking(move || b1.handle_plugin_publish(&c1, &p1));
    let t2 = tokio::task::spawn_blocking(move || b2.handle_plugin_publish(&c2, &p2));
    let r1 = t1.await.expect("join 1");
    let r2 = t2.await.expect("join 2");

    let results = [r1, r2];
    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    let stale_count = results
        .iter()
        .filter(|r| {
            matches!(
                r,
                Err(BrokerError::StaleRequestId { canonical: ref c, id: ref i })
                    if c == &canonical && i == &id
            )
        })
        .count();
    assert_eq!(ok_count, 1, "exactly one publish must succeed: {results:?}");
    assert_eq!(
        stale_count, 1,
        "exactly one publish must fail with StaleRequestId: {results:?}"
    );
}
