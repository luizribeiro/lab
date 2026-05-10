//! c23 (deferred from c22) — fixture in `call_core_then_exit`
//! mode peer-calls `core.fixture.ping` on core, exits 0 on Ok.
//! The harness's extra service registers `core.fixture.ping`
//! returning `{"echo": "ok"}`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};
use rafaello_core::error::ReaperOutcome;

#[tokio::test(flavor = "multi_thread")]
async fn call_core_then_exit_completes() {
    let spec = FixtureSpec::new("caller", "call_core_then_exit");
    let canonical = spec.canonical.clone();
    let built = FixtureLockBuilder::new().add(spec).build();

    let opts = SpawnOptions {
        ping: Some(Arc::new(|_params| json!({"echo": "ok"}))),
    };
    let harness = Spawn::launch(built, opts).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle");
    handle
        .peer()
        .call("core.fixture.start", json!({}))
        .await
        .expect("start ack");

    let outcome = tokio::time::timeout(Duration::from_secs(5), handle.wait())
        .await
        .expect("wait timed out");
    match &*outcome {
        ReaperOutcome::Exited(s) => assert_eq!(s.code(), Some(0), "expected exit 0"),
        other => panic!("expected Exited(0), got {other:?}"),
    }

    harness.kill_all();
    harness.wait_all().await;
}
