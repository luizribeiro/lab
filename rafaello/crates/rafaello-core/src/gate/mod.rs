//! Confirmation gate (scope §CG1, §CG2 steps 1-5, §CG3, §CG8
//! partial).
//!
//! `ConfirmationGate` subscribes internally to
//! `core.session.tool_request`; for each event it resolves the
//! `dispatch_target` to its `CompiledPlugin`, computes
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
//! The CG4 answer path and the CG5 timeout body land in c22 / c23.

pub mod confirm_state;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use serde_json::Value;
use tokio::task::JoinHandle;
use ulid::Ulid;

use crate::audit::{AuditKind, AuditWriter};
use crate::bus::{Broker, BusEvent, JsonRpcId, TaintEntry};
use crate::compile::CompiledPlugin;
use crate::lock::canonical_id::CanonicalId;
use crate::user_grants::UserGrants;

pub use confirm_state::{ConfirmState, HeldConfirmation, MarkError, PriorOutcome};

const GATE_CHANNEL_CAPACITY: usize = 256;
const TOOL_REQUEST_TOPIC: &str = "core.session.tool_request";
const CONFIRM_TTL: Duration = Duration::from_secs(60);

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
        let (rx, subscription) = self
            .broker
            .subscribe_internal(vec![TOOL_REQUEST_TOPIC.to_string()], GATE_CHANNEL_CAPACITY);

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
                handle_event(
                    &broker,
                    &user_grants,
                    &audit,
                    &state,
                    &timeout_tasks,
                    &compiled,
                    &event,
                );
            }
        })
    }

    /// CG5 deadline body — lands in c23. The c21 timer arms a
    /// `sleep_until(deadline)` task that delegates here once the
    /// TTL elapses; for now this is a no-op so the timer plumbing
    /// can be exercised end-to-end before the deadline semantics
    /// arrive.
    async fn handle_timeout(_state: Arc<ConfirmState>, _confirm_id: JsonRpcId) {}
}

#[allow(clippy::too_many_arguments)]
fn handle_event(
    broker: &Broker,
    user_grants: &RwLock<UserGrants>,
    audit: &AuditWriter,
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
    broker: &Broker,
    audit: &AuditWriter,
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

    let state_for_timer = Arc::clone(state);
    let timeout_tasks_for_cleanup = Arc::clone(timeout_tasks);
    let confirm_id_for_timer = confirm_id.clone();
    let confirm_id_for_cleanup = confirm_id.clone();
    let join = tokio::spawn(async move {
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
        ConfirmationGate::handle_timeout(state_for_timer, confirm_id_for_timer).await;
        timeout_tasks_for_cleanup
            .lock()
            .remove(&confirm_id_for_cleanup);
    });
    timeout_tasks.lock().insert(confirm_id, join);
}
