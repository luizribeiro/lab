//! Per scope §B6 step 8 (pi-3 B-1 discard+replace), any `taint` field
//! supplied on a provider publish is stripped: the emitted inbound
//! `BusEvent.taint` is `None` (c10).

#![cfg(feature = "test-fixture")]

use rafaello_core::bus::JsonRpcId;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn supplied_taint_discarded() {
    let (broker, canonical) = provider_broker();
    let observed = JsonRpcId::from("tr-1");
    broker.seed_provider_observed_result_for_test(&canonical, observed.clone());

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

    let events = broker.drain_inbound_provider_events_for_test();
    assert_eq!(events.len(), 1);
    assert!(
        events[0].taint.is_none(),
        "provider-supplied taint must be discarded; got {:?}",
        events[0].taint
    );
}
