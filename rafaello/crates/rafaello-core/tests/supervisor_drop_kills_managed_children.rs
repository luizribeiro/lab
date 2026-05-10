//! c26 SP5 / pi-1 §28 / pi-6 nb#4 — `Drop for PluginSupervisor`
//! delivers best-effort SIGKILL to every managed child, drops
//! the supervisor-owned `RegisteredPlugin` (clearing broker
//! registration) and `ProxyHandle`, aborts the serve loops, and
//! hands off to the reaper task. External `SpawnHandle` clones
//! still observe the cached `ReaperOutcome` because they only
//! hold `Arc<SpawnObservation>` (pi-1 B2).

#![cfg(all(feature = "test-fixture", target_os = "linux"))]

use std::os::unix::process::ExitStatusExt;
use std::time::Duration;

mod common;

use common::m2_harness::{FixtureLockBuilder, FixtureSpec, Harness, Spawn, SpawnOptions};
use rafaello_core::error::ReaperOutcome;

#[tokio::test(flavor = "multi_thread")]
async fn drop_kills_managed_children_and_clears_registration() {
    let built = FixtureLockBuilder::new()
        .add(FixtureSpec::new("alpha", "respond_peer_call"))
        .add(FixtureSpec::new("beta", "respond_peer_call"))
        .build();

    let harness = Spawn::launch(built, SpawnOptions::default()).await;

    let canonicals: Vec<_> = harness.handles.keys().cloned().collect();
    for c in &canonicals {
        harness.readiness.wait_for(c, Duration::from_secs(5)).await;
    }
    let canonical_a = canonicals[0].clone();
    let canonical_b = canonicals[1].clone();

    let twin = harness.handles[&canonical_a].clone();
    let pid_a = harness.handles[&canonical_a]
        .child_pid()
        .expect("pid_a cached");
    let pid_b = harness.handles[&canonical_b]
        .child_pid()
        .expect("pid_b cached");

    let Harness {
        broker,
        supervisor,
        handles,
        layout: _layout,
        ..
    } = harness;
    drop(handles);
    drop(supervisor);

    let pids = [pid_a, pid_b];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let all_gone = pids.iter().all(|p| {
            matches!(
                nix::sys::signal::kill(nix::unistd::Pid::from_raw(*p as i32), None),
                Err(nix::errno::Errno::ESRCH),
            )
        });
        if all_gone {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("fixture pids still alive after supervisor drop: {pids:?}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let outcome = tokio::time::timeout(Duration::from_secs(5), twin.wait())
        .await
        .expect("clone wait timed out");
    match &*outcome {
        ReaperOutcome::Exited(s) => {
            assert_eq!(s.signal(), Some(9), "expected SIGKILL, got {s:?}");
        }
        other => panic!("expected Exited(SIGKILL), got {other:?}"),
    }

    broker
        .try_reserve_registration(&canonical_a)
        .expect("canonical_a registration cleared");
    broker
        .try_reserve_registration(&canonical_b)
        .expect("canonical_b registration cleared");
}
