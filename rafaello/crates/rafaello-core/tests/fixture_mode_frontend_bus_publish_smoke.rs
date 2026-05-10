//! c16 §L1a — `frontend_bus_publish` mode adopts `RFL_BUS_FD`,
//! signals `frontend.ready`, then notifies `bus.publish` with
//! the topic from `RFL_FIXTURE_PUBLISH_TOPIC`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::fixture_smoke::{spawn_fixture_with_bus, wait_for_method};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_bus_publish_calls_ready_then_notifies_bus_publish() {
    let topic = "frontend.tui.user_message";
    let mut smoke = spawn_fixture_with_bus(
        "frontend_bus_publish",
        &[
            ("RFL_FIXTURE_MAX_LIFETIME", "3"),
            ("RFL_FIXTURE_PUBLISH_TOPIC", topic),
        ],
    );

    let ready = wait_for_method(&mut smoke.events, "frontend.ready", Duration::from_secs(5)).await;
    assert!(!ready.is_notification, "frontend.ready must be peer-call");

    let publish = wait_for_method(&mut smoke.events, "bus.publish", Duration::from_secs(5)).await;
    assert!(
        publish.is_notification,
        "bus.publish must be a notification"
    );
    assert_eq!(
        publish.params.get("topic").and_then(|v| v.as_str()),
        Some(topic),
        "bus.publish payload must carry RFL_FIXTURE_PUBLISH_TOPIC"
    );
    assert_eq!(
        publish.params.get("payload").cloned(),
        Some(serde_json::json!({})),
        "bus.publish payload must be empty object"
    );

    let status = tokio::time::timeout(Duration::from_secs(5), smoke.child.wait())
        .await
        .expect("self-exit timed out")
        .expect("child wait");
    assert_eq!(status.code(), Some(0), "self-timeout exits 0");
}
