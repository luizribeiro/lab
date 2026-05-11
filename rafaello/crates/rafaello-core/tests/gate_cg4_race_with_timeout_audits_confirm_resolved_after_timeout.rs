//! c22 §CG4 step 1 race-loser path: when CG5's
//! `try_take_for_timeout` already flipped the entry to `TimedOut`
//! before the `confirm_reply` is delivered, `try_resolve` returns
//! `None`; the gate audits `confirm_resolved_after_timeout` and
//! drops the reply without dispatching.

use std::time::Duration;

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig, publish_confirm_reply, seed_held};

#[tokio::test(flavor = "current_thread")]
async fn gate_cg4_race_with_timeout_audits_confirm_resolved_after_timeout() {
    let mut rig = build_gate_rig();
    let (confirm_id, _tool_request_id) =
        seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));

    let taken = rig.state.try_take_for_timeout(&confirm_id);
    assert!(taken.is_some(), "pre-condition: entry was Active");

    publish_confirm_reply(&rig.broker, &confirm_id, "allow");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let kinds = audit_kinds(&rig, &confirm_id);
            if kinds.contains(&"confirm_resolved_after_timeout".to_string()) {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("confirm_resolved_after_timeout audit observed");

    assert!(
        rig.peer_rx.try_recv().is_err(),
        "no dispatch on race-loser path",
    );

    let kinds = audit_kinds(&rig, &confirm_id);
    assert!(
        !kinds.contains(&"confirm_allowed".to_string()),
        "no confirm_allowed audit; got {kinds:?}",
    );
    assert!(
        !kinds.contains(&"confirm_denied".to_string()),
        "no confirm_denied audit; got {kinds:?}",
    );
}
