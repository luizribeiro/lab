#![allow(dead_code)]
//! c38 — `gate_or_synthetic_dispatch` test helper.
//!
//! Mirrors the [`crate::gate::ConfirmationGate`] passthrough arm
//! (scope §CG2 step 1) without needing the full gate machinery. m4
//! agent-loop tests that exercised the (now-removed) direct dispatch
//! path use this helper to drive `core.session.tool_request` →
//! `plugin.<topic-id>.tool_request` through a minimal task.
//!
//! Spawns a tokio task that subscribes internally to
//! `core.session.tool_request`, reads the `dispatch_target` field,
//! and re-publishes the inner payload via
//! [`Broker::publish_for_tool_dispatch`]. Returns the join handle
//! and a shutdown trigger.

use tokio::sync::watch;
use tokio::task::JoinHandle;

use rafaello_core::bus::Broker;
use rafaello_core::lock::canonical_id::CanonicalId;

const CHANNEL_CAPACITY: usize = 64;

pub struct SyntheticDispatch {
    pub join: JoinHandle<()>,
    pub shutdown: watch::Sender<bool>,
}

pub fn spawn(broker: Broker) -> SyntheticDispatch {
    let (rx, subscription) = broker.subscribe_internal(
        vec!["core.session.tool_request".to_string()],
        CHANNEL_CAPACITY,
    );
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    let join = tokio::spawn(async move {
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
                maybe = rx.recv() => {
                    let Some(event) = maybe else { break };
                    let Some(obj) = event.payload.as_object() else { continue };
                    let Some(target) = obj
                        .get("dispatch_target")
                        .and_then(|v| v.as_str())
                        .and_then(|s| CanonicalId::parse(s).ok())
                    else {
                        continue;
                    };
                    let tool = obj.get("tool").cloned().unwrap_or(serde_json::Value::Null);
                    let args = obj.get("args").cloned().unwrap_or(serde_json::Value::Null);
                    let Some(request_id) = event.request_id.clone() else { continue };
                    let _ = broker.publish_for_tool_dispatch(
                        &target,
                        serde_json::json!({"tool": tool, "args": args}),
                        request_id,
                        event.in_reply_to.clone(),
                        event.taint.clone(),
                        event.taint.clone().unwrap_or_default(),
                    );
                }
            }
        }
    });

    SyntheticDispatch {
        join,
        shutdown: shutdown_tx,
    }
}
