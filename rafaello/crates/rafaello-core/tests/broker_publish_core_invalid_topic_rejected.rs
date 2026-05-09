//! `Broker::publish_core` enforces the §B5 grammar-before-namespace order
//! and the §B3 structural namespace check with `Publisher::Core`
//! (scope §B1, §B5, c13).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::error::Publisher;
use rafaello_core::BrokerError;

#[test]
fn publish_core_rejects_reserved_unknown_and_invalid_topics() {
    let broker = Broker::new(BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
    })
    .expect("empty acl is well-formed");

    let err = broker
        .publish_core("plugin.x.y", serde_json::json!({}))
        .expect_err("plugin.* must be rejected for core publisher");
    assert!(
        matches!(
            err,
            BrokerError::PublishOnReservedNamespace {
                publisher: Publisher::Core,
                ref topic,
            } if topic == "plugin.x.y"
        ),
        "expected PublishOnReservedNamespace {{ publisher: Core }}, got {err:?}"
    );

    let err = broker
        .publish_core("core.Bad", serde_json::json!({}))
        .expect_err("core.Bad must fail grammar before namespace check");
    assert!(
        matches!(
            err,
            BrokerError::InvalidTopic {
                publisher: Publisher::Core,
                ..
            }
        ),
        "expected InvalidTopic {{ publisher: Core }}, got {err:?}"
    );

    let err = broker
        .publish_core("evil.foo", serde_json::json!({}))
        .expect_err("unknown top-level segment must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::UnknownNamespace {
                publisher: Publisher::Core,
                ref topic,
            } if topic == "evil.foo"
        ),
        "expected UnknownNamespace {{ publisher: Core }}, got {err:?}"
    );
}
