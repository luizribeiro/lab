//! c20 acceptance: `bus.publish` notifications from the child are
//! routed through `FrontendBusPublishService` into
//! `Broker::handle_frontend_publish`. When the topic is outside the
//! frontend's grant set, the broker emits
//! `core.lifecycle.publish_rejected` with `code = "publish_outside_grant"`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::frontend_test_kit::{
    broker_with_attach_and_observer, fixture_plan, live_paths, KNOWN_ATTACH_ID,
};
use common::peer_test_kit::fresh_peer;
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};
use rafaello_core::lock::canonical_id::CanonicalId;

#[tokio::test(flavor = "multi_thread")]
async fn frontend_bus_publish_service_routes_to_handle_frontend_publish() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let observer = CanonicalId::parse("local:observer@0.0.0").expect("canonical id");
    let broker = broker_with_attach_and_observer(KNOWN_ATTACH_ID, &observer);

    let (peer, mut notifications) = fresh_peer();
    let _registered = broker
        .register_plugin(observer.clone(), peer)
        .expect("observer plugin registers");

    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let plan = fixture_plan(
        KNOWN_ATTACH_ID,
        "frontend_bus_publish",
        &[
            ("RFL_FIXTURE_MAX_LIFETIME", "5"),
            ("RFL_FIXTURE_PUBLISH_TOPIC", "frontend.tui.confirm_answer"),
        ],
    );
    let paths = live_paths(&tmp);
    let _handle = supervisor.spawn(&plan, &paths).await.expect("spawn ok");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for publish_rejected event");
        }
        let n = tokio::time::timeout(remaining, notifications.recv())
            .await
            .expect("observer channel timed out")
            .expect("observer channel closed");
        if n.method != "bus.event" {
            continue;
        }
        let topic = n.params.get("topic").and_then(|v| v.as_str()).unwrap_or("");
        if topic != "core.lifecycle.publish_rejected" {
            continue;
        }
        let payload = n.params.get("payload").cloned().unwrap_or_default();
        let code = payload.get("code").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(
            code, "publish_outside_grant",
            "payload.code mismatch: {payload}"
        );
        let inner_topic = payload.get("topic").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(inner_topic, "frontend.tui.confirm_answer");
        return;
    }
}
