//! c31 §OP2 item 2 — signature compile check:
//! `PluginSupervisor::new` accepts `Arc<ToolSchemaCatalog>` as its
//! third argument.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::supervisor::{PluginSupervisor, SupervisorConfig, ToolSchemaCatalog};

#[test]
fn supervisor_new_accepts_tool_catalog_arg() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("empty ACL is valid");
    let catalog = ToolSchemaCatalog::empty_for_tests();
    let _sup = PluginSupervisor::new(broker, SupervisorConfig::default(), catalog);
}
