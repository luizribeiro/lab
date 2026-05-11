//! Confirmation gate (scope §CG1, §CG2 steps 1-5, §CG3, §CG4,
//! §CG4a, §CG8 partial).
//!
//! `ConfirmationGate` subscribes internally to
//! `core.session.tool_request` and `core.session.confirm_reply`.
//! For tool_request events it resolves the `dispatch_target` to its
//! `CompiledPlugin`, computes
//! `gate_required = !sinks.is_empty() || always_confirm`, and
//! either passes the call through (`publish_for_tool_dispatch` +
//! `gate_passthrough` audit) or — when gating is required —
//! consults `UserGrants::matches` and passes through on a hit
//! (`gate_grant_match` audit). Otherwise the gate runs the §CG2
//! step 5 hold path: it allocates a fresh confirm correlation id,
//! reserves a `HeldConfirmation` in the shared `ConfirmState`,
//! publishes `core.session.confirm_request` with the §CG3 payload,
//! and arms a `tokio::time::sleep_until(deadline)` task whose
//! `JoinHandle` it parks in an internal map for abort-on-resolve.
//! For confirm_reply events it runs §CG4: `try_resolve` →
//! dispatch on allow / synthesise a deny tool_result on deny, with
//! the timeout-race path auditing `confirm_resolved_after_timeout`.
//! §CG5: the per-`reserve` timer task calls `handle_timeout`, which
//! consumes the held entry via `try_take_for_timeout` and publishes
//! the synthetic deny `tool_result` with `reason = ConfirmTimeout`
//! (audit `confirm_timeout`); if the entry was already resolved by
//! an answer the timeout task exits silently.
//! §CG7: after CG4's `always_allow_session` path inserts a new
//! `UserGrant`, the gate walks `ConfirmState::active_entries_snapshot`
//! and short-circuits any Active entry that newly matches a grant —
//! dispatch + audit `gate_grant_match_short_circuit` + publish
//! `core.session.confirm_resolved` (`reason: "grant_short_circuit"`)
//! so the TUI's queue-pruning subscriber sees the resolution.

pub mod confirm_state;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use serde_json::Value;
use tokio::task::JoinHandle;
use ulid::Ulid;

use crate::audit::{AuditKind, AuditWriter};
use crate::bus::{Broker, BusEvent, JsonRpcId, TaintEntry, CORE_SESSION_CONFIRM_RESOLVED};
use crate::compile::CompiledPlugin;
use crate::lock::canonical_id::CanonicalId;
use crate::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};

pub use confirm_state::{ConfirmState, HeldConfirmation, MarkError, PriorOutcome};

const GATE_CHANNEL_CAPACITY: usize = 256;
const TOOL_REQUEST_TOPIC: &str = "core.session.tool_request";
const CONFIRM_REPLY_TOPIC: &str = "core.session.confirm_reply";
const TOOL_RESULT_TOPIC: &str = "core.session.tool_result";
const CONFIRM_TTL: Duration = Duration::from_secs(60);

/// Arguments for `Broker::publish_core_with_taint` packaged as a
/// value (scope §CG4a). The deny-synthesis helper returns this so
/// the caller can publish without re-deriving the envelope.
#[derive(Debug, Clone)]
pub struct PublishCoreArgs {
    pub topic: &'static str,
    pub payload: Value,
    pub request_id: Option<JsonRpcId>,
    pub in_reply_to: Option<Vec<JsonRpcId>>,
    pub taint: Option<Vec<TaintEntry>>,
}

/// Reason carried in the §CG4a synthetic `tool_result.error` field
/// (and mirrored as the `taint.detail`). `UserDenied` is published
/// by CG4's deny path; `ConfirmTimeout` is published by the CG5
/// timeout body in c23.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    UserDenied,
    ConfirmTimeout,
}

impl DenyReason {
    fn error_str(self) -> &'static str {
        match self {
            DenyReason::UserDenied => "user_denied",
            DenyReason::ConfirmTimeout => "confirm_timeout",
        }
    }
}

/// Build the §CG4a synthetic `core.session.tool_result` for a held
/// confirmation that was denied (either explicitly by the user via
/// CG4, or implicitly by CG5's timeout). The returned envelope
/// satisfies m4's §B0 / §B8 rules (`request_id Some`, `taint
/// non-empty`) and the agent loop's `handle_tool_result` reader
/// (`ok: false`, `content: ""`, `in_reply_to[0]` = held
/// tool_request id).
pub fn synthesise_deny_tool_result(held: &HeldConfirmation, reason: DenyReason) -> PublishCoreArgs {
    let held_request_id = held
        .tool_request
        .request_id
        .clone()
        .expect("held tool_request always carries request_id (gate hold path enforces this)");
    PublishCoreArgs {
        topic: TOOL_RESULT_TOPIC,
        payload: serde_json::json!({
            "ok": false,
            "error": reason.error_str(),
            "content": "",
        }),
        request_id: Some(JsonRpcId::from(Ulid::new().to_string())),
        in_reply_to: Some(vec![held_request_id]),
        taint: Some(vec![TaintEntry {
            source: "system".to_string(),
            detail: Some(reason.error_str().to_string()),
        }]),
    }
}

type TimeoutTasks = Arc<Mutex<HashMap<JsonRpcId, JoinHandle<()>>>>;

pub struct ConfirmationGate {
    broker: Arc<Broker>,
    user_grants: Arc<RwLock<UserGrants>>,
    audit: Arc<AuditWriter>,
    state: Arc<ConfirmState>,
    compiled: BTreeMap<CanonicalId, CompiledPlugin>,
    events_seen: Arc<AtomicUsize>,
    timeout_tasks: TimeoutTasks,
}

impl ConfirmationGate {
    pub fn new(
        broker: Arc<Broker>,
        user_grants: Arc<RwLock<UserGrants>>,
        audit: Arc<AuditWriter>,
        state: Arc<ConfirmState>,
        compiled: BTreeMap<CanonicalId, CompiledPlugin>,
    ) -> Self {
        Self {
            broker,
            user_grants,
            audit,
            state,
            compiled,
            events_seen: Arc::new(AtomicUsize::new(0)),
            timeout_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Test hook (§CG8): observers of "events seen so far" can clone
    /// this handle before [`spawn`] and watch it increment as the
    /// gate's task drains `core.session.tool_request`.
    pub fn events_seen_handle(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.events_seen)
    }

    pub fn spawn(self) -> JoinHandle<()> {
        let (rx, subscription) = self.broker.subscribe_internal(
            vec![
                TOOL_REQUEST_TOPIC.to_string(),
                CONFIRM_REPLY_TOPIC.to_string(),
            ],
            GATE_CHANNEL_CAPACITY,
        );

        let broker = Arc::clone(&self.broker);
        let user_grants = Arc::clone(&self.user_grants);
        let audit = Arc::clone(&self.audit);
        let state = Arc::clone(&self.state);
        let compiled = self.compiled;
        let events_seen = Arc::clone(&self.events_seen);
        let timeout_tasks = Arc::clone(&self.timeout_tasks);

        tokio::spawn(async move {
            let _subscription = subscription;
            let mut rx = rx;
            while let Some(event) = rx.recv().await {
                events_seen.fetch_add(1, Ordering::SeqCst);
                match event.topic.as_str() {
                    TOOL_REQUEST_TOPIC => handle_tool_request(
                        &broker,
                        &user_grants,
                        &audit,
                        &state,
                        &timeout_tasks,
                        &compiled,
                        &event,
                    ),
                    CONFIRM_REPLY_TOPIC => {
                        handle_confirm_reply(&broker, &user_grants, &audit, &state, &event)
                    }
                    other => {
                        tracing::error!(topic = %other, "gate: unexpected topic in subscription");
                    }
                }
            }
        })
    }

    /// §CG5 deadline body. The per-`reserve` timer task awaits its
    /// `sleep_until(deadline)` and then calls this method. On
    /// `Some(held)`: publish the synthetic deny `tool_result` via
    /// the §CG4a helper with `reason = ConfirmTimeout` and audit
    /// `confirm_timeout`. On `None`: the answer arm won the race;
    /// exit silently with no audit and no publish.
    async fn handle_timeout(
        broker: Arc<Broker>,
        audit: Arc<AuditWriter>,
        state: Arc<ConfirmState>,
        confirm_id: JsonRpcId,
    ) {
        let Some(held) = state.try_take_for_timeout(&confirm_id) else {
            return;
        };
        let args = synthesise_deny_tool_result(&held, DenyReason::ConfirmTimeout);
        if let Err(err) = broker.publish_core_with_taint(
            args.topic,
            args.payload,
            args.request_id,
            args.in_reply_to,
            args.taint,
            None,
        ) {
            tracing::error!(error = %err, "gate: CG5 timeout synthetic tool_result publish failed");
            return;
        }
        let tool = held
            .tool_request
            .payload
            .as_object()
            .and_then(|o| o.get("tool"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let _ = audit.record(
            AuditKind::ConfirmTimeout,
            Some(&confirm_id),
            &serde_json::json!({
                "tool": tool,
                "plugin": held.dispatch_target.to_string(),
            }),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_tool_request(
    broker: &Arc<Broker>,
    user_grants: &RwLock<UserGrants>,
    audit: &Arc<AuditWriter>,
    state: &Arc<ConfirmState>,
    timeout_tasks: &TimeoutTasks,
    compiled: &BTreeMap<CanonicalId, CompiledPlugin>,
    event: &BusEvent,
) {
    let Some(obj) = event.payload.as_object() else {
        tracing::error!(topic = %event.topic, "gate: tool_request payload not a JSON object");
        return;
    };
    let tool = obj
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let args = obj.get("args").cloned().unwrap_or(Value::Null);
    let dispatch_target = match obj
        .get("dispatch_target")
        .and_then(|v| v.as_str())
        .and_then(|s| CanonicalId::parse(s).ok())
    {
        Some(c) => c,
        None => {
            tracing::error!("gate: tool_request missing or invalid dispatch_target");
            return;
        }
    };
    let Some(plugin) = compiled.get(&dispatch_target) else {
        tracing::error!(target = %dispatch_target, "gate: dispatch_target not in compiled map");
        return;
    };
    let Some(request_id) = event.request_id.clone() else {
        tracing::error!("gate: tool_request missing request_id");
        return;
    };

    let sink_classes = plugin.tool_sink_classes(&tool);
    let sinks_raw: Vec<String> = plugin
        .tool_sinks(&tool)
        .map(|s| s.to_vec())
        .unwrap_or_default();
    let always_confirm = plugin.tool_always_confirm(&tool);
    let gate_required = !sink_classes.is_empty() || always_confirm;

    if !gate_required {
        if let Err(err) = broker.publish_for_tool_dispatch(
            &dispatch_target,
            serde_json::json!({"tool": tool, "args": args}),
            request_id.clone(),
            event.in_reply_to.clone(),
            event.taint.clone(),
        ) {
            tracing::error!(error = %err, "gate: passthrough dispatch publish failed");
            return;
        }
        let _ = audit.record(
            AuditKind::GatePassthrough,
            Some(&request_id),
            &serde_json::json!({
                "tool": tool,
                "plugin": dispatch_target.to_string(),
            }),
        );
        return;
    }

    let grant_id = user_grants.read().matches(&dispatch_target, &tool, &args);
    if let Some(grant_id) = grant_id {
        if let Err(err) = broker.publish_for_tool_dispatch(
            &dispatch_target,
            serde_json::json!({"tool": tool, "args": args}),
            request_id.clone(),
            event.in_reply_to.clone(),
            event.taint.clone(),
        ) {
            tracing::error!(error = %err, "gate: grant-match dispatch publish failed");
            return;
        }
        let _ = audit.record(
            AuditKind::GateGrantMatch,
            Some(&request_id),
            &serde_json::json!({
                "tool": tool,
                "plugin": dispatch_target.to_string(),
                "grant_id": grant_id.0.to_string(),
            }),
        );
        return;
    }

    hold_for_confirmation(
        broker,
        audit,
        state,
        timeout_tasks,
        event,
        &dispatch_target,
        &tool,
        &args,
        &sinks_raw,
        always_confirm,
        &request_id,
    );
}

#[allow(clippy::too_many_arguments)]
fn hold_for_confirmation(
    broker: &Arc<Broker>,
    audit: &Arc<AuditWriter>,
    state: &Arc<ConfirmState>,
    timeout_tasks: &TimeoutTasks,
    event: &BusEvent,
    dispatch_target: &CanonicalId,
    tool: &str,
    args: &Value,
    sinks: &[String],
    always_confirm: bool,
    held_tool_request_id: &JsonRpcId,
) {
    let confirm_id = JsonRpcId::String(Ulid::new().to_string());
    let deadline = Instant::now() + CONFIRM_TTL;
    state.reserve(
        confirm_id.clone(),
        HeldConfirmation {
            tool_request: event.clone(),
            deadline,
            dispatch_target: dispatch_target.clone(),
        },
    );

    let summary = format!(
        "{tool} via {plugin} — sinks: [{classes}]",
        plugin = dispatch_target,
        classes = sinks.join(", "),
    );
    let taint = event.taint.clone().unwrap_or_default();
    let payload = serde_json::json!({
        "request_id": confirm_id.to_string(),
        "what": "tool_call",
        "summary": summary,
        "details": {
            "tool_call_id": held_tool_request_id.to_string(),
            "tool": tool,
            "args": args,
            "sinks": sinks,
            "always_confirm": always_confirm,
            "taint": taint,
        },
        "default": "deny",
        "ttl_seconds": CONFIRM_TTL.as_secs(),
    });

    if let Err(err) = broker.publish_core_with_taint(
        "core.session.confirm_request",
        payload,
        Some(confirm_id.clone()),
        Some(vec![held_tool_request_id.clone()]),
        Some(vec![TaintEntry {
            source: "system".to_string(),
            detail: Some("confirm_request".to_string()),
        }]),
        None,
    ) {
        tracing::error!(error = %err, "gate: confirm_request publish failed");
        return;
    }
    let _ = audit.record(
        AuditKind::ConfirmRequest,
        Some(&confirm_id),
        &serde_json::json!({
            "tool": tool,
            "plugin": dispatch_target.to_string(),
            "tool_call_id": held_tool_request_id.to_string(),
            "sinks": sinks,
        }),
    );

    let broker_for_timer = Arc::clone(broker);
    let audit_for_timer = Arc::clone(audit);
    let state_for_timer = Arc::clone(state);
    let timeout_tasks_for_cleanup = Arc::clone(timeout_tasks);
    let confirm_id_for_timer = confirm_id.clone();
    let confirm_id_for_cleanup = confirm_id.clone();
    // Anchor the sleep on tokio's clock (not the std `deadline`
    // stored in `HeldConfirmation`) so `tokio::time::pause` +
    // `advance` drives it in CG5 tests; semantically equivalent
    // because `reserve` and the timer spawn run back-to-back.
    let timer_deadline = tokio::time::Instant::now() + CONFIRM_TTL;
    let join = tokio::spawn(async move {
        tokio::time::sleep_until(timer_deadline).await;
        ConfirmationGate::handle_timeout(
            broker_for_timer,
            audit_for_timer,
            state_for_timer,
            confirm_id_for_timer,
        )
        .await;
        timeout_tasks_for_cleanup
            .lock()
            .remove(&confirm_id_for_cleanup);
    });
    timeout_tasks.lock().insert(confirm_id, join);
}

/// §CG4: handle a re-emitted `core.session.confirm_reply`. The
/// reply's envelope `in_reply_to[0]` is the confirm correlation id
/// (= payload `request_id`). `try_resolve` atomically consumes the
/// held entry; `None` means CG5's timeout won the race.
fn handle_confirm_reply(
    broker: &Broker,
    user_grants: &RwLock<UserGrants>,
    audit: &AuditWriter,
    state: &Arc<ConfirmState>,
    event: &BusEvent,
) {
    let Some(correlation_id) = event.in_reply_to.as_ref().and_then(|v| v.first()).cloned() else {
        tracing::error!("gate: confirm_reply missing in_reply_to[0]");
        return;
    };

    let Some((held, session_grant_requested)) = state.try_resolve(&correlation_id) else {
        let _ = audit.record(
            AuditKind::ConfirmResolvedAfterTimeout,
            Some(&correlation_id),
            &serde_json::json!({}),
        );
        return;
    };

    let answer = event
        .payload
        .as_object()
        .and_then(|o| o.get("answer"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    match answer {
        "allow" => dispatch_allow(
            broker,
            user_grants,
            audit,
            state,
            &correlation_id,
            held,
            session_grant_requested,
        ),
        "deny" => dispatch_deny(broker, audit, &correlation_id, &held),
        other => {
            tracing::error!(
                answer = %other,
                "gate: confirm_reply has unexpected answer (re-emit must restrict to allow|deny)"
            );
        }
    }
}

fn dispatch_allow(
    broker: &Broker,
    user_grants: &RwLock<UserGrants>,
    audit: &AuditWriter,
    state: &Arc<ConfirmState>,
    correlation_id: &JsonRpcId,
    held: HeldConfirmation,
    session_grant_requested: bool,
) {
    let obj = held
        .tool_request
        .payload
        .as_object()
        .cloned()
        .unwrap_or_default();
    let tool = obj
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let args = obj.get("args").cloned().unwrap_or(Value::Null);

    if session_grant_requested {
        let grant = UserGrant {
            plugin: held.dispatch_target.clone(),
            tool: tool.clone(),
            matcher: GrantMatcher::Structural {
                template: args.clone(),
            },
            added_at: Utc::now(),
            source: GrantSource::AlwaysAllowSession,
        };
        let grant_id = user_grants.write().add(grant);
        let _ = audit.record(
            AuditKind::GrantAdded,
            Some(correlation_id),
            &serde_json::json!({
                "grant_id": grant_id.0.to_string(),
                "plugin": held.dispatch_target.to_string(),
                "tool": tool,
                "source": "AlwaysAllowSession",
            }),
        );
        short_circuit_pending_after_grant(broker, user_grants, audit, state);
    }

    let tool_request_id = held
        .tool_request
        .request_id
        .clone()
        .expect("held tool_request always carries request_id");
    if let Err(err) = broker.publish_for_tool_dispatch(
        &held.dispatch_target,
        serde_json::json!({"tool": tool, "args": args}),
        tool_request_id,
        held.tool_request.in_reply_to.clone(),
        held.tool_request.taint.clone(),
    ) {
        tracing::error!(error = %err, "gate: CG4 allow dispatch publish failed");
        return;
    }

    let kind = if session_grant_requested {
        AuditKind::ConfirmAllowedWithSessionGrant
    } else {
        AuditKind::ConfirmAllowed
    };
    let _ = audit.record(
        kind,
        Some(correlation_id),
        &serde_json::json!({
            "tool": tool,
            "plugin": held.dispatch_target.to_string(),
        }),
    );
}

/// §CG7 short-circuit walk. After CG4's `always_allow_session`
/// path inserts a new `UserGrant`, every still-`Active` held entry
/// whose `(plugin, tool, args)` now matches a grant is resolved
/// in-place: dispatch via `publish_for_tool_dispatch`, audit
/// `gate_grant_match_short_circuit`, and publish
/// `core.session.confirm_resolved` (pi-1 M-1) as the bus-visible
/// signal the TUI subscribes to for queue pruning.
fn short_circuit_pending_after_grant(
    broker: &Broker,
    user_grants: &RwLock<UserGrants>,
    audit: &AuditWriter,
    state: &Arc<ConfirmState>,
) {
    for (confirm_id, plugin, tool, args) in state.active_entries_snapshot() {
        let grant_id = match user_grants.read().matches(&plugin, &tool, &args) {
            Some(id) => id,
            None => continue,
        };
        let Some((held, _)) = state.try_resolve(&confirm_id) else {
            continue;
        };
        let tool_request_id = held
            .tool_request
            .request_id
            .clone()
            .expect("held tool_request always carries request_id");
        if let Err(err) = broker.publish_for_tool_dispatch(
            &held.dispatch_target,
            serde_json::json!({"tool": tool, "args": args}),
            tool_request_id,
            held.tool_request.in_reply_to.clone(),
            held.tool_request.taint.clone(),
        ) {
            tracing::error!(error = %err, "gate: short-circuit dispatch publish failed");
            continue;
        }
        let _ = audit.record(
            AuditKind::GateGrantMatchShortCircuit,
            Some(&confirm_id),
            &serde_json::json!({
                "tool": tool,
                "plugin": plugin.to_string(),
                "grant_id": grant_id.0.to_string(),
            }),
        );
        if let Err(err) = broker.publish_core_with_taint(
            CORE_SESSION_CONFIRM_RESOLVED,
            serde_json::json!({
                "request_id": confirm_id.to_string(),
                "reason": "grant_short_circuit",
            }),
            Some(JsonRpcId::from(Ulid::new().to_string())),
            Some(vec![confirm_id.clone()]),
            None,
            None,
        ) {
            tracing::error!(error = %err, "gate: confirm_resolved publish failed");
        }
    }
}

fn dispatch_deny(
    broker: &Broker,
    audit: &AuditWriter,
    correlation_id: &JsonRpcId,
    held: &HeldConfirmation,
) {
    let args = synthesise_deny_tool_result(held, DenyReason::UserDenied);
    if let Err(err) = broker.publish_core_with_taint(
        args.topic,
        args.payload,
        args.request_id,
        args.in_reply_to,
        args.taint,
        None,
    ) {
        tracing::error!(error = %err, "gate: CG4 deny synthetic tool_result publish failed");
        return;
    }
    let tool = held
        .tool_request
        .payload
        .as_object()
        .and_then(|o| o.get("tool"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let _ = audit.record(
        AuditKind::ConfirmDenied,
        Some(correlation_id),
        &serde_json::json!({
            "tool": tool,
            "plugin": held.dispatch_target.to_string(),
        }),
    );
}
