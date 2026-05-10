//! Frontend publishing on `core.*`, `provider.*`, or `plugin.*` is
//! rejected as `PublishOnReservedNamespace` (scope §B4, c15).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

fn assert_reserved(bad_topic: &str) {
    let attach_id = AttachId::new("ui").expect("attach id");
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach_id.clone(),
        FrontendAcl {
            subscribe_patterns: ["core.session.**".to_string()].into_iter().collect(),
            auto_subscribes: Default::default(),
            publish_topics: ["frontend.ui.confirm_answer".to_string()]
                .into_iter()
                .collect(),
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_frontend(attach_id.clone(), peer)
        .expect("registration succeeds");

    let params = serde_json::json!({"topic": bad_topic, "payload": {}});
    let err = broker
        .handle_frontend_publish(&attach_id, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad_topic),
        "expected PublishOnReservedNamespace for {bad_topic:?}, got {err:?}"
    );
}

#[test]
fn core_namespace_rejected() {
    assert_reserved("core.session.attach");
}

#[test]
fn provider_namespace_rejected() {
    assert_reserved("provider.openai.chat");
}

#[test]
fn plugin_namespace_rejected() {
    assert_reserved("plugin.foo_local_test.event");
}
