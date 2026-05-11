#![allow(clippy::result_large_err)]

//! Core agent loop (scope §AL1-§AL8).
//!
//! The `AgentLoop` is the in-process owner of the dispatch half of the
//! canonical 5-step path (overview §7). It subscribes to the four
//! `core.session.*` events the reemit router produces and:
//!
//! - persists every event as a typed `Entry` via
//!   [`crate::session::SessionController::finalize_entry`];
//! - on `core.session.tool_request`, publishes the per-plugin dispatch
//!   `plugin.<topic-id>.tool_request` via
//!   [`crate::bus::Broker::publish_for_tool_dispatch`].

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use ulid::Ulid;

use crate::broker_acl::BrokerAcl;
use crate::bus::{Broker, BusEvent};
use crate::entry::payloads::{TextPayload, ToolCallPayload, ToolCallStatus, ToolResultPayload};
use crate::entry::{Entry, EntryAuthor, EntryMetadata, RenderNode, StreamState};
use crate::lock::canonical_id::CanonicalId;
use crate::renderer::Capabilities;
use crate::session::SessionController;

const AGENT_CHANNEL_CAPACITY: usize = 256;

pub struct AgentLoop {
    broker: Broker,
    acl: BrokerAcl,
    controller: Arc<SessionController>,
    caps: Capabilities,
    shutdown_rx: watch::Receiver<bool>,
}

impl AgentLoop {
    pub fn new(
        broker: Broker,
        acl: BrokerAcl,
        controller: Arc<SessionController>,
        caps: Capabilities,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            broker,
            acl,
            controller,
            caps,
            shutdown_rx,
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        let patterns = vec![
            "core.session.user_message".to_string(),
            "core.session.assistant_message".to_string(),
            "core.session.tool_request".to_string(),
            "core.session.tool_result".to_string(),
        ];
        let (rx, subscription) = self
            .broker
            .subscribe_internal(patterns, AGENT_CHANNEL_CAPACITY);

        let broker = self.broker.clone();
        let acl = self.acl.clone();
        let controller = self.controller;
        let caps = self.caps;
        let mut shutdown_rx = self.shutdown_rx;

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
                                handle_event(&broker, &acl, &controller, &caps, event).await;
                            }
                            None => break,
                        }
                    }
                }
            }
        })
    }
}

async fn handle_event(
    broker: &Broker,
    acl: &BrokerAcl,
    controller: &SessionController,
    caps: &Capabilities,
    event: BusEvent,
) {
    match event.topic.as_str() {
        "core.session.user_message" => handle_user_message(controller, caps, &event).await,
        "core.session.assistant_message" => {
            handle_assistant_message(controller, caps, &event).await
        }
        "core.session.tool_request" => {
            handle_tool_request(broker, acl, controller, caps, &event).await
        }
        "core.session.tool_result" => handle_tool_result(controller, caps, &event).await,
        _ => {}
    }
}

async fn handle_user_message(
    controller: &SessionController,
    caps: &Capabilities,
    event: &BusEvent,
) {
    let text = extract_text(&event.payload);
    let entry = build_text_entry(EntryAuthor::User, text);
    if let Err(err) = controller.finalize_entry(entry, caps).await {
        tracing::error!(error = %err, "agent_loop: failed to persist user_message entry");
    }
}

async fn handle_assistant_message(
    controller: &SessionController,
    caps: &Capabilities,
    event: &BusEvent,
) {
    let text = extract_text(&event.payload);
    let entry = build_text_entry(EntryAuthor::Assistant, text);
    if let Err(err) = controller.finalize_entry(entry, caps).await {
        tracing::error!(error = %err, "agent_loop: failed to persist assistant_message entry");
    }
}

async fn handle_tool_request(
    broker: &Broker,
    _acl: &BrokerAcl,
    controller: &SessionController,
    caps: &Capabilities,
    event: &BusEvent,
) {
    let obj = match event.payload.as_object() {
        Some(o) => o,
        None => {
            tracing::error!(topic = %event.topic, "agent_loop: tool_request payload not a JSON object");
            return;
        }
    };
    let tool = obj
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let args = obj.get("args").cloned().unwrap_or(Value::Null);
    let dispatch_target = obj
        .get("dispatch_target")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let request_id = match event.request_id.as_ref() {
        Some(r) => r.clone(),
        None => {
            tracing::error!("agent_loop: tool_request missing request_id");
            return;
        }
    };

    let entry = Entry {
        id: Ulid::new(),
        parent: None,
        kind: "tool_call".to_string(),
        schema: None,
        payload: serde_json::to_value(ToolCallPayload {
            id: request_id.to_string(),
            name: tool.clone(),
            args: args.clone(),
            status: ToolCallStatus::Pending,
        })
        .expect("ToolCallPayload serializes"),
        metadata: default_metadata(EntryAuthor::Assistant),
        fallback: None,
    };
    if let Err(err) = controller.finalize_entry(entry, caps).await {
        tracing::error!(error = %err, "agent_loop: failed to persist tool_call entry");
    }

    let canonical = match dispatch_target
        .as_deref()
        .and_then(|s| CanonicalId::parse(s).ok())
    {
        Some(c) => c,
        None => {
            tracing::error!(
                ?dispatch_target,
                "agent_loop: tool_request missing or invalid dispatch_target"
            );
            return;
        }
    };
    let dispatch_payload = serde_json::json!({"tool": tool, "args": args});
    if let Err(err) = broker.publish_for_tool_dispatch(
        &canonical,
        dispatch_payload,
        request_id,
        event.in_reply_to.clone(),
        event.taint.clone(),
    ) {
        tracing::error!(error = %err, "agent_loop: tool dispatch publish failed");
    }
}

async fn handle_tool_result(controller: &SessionController, caps: &Capabilities, event: &BusEvent) {
    let obj = match event.payload.as_object() {
        Some(o) => o,
        None => {
            tracing::error!(topic = %event.topic, "agent_loop: tool_result payload not a JSON object");
            return;
        }
    };
    let ok = obj.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let content = obj
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let call_id = match event.in_reply_to.as_ref().and_then(|v| v.first()) {
        Some(id) => id.to_string(),
        None => {
            tracing::error!("agent_loop: tool_result missing in_reply_to[0]");
            return;
        }
    };
    let entry = Entry {
        id: Ulid::new(),
        parent: None,
        kind: "tool_result".to_string(),
        schema: None,
        payload: serde_json::to_value(ToolResultPayload {
            call_id,
            ok,
            content: RenderNode::Code {
                code: content,
                lang: None,
            },
            details: None,
        })
        .expect("ToolResultPayload serializes"),
        metadata: default_metadata(EntryAuthor::Tool),
        fallback: None,
    };
    if let Err(err) = controller.finalize_entry(entry, caps).await {
        tracing::error!(error = %err, "agent_loop: failed to persist tool_result entry");
    }
}

fn extract_text(payload: &Value) -> String {
    payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn build_text_entry(author: EntryAuthor, text: String) -> Entry {
    Entry {
        id: Ulid::new(),
        parent: None,
        kind: "text".to_string(),
        schema: None,
        payload: serde_json::to_value(TextPayload {
            text,
            markdown: false,
        })
        .expect("TextPayload serializes"),
        metadata: default_metadata(author),
        fallback: None,
    }
}

fn default_metadata(author: EntryAuthor) -> EntryMetadata {
    EntryMetadata {
        created_at: chrono::Utc::now(),
        updated_at: None,
        author,
        plugin: None,
        stream_state: StreamState::Final,
        seq: None,
        tags: Vec::new(),
    }
}
