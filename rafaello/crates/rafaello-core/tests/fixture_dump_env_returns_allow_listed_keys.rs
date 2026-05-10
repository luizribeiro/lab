//! c23 (deferred from c22) — fixture in `respond_peer_call`
//! mode answers `core.fixture.dump_env` with the env values for
//! the keys allow-listed via `RFL_FIXTURE_ENV_KEYS`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn dump_env_returns_allow_listed_keys() {
    let spec = FixtureSpec::new("dumper", "respond_peer_call")
        .env("RFL_FIXTURE_ENV_KEYS", "RFL_BUS_FD,RFL_PLUGIN");
    let canonical = spec.canonical.clone();
    let built = FixtureLockBuilder::new().add(spec).build();
    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle");
    let response = handle
        .peer()
        .call("core.fixture.dump_env", json!({}))
        .await
        .expect("dump_env call");

    let env = response.get("env").expect("env field present");
    let bus_fd = env
        .get("RFL_BUS_FD")
        .and_then(|v| v.as_str())
        .expect("RFL_BUS_FD value");
    assert_eq!(bus_fd, "3", "RFL_BUS_FD should be 3");

    let plugin = env
        .get("RFL_PLUGIN")
        .and_then(|v| v.as_str())
        .expect("RFL_PLUGIN value");
    assert_eq!(
        plugin,
        canonical.to_string(),
        "RFL_PLUGIN should be canonical id"
    );

    harness.kill_all();
    harness.wait_all().await;
}
