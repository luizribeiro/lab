//! Core re-emit router (scope §CR1 + §CR6 + §CR7).
//!
//! The `ReemitRouter` is the in-process owner of the four wire paths
//! that produce canonical `core.session.*` events:
//!
//! - `frontend.tui.user_message` → `core.session.user_message`
//! - `provider.<id>.tool_request` → `core.session.tool_request`
//! - `provider.<id>.assistant_message` → `core.session.assistant_message`
//! - `plugin.<topic-id>.tool_result` → `core.session.tool_result`
//!
//! This commit (m4 c17) lands the task structure: subscription,
//! shutdown wiring, the §CR7 failure path, and the pi-2 H-1
//! fault-injection seam used by future tests. The per-direction
//! re-emit logic lands in c18 — the handlers below are placeholders
//! that run the fault-injection check then log + drop.

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::broker_acl::BrokerAcl;
use crate::bus::{Broker, BusEvent};
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
    #[allow(dead_code)] // c18 wires the tool_routes lookup.
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

                                dispatch_event(&broker, &event, injected);
                            }
                            None => break,
                        }
                    }
                }
            }
        })
    }
}

fn dispatch_event(broker: &Broker, event: &BusEvent, injected: Option<BrokerError>) {
    if let Some(err) = injected {
        report_reemit_failure(broker, event, &err);
        return;
    }
    // c17 placeholder: per-direction handlers land in c18. Log and
    // drop so the crate builds green and the subscription stays warm.
    tracing::debug!(
        topic = %event.topic,
        "reemit router received event (c17 placeholder — c18 lights up dispatch)"
    );
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
