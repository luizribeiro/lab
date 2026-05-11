#![allow(clippy::result_large_err)]

//! Core re-emit router (scope §CR1 + §CR2 + §CR3 + §CR4 + §CR5
//! + §CR6 + §CR7 + §CT5).
//!
//! The `ReemitRouter` is the in-process owner of the wire paths that
//! produce canonical `core.session.*` events:
//!
//! - `frontend.tui.user_message` → `core.session.user_message`
//! - `provider.<id>.tool_request` → `core.session.tool_request`
//! - `provider.<id>.assistant_message` → `core.session.assistant_message`
//! - `plugin.<topic-id>.tool_result` → `core.session.tool_result`
//! - `frontend.tui.confirm_answer` → `core.session.confirm_reply`
//!
//! c17 landed the task structure (subscription, shutdown, the §CR7
//! failure path, the pi-2 H-1 fault-injection seam). c18 lights up
//! per-direction dispatch. c14 lights up the §CT5 `confirm_answer`
//! arm, gated on the optional `confirm_state` + `audit` builder so
//! m5a's gradual rollout keeps m4-shaped callers working.

pub mod taint_match;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::audit::{AuditKind, AuditWriter};
use crate::broker_acl::BrokerAcl;
use crate::bus::{
    Broker, BusEvent, JsonRpcId, PublisherIdentity, TaintEntry, CORE_SESSION_CONFIRM_REPLY,
};
use crate::error::BrokerError;
use crate::gate::{ConfirmState, MarkError, PriorOutcome};
use crate::lock::canonical_id::CanonicalId;
use crate::reemit::taint_match::TaintMatchMap;

/// Default TTL for the router-owned `TaintMatchMap` (scope §A4 / pi-6
/// owner-judgment item 4).
const DEFAULT_TAINT_MATCH_TTL: Duration = Duration::from_secs(300);

/// Default substring-arm minimum byte length for the router-owned
/// `TaintMatchMap` (scope §A3 / pi-6 owner-judgment item 5, N-1).
const DEFAULT_TAINT_MATCH_SUBSTRING_MIN_BYTES: usize = 16;

const REEMIT_CHANNEL_CAPACITY: usize = 256;

/// Test-only fault injector seam (pi-2 H-1). When set, every
/// per-direction handler calls it BEFORE the real re-emit; on
/// `Some(err)` the handler skips the canonical publish and runs the
/// §CR7 failure path. Drives the failure path through the real router
/// body rather than a side-channel.
#[cfg(any(test, feature = "test-fixture"))]
pub type TestFaultInjector = std::sync::Arc<dyn Fn(&BusEvent) -> Option<BrokerError> + Send + Sync>;

/// Test-only seam for the pi-5 M-1 race coverage. The hook fires
/// inside the `always_allow_session` arm between the step-4
/// `prior_outcome == Held` read and the step-5
/// `mark_session_grant_requested` call, letting tests deterministically
/// interleave CG5's timeout (or any other resolver) with re-emit's
/// atomic mark.
#[cfg(any(test, feature = "test-fixture"))]
pub type TestConfirmRaceHook = std::sync::Arc<dyn Fn() + Send + Sync>;

/// §CT5 validation errors (per row c14). These are surfaced through the
/// §CR7 failure path (`report_reemit_failure`) — no separate wire-level
/// surface; tests assert via `core.lifecycle.reemit_rejected`.
#[derive(Debug, thiserror::Error)]
pub enum ReemitError {
    #[error(
        "confirm_answer: envelope `in_reply_to` does not equal payload `request_id` (correlation mismatch)"
    )]
    ConfirmAnswerCorrelationMismatch,
    #[error("confirm_answer: payload `answer` is not one of allow|deny|always_allow_session")]
    ConfirmAnswerMalformed,
}

pub struct ReemitRouter {
    broker: Broker,
    acl: BrokerAcl,
    active_provider: CanonicalId,
    shutdown_rx: watch::Receiver<bool>,
    confirm_state: Option<Arc<ConfirmState>>,
    audit: Option<Arc<AuditWriter>>,
    taint_match: Arc<TaintMatchMap>,
    #[cfg(any(test, feature = "test-fixture"))]
    fault_injector: Option<TestFaultInjector>,
    #[cfg(any(test, feature = "test-fixture"))]
    confirm_race_hook: Option<TestConfirmRaceHook>,
}

impl ReemitRouter {
    pub fn new(
        broker: Broker,
        acl: BrokerAcl,
        active_provider: CanonicalId,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            broker,
            acl,
            active_provider,
            shutdown_rx,
            confirm_state: None,
            audit: None,
            taint_match: Arc::new(TaintMatchMap::new(
                DEFAULT_TAINT_MATCH_TTL,
                DEFAULT_TAINT_MATCH_SUBSTRING_MIN_BYTES,
            )),
            #[cfg(any(test, feature = "test-fixture"))]
            fault_injector: None,
            #[cfg(any(test, feature = "test-fixture"))]
            confirm_race_hook: None,
        }
    }

    /// pi-1 B-2 / c38 wiring point — opt-in to the §CT5
    /// `confirm_answer` arm. Until this is called, the arm logs a
    /// `tracing::warn!` and drops the event so m4-shaped callers keep
    /// working during the gradual rollout (c14..c38).
    pub fn with_confirm_state_and_audit(
        mut self,
        confirm_state: Arc<ConfirmState>,
        audit: Arc<AuditWriter>,
    ) -> Self {
        self.confirm_state = Some(confirm_state);
        self.audit = Some(audit);
        self
    }

    /// Scope §TM3 / §A4 wiring point — swap the router's default
    /// `TaintMatchMap` for a caller-owned one. The `Arc` is shared so
    /// callers can inspect the map's state (e.g. from tests) while the
    /// spawned task continues to mutate it via §TR1/§TR2 (c10).
    pub fn with_taint_match_map(mut self, map: Arc<TaintMatchMap>) -> Self {
        self.taint_match = map;
        self
    }

    /// Test-only accessor exposing the router's `Arc<TaintMatchMap>` so
    /// tests can observe defaults and post-shutdown clear semantics
    /// without touching the spawned task. Gated like the other m5a/c02
    /// test seams.
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn taint_match_for_test(&self) -> Arc<TaintMatchMap> {
        self.taint_match.clone()
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn with_test_fault_injector(mut self, inject: TestFaultInjector) -> Self {
        self.fault_injector = Some(inject);
        self
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn with_test_confirm_race_hook(mut self, hook: TestConfirmRaceHook) -> Self {
        self.confirm_race_hook = Some(hook);
        self
    }

    pub fn start(self) -> JoinHandle<()> {
        let plugin_acl = self
            .acl
            .plugins
            .get(&self.active_provider)
            .unwrap_or_else(|| {
                panic!(
                    "ReemitRouter: active provider {} not present in BrokerAcl.plugins — \
                 validate::lock should have rejected this lockfile",
                    self.active_provider
                )
            });
        let provider_id = plugin_acl.provider_id.clone().unwrap_or_else(|| {
            panic!(
                "ReemitRouter: active provider {} has no provider_id in BrokerAcl — \
                 plugin lacks `provider = true` binding (validate::lock should reject)",
                self.active_provider
            )
        });

        let patterns = vec![
            "frontend.tui.user_message".to_string(),
            "frontend.tui.confirm_answer".to_string(),
            format!("provider.{}.**", provider_id),
            "plugin.*.tool_result".to_string(),
        ];
        let (rx, subscription) = self
            .broker
            .subscribe_internal(patterns, REEMIT_CHANNEL_CAPACITY);

        let broker = self.broker.clone();
        let acl = self.acl.clone();
        let active_provider = self.active_provider.clone();
        let confirm_state = self.confirm_state.clone();
        let audit = self.audit.clone();
        let taint_match = self.taint_match.clone();
        let mut shutdown_rx = self.shutdown_rx;
        #[cfg(any(test, feature = "test-fixture"))]
        let fault_injector = self.fault_injector;
        #[cfg(any(test, feature = "test-fixture"))]
        let confirm_race_hook = self.confirm_race_hook;

        tokio::spawn(async move {
            let _subscription = subscription;
            let mut rx = rx;
            loop {
                tokio::select! {
                    biased;
                    res = shutdown_rx.changed() => {
                        if res.is_err() || *shutdown_rx.borrow() {
                            taint_match.clear();
                            break;
                        }
                    }
                    maybe_event = rx.recv() => {
                        match maybe_event {
                            Some(event) => {
                                #[cfg(any(test, feature = "test-fixture"))]
                                let injected = fault_injector
                                    .as_ref()
                                    .and_then(|f| f(&event));
                                #[cfg(not(any(test, feature = "test-fixture")))]
                                let injected: Option<BrokerError> = None;

                                dispatch_event(
                                    &broker,
                                    &acl,
                                    &active_provider,
                                    &provider_id,
                                    confirm_state.as_ref(),
                                    audit.as_ref(),
                                    #[cfg(any(test, feature = "test-fixture"))]
                                    confirm_race_hook.as_ref(),
                                    &event,
                                    injected,
                                );
                            }
                            None => break,
                        }
                    }
                }
            }
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_event(
    broker: &Broker,
    acl: &BrokerAcl,
    active_provider: &CanonicalId,
    provider_id: &str,
    confirm_state: Option<&Arc<ConfirmState>>,
    audit: Option<&Arc<AuditWriter>>,
    #[cfg(any(test, feature = "test-fixture"))] confirm_race_hook: Option<&TestConfirmRaceHook>,
    event: &BusEvent,
    injected: Option<BrokerError>,
) {
    if let Some(err) = injected {
        report_reemit_failure(broker, event, &err.to_string());
        return;
    }

    let segments: Vec<&str> = event.topic.split('.').collect();
    let result: Result<(), String> = match segments.as_slice() {
        ["frontend", "tui", "user_message"] => {
            handle_user_message(broker, event).map_err(|e| e.to_string())
        }
        ["frontend", "tui", "confirm_answer"] => handle_confirm_answer(
            broker,
            confirm_state,
            audit,
            #[cfg(any(test, feature = "test-fixture"))]
            confirm_race_hook,
            event,
        ),
        ["provider", _, "tool_request"] => {
            handle_tool_request(broker, acl, active_provider, provider_id, event)
                .map_err(|e| e.to_string())
        }
        ["provider", _, "assistant_message"] => {
            handle_assistant_message(broker, provider_id, event).map_err(|e| e.to_string())
        }
        seg if seg.len() == 3 && seg[0] == "plugin" && seg[2] == "tool_result" => {
            handle_tool_result(broker, acl, event).map_err(|e| e.to_string())
        }
        _ => {
            tracing::debug!(
                topic = %event.topic,
                "reemit router: no handler matches inbound topic"
            );
            return;
        }
    };

    if let Err(err) = result {
        report_reemit_failure(broker, event, &err);
    }
}

/// §CR5: `frontend.tui.user_message` → `core.session.user_message`.
fn handle_user_message(broker: &Broker, event: &BusEvent) -> Result<(), BrokerError> {
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    broker.publish_core_with_taint(
        "core.session.user_message",
        event.payload.clone(),
        event.request_id.clone(),
        None,
        Some(taint),
        None,
    )
}

/// §CR2: `provider.<id>.tool_request` → `core.session.tool_request`.
fn handle_tool_request(
    broker: &Broker,
    acl: &BrokerAcl,
    active_provider: &CanonicalId,
    provider_id: &str,
    event: &BusEvent,
) -> Result<(), BrokerError> {
    let obj = event
        .payload
        .as_object()
        .ok_or_else(|| BrokerError::Internal {
            detail: format!(
                "reemit: tool_request payload not a JSON object (topic `{}`)",
                event.topic
            ),
        })?;
    let tool = obj
        .get("tool")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| BrokerError::Internal {
            detail: format!(
                "reemit: tool_request payload missing `tool: String` (topic `{}`)",
                event.topic
            ),
        })?;
    let args = obj.get("args").cloned().unwrap_or(serde_json::Value::Null);

    let target = match acl.tool_routes.get(&tool) {
        Some(t) => t.clone(),
        None => {
            let payload = serde_json::json!({
                "tool": tool,
                "reason": "unknown_tool",
            });
            if let Err(publish_err) =
                broker.publish_core("core.lifecycle.tool_dispatch_rejected", payload)
            {
                tracing::error!(
                    error = %publish_err,
                    tool = %tool,
                    "tool_dispatch_rejected observability event failed to publish",
                );
            }
            return Ok(());
        }
    };

    let taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some(provider_id.to_string()),
    }];
    let canonical_payload = serde_json::json!({
        "tool": tool,
        "args": args,
        "dispatch_target": target.to_string(),
    });
    broker.publish_core_with_taint(
        "core.session.tool_request",
        canonical_payload,
        event.request_id.clone(),
        event.in_reply_to.clone(),
        Some(taint),
        Some(active_provider.clone()),
    )
}

/// §CR4: `provider.<id>.assistant_message` → `core.session.assistant_message`.
fn handle_assistant_message(
    broker: &Broker,
    provider_id: &str,
    event: &BusEvent,
) -> Result<(), BrokerError> {
    let taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some(provider_id.to_string()),
    }];
    broker.publish_core_with_taint(
        "core.session.assistant_message",
        event.payload.clone(),
        event.request_id.clone(),
        event.in_reply_to.clone(),
        Some(taint),
        None,
    )
}

/// §CR3: `plugin.<topic-id>.tool_result` → `core.session.tool_result`.
fn handle_tool_result(
    broker: &Broker,
    acl: &BrokerAcl,
    event: &BusEvent,
) -> Result<(), BrokerError> {
    let canonical_str = match &event.publisher {
        PublisherIdentity::Plugin { canonical, .. } => canonical.clone(),
        other => {
            return Err(BrokerError::Internal {
                detail: format!("reemit: tool_result publisher is not Plugin (got {other:?})"),
            });
        }
    };
    let canonical = CanonicalId::parse(&canonical_str).map_err(|e| BrokerError::Internal {
        detail: format!("reemit: tool_result publisher canonical `{canonical_str}` invalid: {e:?}"),
    })?;
    if !acl.plugins.contains_key(&canonical) {
        return Err(BrokerError::Internal {
            detail: format!("reemit: tool_result publisher canonical `{canonical}` not in ACL"),
        });
    }
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some(canonical.to_string()),
    }];
    broker.publish_core_with_taint(
        "core.session.tool_result",
        event.payload.clone(),
        event.request_id.clone(),
        event.in_reply_to.clone(),
        Some(taint),
        None,
    )
}

/// §CT5: `frontend.tui.confirm_answer` → `core.session.confirm_reply`.
/// Implements the full step 1-7 algorithm from scope §CT5.
fn handle_confirm_answer(
    broker: &Broker,
    confirm_state: Option<&Arc<ConfirmState>>,
    audit: Option<&Arc<AuditWriter>>,
    #[cfg(any(test, feature = "test-fixture"))] confirm_race_hook: Option<&TestConfirmRaceHook>,
    event: &BusEvent,
) -> Result<(), String> {
    let (state, audit) = match (confirm_state, audit) {
        (Some(s), Some(a)) => (s.as_ref(), a.as_ref()),
        _ => {
            tracing::warn!(
                topic = %event.topic,
                "reemit: confirm_state-not-wired; dropping `frontend.tui.confirm_answer` \
                 (transitional drop until c38 calls `with_confirm_state_and_audit`)"
            );
            return Ok(());
        }
    };

    // Step 1: payload.request_id is a valid ULID.
    let payload_obj = event.payload.as_object().ok_or_else(|| {
        format!(
            "reemit: confirm_answer payload not a JSON object (topic `{}`)",
            event.topic
        )
    })?;
    let payload_request_id_str = payload_obj
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "reemit: confirm_answer payload missing `request_id: String`".to_string())?;
    if ulid::Ulid::from_string(payload_request_id_str).is_err() {
        return Err(format!(
            "reemit: confirm_answer payload `request_id` is not a valid ULID (`{payload_request_id_str}`)"
        ));
    }
    let correlation_id = JsonRpcId::from(payload_request_id_str);

    // Step 2: in_reply_to == [payload.request_id]. Broker already
    // enforces exactly-one cardinality on `frontend.tui.confirm_answer`;
    // we only check the equality. Never touches ConfirmState.
    let cited = event.in_reply_to.as_ref().and_then(|v| v.first());
    if cited != Some(&correlation_id) {
        return Err(ReemitError::ConfirmAnswerCorrelationMismatch.to_string());
    }

    // Step 3: answer ∈ {allow, deny, always_allow_session}. Audit
    // `confirm_malformed` without touching ConfirmState (pi-3 M-3).
    let answer = payload_obj.get("answer").and_then(|v| v.as_str());
    let answer = match answer {
        Some(a) if a == "allow" || a == "deny" || a == "always_allow_session" => a,
        other => {
            let _ = audit.record(
                AuditKind::ConfirmMalformed,
                Some(&correlation_id),
                &serde_json::json!({"answer": other}),
            );
            return Err(ReemitError::ConfirmAnswerMalformed.to_string());
        }
    };

    // Step 4: classify against the shared map (read-only).
    match state.prior_outcome(&correlation_id) {
        PriorOutcome::Held => {}
        PriorOutcome::Duplicate => {
            let _ = audit.record(
                AuditKind::ConfirmDuplicate,
                Some(&correlation_id),
                &serde_json::json!({}),
            );
            return Ok(());
        }
        PriorOutcome::Late => {
            let _ = audit.record(
                AuditKind::ConfirmLate,
                Some(&correlation_id),
                &serde_json::json!({}),
            );
            return Ok(());
        }
        PriorOutcome::Unknown => {
            let _ = audit.record(
                AuditKind::ConfirmUnknown,
                Some(&correlation_id),
                &serde_json::json!({}),
            );
            return Ok(());
        }
    }

    // Step 5: special-case `always_allow_session` (pi-3 B-2 + pi-5 M-1).
    let outbound_answer = if answer == "always_allow_session" {
        #[cfg(any(test, feature = "test-fixture"))]
        if let Some(hook) = confirm_race_hook {
            (hook)();
        }
        match state.mark_session_grant_requested(&correlation_id) {
            Ok(()) => "allow",
            Err(MarkError::NotActive) => {
                let kind = match state.prior_outcome(&correlation_id) {
                    PriorOutcome::Late => AuditKind::ConfirmLate,
                    PriorOutcome::Duplicate => AuditKind::ConfirmDuplicate,
                    PriorOutcome::Unknown | PriorOutcome::Held => AuditKind::ConfirmUnknown,
                };
                let _ = audit.record(kind, Some(&correlation_id), &serde_json::json!({}));
                return Ok(());
            }
        }
    } else {
        answer
    };

    // Step 6 + 7: synthesise user taint and publish canonical reply.
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    let canonical_payload = serde_json::json!({
        "request_id": payload_request_id_str,
        "answer": outbound_answer,
    });
    broker
        .publish_core_with_taint(
            CORE_SESSION_CONFIRM_REPLY,
            canonical_payload,
            event.request_id.clone(),
            Some(vec![correlation_id]),
            Some(taint),
            None,
        )
        .map_err(|e| e.to_string())
}

/// §CR7 failure path: log at `tracing::error!` and emit
/// `core.lifecycle.reemit_rejected` for observability. No process kill.
fn report_reemit_failure(broker: &Broker, event: &BusEvent, reason: &str) {
    tracing::error!(
        topic = %event.topic,
        error = %reason,
        "reemit rejected — canonical publish failed"
    );
    let payload = serde_json::json!({
        "inbound_topic": event.topic,
        "reason": reason,
    });
    if let Err(publish_err) = broker.publish_core("core.lifecycle.reemit_rejected", payload) {
        tracing::error!(
            error = %publish_err,
            "reemit_rejected observability event failed to publish",
        );
    }
}
