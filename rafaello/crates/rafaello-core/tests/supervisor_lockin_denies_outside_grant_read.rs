//! c29 — lockin denial proof (read outside grant).
//!
//! Fixture A is granted `read_dirs = ["${project}"]` only and asked to
//! `std::fs::read("/etc/passwd")` from inside the sandbox. The lockin
//! must refuse: `ok == false` with errno in {EPERM, EACCES, ENOENT}
//! (matched via `matches!` to tolerate syd / seatbelt / OS variance).
//!
//! The scope §"Demo bar" lists `/etc/hosts` as the canonical
//! cross-platform target; on NixOS dev hosts `/etc/hosts` is a
//! symlink into `/nix/store`, which the plugin's exec-dirs grant
//! covers recursively, so the read would succeed. Per scope
//! §"Platform gating" (pi runtime-extensibility tweak) `/etc/passwd`
//! is the equivalent picked here: a real file on both Linux and
//! macOS that is never under `/nix/store`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_lockin_denies_outside_grant_read() {
    let spec = FixtureSpec::new("denyread", "respond_peer_call")
        .env("RFL_FIXTURE_OPEN_PATH", "/etc/passwd")
        .with_read_dirs(vec!["${project}".to_string()]);
    let canonical = spec.canonical.clone();

    let built = FixtureLockBuilder::new().add(spec).build();
    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(10))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle present");
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        handle
            .peer()
            .call("core.fixture.report_open_result", json!({})),
    )
    .await
    .expect("report_open_result timed out")
    .expect("report_open_result rpc failed");

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
    assert!(!ok, "expected denied open, got: {result}");
    let errno = result.get("errno").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    const EPERM: i32 = 1;
    const ENOENT: i32 = 2;
    const EACCES: i32 = 13;
    assert!(
        matches!(errno, EPERM | EACCES | ENOENT),
        "expected EPERM/EACCES/ENOENT, got errno={errno} in {result}"
    );

    harness.kill_all();
    harness.wait_all().await;
}
