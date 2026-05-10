//! pi-1 B4 — when both `env.pass` and `env.set` name the same key,
//! the literal `env.set` value wins (scope §"Demo bar" positives).

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
async fn set_overrides_pass() {
    let _guard = EnvGuard("FOO_VAR");
    std::env::set_var("FOO_VAR", "pass-loses");

    let spec =
        FixtureSpec::new("envoverride", "respond_peer_call").env("RFL_FIXTURE_ENV_KEYS", "FOO_VAR");
    let canonical = spec.canonical.clone();
    let mut built = FixtureLockBuilder::new().add(spec).build();
    let plan = built
        .plans
        .iter_mut()
        .find(|p| p.canonical == canonical)
        .expect("plan present");
    plan.env.pass.push("FOO_VAR".to_string());
    plan.env
        .set
        .insert("FOO_VAR".to_string(), "set-wins".to_string());

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

    assert_eq!(
        env.get("FOO_VAR").and_then(|v| v.as_str()),
        Some("set-wins"),
        "env.set should override env.pass when both name the same key",
    );

    harness.kill_all();
    harness.wait_all().await;
}
