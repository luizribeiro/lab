//! c30 — `RFL_PRIVATE_STATE_DIR` is writable. A in `respond_peer_call`
//! mode; harness peer-calls `core.fixture.write_private_state`; the
//! fixture writes `<RFL_PRIVATE_STATE_DIR>/marker` and the test
//! verifies the file exists at
//! `<project_root>/.rafaello-plugin-data/<topic_id>/marker`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_private_state_dir_writable() {
    let spec = FixtureSpec::new("statewriter", "respond_peer_call");
    let canonical = spec.canonical.clone();
    let topic_id = spec.topic_id();

    let built = FixtureLockBuilder::new().add(spec).build();
    let project_root = built.layout.project.path().to_path_buf();
    let expected_path = project_root
        .join(".rafaello-plugin-data")
        .join(&topic_id)
        .join("marker");

    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle");
    let result = handle
        .peer()
        .call("core.fixture.write_private_state", json!({}))
        .await
        .expect("write_private_state ack");

    assert_eq!(
        result.get("wrote").and_then(|v| v.as_str()),
        Some(expected_path.to_string_lossy().as_ref()),
        "fixture wrote at unexpected path: {result}"
    );
    assert!(
        expected_path.exists(),
        "marker file missing at {}",
        expected_path.display()
    );

    harness.kill_all();
    harness.wait_all().await;
}
