//! c29 — lockin denial proof (write outside grant).
//!
//! Fixture A's only write grant is its private-state dir (auto-added
//! by m1 compile); the user-grant write_dirs is empty. The fixture is
//! asked to write `<project-root>/forbidden`. The lockin must refuse:
//! `ok == false` with errno in {EPERM, EACCES, ENOENT} (matched via
//! `matches!`), and the file must not exist on disk after.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::time::Duration;

use serde_json::json;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_lockin_denies_outside_grant_write() {
    let project_root_template = "${project}".to_string();
    let forbidden_template = "${project}/forbidden".to_string();

    let spec = FixtureSpec::new("denywrite", "respond_peer_call")
        .env("RFL_FIXTURE_WRITE_PATH", &forbidden_template)
        .with_read_dirs(vec![project_root_template])
        .with_write_dirs(Vec::new());
    let canonical = spec.canonical.clone();

    let built = FixtureLockBuilder::new().add(spec).build();
    let project_root = built.layout.project.path().to_path_buf();

    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(10))
        .await;

    let handle = harness.handles.get(&canonical).expect("handle present");

    let resolved_forbidden = project_root.join("forbidden");

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        handle.peer().call("core.fixture.try_write_path", json!({})),
    )
    .await
    .expect("try_write_path timed out")
    .expect("try_write_path rpc failed");

    let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
    assert!(!ok, "expected denied write, got: {result}");
    let errno = result.get("errno").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    const EPERM: i32 = 1;
    const ENOENT: i32 = 2;
    const EACCES: i32 = 13;
    assert!(
        matches!(errno, EPERM | EACCES | ENOENT),
        "expected EPERM/EACCES/ENOENT, got errno={errno} in {result}"
    );

    assert!(
        !resolved_forbidden.exists(),
        "forbidden file unexpectedly exists at {}",
        resolved_forbidden.display()
    );

    harness.kill_all();
    harness.wait_all().await;
}
