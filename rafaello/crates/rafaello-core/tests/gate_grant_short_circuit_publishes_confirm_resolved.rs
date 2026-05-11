//! c24 / pi-1 M-1: when CG4's `always_allow_session` path
//! short-circuits a pending held entry, the gate publishes
//! `core.session.confirm_resolved` with `reason:
//! "grant_short_circuit"`, payload `request_id == <confirm_id>`,
//! envelope `in_reply_to == [<confirm_id>]`. This is the
//! bus-visible signal the TUI's c25 subscriber prunes its
//! queued prompts on.

use std::time::Duration;

use rafaello_core::bus::CORE_SESSION_CONFIRM_RESOLVED;

mod common;
use common::gate_test_kit::{build_gate_rig, publish_confirm_reply, seed_held};

#[tokio::test(flavor = "current_thread")]
async fn gate_grant_short_circuit_publishes_confirm_resolved() {
    let rig = build_gate_rig();
    let (mut resolved_rx, _sub) = rig
        .broker
        .subscribe_internal(vec![CORE_SESSION_CONFIRM_RESOLVED.to_string()], 8);

    let (confirm_a, _) = seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));
    let (confirm_b, _) = seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));
    rig.state
        .mark_session_grant_requested(&confirm_b)
        .expect("B is Active");

    publish_confirm_reply(&rig.broker, &confirm_b, "allow");

    let event = tokio::time::timeout(Duration::from_secs(1), resolved_rx.recv())
        .await
        .expect("confirm_resolved observed within timeout")
        .expect("channel open");
    assert_eq!(event.topic, CORE_SESSION_CONFIRM_RESOLVED);
    assert_eq!(
        event.payload.get("reason").and_then(|v| v.as_str()),
        Some("grant_short_circuit"),
    );
    assert_eq!(
        event.payload.get("request_id").and_then(|v| v.as_str()),
        Some(confirm_a.to_string().as_str()),
    );
    let in_reply_to = event.in_reply_to.as_ref().expect("in_reply_to present");
    assert_eq!(in_reply_to.len(), 1);
    assert_eq!(&in_reply_to[0], &confirm_a);

    assert!(
        resolved_rx.try_recv().is_err(),
        "exactly one confirm_resolved event fires for the single short-circuited entry",
    );
}
