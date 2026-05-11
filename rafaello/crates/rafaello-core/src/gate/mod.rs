//! Confirmation gate (scope §CG1, §CG2 steps 1-4, §CG8 partial).
//!
//! `ConfirmationGate` subscribes internally to
//! `core.session.tool_request`; for each event it resolves the
//! `dispatch_target` to its `CompiledPlugin`, computes
//! `gate_required = !sinks.is_empty() || always_confirm`, and
//! either passes the call through (`publish_for_tool_dispatch` +
//! `gate_passthrough` audit) or — when gating is required —
//! consults `UserGrants::matches` and passes through on a hit
//! (`gate_grant_match` audit). The hold path (§CG2 step 5) and
//! the CG4 / CG5 handlers land in c21 / c22 / c23.

pub mod confirm_state;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::Value;
use tokio::task::JoinHandle;

use crate::audit::{AuditKind, AuditWriter};
use crate::bus::{Broker, BusEvent};
use crate::compile::CompiledPlugin;
use crate::lock::canonical_id::CanonicalId;
use crate::user_grants::UserGrants;

pub use confirm_state::{ConfirmState, HeldConfirmation, MarkError, PriorOutcome};

const GATE_CHANNEL_CAPACITY: usize = 256;
const TOOL_REQUEST_TOPIC: &str = "core.session.tool_request";

pub struct ConfirmationGate {
    broker: Arc<Broker>,
    user_grants: Arc<RwLock<UserGrants>>,
    audit: Arc<AuditWriter>,
    state: Arc<ConfirmState>,
    compiled: BTreeMap<CanonicalId, CompiledPlugin>,
    events_seen: Arc<AtomicUsize>,
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
        let _state = Arc::clone(&self.state); // CG2 step 5 / CG4 / CG5 land in c21+
        let compiled = self.compiled;
        let events_seen = Arc::clone(&self.events_seen);

        tokio::spawn(async move {
            let _subscription = subscription;
            let mut rx = rx;
            while let Some(event) = rx.recv().await {
                events_seen.fetch_add(1, Ordering::SeqCst);
                handle_event(&broker, &user_grants, &audit, &compiled, &event);
            }
        })
    }
}

fn handle_event(
    broker: &Broker,
    user_grants: &RwLock<UserGrants>,
    audit: &AuditWriter,
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

    let sinks = plugin.tool_sink_classes(&tool);
    let always_confirm = plugin.tool_always_confirm(&tool);
    let gate_required = !sinks.is_empty() || always_confirm;

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
    }
    // CG2 step 5 (hold path) lands in c21.
}
