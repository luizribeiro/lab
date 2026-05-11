//! c15 / §CD1 — when the inbound `core.session.tool_request`
//! envelope carries the c12 value-driven union shape
//! (`provider-identity ∪ tool-source value-match`), the gate's
//! `confirm_request` payload preserves the full union in
//! `details.taint`.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{build_gate_rig, MAILER_CANONICAL};

#[tokio::test(flavor = "current_thread")]
async fn gate_confirm_request_details_taint_carries_value_driven_union() {
    let rig = build_gate_rig();

    let confirm_rx = Arc::new(Mutex::new(Vec::<BusEvent>::new()));
    let (mut internal_rx, _sub) = rig
        .broker
        .subscribe_internal(vec!["core.session.confirm_request".to_string()], 16);
    let confirm_rx_for_task = Arc::clone(&confirm_rx);
    let collector = tokio::spawn(async move {
        while let Some(event) = internal_rx.recv().await {
            confirm_rx_for_task.lock().push(event);
        }
    });

    let provider_entry = TaintEntry {
        source: "provider".to_string(),
        detail: Some("mock-provider".to_string()),
    };
    let value_match_entry = TaintEntry {
        source: "tool".to_string(),
        detail: Some("local/test:readfile@0.1.0".to_string()),
    };
    let inbound_taint = vec![provider_entry.clone(), value_match_entry.clone()];

    let tool_call_id = JsonRpcId::from(Ulid::new().to_string());
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "a@b.c", "body": "match-me-please-32bytes"},
                "dispatch_target": MAILER_CANONICAL,
            }),
            Some(tool_call_id.clone()),
            None,
            Some(inbound_taint.clone()),
            None,
        )
        .expect("publish tool_request");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !confirm_rx.lock().is_empty() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("confirm_request observed within timeout");

    let confirm_event = confirm_rx.lock().clone().into_iter().next().unwrap();
    let details_taint: Vec<TaintEntry> =
        serde_json::from_value(confirm_event.payload["details"]["taint"].clone())
            .expect("details.taint parses as Vec<TaintEntry>");
    assert_eq!(
        details_taint, inbound_taint,
        "details.taint preserves value-driven union (provider ∪ tool-source) verbatim",
    );
    assert!(details_taint.contains(&provider_entry));
    assert!(details_taint.contains(&value_match_entry));

    collector.abort();
}
