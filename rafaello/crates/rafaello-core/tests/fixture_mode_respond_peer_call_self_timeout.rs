//! c16 §L1 — the existing m2 `respond_peer_call` mode honors
//! the new `RFL_FIXTURE_MAX_LIFETIME` env: with `=1`, the
//! fixture self-exits 0 after one second even without SIGTERM
//! (no harness-side kill).

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};
use rafaello_core::error::ReaperOutcome;

#[tokio::test(flavor = "multi_thread")]
async fn respond_peer_call_self_timeout_exits_zero() {
    let spec = FixtureSpec::new("alpha", "respond_peer_call").env("RFL_FIXTURE_MAX_LIFETIME", "1");
    let canonical = spec.canonical.clone();
    let built = FixtureLockBuilder::new().add(spec).build();
    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle");
    let outcome = tokio::time::timeout(Duration::from_secs(10), handle.wait())
        .await
        .expect("self-timeout wait timed out");
    match &*outcome {
        ReaperOutcome::Exited(s) => assert_eq!(s.code(), Some(0), "self-timeout exits 0"),
        other => panic!("expected Exited(0), got {other:?}"),
    }
}
