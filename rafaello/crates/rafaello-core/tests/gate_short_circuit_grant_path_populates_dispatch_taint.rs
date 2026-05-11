//! c04 — the §CG7 short-circuit walk (after CG4 inserts an
//! always_allow_session grant) also reaches
//! `publish_for_tool_dispatch`, so the inserted
//! `OutstandingDispatch` entry must carry the canonical
//! `tool_request_taint`.

#![cfg(feature = "test-fixture")]

use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::lock::canonical_id::CanonicalId;

mod common;
use common::gate_test_kit::{build_gate_rig, publish_confirm_reply, seed_held, MAILER_CANONICAL};

#[tokio::test(flavor = "current_thread")]
async fn short_circuit_grant_path_populates_dispatch_taint() {
    let rig = build_gate_rig();

    let (_confirm_a, tool_request_a) =
        seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));
    let (confirm_b, _tool_request_b) =
        seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));

    rig.state
        .mark_session_grant_requested(&confirm_b)
        .expect("B is Active");

    publish_confirm_reply(&rig.broker, &confirm_b, "allow");

    let target = CanonicalId::parse(MAILER_CANONICAL).expect("canonical");
    let entry = wait_for_outstanding(&rig.broker, &target, &tool_request_a).await;
    assert_eq!(
        entry.tool_request_taint,
        vec![TaintEntry {
            source: "user".to_string(),
            detail: None,
        }],
        "short-circuit dispatch carries A's canonical tool_request taint",
    );
}

async fn wait_for_outstanding(
    broker: &rafaello_core::bus::Broker,
    canonical: &CanonicalId,
    id: &JsonRpcId,
) -> rafaello_core::bus::OutstandingDispatch {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(entry) = broker.peek_outstanding_for_test(canonical, id) {
            return entry;
        }
        if std::time::Instant::now() >= deadline {
            panic!("outstanding entry never populated for {id:?}");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
