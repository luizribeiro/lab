//! c22 §CG4: on `core.session.confirm_reply { answer: "allow" }`,
//! the gate consumes the held `tool_request` and dispatches it via
//! `publish_for_tool_dispatch`. Audit row `confirm_allowed` (no
//! grant created when `session_grant_requested == false`).

use std::time::Duration;

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig, publish_confirm_reply, seed_held};

#[tokio::test(flavor = "current_thread")]
async fn gate_dispatches_on_allow() {
    let mut rig = build_gate_rig();
    let (confirm_id, tool_request_id) =
        seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));

    publish_confirm_reply(&rig.broker, &confirm_id, "allow");

    let notification = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(n) = rig.peer_rx.recv().await {
                return n;
            }
        }
    })
    .await
    .expect("peer receives dispatch within timeout");

    assert_eq!(
        notification.params["payload"]["tool"],
        serde_json::json!("send_mail")
    );
    assert_eq!(
        notification.params["payload"]["args"]["to"],
        serde_json::json!("a@b.c")
    );
    assert_eq!(
        notification.params["request_id"],
        serde_json::json!(tool_request_id.to_string()),
        "dispatch forwards the held tool_request's id, not a fresh one",
    );

    let kinds = audit_kinds(&rig, &confirm_id);
    assert!(
        kinds.contains(&"confirm_allowed".to_string()),
        "expected confirm_allowed audit; got {kinds:?}",
    );
    assert!(
        !kinds.contains(&"grant_added".to_string()),
        "no grant on plain allow; got {kinds:?}",
    );
    assert_eq!(rig.user_grants.read().list().len(), 0);
}
