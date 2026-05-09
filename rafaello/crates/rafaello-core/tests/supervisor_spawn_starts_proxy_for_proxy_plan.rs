//! c17 SP4 Phase B step 9 — under `NetworkPlan::Proxy`, spawn
//! starts an `outpost_proxy` listener (counter increments to 1)
//! and the listener stops accepting connections shortly after
//! `ProxyHandle` is dropped on unwind.

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
use rafaello_core::topic_id;

#[tokio::test]
async fn spawn_with_proxy_plan_starts_proxy_then_unbinds_after_unwind() {
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
        network: NetworkPlan::Proxy {
            allow_hosts: vec!["example.com".to_string()],
        },
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
        panic!("expected step-13 stub error");
    }

    assert_eq!(hooks.outpost_starts.load(Ordering::SeqCst), 1);
    assert_eq!(hooks.socketpair_creates.load(Ordering::SeqCst), 1);

    let port = hooks.last_proxy_port.load(Ordering::SeqCst);
    assert_ne!(port, 0, "expected last_proxy_port to be set");

    // ProxyHandle::drop is async — poll the loopback port until
    // it stops accepting (or the timeout elapses).
    let deadline = Instant::now() + Duration::from_millis(500);
    let addr = format!("127.0.0.1:{port}");
    let mut unbound = false;
    while Instant::now() < deadline {
        let connect = tokio::time::timeout(
            Duration::from_millis(50),
            tokio::net::TcpStream::connect(&addr),
        )
        .await;
        match connect {
            Ok(Err(_)) => {
                unbound = true;
                break;
            }
            Ok(Ok(stream)) => drop(stream),
            Err(_) => {}
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        unbound,
        "proxy port {port} never stopped accepting within 500ms"
    );
}
