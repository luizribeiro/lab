//! c23 §CG5: after the 60 s deadline a held confirmation auto-denies.
//! Under `tokio::time::pause`, advancing past 60 s fires the timer
//! task → `try_take_for_timeout` → synthetic deny `tool_result`
//! (`error == "confirm_timeout"`) and audit `confirm_timeout`.

use std::time::Duration;

use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig};

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn gate_times_out_to_deny_after_60s() {
    let rig = build_gate_rig();

    let (mut confirm_rx, _csub) = rig
        .broker
        .subscribe_internal(vec!["core.session.confirm_request".to_string()], 16);
    let (mut result_rx, _rsub) = rig
        .broker
        .subscribe_internal(vec!["core.session.tool_result".to_string()], 16);

    let tool_request_id = JsonRpcId::from(Ulid::new().to_string());
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "a@b.c"},
                "dispatch_target": rig.target.to_string(),
            }),
            Some(tool_request_id.clone()),
            None,
            Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            None,
        )
        .expect("publish tool_request");

    let confirm_event: BusEvent = tokio::time::timeout(Duration::from_secs(1), confirm_rx.recv())
        .await
        .expect("confirm_request observed")
        .expect("confirm_request channel open");
    let confirm_id = confirm_event
        .request_id
        .clone()
        .expect("confirm_request carries request_id");

    tokio::time::advance(Duration::from_secs(61)).await;

    let result_event: BusEvent = tokio::time::timeout(Duration::from_secs(1), result_rx.recv())
        .await
        .expect("synthetic deny tool_result observed")
        .expect("tool_result channel open");
    assert_eq!(result_event.payload["ok"], false);
    assert_eq!(result_event.payload["error"], "confirm_timeout");
    assert_eq!(
        result_event
            .in_reply_to
            .as_ref()
            .and_then(|v| v.first())
            .expect("in_reply_to[0] present"),
        &tool_request_id,
    );

    let kinds = audit_kinds(&rig, &confirm_id);
    assert!(
        kinds.contains(&"confirm_timeout".to_string()),
        "expected confirm_timeout audit; got {kinds:?}",
    );
}
