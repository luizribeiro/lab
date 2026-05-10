//! pi-1 B4 — supervisor applies `env.pass` (parent var forwarded)
//! and `env.set` (literal value), and reserved `RFL_*` vars are
//! present in the child env (scope §"Demo bar" positives).

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
async fn pass_set_applied() {
    let _guard = EnvGuard("FAKE_PUBLIC_ENV");
    std::env::set_var("FAKE_PUBLIC_ENV", "abc");

    let spec = FixtureSpec::new("envpass", "respond_peer_call").env(
        "RFL_FIXTURE_ENV_KEYS",
        "FAKE_PUBLIC_ENV,FOO,RFL_BUS_FD,RFL_PLUGIN,RFL_TOPIC_ID,RFL_PROJECT_ROOT,RFL_PRIVATE_STATE_DIR",
    );
    let canonical = spec.canonical.clone();
    let mut built = FixtureLockBuilder::new().add(spec).build();
    let plan = built
        .plans
        .iter_mut()
        .find(|p| p.canonical == canonical)
        .expect("plan present");
    plan.env.pass.push("FAKE_PUBLIC_ENV".to_string());
    plan.env.set.insert("FOO".to_string(), "bar".to_string());

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
        env.get("FAKE_PUBLIC_ENV").and_then(|v| v.as_str()),
        Some("abc"),
        "env.pass should forward parent value",
    );
    assert_eq!(
        env.get("FOO").and_then(|v| v.as_str()),
        Some("bar"),
        "env.set should apply literal value",
    );
    assert_eq!(
        env.get("RFL_BUS_FD").and_then(|v| v.as_str()),
        Some("3"),
        "RFL_BUS_FD reserved var should be set",
    );
    assert_eq!(
        env.get("RFL_PLUGIN").and_then(|v| v.as_str()),
        Some(canonical.to_string().as_str()),
        "RFL_PLUGIN reserved var should be canonical id",
    );
    for key in ["RFL_TOPIC_ID", "RFL_PROJECT_ROOT", "RFL_PRIVATE_STATE_DIR"] {
        assert!(
            env.get(key)
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "{key} reserved var should be present and non-empty",
        );
    }

    harness.kill_all();
    harness.wait_all().await;
}
