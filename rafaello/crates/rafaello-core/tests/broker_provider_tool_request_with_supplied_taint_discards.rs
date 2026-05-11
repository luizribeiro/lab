//! Per scope §B6 step 8 (pi-3 B-1 discard+replace), any `taint` field
//! supplied on a provider publish is stripped: the emitted inbound
//! `BusEvent.taint` is `None`. Observed via the c11 internal-subscriber
//! channel (pi-2 M-1).

#![cfg(feature = "test-fixture")]

use rafaello_core::bus::JsonRpcId;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn supplied_taint_discarded() {
    let (broker, canonical) = provider_broker();
    let observed = JsonRpcId::from("tr-1");
    broker.seed_provider_observed_result_for_test(&canonical, observed.clone());

    let (mut rx, _guard) = broker.subscribe_internal(vec!["provider.**".to_string()], 8);

    let topic = "provider.mock.tool_request";
    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [observed],
        "request_id": JsonRpcId::from("req-1"),
        "taint": [{"source": "user", "detail": null}],
    });
    broker
        .handle_provider_publish(&canonical, &params)
        .expect("publish accepted");

    let event = rx.try_recv().expect("internal subscriber received event");
    assert!(
        event.taint.is_none(),
        "provider-supplied taint must be discarded; got {:?}",
        event.taint
    );
    assert!(
        rx.try_recv().is_err(),
        "exactly one event delivered to the internal subscriber"
    );
}
