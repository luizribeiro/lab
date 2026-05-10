//! c20 acceptance: spawning a frontend creates the per-attach
//! private state dir under `${project_root}/.rafaello-frontend-data/`.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

mod common;

use common::frontend_test_kit::{broker_with_attach, fixture_plan, live_paths, KNOWN_ATTACH_ID};
use rafaello_core::frontend::{FrontendConfig, FrontendSupervisor};

#[tokio::test(flavor = "multi_thread")]
async fn frontend_spawn_creates_private_state_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let broker = broker_with_attach(KNOWN_ATTACH_ID);
    let supervisor = FrontendSupervisor::new(broker, FrontendConfig::default());
    let plan = fixture_plan(
        KNOWN_ATTACH_ID,
        "hold_silent",
        &[("RFL_FIXTURE_MAX_LIFETIME", "2")],
    );
    let paths = live_paths(&tmp);

    let handle = supervisor.spawn(&plan, &paths).await.expect("spawn ok");
    let expected = tmp
        .path()
        .join(".rafaello-frontend-data")
        .join(KNOWN_ATTACH_ID);
    assert!(
        expected.is_dir(),
        "private state dir should exist at {}",
        expected.display()
    );
    drop(handle);
}
