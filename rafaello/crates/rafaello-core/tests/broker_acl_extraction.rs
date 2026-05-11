//! c35 — `broker_acl::compile` extracts per-plugin publish /
//! subscribe authority, the auto-inserted
//! `plugin.<topic-id>.tool_request` self-subscribe, and the
//! provider id for the provider plugin (G1).

mod common;

use rafaello_core::broker_acl;
use rafaello_core::lock::SessionTable;
use rafaello_core::topic_id;

use common::{canonical, entry, lock_with};

#[test]
fn two_plugin_lock_extracts_per_plugin_acl() {
    let id_a = canonical("github.com/acme:writer@1.0.0");
    let id_b = canonical("github.com/acme:llm@2.0.0");

    let topic_a = topic_id::derive(&id_a.to_string());
    let topic_b = topic_id::derive(&id_b.to_string());

    let mut a = entry(&["write_doc"], false, None);
    a.grant.publishes = vec![format!("plugin.{}.doc_written", topic_a)];
    a.grant.subscribes = vec!["core.session.tool_result".to_owned()];

    let mut b = entry(&["chat"], true, Some("openai"));
    b.grant.publishes = vec![format!("plugin.{}.completion", topic_b)];
    b.grant.subscribes = vec![format!("plugin.{}.**", topic_a)];

    let lock = lock_with(
        vec![(id_a.clone(), a), (id_b.clone(), b)],
        SessionTable::default(),
    );

    let acl = broker_acl::compile(&lock).expect("broker_acl::compile succeeds");

    let plugin_a = acl.plugins.get(&id_a).expect("plugin A acl present");
    assert_eq!(plugin_a.topic_id, topic_a);
    assert_eq!(
        plugin_a.publish_topics,
        vec![
            format!("plugin.{}.doc_written", topic_a),
            format!("plugin.{}.tool_result", topic_a),
        ]
    );
    assert_eq!(
        plugin_a.subscribe_patterns,
        vec!["core.session.tool_result".to_owned()]
    );
    assert_eq!(
        plugin_a.auto_subscribes,
        vec![format!("plugin.{}.tool_request", topic_a)]
    );
    assert_eq!(plugin_a.provider_id, None);

    let plugin_b = acl.plugins.get(&id_b).expect("plugin B acl present");
    assert_eq!(plugin_b.topic_id, topic_b);
    assert_eq!(
        plugin_b.publish_topics,
        vec![
            format!("plugin.{}.completion", topic_b),
            format!("plugin.{}.tool_result", topic_b),
        ]
    );
    assert_eq!(
        plugin_b.subscribe_patterns,
        vec![format!("plugin.{}.**", topic_a)]
    );
    assert_eq!(
        plugin_b.auto_subscribes,
        vec![format!("plugin.{}.tool_request", topic_b)]
    );
    assert_eq!(plugin_b.provider_id, Some("openai".to_owned()));

    assert_eq!(acl.tool_routes.get("write_doc"), Some(&id_a));
    assert_eq!(acl.tool_routes.get("chat"), Some(&id_b));
}
