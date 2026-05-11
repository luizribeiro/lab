//! c15 / §CD1 — when the inbound `core.session.tool_request`
//! envelope carries the c12 referenced-union shape
//! (`provider-identity ∪ entries pulled from a prior tool_result
//! cited via in_reply_to`), the gate's `confirm_request` payload
//! preserves the full union in `details.taint`.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{build_gate_rig, MAILER_CANONICAL};

#[tokio::test(flavor = "current_thread")]
async fn gate_confirm_request_details_taint_carries_referenced_union() {
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
    let referenced_user_entry = TaintEntry {
        source: "user".to_string(),
        detail: None,
    };
    let inbound_taint = vec![provider_entry.clone(), referenced_user_entry.clone()];

    let prior_result_id = JsonRpcId::from(Ulid::new().to_string());
    let tool_call_id = JsonRpcId::from(Ulid::new().to_string());
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "a@b.c"},
                "dispatch_target": MAILER_CANONICAL,
            }),
            Some(tool_call_id.clone()),
            Some(vec![prior_result_id]),
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
        "details.taint preserves referenced-union (provider ∪ user-from-prior-result) verbatim",
    );
    assert!(details_taint.contains(&provider_entry));
    assert!(details_taint.contains(&referenced_user_entry));

    collector.abort();
}
