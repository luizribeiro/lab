//! c06 — scope §M1.3 + pi-3 B-6. `broker_acl::compile`
//! auto-inserts `plugin.<topic-id>.tool_result` into
//! `PluginAcl.publish_topics` for any plugin with non-empty
//! `bindings.tools`. Closes the "non-existent placeholder
//! substitution" issue surfaced by pi-1 B-6.

mod common;

use rafaello_core::broker_acl;
use rafaello_core::lock::SessionTable;
use rafaello_core::topic_id;

use common::{canonical, entry, lock_with};

#[test]
fn tool_plugin_gets_auto_tool_result_publish() {
    let id = canonical("github.com/acme:read-file@1.0.0");
    let topic = topic_id::derive(&id.to_string());

    let e = entry(&["read-file"], false, None);
    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());

    let acl = broker_acl::compile(&lock).expect("broker_acl::compile succeeds");

    let plugin = acl.plugins.get(&id).expect("plugin acl present");
    let expected = format!("plugin.{}.tool_result", topic);
    assert!(
        plugin.publish_topics.contains(&expected),
        "expected publish_topics to contain {expected}, got {:?}",
        plugin.publish_topics
    );
}
