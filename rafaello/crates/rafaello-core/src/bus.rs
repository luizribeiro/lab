#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

pub use fittings_core::context::PeerHandle;
pub use fittings_core::message::JsonRpcId;

use crate::broker_acl::{BrokerAcl, PluginAcl};
use crate::error::{BrokerError, InReplyToReason, Publisher};
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublisherIdentity {
    Core,
    Plugin { canonical: String, topic_id: String },
}

struct PluginConn {
    peer: PeerHandle,
}

struct BrokerState {
    registry: BTreeMap<CanonicalId, PluginConn>,
}

struct BrokerInner {
    acl: BrokerAcl,
    state: Mutex<BrokerState>,
}

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
        Ok(Self(Arc::new(BrokerInner {
            acl,
            state: Mutex::new(BrokerState {
                registry: BTreeMap::new(),
            }),
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

    pub fn shutdown(&self) {
        self.0.state.lock().registry.clear();
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
                canonical: canonical.clone(),
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
                    canonical: canonical.clone(),
                    topic: msg.topic.clone(),
                    reason,
                });
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
        };

        if last == "tool_result" || last == "rpc_reply" {
            tracing::debug!(
                topic = %msg.topic,
                publisher = %canonical,
                "result-routing protection: skipping per-subscriber fan-out"
            );
            let _ = event;
            return Ok(());
        }

        self.fan_out(&event, Some(canonical));
        Ok(())
    }

    /// Publish a `core.*` event from the broker itself (scope §B1, §B5).
    ///
    /// Validates grammar first, then performs the §B3 structural namespace
    /// check with `Publisher::Core`: only `core.*` topics are accepted.
    /// No publisher exclusion: the broker is not registered as a subscriber.
    pub fn publish_core(&self, topic: &str, payload: serde_json::Value) -> Result<(), BrokerError> {
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
        self.publish_core_internal(topic, payload)
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
        };
        self.fan_out(&event, None);
        Ok(())
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

    fn fan_out(&self, event: &BusEvent, exclude: Option<&CanonicalId>) {
        let value =
            serde_json::to_value(event).expect("BusEvent always serialises to a JSON value");
        let recipients: Vec<(CanonicalId, PeerHandle, Vec<String>)> = {
            let state = self.0.state.lock();
            state
                .registry
                .iter()
                .filter(|(c, _)| exclude != Some(*c))
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
        };

        for (recipient, peer, patterns) in recipients {
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

#[cfg(test)]
mod static_assertions {
    use super::RegisteredPlugin;

    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}

    #[allow(dead_code)]
    fn assertions() {
        assert_send_sync::<RegisteredPlugin>();
    }
}
