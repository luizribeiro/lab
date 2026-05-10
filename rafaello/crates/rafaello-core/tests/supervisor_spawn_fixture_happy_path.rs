//! c30 — canonical scope-named headline test (pi-2 B5).
//! Two fixtures across the c23 harness: A in `publish_one` publishes
//! `plugin.<A>.hello` with payload `{"msg":"hi"}`; B in `observer`
//! mode subscribed to `Observer::watch_all`. The harness handshake
//! gates both readiness signals, then `core.fixture.start` triggers
//! A's publish. Observer receives one `bus.event` matching topic,
//! payload, and `publisher == Plugin { canonical, topic_id }`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Observer, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_spawn_fixture_happy_path() {
    let publisher = FixtureSpec::new("publisher", "publish_one")
        .env("RFL_FIXTURE_PAYLOAD_JSON", r#"{"msg":"hi"}"#);
    let publisher_topic_id = publisher.topic_id();
    let publish_topic = format!("plugin.{publisher_topic_id}.hello");
    let publisher = publisher
        .publishes(vec![publish_topic.clone()])
        .env("RFL_FIXTURE_TOPIC", &publish_topic);

    let observer = FixtureSpec::new("watcher", "observer").subscribes(Observer::watch_all());

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
        Some(json!({"msg": "hi"})),
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
