#![allow(clippy::result_large_err)]

//! Core re-emit router (scope §CR1 + §CR2 + §CR3 + §CR4 + §CR5
//! + §CR6 + §CR7).
//!
//! The `ReemitRouter` is the in-process owner of the four wire paths
//! that produce canonical `core.session.*` events:
//!
//! - `frontend.tui.user_message` → `core.session.user_message`
//! - `provider.<id>.tool_request` → `core.session.tool_request`
//! - `provider.<id>.assistant_message` → `core.session.assistant_message`
//! - `plugin.<topic-id>.tool_result` → `core.session.tool_result`
//!
//! c17 landed the task structure (subscription, shutdown, the §CR7
//! failure path, the pi-2 H-1 fault-injection seam). c18 lights up
//! per-direction dispatch.

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::broker_acl::BrokerAcl;
use crate::bus::{Broker, BusEvent, PublisherIdentity, TaintEntry};
use crate::error::BrokerError;
use crate::lock::canonical_id::CanonicalId;

const REEMIT_CHANNEL_CAPACITY: usize = 256;

/// Test-only fault injector seam (pi-2 H-1). When set, every
/// per-direction handler calls it BEFORE the real re-emit; on
/// `Some(err)` the handler skips the canonical publish and runs the
/// §CR7 failure path. Drives the failure path through the real router
/// body rather than a side-channel.
#[cfg(any(test, feature = "test-fixture"))]
pub type TestFaultInjector = std::sync::Arc<dyn Fn(&BusEvent) -> Option<BrokerError> + Send + Sync>;

pub struct ReemitRouter {
    broker: Broker,
    acl: BrokerAcl,
    active_provider: CanonicalId,
    shutdown_rx: watch::Receiver<bool>,
    #[cfg(any(test, feature = "test-fixture"))]
    fault_injector: Option<TestFaultInjector>,
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
            #[cfg(any(test, feature = "test-fixture"))]
            fault_injector: None,
        }
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub fn with_test_fault_injector(mut self, inject: TestFaultInjector) -> Self {
        self.fault_injector = Some(inject);
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
            format!("provider.{}.**", provider_id),
            "plugin.*.tool_result".to_string(),
        ];
        let (rx, subscription) = self
            .broker
            .subscribe_internal(patterns, REEMIT_CHANNEL_CAPACITY);

        let broker = self.broker.clone();
        let acl = self.acl.clone();
        let active_provider = self.active_provider.clone();
        let mut shutdown_rx = self.shutdown_rx;
        #[cfg(any(test, feature = "test-fixture"))]
        let fault_injector = self.fault_injector;

        tokio::spawn(async move {
            let _subscription = subscription;
            let mut rx = rx;
            loop {
                tokio::select! {
                    biased;
                    res = shutdown_rx.changed() => {
                        if res.is_err() || *shutdown_rx.borrow() {
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

fn dispatch_event(
    broker: &Broker,
    acl: &BrokerAcl,
    active_provider: &CanonicalId,
    provider_id: &str,
    event: &BusEvent,
    injected: Option<BrokerError>,
) {
    if let Some(err) = injected {
        report_reemit_failure(broker, event, &err);
        return;
    }

    let segments: Vec<&str> = event.topic.split('.').collect();
    let result: Result<(), BrokerError> = match segments.as_slice() {
        ["frontend", "tui", "user_message"] => handle_user_message(broker, event),
        ["provider", _, "tool_request"] => {
            handle_tool_request(broker, acl, active_provider, provider_id, event)
        }
        ["provider", _, "assistant_message"] => {
            handle_assistant_message(broker, provider_id, event)
        }
        seg if seg.len() == 3 && seg[0] == "plugin" && seg[2] == "tool_result" => {
            handle_tool_result(broker, acl, event)
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

/// §CR7 failure path: log at `tracing::error!` and emit
/// `core.lifecycle.reemit_rejected` for observability. No process kill.
fn report_reemit_failure(broker: &Broker, event: &BusEvent, err: &BrokerError) {
    tracing::error!(
        topic = %event.topic,
        error = %err,
        "reemit rejected — canonical publish failed"
    );
    let payload = serde_json::json!({
        "inbound_topic": event.topic,
        "reason": err.to_string(),
    });
    if let Err(publish_err) = broker.publish_core("core.lifecycle.reemit_rejected", payload) {
        tracing::error!(
            error = %publish_err,
            "reemit_rejected observability event failed to publish",
        );
    }
}
