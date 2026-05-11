//! c24 §CG7: hold entry A; then the operator answers
//! `always_allow_session` on a separate held entry B whose grant
//! structurally covers A. The gate's CG4 grant-creation site walks
//! the held map post-add, dispatches A via
//! `publish_for_tool_dispatch`, and audits
//! `gate_grant_match_short_circuit`.

use std::time::Duration;

use fittings_core::context::OutboundNotification;

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig, publish_confirm_reply, seed_held};

async fn drain_dispatches(
    rx: &mut tokio::sync::mpsc::Receiver<OutboundNotification>,
    expect: usize,
) -> Vec<OutboundNotification> {
    let mut out = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while out.len() < expect {
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .unwrap_or(Duration::ZERO);
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(n)) => out.push(n),
            _ => break,
        }
    }
    out
}

#[tokio::test(flavor = "current_thread")]
async fn gate_grant_short_circuits_pending_held_entry() {
    let mut rig = build_gate_rig();

    let (confirm_a, _tr_a) = seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));
    let (confirm_b, _tr_b) = seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));

    rig.state
        .mark_session_grant_requested(&confirm_b)
        .expect("B is Active");

    publish_confirm_reply(&rig.broker, &confirm_b, "allow");

    let dispatches = drain_dispatches(&mut rig.peer_rx, 2).await;
    assert_eq!(
        dispatches.len(),
        2,
        "B's allow dispatch + A's short-circuit dispatch both fire; got {dispatches:?}",
    );

    let kinds_a = audit_kinds(&rig, &confirm_a);
    assert!(
        kinds_a.contains(&"gate_grant_match_short_circuit".to_string()),
        "A's audit contains the short-circuit row; got {kinds_a:?}",
    );

    let grants = rig.user_grants.read().list().len();
    assert_eq!(grants, 1, "exactly one grant added");
}
