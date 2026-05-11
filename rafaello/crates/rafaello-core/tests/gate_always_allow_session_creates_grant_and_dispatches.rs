//! c22 §CG4 + pi-4 B-1: when `try_resolve` returns
//! `Some((held, true))` (re-emit flagged via
//! `mark_session_grant_requested`), the gate inserts a structural
//! `UserGrant`, audits `grant_added`, dispatches the held call,
//! and audits `confirm_allowed_with_session_grant`.

use std::time::Duration;

use rafaello_core::user_grants::{GrantMatcher, GrantSource};

mod common;
use common::gate_test_kit::{audit_kinds, build_gate_rig, publish_confirm_reply, seed_held};

#[tokio::test(flavor = "current_thread")]
async fn gate_always_allow_session_creates_grant_and_dispatches() {
    let mut rig = build_gate_rig();
    let (confirm_id, _tool_request_id) =
        seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));

    rig.state
        .mark_session_grant_requested(&confirm_id)
        .expect("entry is Active");

    publish_confirm_reply(&rig.broker, &confirm_id, "allow");

    let _notification = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let Some(n) = rig.peer_rx.recv().await {
                return n;
            }
        }
    })
    .await
    .expect("peer receives dispatch within timeout");

    let grants = rig
        .user_grants
        .read()
        .list()
        .into_iter()
        .map(|(_, g)| g.clone())
        .collect::<Vec<_>>();
    assert_eq!(grants.len(), 1, "exactly one grant added");
    assert_eq!(grants[0].tool, "send_mail");
    assert_eq!(grants[0].plugin, rig.target);
    assert!(matches!(grants[0].source, GrantSource::AlwaysAllowSession));
    match &grants[0].matcher {
        GrantMatcher::Structural { template } => {
            assert_eq!(template, &serde_json::json!({"to": "a@b.c"}));
        }
        other => panic!("expected Structural matcher, got {other:?}"),
    }

    let kinds = audit_kinds(&rig, &confirm_id);
    let grant_added_pos = kinds.iter().position(|k| k == "grant_added");
    let allowed_pos = kinds
        .iter()
        .position(|k| k == "confirm_allowed_with_session_grant");
    assert!(
        grant_added_pos.is_some() && allowed_pos.is_some(),
        "expected grant_added + confirm_allowed_with_session_grant; got {kinds:?}",
    );
    assert!(
        grant_added_pos.unwrap() < allowed_pos.unwrap(),
        "grant_added precedes confirm_allowed_with_session_grant in audit log",
    );
}
