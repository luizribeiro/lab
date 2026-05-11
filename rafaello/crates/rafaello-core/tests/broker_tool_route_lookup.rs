//! c19 / scope §TD1: `Broker::tool_route` is a thin accessor over
//! `BrokerAcl.tool_routes`. Returns `Some(canonical)` for a declared
//! tool and `None` for an unknown name.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

#[test]
fn tool_route_returns_expected_canonical() {
    let readfile = CanonicalId::parse("local/test:readfile@0.1.0").expect("canonical");
    let plugin = PluginAcl {
        topic_id: "readfile_local_test".to_string(),
        publish_topics: vec!["plugin.readfile_local_test.tool_result".to_string()],
        subscribe_patterns: vec![],
        auto_subscribes: vec!["plugin.readfile_local_test.tool_request".to_string()],
        provider_id: None,
    };
    let mut plugins = BTreeMap::new();
    plugins.insert(readfile.clone(), plugin);
    let mut tool_routes = BTreeMap::new();
    tool_routes.insert("read-file".to_string(), readfile.clone());
    let acl = BrokerAcl {
        plugins,
        tool_routes,
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    assert_eq!(broker.tool_route("read-file"), Some(readfile));
    assert_eq!(broker.tool_route("unknown-tool"), None);
}
