#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub use fittings_core::context::PeerHandle;
pub use fittings_core::message::JsonRpcId;

use crate::audit::AuditWriter;
use crate::broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
use crate::error::{BrokerError, InReplyToReason, Publisher, TaintReason};

/// `core.session.confirm_request` — gate-emitted prompt requesting an
/// operator decision on a tool call (scope §CT0 / §CT1).
pub const CORE_SESSION_CONFIRM_REQUEST: &str = "core.session.confirm_request";
/// `core.session.confirm_reply` — canonical reply form re-emitted from
/// `frontend.tui.confirm_answer` after re-emit validation (scope §CT0 / §CT1).
pub const CORE_SESSION_CONFIRM_REPLY: &str = "core.session.confirm_reply";
/// `frontend.tui.confirm_answer` — raw inbound answer published by the
/// TUI overlay (scope §CT0 / §CT1).
pub const FRONTEND_TUI_CONFIRM_ANSWER: &str = "frontend.tui.confirm_answer";
/// `frontend.tui.slash_command` — root slash-command event published by
/// the TUI overlay (scope §SL0).
pub const FRONTEND_TUI_SLASH_COMMAND: &str = "frontend.tui.slash_command";
/// `core.session.command_result` — canonical result of a slash-command
/// dispatch (scope §SL0).
pub const CORE_SESSION_COMMAND_RESULT: &str = "core.session.command_result";
/// `core.session.confirm_resolved` — bus-visible resolution signal
/// published by the gate when a confirm is short-circuited; distinct
/// from `confirm_reply` so the gate's CG4 handler does not observe its
/// own signal (pi-1 M-1).
pub const CORE_SESSION_CONFIRM_RESOLVED: &str = "core.session.confirm_resolved";

const REQUEST_ID_REQUIRED_SUFFIXES: &[&str] = &[
    "tool_request",
    "tool_result",
    "assistant_message",
    "user_message",
    "confirm_request",
    "confirm_reply",
    "confirm_answer",
    "slash_command",
    "command_result",
    "confirm_resolved",
];

const IN_REPLY_TO_MANDATORY_TOPICS: &[&str] = &[
    FRONTEND_TUI_CONFIRM_ANSWER,
    CORE_SESSION_CONFIRM_REPLY,
    CORE_SESSION_COMMAND_RESULT,
    CORE_SESSION_CONFIRM_RESOLVED,
];

fn enforce_exactly_one_in_reply_to(
    make_publisher: impl FnOnce() -> Publisher,
    topic: &str,
    in_reply_to: Option<&Vec<JsonRpcId>>,
) -> Result<(), BrokerError> {
    if !IN_REPLY_TO_MANDATORY_TOPICS.contains(&topic) {
        return Ok(());
    }
    let reason = match in_reply_to {
        None => Some(InReplyToReason::Missing),
        Some(ids) if ids.is_empty() => Some(InReplyToReason::EmptyArray),
        Some(ids) if ids.len() > 1 => Some(InReplyToReason::UnexpectedMultiple),
        Some(_) => None,
    };
    if let Some(reason) = reason {
        return Err(BrokerError::InvalidInReplyTo {
            publisher: make_publisher(),
            topic: topic.to_string(),
            reason,
        });
    }
    Ok(())
}

fn enforce_b0_request_id(
    make_publisher: impl FnOnce() -> Publisher,
    topic: &str,
    msg_request_id: Option<&JsonRpcId>,
) -> Result<(), BrokerError> {
    let last = topic.rsplit('.').next().unwrap_or("");
    if REQUEST_ID_REQUIRED_SUFFIXES.contains(&last) && msg_request_id.is_none() {
        return Err(BrokerError::MissingRequestId {
            publisher: make_publisher(),
            topic: topic.to_string(),
        });
    }
    Ok(())
}
use crate::lock::canonical_id::CanonicalId;
use crate::validate::topic::{pattern_matches_topic, validate_pattern, validate_topic};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishMsg {
    pub topic: String,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub in_reply_to: Option<Vec<JsonRpcId>>,
    #[serde(default)]
    pub taint: Option<Vec<TaintEntry>>,
    #[serde(default)]
    pub request_id: Option<JsonRpcId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaintEntry {
    pub source: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BusEvent {
    pub topic: String,
    pub payload: serde_json::Value,
    pub publisher: PublisherIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<Vec<JsonRpcId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taint: Option<Vec<TaintEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<JsonRpcId>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublisherIdentity {
    Core,
    Plugin {
        canonical: String,
        topic_id: String,
    },
    Frontend {
        attach_id: String,
    },
    Provider {
        canonical: String,
        provider_id: String,
        topic_id: String,
    },
}

struct PluginConn {
    peer: PeerHandle,
}

struct FrontendConn {
    peer: PeerHandle,
}

struct ProviderConn {
    #[allow(dead_code)]
    peer: PeerHandle,
}

/// Per-dispatch record for a `tool_request` routed to a specific plugin
/// (scope §OM1, §PT1 data model). The `dispatched_at` instant is unused
/// in m5a but stored for an m6 metrics hook. `tool_request_taint`
/// records the canonical inbound `core.session.tool_request` taint at
/// dispatch time so the c14 enforcement path can superset-check a
/// plugin's later `tool_result` taint against it without re-resolving
/// the originating event (scope §PT1 step 2).
#[derive(Debug, Clone)]
pub struct OutstandingDispatch {
    pub request_id: JsonRpcId,
    pub dispatched_at: Instant,
    pub tool_request_taint: Vec<TaintEntry>,
}

struct BrokerState {
    registry: BTreeMap<CanonicalId, PluginConn>,
    frontends: BTreeMap<AttachId, FrontendConn>,
    providers: BTreeMap<CanonicalId, ProviderConn>,
    outstanding_dispatched: BTreeMap<CanonicalId, HashMap<JsonRpcId, OutstandingDispatch>>,
}

impl BrokerState {
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn outstanding_dispatched_count(&self, canonical: &CanonicalId) -> usize {
        self.outstanding_dispatched
            .get(canonical)
            .map(|m| m.len())
            .unwrap_or(0)
    }
}

struct InternalSlot {
    id: u64,
    patterns: Vec<String>,
    sender: mpsc::Sender<BusEvent>,
}

struct BrokerInner {
    acl: BrokerAcl,
    state: Mutex<BrokerState>,
    provider_observed_results: Mutex<BTreeMap<CanonicalId, HashSet<JsonRpcId>>>,
    provider_observed_user_messages: Mutex<BTreeMap<CanonicalId, HashSet<JsonRpcId>>>,
    internal_subscribers: Mutex<Vec<InternalSlot>>,
    next_slot_id: AtomicU64,
    /// Interior-mutable audit-writer slot (scope §PT1 / §A2). Shared
    /// across every `Broker::clone()` because all clones hold the
    /// same `Arc<BrokerInner>`. Stored as `Mutex<Option<_>>` rather
    /// than `Option<_>` so `Broker::set_audit_writer` can take
    /// `&self` and every already-cloned handle observes the write
    /// atomically (pi-3 B-2).
    audit: Mutex<Option<Arc<AuditWriter>>>,
    /// Fault-injection seam for tests (scope §TM4 / pi-6 M-1). The
    /// hook fires after a `BusEvent` has been constructed and any
    /// record-side state has been written, but before `fan_out`
    /// reaches subscribers. `Some(err)` short-circuits the publish
    /// with that error; `None` proceeds normally. Last-writer-wins
    /// (pi-6 N-5): a second `install_publish_test_hook` replaces
    /// the slot; no explicit clear method.
    #[cfg(any(test, feature = "test-fixture"))]
    publish_test_hook: Mutex<Option<PublishTestHook>>,
}

#[cfg(any(test, feature = "test-fixture"))]
pub type PublishTestHook = Arc<dyn Fn(&BusEvent) -> Option<BrokerError> + Send + Sync>;

#[derive(Clone)]
pub struct Broker(Arc<BrokerInner>);

impl std::fmt::Debug for Broker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Broker")
            .field("acl_plugins", &self.0.acl.plugins.len())
            .field("registered", &self.0.state.lock().registry.len())
            .finish()
    }
}

impl Broker {
    pub fn new(acl: BrokerAcl) -> Result<Self, BrokerError> {
        for (canonical, plugin_acl) in &acl.plugins {
            for topic in &plugin_acl.publish_topics {
                validate_topic(topic).map_err(|e| BrokerError::InvalidTopic {
                    publisher: Publisher::Plugin(canonical.clone()),
                    topic: topic.clone(),
                    reason: e.to_string(),
                })?;
                if let Some(provider_id) = plugin_acl.provider_id.as_deref() {
                    if let Some(rest) = topic.strip_prefix("provider.") {
                        let second = rest.split('.').next().unwrap_or("");
                        if second != provider_id {
                            return Err(BrokerError::InvalidTopic {
                                publisher: Publisher::Provider {
                                    canonical: canonical.clone(),
                                    provider_id: provider_id.to_string(),
                                },
                                topic: topic.clone(),
                                reason: format!(
                                    "publish_topic `{topic}` second segment `{second}` does not match registered provider_id `{provider_id}`"
                                ),
                            });
                        }
                    }
                }
            }
            for pattern in plugin_acl
                .subscribe_patterns
                .iter()
                .chain(plugin_acl.auto_subscribes.iter())
            {
                validate_pattern(pattern).map_err(|e| BrokerError::InvalidPattern {
                    reason: e.to_string(),
                })?;
            }
        }
        for (attach_id, frontend_acl) in &acl.frontends {
            for topic in &frontend_acl.publish_topics {
                validate_topic(topic).map_err(|e| BrokerError::InvalidTopic {
                    publisher: Publisher::Frontend(attach_id.clone()),
                    topic: topic.clone(),
                    reason: e.to_string(),
                })?;
            }
            for pattern in frontend_acl
                .subscribe_patterns
                .iter()
                .chain(frontend_acl.auto_subscribes.iter())
            {
                validate_pattern(pattern).map_err(|e| BrokerError::InvalidPattern {
                    reason: e.to_string(),
                })?;
            }
        }
        Ok(Self(Arc::new(BrokerInner {
            acl,
            state: Mutex::new(BrokerState {
                registry: BTreeMap::new(),
                frontends: BTreeMap::new(),
                providers: BTreeMap::new(),
                outstanding_dispatched: BTreeMap::new(),
            }),
            provider_observed_results: Mutex::new(BTreeMap::new()),
            provider_observed_user_messages: Mutex::new(BTreeMap::new()),
            internal_subscribers: Mutex::new(Vec::new()),
            next_slot_id: AtomicU64::new(0),
            audit: Mutex::new(None),
            #[cfg(any(test, feature = "test-fixture"))]
            publish_test_hook: Mutex::new(None),
        })))
    }

    pub fn try_reserve_registration(&self, canonical: &CanonicalId) -> Result<(), BrokerError> {
        if !self.0.acl.plugins.contains_key(canonical) {
            return Err(BrokerError::NotInAcl(canonical.clone()));
        }
        if self.0.state.lock().registry.contains_key(canonical) {
            return Err(BrokerError::AlreadyRegistered(canonical.clone()));
        }
        Ok(())
    }

    pub fn register_plugin(
        &self,
        canonical: CanonicalId,
        peer: PeerHandle,
    ) -> Result<RegisteredPlugin, BrokerError> {
        if !self.0.acl.plugins.contains_key(&canonical) {
            return Err(BrokerError::NotInAcl(canonical));
        }
        let mut state = self.0.state.lock();
        if state.registry.contains_key(&canonical) {
            return Err(BrokerError::AlreadyRegistered(canonical));
        }
        state
            .registry
            .insert(canonical.clone(), PluginConn { peer });
        drop(state);
        Ok(RegisteredPlugin {
            broker: Arc::clone(&self.0),
            canonical: Some(canonical),
        })
    }

    pub fn contains_plugin(&self, canonical: &CanonicalId) -> bool {
        self.0.acl.plugins.contains_key(canonical)
    }

    pub fn plugin_acl(&self, canonical: &CanonicalId) -> Option<PluginAcl> {
        self.0.acl.plugins.get(canonical).cloned()
    }

    pub fn frontend_acl(&self, attach_id: &AttachId) -> Option<FrontendAcl> {
        self.0.acl.frontends.get(attach_id).cloned()
    }

    pub fn try_reserve_frontend_registration(
        &self,
        attach_id: &AttachId,
    ) -> Result<(), BrokerError> {
        if !self.0.acl.frontends.contains_key(attach_id) {
            return Err(BrokerError::FrontendNotInAcl(attach_id.clone()));
        }
        if self.0.state.lock().frontends.contains_key(attach_id) {
            return Err(BrokerError::FrontendAlreadyRegistered(attach_id.clone()));
        }
        Ok(())
    }

    pub fn register_frontend(
        &self,
        attach_id: AttachId,
        peer: PeerHandle,
    ) -> Result<RegisteredFrontend, BrokerError> {
        if !self.0.acl.frontends.contains_key(&attach_id) {
            return Err(BrokerError::FrontendNotInAcl(attach_id));
        }
        let mut state = self.0.state.lock();
        if state.frontends.contains_key(&attach_id) {
            return Err(BrokerError::FrontendAlreadyRegistered(attach_id));
        }
        state
            .frontends
            .insert(attach_id.clone(), FrontendConn { peer });
        drop(state);
        Ok(RegisteredFrontend {
            broker: Arc::clone(&self.0),
            attach_id: Some(attach_id),
        })
    }

    pub fn try_reserve_provider_registration(
        &self,
        canonical: &CanonicalId,
    ) -> Result<(), BrokerError> {
        match self.0.acl.plugins.get(canonical) {
            None => return Err(BrokerError::ProviderNotInAcl(canonical.clone())),
            Some(acl) if acl.provider_id.is_none() => {
                return Err(BrokerError::ProviderNotInAcl(canonical.clone()));
            }
            Some(_) => {}
        }
        if self.0.state.lock().providers.contains_key(canonical) {
            return Err(BrokerError::ProviderAlreadyRegistered(canonical.clone()));
        }
        Ok(())
    }

    pub fn register_provider(
        &self,
        canonical: CanonicalId,
        peer: PeerHandle,
    ) -> Result<RegisteredProvider, BrokerError> {
        match self.0.acl.plugins.get(&canonical) {
            None => return Err(BrokerError::ProviderNotInAcl(canonical)),
            Some(acl) if acl.provider_id.is_none() => {
                return Err(BrokerError::ProviderNotInAcl(canonical));
            }
            Some(_) => {}
        }
        let mut state = self.0.state.lock();
        if state.providers.contains_key(&canonical) {
            return Err(BrokerError::ProviderAlreadyRegistered(canonical));
        }
        state
            .providers
            .insert(canonical.clone(), ProviderConn { peer });
        drop(state);
        Ok(RegisteredProvider {
            broker: Arc::clone(&self.0),
            canonical: Some(canonical),
        })
    }

    pub fn contains_provider(&self, canonical: &CanonicalId) -> bool {
        self.0.state.lock().providers.contains_key(canonical)
    }

    /// Install the audit writer all clones of this `Broker` will see
    /// (scope §PT1 / §A2). Takes `&self` because the writer lives
    /// behind a `Mutex` in the shared `Arc<BrokerInner>` — every
    /// already-cloned handle observes the write atomically (pi-3 B-2).
    /// Idempotent in the sense that a second call replaces the
    /// writer; production wires it exactly once before plugin spawn.
    pub fn set_audit_writer(&self, writer: Arc<AuditWriter>) {
        *self.0.audit.lock() = Some(writer);
    }

    /// Clone the installed audit writer out of the shared slot, or
    /// `None` if [`Self::set_audit_writer`] has not yet been called.
    /// Callers hold the returned `Arc` briefly and drop it; the
    /// inner `Mutex` is not held across the call.
    pub fn audit_writer(&self) -> Option<Arc<AuditWriter>> {
        self.0.audit.lock().clone()
    }

    /// Test seam: record an audit row through the installed writer
    /// (scope §AL2). Returns `None` when no writer is installed —
    /// the production `rfl chat` path always installs one before
    /// plugin spawn, but unit tests construct bare brokers without
    /// it. Mirrors the m5a `outstanding_dispatched_count_for_test`
    /// gating pattern.
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn record_audit_for_test(
        &self,
        kind: crate::audit::AuditKind,
        request_id: Option<&JsonRpcId>,
        payload: &serde_json::Value,
    ) -> Option<Result<i64, crate::audit::AuditError>> {
        let writer = self.audit_writer()?;
        Some(writer.record(kind, request_id, payload))
    }

    /// Install a fault-injection hook consulted by publish paths just
    /// before `fan_out` (scope §TM4 / pi-6 M-1). Last-writer-wins per
    /// pi-6 N-5; no explicit clear method — install a no-op hook if
    /// removal is needed. The hook's `&BusEvent` argument is the
    /// post-record event so tests may inspect index state at hook-fire
    /// time.
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn install_publish_test_hook(&self, hook: PublishTestHook) {
        *self.0.publish_test_hook.lock() = Some(hook);
    }

    #[cfg(any(test, feature = "test-fixture"))]
    fn check_publish_test_hook(&self, event: &BusEvent) -> Option<BrokerError> {
        let hook = self.0.publish_test_hook.lock().clone();
        hook.and_then(|h| h(event))
    }

    #[cfg(not(any(test, feature = "test-fixture")))]
    fn check_publish_test_hook(&self, _event: &BusEvent) -> Option<BrokerError> {
        None
    }

    pub fn shutdown(&self) {
        let mut state = self.0.state.lock();
        state.registry.clear();
        state.frontends.clear();
        state.providers.clear();
        state.outstanding_dispatched.clear();
    }

    /// Test-only accessor over [`BrokerState::outstanding_dispatched`]
    /// (scope §OM1). Returns the number of dispatched-but-not-yet-replied
    /// `tool_request` ids targeting `canonical`.
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn outstanding_dispatched_count(&self, canonical: &CanonicalId) -> usize {
        self.0.state.lock().outstanding_dispatched_count(canonical)
    }

    /// Test-only inspector over [`BrokerState::outstanding_dispatched`]
    /// (scope §PT1 data model). Returns a clone of the
    /// [`OutstandingDispatch`] record for `(canonical, id)` if one is
    /// currently pending. Mirrors the gating pattern of
    /// [`Self::outstanding_dispatched_count`].
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn peek_outstanding_for_test(
        &self,
        canonical: &CanonicalId,
        id: &JsonRpcId,
    ) -> Option<OutstandingDispatch> {
        self.0
            .state
            .lock()
            .outstanding_dispatched
            .get(canonical)
            .and_then(|m| m.get(id))
            .cloned()
    }

    pub fn handle_plugin_publish(
        &self,
        canonical: &CanonicalId,
        raw_params: &serde_json::Value,
    ) -> Result<(), BrokerError> {
        let result = self.handle_plugin_publish_inner(canonical, raw_params);
        if let Err(ref err) = result {
            self.emit_publish_rejected_for_plugin(canonical, raw_params, err);
        }
        result
    }

    fn handle_plugin_publish_inner(
        &self,
        canonical: &CanonicalId,
        raw_params: &serde_json::Value,
    ) -> Result<(), BrokerError> {
        if !self.0.state.lock().registry.contains_key(canonical) {
            return Err(BrokerError::NotRegistered(canonical.clone()));
        }
        let msg: PublishMsg = serde_json::from_value(raw_params.clone()).map_err(|e| {
            BrokerError::InvalidPayload {
                publisher: Publisher::Plugin(canonical.clone()),
                reason: e.to_string(),
            }
        })?;
        validate_topic(&msg.topic).map_err(|ve| BrokerError::InvalidTopic {
            publisher: Publisher::Plugin(canonical.clone()),
            topic: msg.topic.clone(),
            reason: ve.to_string(),
        })?;
        enforce_b0_request_id(
            || Publisher::Plugin(canonical.clone()),
            &msg.topic,
            msg.request_id.as_ref(),
        )?;
        let segments: Vec<&str> = msg.topic.split('.').collect();
        let publisher_acl = self
            .0
            .acl
            .plugins
            .get(canonical)
            .expect("registered plugin has acl entry");
        match segments[0] {
            "core" | "provider" | "frontend" => {
                return Err(BrokerError::PublishOnReservedNamespace {
                    publisher: Publisher::Plugin(canonical.clone()),
                    topic: msg.topic.clone(),
                });
            }
            "plugin" => {
                if segments.len() < 3 || segments[1] != publisher_acl.topic_id {
                    return Err(BrokerError::PublishOnReservedNamespace {
                        publisher: Publisher::Plugin(canonical.clone()),
                        topic: msg.topic.clone(),
                    });
                }
            }
            _ => {
                return Err(BrokerError::UnknownNamespace {
                    publisher: Publisher::Plugin(canonical.clone()),
                    topic: msg.topic.clone(),
                });
            }
        }
        if !publisher_acl.publish_topics.iter().any(|t| t == &msg.topic) {
            return Err(BrokerError::PublishOutsideGrant {
                publisher: Publisher::Plugin(canonical.clone()),
                topic: msg.topic.clone(),
            });
        }
        let last = *segments.last().expect("validate_topic ensures non-empty");
        if last == "tool_result" || last == "rpc_reply" {
            let reason = match msg.in_reply_to.as_ref() {
                None => Some(InReplyToReason::Missing),
                Some(ids) if ids.is_empty() => Some(InReplyToReason::EmptyArray),
                Some(ids) if ids.len() > 1 => Some(InReplyToReason::UnexpectedMultiple),
                Some(_) => None,
            };
            if let Some(reason) = reason {
                return Err(BrokerError::InvalidInReplyTo {
                    publisher: Publisher::Plugin(canonical.clone()),
                    topic: msg.topic.clone(),
                    reason,
                });
            }
        }

        // Scope §OM2 / §PT1: atomic intake check + superset check on
        // plugin `tool_result`. The entry is drained inside the same
        // critical section as the presence + superset check so a
        // duplicate publish (§OM2) or a violating publish (§PT1) both
        // remove the entry — neither gets a retry window.
        if last == "tool_result" {
            let id = msg
                .in_reply_to
                .as_ref()
                .and_then(|v| v.first())
                .cloned()
                .expect("in_reply_to validated above");
            let mut state = self.0.state.lock();
            let entry = state
                .outstanding_dispatched
                .get_mut(canonical)
                .and_then(|m| m.remove(&id));
            // pi-3 M-2: release the `state` lock before any subsequent
            // publish (the synthetic deny re-enters `fan_out` which
            // takes the recipient-collection lock).
            drop(state);
            let Some(entry) = entry else {
                return Err(BrokerError::StaleRequestId {
                    canonical: canonical.clone(),
                    id,
                });
            };
            // §PT1 superset check. pi-2 M-5: `None` or `Some(vec![])`
            // taint is a no-plugin-claim signal — the check is skipped
            // entirely (m4 behaviour preserved). Otherwise every entry
            // in the dispatch's canonical `tool_request_taint` must
            // appear in the plugin-published `taint`.
            if let Some(pub_taint) = msg.taint.as_ref().filter(|v| !v.is_empty()) {
                let missing: Vec<TaintEntry> = entry
                    .tool_request_taint
                    .iter()
                    .filter(|e| !pub_taint.contains(e))
                    .cloned()
                    .collect();
                if !missing.is_empty() {
                    if let Some(writer) = self.audit_writer() {
                        let _ = writer.record(
                            crate::audit::AuditKind::PluginPublishRejectedTaintSuperset,
                            Some(&id),
                            &serde_json::json!({
                                "canonical": canonical.to_string(),
                                "request_id": id.to_string(),
                                "missing": missing,
                                "published_taint": pub_taint,
                            }),
                        );
                    }
                    let _ = self.publish_core_with_taint(
                        "core.session.tool_result",
                        serde_json::json!({
                            "ok": false,
                            "error": "plugin_taint_superset_violation",
                            "content": "",
                        }),
                        Some(JsonRpcId::String(ulid::Ulid::new().to_string())),
                        Some(vec![id.clone()]),
                        Some(entry.tool_request_taint.clone()),
                        None,
                    );
                    return Err(BrokerError::TaintSupersetViolated {
                        publisher: Publisher::Plugin(canonical.clone()),
                        topic: msg.topic.clone(),
                        missing,
                    });
                }
            }
        }

        let event = BusEvent {
            topic: msg.topic.clone(),
            payload: msg.payload.clone(),
            publisher: PublisherIdentity::Plugin {
                canonical: canonical.to_string(),
                topic_id: publisher_acl.topic_id.clone(),
            },
            in_reply_to: msg.in_reply_to.clone(),
            taint: msg.taint.clone(),
            request_id: msg.request_id.clone(),
        };

        if last == "tool_result" || last == "rpc_reply" {
            tracing::debug!(
                topic = %msg.topic,
                publisher = %canonical,
                "result-routing protection: skipping per-subscriber fan-out"
            );
            self.notify_internal_subscribers(&event);
            return Ok(());
        }

        self.fan_out(&event, Some(canonical), None, None);
        Ok(())
    }

    pub fn handle_provider_publish(
        &self,
        canonical: &CanonicalId,
        raw_params: &serde_json::Value,
    ) -> Result<(), BrokerError> {
        if !self.0.state.lock().providers.contains_key(canonical) {
            return Err(BrokerError::ProviderNotRegistered(canonical.clone()));
        }
        let publisher_acl = self
            .0
            .acl
            .plugins
            .get(canonical)
            .expect("registered provider has acl entry");
        let provider_id = publisher_acl
            .provider_id
            .clone()
            .expect("registered provider has provider_id");
        let make_publisher = || Publisher::Provider {
            canonical: canonical.clone(),
            provider_id: provider_id.clone(),
        };
        let msg: PublishMsg = serde_json::from_value(raw_params.clone()).map_err(|e| {
            BrokerError::InvalidPayload {
                publisher: make_publisher(),
                reason: e.to_string(),
            }
        })?;
        validate_topic(&msg.topic).map_err(|ve| BrokerError::InvalidTopic {
            publisher: make_publisher(),
            topic: msg.topic.clone(),
            reason: ve.to_string(),
        })?;
        let segments: Vec<&str> = msg.topic.split('.').collect();
        match segments[0] {
            "core" | "plugin" | "frontend" => {
                return Err(BrokerError::PublishOnReservedNamespace {
                    publisher: make_publisher(),
                    topic: msg.topic.clone(),
                });
            }
            "provider" => {
                if segments.len() < 3 || segments[1] != provider_id.as_str() {
                    return Err(BrokerError::PublishOnReservedNamespace {
                        publisher: make_publisher(),
                        topic: msg.topic.clone(),
                    });
                }
            }
            _ => {
                return Err(BrokerError::UnknownNamespace {
                    publisher: make_publisher(),
                    topic: msg.topic.clone(),
                });
            }
        }
        if !publisher_acl.publish_topics.iter().any(|t| t == &msg.topic) {
            return Err(BrokerError::PublishOutsideGrant {
                publisher: make_publisher(),
                topic: msg.topic.clone(),
            });
        }
        enforce_b0_request_id(make_publisher, &msg.topic, msg.request_id.as_ref())?;

        let last = *segments.last().expect("validate_topic ensures non-empty");
        if last == "tool_request" {
            let ids = match msg.in_reply_to.as_ref() {
                None => {
                    return Err(BrokerError::InvalidInReplyTo {
                        publisher: make_publisher(),
                        topic: msg.topic.clone(),
                        reason: InReplyToReason::Missing,
                    });
                }
                Some(v) => v,
            };
            let results = self.0.provider_observed_results.lock();
            let empty = HashSet::new();
            let observed = results.get(canonical).unwrap_or(&empty);
            for id in ids {
                if !observed.contains(id) {
                    return Err(BrokerError::InvalidInReplyTo {
                        publisher: make_publisher(),
                        topic: msg.topic.clone(),
                        reason: InReplyToReason::StaleRequestId { id: id.clone() },
                    });
                }
            }
        } else if last == "assistant_message" {
            let ids = match msg.in_reply_to.as_ref() {
                None => {
                    return Err(BrokerError::InvalidInReplyTo {
                        publisher: make_publisher(),
                        topic: msg.topic.clone(),
                        reason: InReplyToReason::Missing,
                    });
                }
                Some(v) => v,
            };
            let results = self.0.provider_observed_results.lock();
            let user_msgs = self.0.provider_observed_user_messages.lock();
            let empty = HashSet::new();
            let r = results.get(canonical).unwrap_or(&empty);
            let u = user_msgs.get(canonical).unwrap_or(&empty);
            for id in ids {
                if !r.contains(id) && !u.contains(id) {
                    return Err(BrokerError::InvalidInReplyTo {
                        publisher: make_publisher(),
                        topic: msg.topic.clone(),
                        reason: InReplyToReason::StaleRequestId { id: id.clone() },
                    });
                }
            }
        }

        let event = BusEvent {
            topic: msg.topic.clone(),
            payload: msg.payload.clone(),
            publisher: PublisherIdentity::Provider {
                canonical: canonical.to_string(),
                provider_id: provider_id.clone(),
                topic_id: publisher_acl.topic_id.clone(),
            },
            in_reply_to: msg.in_reply_to.clone(),
            taint: None,
            request_id: msg.request_id.clone(),
        };
        self.notify_internal_subscribers(&event);
        Ok(())
    }

    /// Register an in-process subscriber on the bus. Returns the receiver
    /// half of a bounded channel and an [`InternalSubscription`] RAII
    /// guard. Dropping the guard removes the slot. The notify path
    /// is internal-only — internal subscribers see events at or before
    /// any external subscriber (see the ordering note in `fan_out`).
    pub fn subscribe_internal(
        &self,
        patterns: Vec<String>,
        capacity: usize,
    ) -> (mpsc::Receiver<BusEvent>, InternalSubscription) {
        let (tx, rx) = mpsc::channel(capacity);
        let slot_id = self.0.next_slot_id.fetch_add(1, Ordering::SeqCst);
        self.0.internal_subscribers.lock().push(InternalSlot {
            id: slot_id,
            patterns,
            sender: tx,
        });
        (
            rx,
            InternalSubscription {
                broker: Arc::clone(&self.0),
                slot_id,
            },
        )
    }

    /// Fan an event out to every matching internal subscriber. On
    /// channel-full, log `tracing::warn!` and continue; on
    /// receiver-closed, log `tracing::debug!`. Called inside `fan_out`
    /// **before** external recipient loops (pi-2 M-1 ordering rule).
    fn notify_internal_subscribers(&self, event: &BusEvent) {
        let slots = self.0.internal_subscribers.lock();
        for slot in slots.iter() {
            let matches = slot
                .patterns
                .iter()
                .any(|p| pattern_matches_topic(p, &event.topic));
            if !matches {
                continue;
            }
            match slot.sender.try_send(event.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!(
                        slot_id = slot.id,
                        topic = %event.topic,
                        "internal subscriber dropped event — channel full"
                    );
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    tracing::debug!(
                        slot_id = slot.id,
                        topic = %event.topic,
                        "internal subscriber dropped event — receiver closed"
                    );
                }
            }
        }
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn seed_provider_observed_result_for_test(&self, canonical: &CanonicalId, id: JsonRpcId) {
        self.0
            .provider_observed_results
            .lock()
            .entry(canonical.clone())
            .or_default()
            .insert(id);
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn seed_provider_observed_user_message_for_test(
        &self,
        canonical: &CanonicalId,
        id: JsonRpcId,
    ) {
        self.0
            .provider_observed_user_messages
            .lock()
            .entry(canonical.clone())
            .or_default()
            .insert(id);
    }

    pub fn handle_frontend_publish(
        &self,
        attach_id: &AttachId,
        raw_params: &serde_json::Value,
    ) -> Result<(), BrokerError> {
        let result = self.handle_frontend_publish_inner(attach_id, raw_params);
        if let Err(ref err) = result {
            self.emit_publish_rejected_for_frontend(attach_id, raw_params, err);
        }
        result
    }

    fn handle_frontend_publish_inner(
        &self,
        attach_id: &AttachId,
        raw_params: &serde_json::Value,
    ) -> Result<(), BrokerError> {
        if !self.0.state.lock().frontends.contains_key(attach_id) {
            return Err(BrokerError::FrontendNotRegistered(attach_id.clone()));
        }
        let msg: PublishMsg = serde_json::from_value(raw_params.clone()).map_err(|e| {
            BrokerError::InvalidPayload {
                publisher: Publisher::Frontend(attach_id.clone()),
                reason: e.to_string(),
            }
        })?;
        validate_topic(&msg.topic).map_err(|ve| BrokerError::InvalidTopic {
            publisher: Publisher::Frontend(attach_id.clone()),
            topic: msg.topic.clone(),
            reason: ve.to_string(),
        })?;
        enforce_b0_request_id(
            || Publisher::Frontend(attach_id.clone()),
            &msg.topic,
            msg.request_id.as_ref(),
        )?;
        let segments: Vec<&str> = msg.topic.split('.').collect();
        let frontend_acl = self
            .0
            .acl
            .frontends
            .get(attach_id)
            .expect("registered frontend has acl entry");
        match segments[0] {
            "core" | "provider" | "plugin" => {
                return Err(BrokerError::PublishOnReservedNamespace {
                    publisher: Publisher::Frontend(attach_id.clone()),
                    topic: msg.topic.clone(),
                });
            }
            "frontend" => {
                if segments.len() < 3 || segments[1] != attach_id.as_str() {
                    return Err(BrokerError::PublishOnReservedNamespace {
                        publisher: Publisher::Frontend(attach_id.clone()),
                        topic: msg.topic.clone(),
                    });
                }
            }
            _ => {
                return Err(BrokerError::UnknownNamespace {
                    publisher: Publisher::Frontend(attach_id.clone()),
                    topic: msg.topic.clone(),
                });
            }
        }
        if !frontend_acl.publish_topics.iter().any(|t| t == &msg.topic) {
            return Err(BrokerError::PublishOutsideGrant {
                publisher: Publisher::Frontend(attach_id.clone()),
                topic: msg.topic.clone(),
            });
        }
        enforce_exactly_one_in_reply_to(
            || Publisher::Frontend(attach_id.clone()),
            &msg.topic,
            msg.in_reply_to.as_ref(),
        )?;

        let event = BusEvent {
            topic: msg.topic.clone(),
            payload: msg.payload.clone(),
            publisher: PublisherIdentity::Frontend {
                attach_id: attach_id.as_str().to_string(),
            },
            in_reply_to: msg.in_reply_to.clone(),
            taint: msg.taint.clone(),
            request_id: msg.request_id.clone(),
        };
        self.fan_out(&event, None, Some(attach_id), None);
        Ok(())
    }

    /// Publish a `core.*` event from the broker itself (scope §B1, §B5).
    ///
    /// Thin wrapper around [`Self::publish_core_with_taint`] for callers
    /// that have no envelope to forward. See that method for the full
    /// validation contract.
    pub fn publish_core(&self, topic: &str, payload: serde_json::Value) -> Result<(), BrokerError> {
        self.publish_core_with_taint(topic, payload, None, None, None, None)
    }

    /// Publish a `core.*` event with an explicit envelope (scope §B8).
    ///
    /// Validates grammar first, then performs the §B3 structural namespace
    /// check with `Publisher::Core`: only `core.*` topics are accepted.
    /// Enforces §B0 (`request_id` required for `.tool_request` /
    /// `.tool_result` / `.assistant_message` / `.user_message` suffixes)
    /// and the §B8 taint-envelope rules for
    /// `core.session.tool_request` / `core.session.tool_result`
    /// (`taint` must be `Some(non_empty_vec)`; every entry's `source` must
    /// be one of `{"user", "provider", "tool", "system"}`).
    ///
    /// When `origin_provider == Some(c)` the fan-out excludes provider `c`
    /// from the recipient set (pi-3 H-2 mechanical exclusion hook).
    pub fn publish_core_with_taint(
        &self,
        topic: &str,
        payload: serde_json::Value,
        request_id: Option<JsonRpcId>,
        in_reply_to: Option<Vec<JsonRpcId>>,
        taint: Option<Vec<TaintEntry>>,
        origin_provider: Option<CanonicalId>,
    ) -> Result<(), BrokerError> {
        validate_topic(topic).map_err(|e| BrokerError::InvalidTopic {
            publisher: Publisher::Core,
            topic: topic.to_string(),
            reason: e.to_string(),
        })?;
        let segments: Vec<&str> = topic.split('.').collect();
        match segments[0] {
            "core" => {}
            "provider" | "plugin" | "frontend" => {
                return Err(BrokerError::PublishOnReservedNamespace {
                    publisher: Publisher::Core,
                    topic: topic.to_string(),
                });
            }
            _ => {
                return Err(BrokerError::UnknownNamespace {
                    publisher: Publisher::Core,
                    topic: topic.to_string(),
                });
            }
        }
        // §B8 taint check runs before §B0 request_id check for the
        // taint-bearing canonical topics so that `publish_core` (the
        // taint-less wrapper) surfaces `InvalidTaint{Missing}` — the
        // defence-in-depth signal scope §B8 names — rather than the
        // generic missing-request-id error.
        if topic == "core.session.tool_request" || topic == "core.session.tool_result" {
            match taint.as_ref() {
                None => {
                    return Err(BrokerError::InvalidTaint {
                        publisher: Publisher::Core,
                        topic: topic.to_string(),
                        reason: TaintReason::Missing,
                    });
                }
                Some(entries) if entries.is_empty() => {
                    return Err(BrokerError::InvalidTaint {
                        publisher: Publisher::Core,
                        topic: topic.to_string(),
                        reason: TaintReason::EmptyArray,
                    });
                }
                Some(entries) => {
                    for e in entries {
                        match e.source.as_str() {
                            "user" | "provider" | "tool" | "system" => {}
                            other => {
                                return Err(BrokerError::InvalidTaint {
                                    publisher: Publisher::Core,
                                    topic: topic.to_string(),
                                    reason: TaintReason::UnknownSource {
                                        source: other.to_string(),
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
        enforce_b0_request_id(|| Publisher::Core, topic, request_id.as_ref())?;
        enforce_exactly_one_in_reply_to(|| Publisher::Core, topic, in_reply_to.as_ref())?;
        let event = BusEvent {
            topic: topic.to_string(),
            payload,
            publisher: PublisherIdentity::Core,
            in_reply_to,
            taint,
            request_id,
        };
        if let Some(err) = self.check_publish_test_hook(&event) {
            return Err(err);
        }
        self.fan_out(&event, None, None, origin_provider.as_ref());
        Ok(())
    }

    /// Look up the canonical id that owns a given tool name (scope §TD1).
    /// Thin accessor over `BrokerAcl.tool_routes`; returns `None` when no
    /// plugin claims the tool.
    pub fn tool_route(&self, name: &str) -> Option<CanonicalId> {
        self.0.acl.tool_routes.get(name).cloned()
    }

    /// Publish `plugin.<topic-id>.tool_request` from the core agent loop
    /// (scope §AL5). Mirrors [`Self::publish_core_with_taint`] but emits
    /// on the per-plugin dispatch topic with `PublisherIdentity::Core`.
    ///
    /// Validates that `canonical` is present in `BrokerAcl.plugins`; the
    /// topic is built from that entry's `topic_id`. This is the only path
    /// from `core.session.tool_request` to a tool plugin (overview §7).
    pub fn publish_for_tool_dispatch(
        &self,
        canonical: &CanonicalId,
        payload: serde_json::Value,
        request_id: JsonRpcId,
        in_reply_to: Option<Vec<JsonRpcId>>,
        taint: Option<Vec<TaintEntry>>,
        tool_request_taint: Vec<TaintEntry>,
    ) -> Result<(), BrokerError> {
        let plugin_acl = self
            .0
            .acl
            .plugins
            .get(canonical)
            .ok_or_else(|| BrokerError::NotInAcl(canonical.clone()))?;
        let topic = format!("plugin.{}.tool_request", plugin_acl.topic_id);
        validate_topic(&topic).map_err(|e| BrokerError::InvalidTopic {
            publisher: Publisher::Core,
            topic: topic.clone(),
            reason: e.to_string(),
        })?;
        let event = BusEvent {
            topic,
            payload,
            publisher: PublisherIdentity::Core,
            in_reply_to,
            taint,
            request_id: Some(request_id.clone()),
        };
        // Scope §OM1: record `(target, id) -> OutstandingDispatch`
        // before handing the event to fan-out, so a `tool_result`
        // observed concurrently from the target plugin cannot race
        // past the intake check in `handle_plugin_publish`.
        self.0
            .state
            .lock()
            .outstanding_dispatched
            .entry(canonical.clone())
            .or_default()
            .insert(
                request_id.clone(),
                OutstandingDispatch {
                    request_id,
                    dispatched_at: Instant::now(),
                    tool_request_taint,
                },
            );
        if let Some(err) = self.check_publish_test_hook(&event) {
            return Err(err);
        }
        self.fan_out(&event, None, None, None);
        Ok(())
    }

    pub fn publish_boot(&self) -> Result<(), BrokerError> {
        let payload = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "plugin_count": self.0.acl.plugins.len(),
        });
        self.publish_core_internal("core.lifecycle.boot", payload)
    }

    /// Internal core-publish path used by `publish_core` (after the
    /// structural namespace check) and by lifecycle-event emission
    /// (scope §B9). Bypasses the structural namespace re-check —
    /// the broker has already constructed the topic correctly — but
    /// still runs grammar revalidation and fan-out.
    fn publish_core_internal(
        &self,
        topic: &str,
        payload: serde_json::Value,
    ) -> Result<(), BrokerError> {
        validate_topic(topic).map_err(|e| BrokerError::InvalidTopic {
            publisher: Publisher::Core,
            topic: topic.to_string(),
            reason: e.to_string(),
        })?;
        let event = BusEvent {
            topic: topic.to_string(),
            payload,
            publisher: PublisherIdentity::Core,
            in_reply_to: None,
            taint: None,
            request_id: None,
        };
        if let Some(err) = self.check_publish_test_hook(&event) {
            return Err(err);
        }
        self.fan_out(&event, None, None, None);
        Ok(())
    }

    fn emit_publish_rejected_for_frontend(
        &self,
        attach_id: &AttachId,
        raw_params: &serde_json::Value,
        err: &BrokerError,
    ) {
        let (topic, code): (Option<String>, &'static str) = match err {
            BrokerError::UnknownNamespace { topic, .. } => {
                (Some(topic.clone()), "unknown_namespace")
            }
            BrokerError::PublishOnReservedNamespace { topic, .. } => {
                (Some(topic.clone()), "publish_on_reserved_namespace")
            }
            BrokerError::PublishOutsideGrant { topic, .. } => {
                (Some(topic.clone()), "publish_outside_grant")
            }
            BrokerError::InvalidTopic { topic, .. } => (Some(topic.clone()), "invalid_topic"),
            BrokerError::InvalidPayload { .. } => {
                let topic = raw_params
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                (topic, "invalid_payload")
            }
            _ => return,
        };
        let payload = serde_json::json!({
            "attach_id": attach_id.as_str(),
            "topic": topic,
            "code": code,
            "message": err.to_string(),
        });
        let _ = self.publish_core_internal("core.lifecycle.publish_rejected", payload);
    }

    fn emit_publish_rejected_for_plugin(
        &self,
        canonical: &CanonicalId,
        raw_params: &serde_json::Value,
        err: &BrokerError,
    ) {
        let (topic, code): (Option<String>, &'static str) = match err {
            BrokerError::UnknownNamespace { topic, .. } => {
                (Some(topic.clone()), "unknown_namespace")
            }
            BrokerError::PublishOnReservedNamespace { topic, .. } => {
                (Some(topic.clone()), "publish_on_reserved_namespace")
            }
            BrokerError::PublishOutsideGrant { topic, .. } => {
                (Some(topic.clone()), "publish_outside_grant")
            }
            BrokerError::InvalidTopic { topic, .. } => (Some(topic.clone()), "invalid_topic"),
            BrokerError::InvalidInReplyTo { topic, reason, .. } => {
                let code = match reason {
                    InReplyToReason::Missing => "invalid_in_reply_to_missing",
                    InReplyToReason::EmptyArray => "invalid_in_reply_to_empty",
                    InReplyToReason::UnexpectedMultiple => "invalid_in_reply_to_multiple",
                    InReplyToReason::StaleRequestId { .. } => "invalid_in_reply_to_stale",
                };
                (Some(topic.clone()), code)
            }
            BrokerError::InvalidPayload { .. } => {
                let topic = raw_params
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                (topic, "invalid_payload")
            }
            BrokerError::TaintSupersetViolated { topic, .. } => {
                (Some(topic.clone()), "taint_superset_violated")
            }
            _ => return,
        };
        let payload = serde_json::json!({
            "canonical": canonical.to_string(),
            "topic": topic,
            "code": code,
            "message": err.to_string(),
        });
        let _ = self.publish_core_internal("core.lifecycle.publish_rejected", payload);
    }

    fn fan_out(
        &self,
        event: &BusEvent,
        exclude_plugin: Option<&CanonicalId>,
        exclude_frontend: Option<&AttachId>,
        exclude_provider: Option<&CanonicalId>,
    ) {
        // Ordering rule (pi-2 M-1): internal subscribers (e.g. the
        // `ReemitRouter`) observe the event before any external
        // recipient. Trusted core composition must see canonical
        // events at or before any external consumer can react.
        self.notify_internal_subscribers(event);

        let value =
            serde_json::to_value(event).expect("BusEvent always serialises to a JSON value");
        let is_core = event.topic.starts_with("core.");
        let plugin_recipients: Vec<(CanonicalId, PeerHandle, Vec<String>)>;
        let frontend_recipients: Vec<(AttachId, PeerHandle, Vec<String>)>;
        let provider_recipients: Vec<(CanonicalId, PeerHandle, Vec<String>)>;
        {
            let state = self.0.state.lock();
            plugin_recipients = state
                .registry
                .iter()
                .filter(|(c, _)| exclude_plugin != Some(*c))
                .filter_map(|(c, conn)| {
                    self.0.acl.plugins.get(c).map(|acl| {
                        let patterns: Vec<String> = acl
                            .subscribe_patterns
                            .iter()
                            .chain(acl.auto_subscribes.iter())
                            .cloned()
                            .collect();
                        (c.clone(), conn.peer.clone(), patterns)
                    })
                })
                .collect();
            frontend_recipients = state
                .frontends
                .iter()
                .filter(|(a, _)| exclude_frontend != Some(*a))
                .filter_map(|(a, conn)| {
                    self.0.acl.frontends.get(a).map(|acl| {
                        let patterns: Vec<String> = acl
                            .subscribe_patterns
                            .iter()
                            .chain(acl.auto_subscribes.iter())
                            .cloned()
                            .collect();
                        (a.clone(), conn.peer.clone(), patterns)
                    })
                })
                .collect();
            provider_recipients = if is_core {
                state
                    .providers
                    .iter()
                    .filter(|(c, _)| exclude_provider != Some(*c))
                    .filter_map(|(c, conn)| {
                        self.0.acl.plugins.get(c).map(|acl| {
                            let patterns: Vec<String> = acl
                                .subscribe_patterns
                                .iter()
                                .chain(acl.auto_subscribes.iter())
                                .cloned()
                                .collect();
                            (c.clone(), conn.peer.clone(), patterns)
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
        }

        for (recipient, peer, patterns) in plugin_recipients {
            let matches = patterns
                .iter()
                .any(|pat| pattern_matches_topic(pat, &event.topic));
            if !matches {
                continue;
            }
            if let Err(e) = peer.notify("bus.event", value.clone()) {
                tracing::warn!(
                    recipient = %recipient,
                    topic = %event.topic,
                    error = ?e,
                    "bus.event fan-out notify failed"
                );
            }
        }

        for (recipient, peer, patterns) in frontend_recipients {
            let matches = patterns
                .iter()
                .any(|pat| pattern_matches_topic(pat, &event.topic));
            if !matches {
                continue;
            }
            if let Err(e) = peer.notify("bus.event", value.clone()) {
                tracing::warn!(
                    recipient = %recipient,
                    topic = %event.topic,
                    error = ?e,
                    "bus.event fan-out notify failed (frontend)"
                );
            }
        }

        for (recipient, peer, patterns) in provider_recipients {
            let matches = patterns
                .iter()
                .any(|pat| pattern_matches_topic(pat, &event.topic));
            if !matches {
                continue;
            }
            if let Err(e) = peer.notify("bus.event", value.clone()) {
                tracing::warn!(
                    recipient = %recipient,
                    topic = %event.topic,
                    error = ?e,
                    "bus.event fan-out notify failed (provider)"
                );
                continue;
            }
            // §B8 / B7b: populate per-recipient observed-id maps so
            // the provider can later cite these ids in its own
            // `tool_request` / `assistant_message` publishes.
            if let Some(id) = event.request_id.as_ref() {
                if event.topic == "core.session.tool_result" {
                    self.0
                        .provider_observed_results
                        .lock()
                        .entry(recipient.clone())
                        .or_default()
                        .insert(id.clone());
                } else if event.topic == "core.session.user_message" {
                    self.0
                        .provider_observed_user_messages
                        .lock()
                        .entry(recipient.clone())
                        .or_default()
                        .insert(id.clone());
                }
            }
        }
    }
}

/// RAII guard for an active broker registration. Dropping the guard removes
/// the plugin's registry entry and drops the broker's clone of the
/// `PeerHandle`. Other clones of the peer handle (held by the supervisor,
/// tests, etc.) are not affected — fan-out to this plugin simply stops.
pub struct RegisteredPlugin {
    broker: Arc<BrokerInner>,
    canonical: Option<CanonicalId>,
}

impl std::fmt::Debug for RegisteredPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredPlugin")
            .field("canonical", &self.canonical)
            .finish()
    }
}

impl Drop for RegisteredPlugin {
    fn drop(&mut self) {
        if let Some(canonical) = self.canonical.take() {
            self.broker.state.lock().registry.remove(&canonical);
        }
    }
}

/// RAII guard for an active broker frontend registration. Dropping the
/// guard removes the frontend's registry entry. Mirrors [`RegisteredPlugin`].
pub struct RegisteredFrontend {
    broker: Arc<BrokerInner>,
    attach_id: Option<AttachId>,
}

impl std::fmt::Debug for RegisteredFrontend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredFrontend")
            .field("attach_id", &self.attach_id)
            .finish()
    }
}

impl Drop for RegisteredFrontend {
    fn drop(&mut self) {
        if let Some(attach_id) = self.attach_id.take() {
            self.broker.state.lock().frontends.remove(&attach_id);
        }
    }
}

/// RAII guard for an active broker provider registration. Dropping the
/// guard removes the provider's registry entry. Mirrors [`RegisteredPlugin`]
/// and [`RegisteredFrontend`] (scope §B5).
pub struct RegisteredProvider {
    broker: Arc<BrokerInner>,
    canonical: Option<CanonicalId>,
}

impl std::fmt::Debug for RegisteredProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredProvider")
            .field("canonical", &self.canonical)
            .finish()
    }
}

impl Drop for RegisteredProvider {
    fn drop(&mut self) {
        if let Some(canonical) = self.canonical.take() {
            self.broker.state.lock().providers.remove(&canonical);
        }
    }
}

/// RAII guard for an internal-subscriber slot. Dropping the guard
/// removes the matching slot from `BrokerInner.internal_subscribers`.
/// If the slot is already gone (broker shutdown cleared it), Drop is
/// a no-op (pi-2 M-1).
pub struct InternalSubscription {
    broker: Arc<BrokerInner>,
    slot_id: u64,
}

impl std::fmt::Debug for InternalSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InternalSubscription")
            .field("slot_id", &self.slot_id)
            .finish()
    }
}

impl Drop for InternalSubscription {
    fn drop(&mut self) {
        let mut slots = self.broker.internal_subscribers.lock();
        if let Some(pos) = slots.iter().position(|s| s.id == self.slot_id) {
            slots.swap_remove(pos);
        }
    }
}

#[cfg(test)]
mod static_assertions {
    use super::{InternalSubscription, RegisteredFrontend, RegisteredPlugin, RegisteredProvider};

    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}

    #[allow(dead_code)]
    fn assertions() {
        assert_send_sync::<RegisteredPlugin>();
        assert_send_sync::<RegisteredFrontend>();
        assert_send_sync::<RegisteredProvider>();
        assert_send_sync::<InternalSubscription>();
    }
}
