//! c15 / §CD1 — when the inbound `core.session.tool_request`
//! envelope carries `taint = Some([provider-identity])` (the
//! base case before any value-match or referenced union enriches
//! it), the gate's `confirm_request` payload preserves it verbatim
//! in `details.taint`.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{build_gate_rig, MAILER_CANONICAL};

#[tokio::test(flavor = "current_thread")]
async fn gate_confirm_request_details_taint_provider_only() {
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
    let inbound_taint = vec![provider_entry.clone()];

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
    assert_eq!(
        confirm_event.payload["details"]["taint"],
        serde_json::to_value(&inbound_taint).unwrap(),
        "details.taint forwards provider-only inbound taint verbatim",
    );
    assert_eq!(
        confirm_event.payload["details"]["taint"]
            .as_array()
            .expect("array")
            .len(),
        1,
    );

    collector.abort();
}
