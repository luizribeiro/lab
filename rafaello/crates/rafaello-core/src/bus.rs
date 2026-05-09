#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

pub use fittings_core::context::PeerHandle;
pub use fittings_core::message::JsonRpcId;

use crate::broker_acl::{BrokerAcl, PluginAcl};
use crate::error::BrokerError;
use crate::lock::canonical_id::CanonicalId;
use crate::validate::topic::{validate_pattern, validate_topic};

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
                    publisher: crate::error::Publisher::Plugin(canonical.clone()),
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
        if !self.0.state.lock().registry.contains_key(canonical) {
            return Err(BrokerError::NotRegistered(canonical.clone()));
        }
        let msg: PublishMsg = serde_json::from_value(raw_params.clone()).map_err(|e| {
            BrokerError::InvalidPayload {
                publisher: crate::error::Publisher::Plugin(canonical.clone()),
                reason: e.to_string(),
            }
        })?;
        validate_topic(&msg.topic).map_err(|ve| BrokerError::InvalidTopic {
            publisher: crate::error::Publisher::Plugin(canonical.clone()),
            topic: msg.topic.clone(),
            reason: ve.to_string(),
        })?;
        Ok(())
    }

    pub fn publish_boot(&self) -> Result<(), BrokerError> {
        let event = BusEvent {
            topic: "core.lifecycle.boot".to_string(),
            payload: serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "plugin_count": self.0.acl.plugins.len(),
            }),
            publisher: PublisherIdentity::Core,
            in_reply_to: None,
            taint: None,
        };
        self.fan_out_one_event(&event);
        Ok(())
    }

    fn fan_out_one_event(&self, event: &BusEvent) {
        let value =
            serde_json::to_value(event).expect("BusEvent always serialises to a JSON value");
        let recipients: Vec<(CanonicalId, PeerHandle)> = {
            let state = self.0.state.lock();
            state
                .registry
                .iter()
                .map(|(canonical, conn)| (canonical.clone(), conn.peer.clone()))
                .collect()
        };
        for (_canonical, peer) in recipients {
            let _ = peer.notify("bus.event", value.clone());
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
