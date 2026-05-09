//! c14 stub: `PluginSupervisor::spawn` returns
//! `SpawnError::SandboxBuild { .. }` because the real
//! implementation lands in c15+. This test is deleted in
//! c15 when Phase A is wired up.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan,
};
use rafaello_core::lock::{CanonicalId, LoadPolicy};
use rafaello_core::supervisor::{PluginSupervisor, SpawnPaths, SupervisorConfig};
use rafaello_core::SpawnError;

fn cid() -> CanonicalId {
    CanonicalId::parse("local/test:plugin@0.1.0").expect("canonical id parses")
}

fn placeholder_plan() -> CompiledPlugin {
    CompiledPlugin {
        canonical: cid(),
        topic_id: "plugin_local_test".to_string(),
        entry_absolute: PathBuf::from("/nonexistent/bin/fixture"),
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::default(),
        env: EnvPlan::default(),
        limits: LimitsPlan::default(),
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta: BTreeMap::new(),
        provider_id: None,
        load: LoadPolicy::default(),
        flags: CompiledFlags::default(),
    }
}

#[tokio::test]
async fn spawn_stub_returns_sandbox_build_error() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("empty ACL is valid");
    let sup = PluginSupervisor::new(broker, SupervisorConfig::default());

    let plan = placeholder_plan();
    let paths = SpawnPaths {
        project_root: PathBuf::from("/tmp/rafaello-c14-project"),
        private_state_dir: PathBuf::from("/tmp/rafaello-c14-private"),
    };

    match sup.spawn(&plan, &paths).await {
        Ok(_) => panic!("c14 stub never returns Ok"),
        Err(SpawnError::SandboxBuild { canonical, source }) => {
            assert_eq!(canonical, cid());
            assert!(
                source.to_string().contains("not yet implemented"),
                "unexpected source error: {source}",
            );
        }
        Err(other) => panic!("expected SandboxBuild, got {other:?}"),
    }
}
