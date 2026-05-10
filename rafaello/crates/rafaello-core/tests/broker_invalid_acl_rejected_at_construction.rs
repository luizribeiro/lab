//! `Broker::new` runs §B10 defence-in-depth grammar revalidation
//! on every `BrokerAcl` entry and rejects hand-constructed ACLs
//! that bypassed `broker_acl::compile` (scope §B10, c07).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

fn acl_with_overrides(publish_topics: Vec<String>, subscribe_patterns: Vec<String>) -> BrokerAcl {
    let canonical = cid("local/test:plug@0.1.0");
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical,
        PluginAcl {
            topic_id: "plug_local_test".to_string(),
            publish_topics,
            subscribe_patterns,
            auto_subscribes: vec!["plugin.plug_local_test.tool_request".to_string()],
            provider_id: None,
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    }
}

#[test]
fn standalone_double_star_subscribe_pattern_rejected() {
    // `**` alone is a single-segment pattern — `validate_pattern`
    // requires ≥ 2 segments. §B10 catches a hand-built ACL that
    // bypassed `broker_acl::compile` here.
    let acl = acl_with_overrides(vec![], vec!["**".to_string()]);
    let err = Broker::new(acl).expect_err("standalone `**` is invalid grammar");
    assert!(
        matches!(err, BrokerError::InvalidPattern { .. }),
        "expected InvalidPattern, got {err:?}",
    );
}

#[test]
fn invalid_topic_literal_in_publish_topics_rejected() {
    // Uppercase segment is illegal under the topic-segment grammar.
    let acl = acl_with_overrides(vec!["plugin.id.UPPER".to_string()], vec![]);
    let err = Broker::new(acl).expect_err("uppercase topic segment is invalid");
    assert!(
        matches!(err, BrokerError::InvalidTopic { .. }),
        "expected InvalidTopic, got {err:?}",
    );
}

#[test]
fn double_star_in_non_final_position_rejected() {
    // `**` is permitted only as the final segment.
    let acl = acl_with_overrides(vec![], vec!["plugin.**.foo".to_string()]);
    let err = Broker::new(acl).expect_err("non-final `**` is invalid grammar");
    assert!(
        matches!(err, BrokerError::InvalidPattern { .. }),
        "expected InvalidPattern, got {err:?}",
    );
}
