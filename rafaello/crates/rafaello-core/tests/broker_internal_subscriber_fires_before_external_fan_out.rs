//! Internal subscribers observe an event **before** external recipient
//! peers do. Verified by recording the order in which two `recv` tasks
//! become ready on a single-threaded tokio runtime, which preserves
//! FIFO wake order: the broker calls `notify_internal_subscribers`
//! before the external plugin-recipient loop in `fan_out`. Scope §B7,
//! pi-2 M-1.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn internal_fires_before_external() {
    let canonical = CanonicalId::parse("local/test:ext@0.1.0").expect("canonical");
    let topic_id = "ext_local_test";
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec!["core.**".to_string()],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (peer, mut external_rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("plugin registers");

    let (mut internal_rx, _isub) = broker.subscribe_internal(vec!["core.**".to_string()], 8);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

    rt.block_on(async {
        let order_i = order.clone();
        let internal_task = tokio::spawn(async move {
            if internal_rx.recv().await.is_some() {
                order_i.lock().push("internal");
            }
        });
        let order_e = order.clone();
        let external_task = tokio::spawn(async move {
            if external_rx.recv().await.is_some() {
                order_e.lock().push("external");
            }
        });

        // Let both tasks park on `recv().await` before publishing.
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }

        broker
            .publish_core("core.lifecycle.boot", serde_json::json!({}))
            .expect("publish accepted");

        let _ = tokio::time::timeout(Duration::from_millis(500), async {
            let _ = internal_task.await;
            let _ = external_task.await;
        })
        .await;
    });

    let observed = order.lock().clone();
    assert_eq!(
        observed,
        vec!["internal", "external"],
        "internal subscriber must observe the event before the external recipient"
    );
}
