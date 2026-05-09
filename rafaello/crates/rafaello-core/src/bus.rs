use serde::{Deserialize, Serialize};

pub use fittings_core::message::JsonRpcId;

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
