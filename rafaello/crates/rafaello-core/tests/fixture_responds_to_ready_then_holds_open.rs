//! c20 — fixture in `respond_peer_call` mode emits the readiness
//! signal then holds open until SIGTERM. Refactored in c23 to use
//! the m2 integration-test harness; the c22 inline socketpair +
//! Server plumbing is gone.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::{json, Value};

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn fixture_responds_to_ready_then_holds_open() {
    let spec = FixtureSpec::new("alpha", "respond_peer_call");
    let canonical = spec.canonical.clone();
    let built = FixtureLockBuilder::new().add(spec).build();
    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle");
    let start = handle
        .peer()
        .call("core.fixture.start", json!({}))
        .await
        .expect("start ack");
    assert_eq!(start, Value::Null);

    let echo = handle
        .peer()
        .call("core.fixture.echo", json!({"x": 1}))
        .await
        .expect("echo");
    assert_eq!(echo, json!({"x": 1}));

    let pid = handle.child_pid().expect("pid before exit");
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .expect("sigkill");
    tokio::time::timeout(Duration::from_secs(5), handle.wait())
        .await
        .expect("wait timed out");
    assert!(handle.child_pid().is_none(), "child_pid clears after exit");
}
