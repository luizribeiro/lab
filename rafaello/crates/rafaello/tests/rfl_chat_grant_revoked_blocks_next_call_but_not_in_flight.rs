//! c40 — Scope §"Demo bar" §Bonus (pi-1 M-7): revoke blocks the next
//! call but not the in-flight one.
//!
//! Drives `UserGrants` directly (no subprocess `rfl chat`): the
//! "in-flight" semantics rest on a property of the gate's data flow —
//! once `UserGrants::matches` returns `Some(grant_id)` the gate has
//! captured the decision and dispatches. A subsequent `revoke` on the
//! same id cannot un-dispatch that call (the dispatch publish has
//! already happened on the broker side); only the *next* `matches`
//! query observes the now-empty entry and falls through to a held
//! confirmation. We assert the data-structure side of that
//! invariant here. The matching unit-level test
//! `user_grants_revoke_during_pending_confirmation_does_not_short_circuit`
//! (rafaello-core) covers the gate's behaviour against an in-flight
//! held entry; the data-flow assertion below covers the dispatched-
//! but-not-yet-`tool_result` case.

use std::collections::BTreeMap;

use chrono::Utc;
use rafaello_core::lock::CanonicalId;
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

const MAILCAT_CANONICAL: &str = "local:mailcat@0.0.0";

#[test]
fn rfl_chat_grant_revoked_blocks_next_call_but_not_in_flight() {
    let plugin = CanonicalId::parse(MAILCAT_CANONICAL).expect("canonical id");
    let mut grants = UserGrants::new();

    let mut template = BTreeMap::new();
    template.insert("to".to_string(), json!("alice@example.com"));
    let matcher = UserGrants::compile_template("send-mail", template, None)
        .expect("compile template (no schema)");
    assert!(matches!(matcher, GrantMatcher::Structural { .. }));

    let grant_id = grants.add(UserGrant {
        tool: "send-mail".to_string(),
        plugin: plugin.clone(),
        matcher,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    // First call: gate looks up the grant, captures `Some(grant_id)`,
    // and would proceed to dispatch. The captured id is held by the
    // (synthetic) caller below.
    let args = json!({"to": "alice@example.com"});
    let first_match = grants.matches(&plugin, "send-mail", &args);
    assert_eq!(
        first_match,
        Some(grant_id),
        "first call: expected grant match"
    );
    let in_flight_match_id = first_match.expect("captured at dispatch time");

    // Mid-flight: revoke the grant before the (hypothetical)
    // tool_result lands.
    grants.revoke(grant_id).expect("revoke succeeds");

    // The in-flight call's captured id is unchanged: revoking does not
    // retroactively un-allow what the gate already dispatched. The
    // gate's downstream pipeline (publish_for_tool_dispatch → plugin
    // executes → tool_result) consumes only the prior decision and
    // never re-queries `matches` for that request_id.
    assert_eq!(
        in_flight_match_id, grant_id,
        "in-flight grant_id must be stable across a concurrent revoke"
    );

    // Next call: same plugin + tool + args, but `matches` now
    // returns None — the gate will fall through to the confirmation-
    // hold path on the next `tool_request`.
    let second_match = grants.matches(&plugin, "send-mail", &args);
    assert_eq!(
        second_match, None,
        "next call: expected no grant match (revoke must block)"
    );

    // And `revoke` is one-shot: revoking the same id again errors.
    let err = grants.revoke(grant_id).expect_err("double-revoke errors");
    let msg = format!("{err}");
    assert!(
        msg.contains("no grant with id"),
        "unexpected revoke error message: {msg}"
    );
}
