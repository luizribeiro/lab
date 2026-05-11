//! c23 §CG5 + §CG7 partial: two held entries each get their own
//! 60 s timer. Under `tokio::time::pause`, staggering the two
//! reserves by 10 s means advancing 61 s fires only the first;
//! a further 11 s advance then fires the second.

use std::time::Duration;

use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig};

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

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn gate_per_held_timeout_independent() {
    let rig = build_gate_rig();

    let (mut confirm_rx, _csub) = rig
        .broker
        .subscribe_internal(vec!["core.session.confirm_request".to_string()], 16);
    let (mut result_rx, _rsub) = rig
        .broker
        .subscribe_internal(vec!["core.session.tool_result".to_string()], 16);

    let tr1 = publish_send_mail(&rig.broker, &rig.target, "a@b.c").await;
    let confirm1: BusEvent = tokio::time::timeout(Duration::from_secs(1), confirm_rx.recv())
        .await
        .expect("first confirm_request observed")
        .expect("channel open");
    let confirm_id1 = confirm1.request_id.clone().expect("request_id");

    tokio::time::advance(Duration::from_secs(10)).await;

    let tr2 = publish_send_mail(&rig.broker, &rig.target, "x@y.z").await;
    let confirm2: BusEvent = tokio::time::timeout(Duration::from_secs(1), confirm_rx.recv())
        .await
        .expect("second confirm_request observed")
        .expect("channel open");
    let confirm_id2 = confirm2.request_id.clone().expect("request_id");

    tokio::time::advance(Duration::from_secs(51)).await;

    let first_result: BusEvent = tokio::time::timeout(Duration::from_secs(1), result_rx.recv())
        .await
        .expect("first timeout's tool_result observed")
        .expect("channel open");
    assert_eq!(first_result.payload["error"], "confirm_timeout");
    assert_eq!(
        first_result
            .in_reply_to
            .as_ref()
            .and_then(|v| v.first())
            .expect("in_reply_to[0]"),
        &tr1,
        "first result must carry the first tool_request id",
    );

    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(
        result_rx.try_recv().is_err(),
        "second held entry must still be waiting on its own deadline",
    );
    let kinds1 = audit_kinds(&rig, &confirm_id1);
    assert!(
        kinds1.contains(&"confirm_timeout".to_string()),
        "first audit recorded; got {kinds1:?}",
    );
    let kinds2 = audit_kinds(&rig, &confirm_id2);
    assert!(
        !kinds2.contains(&"confirm_timeout".to_string()),
        "second must not have timed out yet; got {kinds2:?}",
    );

    tokio::time::advance(Duration::from_secs(11)).await;

    let second_result: BusEvent = tokio::time::timeout(Duration::from_secs(1), result_rx.recv())
        .await
        .expect("second timeout's tool_result observed")
        .expect("channel open");
    assert_eq!(second_result.payload["error"], "confirm_timeout");
    assert_eq!(
        second_result
            .in_reply_to
            .as_ref()
            .and_then(|v| v.first())
            .expect("in_reply_to[0]"),
        &tr2,
        "second result must carry the second tool_request id",
    );
    let kinds2 = audit_kinds(&rig, &confirm_id2);
    assert!(
        kinds2.contains(&"confirm_timeout".to_string()),
        "second audit recorded after its own deadline; got {kinds2:?}",
    );
}
