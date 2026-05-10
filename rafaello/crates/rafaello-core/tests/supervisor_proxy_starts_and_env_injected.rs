//! c28 — proxy startup + env injection end-to-end. Spawns a single
//! fixture under `NetworkPlan::Proxy` and asserts the supervisor
//! both starts an outpost (`outpost_starts == 1`) and injects the
//! full uppercase + lowercase `*_PROXY` env set, empty `NO_PROXY`
//! aliases, and `RFL_BUS_FD = "3"` into the child.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};
use rafaello_core::compile::NetworkPlan;

#[tokio::test(flavor = "multi_thread")]
async fn proxy_starts_and_env_injected() {
    let spec = FixtureSpec::new("proxied", "respond_peer_call")
        .with_network_plan(NetworkPlan::Proxy {
            allow_hosts: vec!["example.com".to_string()],
        })
        .env(
            "RFL_FIXTURE_ENV_KEYS",
            "HTTP_PROXY,HTTPS_PROXY,ALL_PROXY,NO_PROXY,http_proxy,https_proxy,all_proxy,no_proxy,RFL_BUS_FD",
        );
    let canonical = spec.canonical.clone();
    let built = FixtureLockBuilder::new().add(spec).build();
    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let hooks = harness.supervisor.test_hooks();
    assert_eq!(
        hooks.outpost_starts.load(Ordering::SeqCst),
        1,
        "expected exactly one outpost_proxy start under NetworkPlan::Proxy"
    );
    let port = hooks.last_proxy_port.load(Ordering::SeqCst);
    assert_ne!(port, 0, "expected last_proxy_port to be set");
    let proxy_url = format!("http://127.0.0.1:{port}");

    let handle = harness.handles.get(&canonical).expect("handle");
    let response = handle
        .peer()
        .call("core.fixture.dump_env", json!({}))
        .await
        .expect("dump_env call");
    let env = response.get("env").expect("env field present");

    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        let val = env
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{key} missing from dump_env"));
        assert_eq!(val, proxy_url, "{key} should be {proxy_url}");
    }

    for key in ["NO_PROXY", "no_proxy"] {
        let val = env
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{key} missing from dump_env"));
        assert_eq!(val, "", "{key} should be empty");
    }

    let bus_fd = env
        .get("RFL_BUS_FD")
        .and_then(|v| v.as_str())
        .expect("RFL_BUS_FD value");
    assert_eq!(bus_fd, "3", "RFL_BUS_FD should be 3");

    harness.kill_all();
    harness.wait_all().await;
}
