//! c23 §CG5: if `try_resolve` (answer arm) runs first, the timeout
//! task's `try_take_for_timeout` returns `None` — the timer must
//! exit silently: no second `tool_result` publish and no
//! `confirm_timeout` audit.

use std::time::Duration;

use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig, publish_confirm_reply};

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn gate_timeout_after_resolve_is_noop() {
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

    publish_confirm_reply(&rig.broker, &confirm_id, "allow");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let kinds = audit_kinds(&rig, &confirm_id);
            if kinds.contains(&"confirm_allowed".to_string()) {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("confirm_allowed audit observed before timeout fires");

    tokio::time::advance(Duration::from_secs(61)).await;
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }

    assert!(
        result_rx.try_recv().is_err(),
        "no synthetic deny tool_result published when answer arm won the race",
    );
    let kinds = audit_kinds(&rig, &confirm_id);
    assert!(
        !kinds.contains(&"confirm_timeout".to_string()),
        "no confirm_timeout audit; got {kinds:?}",
    );
}
