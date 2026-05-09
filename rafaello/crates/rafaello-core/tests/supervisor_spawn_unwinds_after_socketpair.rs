//! c17 SP4 Phase B steps 8–12 — spawn now creates a socketpair,
//! builds a `lockin::tokio::SandboxedCommand`, then bails at the
//! step-13 stub. This test asserts the unwind closes both halves
//! of the pair (parent fd count returns to baseline on Linux) and
//! that `socketpair_creates` was incremented exactly once.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};
use rafaello_core::{topic_id, SpawnError};

#[cfg(target_os = "linux")]
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .expect("read /proc/self/fd")
        .count()
}

#[tokio::test]
async fn spawn_unwinds_socketpair_and_returns_step_13_stub() {
    let entry = PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"));
    let canonical = CanonicalId::parse("local/test:plugin@0.1.0").unwrap();
    let real_topic = topic_id::derive(&canonical.to_string());

    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: real_topic.clone(),
            publish_topics: Vec::new(),
            subscribe_patterns: Vec::new(),
            auto_subscribes: vec![format!("plugin.{real_topic}.tool_request")],
            provider_id: None,
        },
    );
    let broker = Broker::new(BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
    })
    .unwrap();
    let sup = PluginSupervisor::new(broker, SupervisorConfig::default());
    let hooks = sup.test_hooks();

    let proj = tempfile::tempdir().unwrap();
    let plan = CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: real_topic.clone(),
        entry_absolute: entry,
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::Deny,
        env: EnvPlan::default(),
        limits: LimitsPlan {
            max_cpu_time: 5,
            max_open_files: 64,
            max_address_space: None,
            max_processes: None,
        },
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta: BTreeMap::new(),
        provider_id: None,
        load: LoadPolicy::default(),
        flags: CompiledFlags::default(),
    };
    let paths = SpawnPaths {
        project_root: proj.path().to_path_buf(),
        private_state_dir: proj.path().join(".rafaello-plugin-data").join(&real_topic),
    };

    #[cfg(target_os = "linux")]
    let baseline = open_fd_count();

    let err = match sup.spawn(&plan, &paths).await {
        Ok(_) => panic!("expected step-13 stub error"),
        Err(e) => e,
    };
    match err {
        SpawnError::SandboxBuild {
            canonical: c,
            source,
        } => {
            assert_eq!(c, canonical);
            let msg = format!("{source}");
            assert!(
                msg.contains("Phase B step 13+"),
                "expected step-13 stub message, got: {msg}"
            );
        }
        other => panic!("expected SandboxBuild step-13 stub, got {other:?}"),
    }

    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 0);

    #[cfg(target_os = "linux")]
    {
        let after = open_fd_count();
        assert_eq!(
            after, baseline,
            "open fd count should return to baseline after unwind \
             (baseline={baseline}, after={after})"
        );
    }
}
