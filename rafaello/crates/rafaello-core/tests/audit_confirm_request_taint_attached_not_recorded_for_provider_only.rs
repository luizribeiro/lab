//! c17 / §AL1 — when the gate fires a `confirm_request` whose
//! canonical taint vector contains only `source = "provider"`
//! entries, the §AL1 predicate does not fire: no
//! `confirm_request_taint_attached` audit row is written. The
//! existing `confirm_request` row keeps its m5a shape.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{BusEvent, JsonRpcId, TaintEntry};
use ulid::Ulid;

mod common;
use common::gate_test_kit::{build_gate_rig, MAILER_CANONICAL};

#[tokio::test(flavor = "current_thread")]
async fn audit_confirm_request_taint_attached_not_recorded_for_provider_only() {
    let rig = build_gate_rig();

    let confirm_rx = Arc::new(Mutex::new(Vec::<BusEvent>::new()));
    let (mut internal_rx, _sub) = rig
        .broker
        .subscribe_internal(vec!["core.session.confirm_request".to_string()], 16);
    let confirm_rx_for_task = Arc::clone(&confirm_rx);
    let collector = tokio::spawn(async move {
        while let Some(event) = internal_rx.recv().await {
            confirm_rx_for_task.lock().push(event);
        }
    });

    let inbound_taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some("openai".to_string()),
    }];

    let tool_call_id = JsonRpcId::from(Ulid::new().to_string());
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "a@b.c"},
                "dispatch_target": MAILER_CANONICAL,
            }),
            Some(tool_call_id.clone()),
            None,
            Some(inbound_taint.clone()),
            None,
        )
        .expect("publish tool_request");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !confirm_rx.lock().is_empty() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("confirm_request observed within timeout");

    let confirm_event = confirm_rx.lock().clone().into_iter().next().unwrap();
    let confirm_id = confirm_event
        .request_id
        .clone()
        .expect("confirm_request carries request_id");

    let conn = rusqlite::Connection::open(rig.tmp.path().join("session.sqlite"))
        .expect("readback connection");
    let attached: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events \
             WHERE kind = ?1 AND request_id = ?2",
            ["confirm_request_taint_attached", &confirm_id.to_string()],
            |row| row.get(0),
        )
        .expect("count query");
    assert_eq!(
        attached, 0,
        "no confirm_request_taint_attached row expected for provider-only taint",
    );

    let confirm_kinds: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT kind FROM audit_events WHERE request_id = ?1 ORDER BY seq")
            .expect("prepare");
        stmt.query_map([confirm_id.to_string()], |row| row.get::<_, String>(0))
            .expect("query")
            .filter_map(Result::ok)
            .collect()
    };
    assert_eq!(
        confirm_kinds,
        vec!["confirm_request".to_string()],
        "only the m5a confirm_request row exists for the confirm correlation id",
    );

    collector.abort();
}
