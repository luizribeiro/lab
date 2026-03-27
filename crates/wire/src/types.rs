use fittings_core::message::Metadata;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub id: String,
    pub method: String,
    pub params: Value,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorEnvelope>,
    #[serde(default, skip_serializing_if = "metadata_is_empty")]
    pub metadata: Metadata,
}

fn metadata_is_empty(metadata: &Metadata) -> bool {
    metadata.is_empty()
}

impl ResponseEnvelope {
    pub fn success(id: impl Into<String>, result: Value, metadata: Metadata) -> Self {
        Self {
            id: id.into(),
            result: Some(result),
            error: None,
            metadata,
        }
    }

    pub fn error(id: impl Into<String>, error: ErrorEnvelope, metadata: Metadata) -> Self {
        Self {
            id: id.into(),
            result: None,
            error: Some(error),
            metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
