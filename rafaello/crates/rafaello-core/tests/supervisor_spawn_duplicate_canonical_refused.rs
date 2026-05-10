//! c24 — pi-1 B5 real duplicate-spawn refusal. After a real spawn
//! through c21, a second `supervisor.spawn` for the same canonical
//! fails with `SpawnError::AlreadyRegistered` *before* any resource
//! allocation: no socketpair, no proxy, no child. The Phase A
//! `in_flight` + `try_reserve_registration` path catches it.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use std::sync::atomic::Ordering;
use std::time::Duration;

use rafaello_core::supervisor::SpawnPaths;
use rafaello_core::SpawnError;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Spawn, SpawnOptions};

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_spawn_duplicate_canonical_refused() {
    let spec = FixtureSpec::new("alpha", "respond_peer_call");
    let canonical = spec.canonical.clone();
    let built = FixtureLockBuilder::new().add(spec).build();

    let plan_clone = built
        .plans
        .iter()
        .find(|p| p.canonical == canonical)
        .expect("plan for canonical")
        .clone();
    let project_root = built.layout.project.path().to_path_buf();
    let paths_clone = SpawnPaths {
        project_root: project_root.clone(),
        private_state_dir: project_root
            .join(".rafaello-plugin-data")
            .join(&plan_clone.topic_id),
    };

    let harness = Spawn::launch(built, SpawnOptions::default()).await;
    harness
        .readiness
        .wait_for(&canonical, Duration::from_secs(5))
        .await;

    let hooks = harness.supervisor.test_hooks();
    let outpost_before = hooks.outpost_starts.load(Ordering::SeqCst);
    let socketpair_before = hooks.socketpair_creates.load(Ordering::SeqCst);
    let child_before = hooks.child_spawns.load(Ordering::SeqCst);

    let err = match harness.supervisor.spawn(&plan_clone, &paths_clone).await {
        Ok(_) => panic!("expected duplicate spawn to fail"),
        Err(e) => e,
    };
    assert!(
        matches!(&err, SpawnError::AlreadyRegistered(c) if c == &canonical),
        "expected AlreadyRegistered({canonical}), got {err:?}",
    );

    assert_eq!(
        hooks.outpost_starts.load(Ordering::SeqCst),
        outpost_before,
        "outpost_starts must not tick on duplicate refusal",
    );
    assert_eq!(
        hooks.socketpair_creates.load(Ordering::SeqCst),
        socketpair_before,
        "socketpair_creates must not tick on duplicate refusal",
    );
    assert_eq!(
        hooks.child_spawns.load(Ordering::SeqCst),
        child_before,
        "child_spawns must not tick on duplicate refusal",
    );

    harness.kill_all();
    harness.wait_all().await;
}
