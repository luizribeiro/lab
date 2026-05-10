//! c30 — cross-plugin round-trip variant: B subscribes to the
//! exact topic `plugin.<A>.greet` (an explicit grant, not the
//! broad `plugin.**` from the headline test). Confirms grant-based
//! fan-out works alongside the wildcard pattern case.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_bus_publish_round_trip_two_plugins() {
    let publisher = FixtureSpec::new("publisher", "publish_one")
        .env("RFL_FIXTURE_PAYLOAD_JSON", r#"{"hello":"world"}"#);
    let publisher_topic_id = publisher.topic_id();
    let publish_topic = format!("plugin.{publisher_topic_id}.greet");
    let publisher = publisher
        .publishes(vec![publish_topic.clone()])
        .env("RFL_FIXTURE_TOPIC", &publish_topic);

    let observer = FixtureSpec::new("watcher", "observer").subscribes(vec![publish_topic.clone()]);

    let publisher_canonical = publisher.canonical.clone();
    let observer_canonical = observer.canonical.clone();

    let built = FixtureLockBuilder::new()
        .add(publisher)
        .add(observer)
        .build();

    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&publisher_canonical, Duration::from_secs(15))
        .await;
    harness
        .readiness
        .wait_for(&observer_canonical, Duration::from_secs(15))
        .await;

    let publisher_handle = harness
        .handles
        .get(&publisher_canonical)
        .expect("publisher handle");
    publisher_handle
        .peer()
        .call("core.fixture.start", json!({}))
        .await
        .expect("start publisher");

    let event = harness.observer.next_event(Duration::from_secs(15)).await;
    assert_eq!(
        event.get("topic").and_then(|v| v.as_str()),
        Some(publish_topic.as_str()),
        "event topic mismatch: {event}"
    );
    assert_eq!(
        event.get("payload").cloned(),
        Some(json!({"hello": "world"})),
        "event payload mismatch: {event}"
    );
    assert_eq!(
        event.get("publisher").cloned(),
        Some(json!({
            "kind": "plugin",
            "canonical": publisher_canonical.to_string(),
            "topic_id": publisher_topic_id,
        })),
        "event publisher mismatch: {event}"
    );

    harness.kill_all();
    harness.wait_all().await;
}
