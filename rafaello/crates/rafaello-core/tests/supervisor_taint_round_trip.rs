//! c30 — taint array round-trips byte-equal through the broker.
//! A in `publish_with_taint` mode publishes
//! `plugin.<A>.tainted` with a configured `taint` array; B in
//! `observer` mode receives the event; the test asserts
//! `event.taint` matches the configured value byte-for-byte.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::{json, Value};

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Observer, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_taint_round_trip() {
    let taint_json = r#"[{"source":"test","detail":"x"}]"#;
    let expected_taint: Value = serde_json::from_str(taint_json).unwrap();

    let publisher = FixtureSpec::new("publisher", "publish_with_taint")
        .env("RFL_FIXTURE_PAYLOAD_JSON", r#"{"k":"v"}"#)
        .env("RFL_FIXTURE_TAINT_JSON", taint_json);
    let publisher_topic_id = publisher.topic_id();
    let publish_topic = format!("plugin.{publisher_topic_id}.tainted");
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
        event.get("taint").cloned(),
        Some(expected_taint),
        "taint mismatch: {event}"
    );

    harness.kill_all();
    harness.wait_all().await;
}
