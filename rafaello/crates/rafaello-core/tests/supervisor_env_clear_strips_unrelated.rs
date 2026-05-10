//! pi-1 B4 — supervisor's pre-spawn `env_clear()` strips parent
//! vars that are not in `env.pass` or `env.set` (scope §"Demo bar"
//! positives, §SP4).

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;
use serial_test::serial;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

struct EnvGuard(&'static str);
impl Drop for EnvGuard {
    fn drop(&mut self) {
        std::env::remove_var(self.0);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial(env)]
async fn clear_strips_unrelated() {
    let _guard = EnvGuard("RANDOM_PARENT_VAR");
    std::env::set_var("RANDOM_PARENT_VAR", "secret");

    let spec = FixtureSpec::new("envclear", "respond_peer_call")
        .env("RFL_FIXTURE_ENV_KEYS", "RANDOM_PARENT_VAR,RFL_PLUGIN");
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

    assert!(
        env.get("RANDOM_PARENT_VAR").is_none(),
        "parent var not in env.pass/env.set must be stripped by env_clear, got {:?}",
        env.get("RANDOM_PARENT_VAR"),
    );
    assert_eq!(
        env.get("RFL_PLUGIN").and_then(|v| v.as_str()),
        Some(canonical.to_string().as_str()),
        "sanity: dump_env still returns reserved RFL_PLUGIN",
    );

    harness.kill_all();
    harness.wait_all().await;
}
