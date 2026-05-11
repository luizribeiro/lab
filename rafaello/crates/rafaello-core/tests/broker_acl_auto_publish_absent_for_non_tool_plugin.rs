//! c06 — scope §M1.3 + pi-3 B-6. A plugin with empty
//! `bindings.tools` MUST NOT receive a compiler-inserted
//! `*.tool_result` auto-publish — the insertion is gated on the
//! plugin actually declaring tools.

mod common;

use rafaello_core::broker_acl;
use rafaello_core::lock::SessionTable;

use common::{canonical, entry, lock_with};

#[test]
fn non_tool_plugin_has_no_tool_result_auto_publish() {
    let id = canonical("github.com/acme:observer@1.0.0");

    let mut e = entry(&[], false, None);
    e.grant.publishes = vec![];
    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());

    let acl = broker_acl::compile(&lock).expect("broker_acl::compile succeeds");

    let plugin = acl.plugins.get(&id).expect("plugin acl present");
    assert!(
        !plugin
            .publish_topics
            .iter()
            .any(|t| t.ends_with(".tool_result")),
        "non-tool plugin must not have a *.tool_result auto-publish, got {:?}",
        plugin.publish_topics
    );
}
