use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Metadata = HashMap<String, String>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    pub params: Value,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    pub result: Value,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code: {})", self.message, self.code)
    }
}

impl ServiceError {
    pub const MIN_CODE: i32 = 1;
    pub const MAX_CODE: i32 = 999;

    pub fn is_valid_code_value(code: i32) -> bool {
        (Self::MIN_CODE..=Self::MAX_CODE).contains(&code)
    }

    pub fn has_valid_code(&self) -> bool {
        Self::is_valid_code_value(self.code)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Request, Response, ServiceError};

    #[test]
    fn request_response_serde_roundtrip_and_metadata_isolation() {
        let req_json = json!({
            "id": "req-1",
            "method": "ping",
            "params": {"x": 1}
        });
        let mut request: Request =
            serde_json::from_value(req_json).expect("request should deserialize");
        assert!(request.metadata.is_empty());

        let request_with_metadata: Request = serde_json::from_value(json!({
            "id": "req-1",
            "method": "ping",
            "params": {"x": 1},
            "metadata": {"trace_id": "wire-should-not-pass"}
        }))
        .expect("request with metadata should deserialize");
        assert!(request_with_metadata.metadata.is_empty());

        request.metadata.insert("trace_id".into(), "abc123".into());

        let request_out = serde_json::to_value(&request).expect("request should serialize");
        assert_eq!(request_out["id"], json!("req-1"));
        assert_eq!(request_out["method"], json!("ping"));
        assert_eq!(request_out["params"], json!({"x": 1}));
        assert!(request_out.get("metadata").is_none());

        let mut response = Response {
            id: "req-1".to_string(),
            result: json!({"ok": true}),
            metadata: Default::default(),
        };
        response.metadata.insert("trace_id".into(), "abc123".into());

        let response_out = serde_json::to_value(&response).expect("response should serialize");
        assert!(response_out.get("metadata").is_none());

        let response_back: Response =
            serde_json::from_value(response_out).expect("response should deserialize");
        assert!(response_back.metadata.is_empty());
        assert_eq!(response_back.id, "req-1");
        assert_eq!(response_back.result, json!({"ok": true}));

        let response_with_metadata: Response = serde_json::from_value(json!({
            "id": "req-1",
            "result": {"ok": true},
            "metadata": {"trace_id": "wire-should-not-pass"}
        }))
        .expect("response with metadata should deserialize");
        assert!(response_with_metadata.metadata.is_empty());
    }

    #[test]
    fn service_error_code_helper_accepts_only_rfc_pass_through_range() {
        assert!(ServiceError::is_valid_code_value(1));
        assert!(ServiceError::is_valid_code_value(500));
        assert!(ServiceError::is_valid_code_value(999));

        assert!(!ServiceError::is_valid_code_value(0));
        assert!(!ServiceError::is_valid_code_value(-1));
        assert!(!ServiceError::is_valid_code_value(1_000));

        let valid = ServiceError {
            code: 123,
            message: "ok".into(),
            data: None,
        };
        let invalid = ServiceError {
            code: 1_001,
            message: "too large".into(),
            data: None,
        };

        assert!(valid.has_valid_code());
        assert!(!invalid.has_valid_code());
    }

    #[test]
    fn service_error_serde_supports_optional_data() {
        let with_data = ServiceError {
            code: 7,
            message: "oops".into(),
            data: Some(json!({"detail": "x"})),
        };
        let without_data = ServiceError {
            code: 8,
            message: "oops2".into(),
            data: None,
        };

        let with_data_back: ServiceError =
            serde_json::from_value(serde_json::to_value(&with_data).expect("serialize with data"))
                .expect("deserialize with data");
        let without_data_back: ServiceError = serde_json::from_value(
            serde_json::to_value(&without_data).expect("serialize without data"),
        )
        .expect("deserialize without data");

        assert_eq!(with_data_back, with_data);
        assert_eq!(without_data_back, without_data);
    }
}
