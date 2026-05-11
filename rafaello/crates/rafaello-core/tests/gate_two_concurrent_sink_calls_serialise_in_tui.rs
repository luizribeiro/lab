//! c24 §CG7: two `tool_request` events arrive in quick succession;
//! both are held with their own confirm correlation ids; resolving
//! one leaves the other Active. The TUI's overlay (c25) is what
//! actually serialises the prompts — the gate just maintains an
//! unbounded held queue.

use std::time::Duration;

use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{build_gate_rig, publish_confirm_reply};

async fn publish_send_mail(
    broker: &rafaello_core::bus::Broker,
    target: &rafaello_core::lock::canonical_id::CanonicalId,
    to: &str,
) -> JsonRpcId {
    let id = JsonRpcId::from(Ulid::new().to_string());
    broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": to},
                "dispatch_target": target.to_string(),
            }),
            Some(id.clone()),
            None,
            Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            None,
        )
        .expect("publish tool_request");
    id
}

#[tokio::test(flavor = "current_thread")]
async fn gate_two_concurrent_sink_calls_serialise_in_tui() {
    let rig = build_gate_rig();
    let (mut confirm_rx, _sub) = rig
        .broker
        .subscribe_internal(vec!["core.session.confirm_request".to_string()], 16);

    let _tr1 = publish_send_mail(&rig.broker, &rig.target, "a@b.c").await;
    let confirm1: BusEvent = tokio::time::timeout(Duration::from_secs(1), confirm_rx.recv())
        .await
        .expect("first confirm_request observed")
        .expect("channel open");
    let confirm_id1 = confirm1.request_id.clone().expect("request_id");

    let _tr2 = publish_send_mail(&rig.broker, &rig.target, "x@y.z").await;
    let confirm2: BusEvent = tokio::time::timeout(Duration::from_secs(1), confirm_rx.recv())
        .await
        .expect("second confirm_request observed")
        .expect("channel open");
    let confirm_id2 = confirm2.request_id.clone().expect("request_id");

    let snapshot = rig.state.active_entries_snapshot();
    assert_eq!(
        snapshot.len(),
        2,
        "both held entries Active in the gate's map; got {snapshot:?}",
    );

    publish_confirm_reply(&rig.broker, &confirm_id1, "deny");

    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if !rig.state.is_held(&confirm_id1) {
            break;
        }
        if std::time::Instant::now() >= deadline {
            panic!("first held entry should have been resolved");
        }
        tokio::task::yield_now().await;
    }

    assert!(
        rig.state.is_held(&confirm_id2),
        "the second held entry must remain Active",
    );
    let snapshot = rig.state.active_entries_snapshot();
    assert_eq!(snapshot.len(), 1, "exactly one Active entry remains");
}
