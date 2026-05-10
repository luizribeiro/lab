//! A frontend subscriber receives `core.session.**` events the same way
//! plugins do, via `peer.notify("bus.event", value)` (scope §B5, c15).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn frontend_receives_core_session_event() {
    let attach_id = AttachId::new("ui").expect("attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id.clone(),
        FrontendAcl {
            subscribe_patterns: ["core.session.**".to_string()].into_iter().collect(),
            auto_subscribes: Default::default(),
            publish_topics: Default::default(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, mut rx) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let topic = "core.session.attach";
    let payload = serde_json::json!({"attach_id": attach_id.as_str()});
    broker
        .publish_core(topic, payload.clone())
        .expect("core publish succeeds");

    let notification = rx
        .try_recv()
        .expect("frontend receives one bus.event notification");
    assert_eq!(notification.method, "bus.event");

    let event = &notification.params;
    assert_eq!(event["topic"], serde_json::Value::String(topic.to_string()));
    assert_eq!(event["payload"], payload);
    assert_eq!(event["publisher"], serde_json::json!({"kind": "core"}));

    assert!(
        rx.try_recv().is_err(),
        "no further notifications follow the single publish"
    );
}
