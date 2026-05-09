//! c19 SP4 Phase B post-spawn unwind — the child spawned at step
//! 13 must be SIGKILL'd and reaped before `spawn` returns the
//! step-18+ stub error. The supervisor records the reaped pid in
//! `TestHooks::last_reaped_pid`; on Linux the OS pid table no
//! longer lists the process after the reap completes.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};

#[tokio::test]
async fn spawn_reaps_child_after_post_register_unwind() {
    let entry = PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"));
    let canonical = CanonicalId::parse("local/test:plugin@0.1.0").unwrap();
    let real_topic = rafaello_core::topic_id::derive(&canonical.to_string());

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

    if sup.spawn(&plan, &paths).await.is_ok() {
        panic!("c19 spawn must return the step-18+ stub error");
    }

    assert_eq!(hooks.child_spawns.load(Ordering::SeqCst), 1);

    // Post-spawn unwind reaps synchronously inside `spawn`; poll
    // briefly to tolerate any test-runner scheduling latency.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut reaped: Option<u32> = None;
    while Instant::now() < deadline {
        if let Some(pid) = hooks.last_reaped_pid() {
            reaped = Some(pid);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let pid = reaped.expect("supervisor must record the reaped pid in TestHooks");

    #[cfg(target_os = "linux")]
    {
        let status_path = format!("/proc/{pid}/status");
        match std::fs::metadata(&status_path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => panic!("pid {pid} is still alive after reap (/proc/{pid}/status exists)"),
            Err(e) => panic!("unexpected error stat-ing /proc/{pid}/status: {e}"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
    }
}
