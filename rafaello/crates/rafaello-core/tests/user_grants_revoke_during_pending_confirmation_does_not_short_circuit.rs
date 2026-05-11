use std::sync::Arc;

use chrono::Utc;
use rafaello_core::audit::AuditKind;
use rafaello_core::lock::CanonicalId;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

mod common;
use common::session_test_kit::in_memory_broker_with_tui_and_observer_acl;

#[test]
fn revoke_during_pending_confirmation_does_not_short_circuit() {
    let plugin = CanonicalId::parse("github/acme:mailer@1.0.0").unwrap();
    let mut grants = UserGrants::new();
    let id = grants.add(UserGrant {
        tool: "send_mail".to_owned(),
        plugin: plugin.clone(),
        matcher: GrantMatcher::Any,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    let args = json!({"to": "a@b.c"});
    assert_eq!(grants.matches(&plugin, "send_mail", &args), Some(id));

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let kit = in_memory_broker_with_tui_and_observer_acl();
    let controller = SessionController::new(store, pipeline, kit.broker.clone());
    let writer = controller.audit_writer();
    let short_circuit_seq = writer
        .record(
            AuditKind::GateGrantMatchShortCircuit,
            None,
            &json!({"grant_id": format!("{:?}", id), "tool": "send_mail"}),
        )
        .expect("short-circuit audit row");

    grants.revoke(id).expect("revoke should succeed");

    let conn =
        rusqlite::Connection::open(tmp.path().join("session.sqlite")).expect("readback connection");
    let kind: String = conn
        .query_row(
            "SELECT kind FROM audit_events WHERE seq = ?1",
            [short_circuit_seq],
            |row| row.get(0),
        )
        .expect("short-circuit row still present");
    assert_eq!(kind, "gate_grant_match_short_circuit");

    assert_eq!(grants.matches(&plugin, "send_mail", &args), None);
}
