use fittings_core::message::Metadata;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::types::{RequestEnvelope, ResponseEnvelope};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WireDecodeError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WireEncodeError {
    #[error("encode error: {0}")]
    Encode(String),
}

pub fn decode_request_line(line: &[u8]) -> Result<RequestEnvelope, WireDecodeError> {
    if line.is_empty() || line == b"\n" || line == b"\r\n" {
        return Err(WireDecodeError::InvalidRequest("empty line".to_string()));
    }

    let value: Value =
        serde_json::from_slice(line).map_err(|error| WireDecodeError::Parse(error.to_string()))?;

    let object = value
        .as_object()
        .ok_or_else(|| WireDecodeError::InvalidRequest("request must be a JSON object".into()))?;

    let id = get_required_string(object, "id")?;
    let method = get_required_string(object, "method")?;
    let params = object
        .get("params")
        .cloned()
        .ok_or_else(|| WireDecodeError::InvalidRequest("missing required field `params`".into()))?;
    let metadata = parse_metadata(object.get("metadata"))?;

    Ok(RequestEnvelope {
        id,
        method,
        params,
        metadata,
    })
}

pub fn encode_response_line(resp: &ResponseEnvelope) -> Result<Vec<u8>, WireEncodeError> {
    if resp.result.is_some() == resp.error.is_some() {
        return Err(WireEncodeError::Encode(
            "response must contain exactly one of `result` or `error`".into(),
        ));
    }

    let mut encoded =
        serde_json::to_vec(resp).map_err(|error| WireEncodeError::Encode(error.to_string()))?;
    encoded.push(b'\n');
    Ok(encoded)
}

fn get_required_string(object: &Map<String, Value>, key: &str) -> Result<String, WireDecodeError> {
    let value = object.get(key).ok_or_else(|| {
        WireDecodeError::InvalidRequest(format!("missing required field `{key}`"))
    })?;

    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| WireDecodeError::InvalidRequest(format!("field `{key}` must be a string")))
}

fn parse_metadata(raw_metadata: Option<&Value>) -> Result<Metadata, WireDecodeError> {
    let Some(raw_metadata) = raw_metadata else {
        return Ok(Metadata::default());
    };

    let metadata_obj = raw_metadata.as_object().ok_or_else(|| {
        WireDecodeError::InvalidRequest(
            "field `metadata` must be an object of string values".into(),
        )
    })?;

    let mut metadata = Metadata::with_capacity(metadata_obj.len());
    for (key, value) in metadata_obj {
        let value = value.as_str().ok_or_else(|| {
            WireDecodeError::InvalidRequest(format!("field `metadata.{key}` must be a string"))
        })?;
        metadata.insert(key.clone(), value.to_owned());
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{decode_request_line, encode_response_line, WireDecodeError};
    use crate::types::{ErrorEnvelope, ResponseEnvelope};

    #[test]
    fn valid_line_decode_and_response_encode_roundtrip() {
        let line = br#"{"id":"1","method":"ping","params":{"x":1},"metadata":{"trace":"abc"}}"#;
        let decoded = decode_request_line(line).expect("line should decode");

        assert_eq!(decoded.id, "1");
        assert_eq!(decoded.method, "ping");
        assert_eq!(decoded.params, json!({"x": 1}));
        assert_eq!(decoded.metadata.get("trace"), Some(&"abc".to_string()));

        let response =
            ResponseEnvelope::success(decoded.id, json!({"ok": true}), Default::default());
        let encoded = encode_response_line(&response).expect("response should encode");

        assert!(encoded.ends_with(b"\n"));
        let encoded_json: serde_json::Value = serde_json::from_slice(&encoded[..encoded.len() - 1])
            .expect("encoded JSON should parse");
        assert_eq!(encoded_json, json!({"id":"1","result":{"ok":true}}));
    }

    #[test]
    fn malformed_json_returns_parse_error() {
        let error = decode_request_line(br#"{"id": "1""#).expect_err("must fail");

        assert!(matches!(error, WireDecodeError::Parse(_)));
    }

    #[test]
    fn missing_fields_and_wrong_metadata_type_return_invalid_request() {
        let missing_params =
            decode_request_line(br#"{"id":"1","method":"ping"}"#).expect_err("must fail");
        assert!(matches!(
            missing_params,
            WireDecodeError::InvalidRequest(message) if message.contains("missing required field `params`")
        ));

        let wrong_metadata = decode_request_line(
            br#"{"id":"1","method":"ping","params":{},"metadata":{"trace":42}}"#,
        )
        .expect_err("must fail");
        assert!(matches!(
            wrong_metadata,
            WireDecodeError::InvalidRequest(message) if message.contains("metadata.trace")
        ));
    }

    #[test]
    fn empty_or_non_object_input_returns_invalid_request() {
        let empty = decode_request_line(b"\n").expect_err("must fail");
        assert!(matches!(
            empty,
            WireDecodeError::InvalidRequest(message) if message.contains("empty line")
        ));

        let non_object = decode_request_line(br#"[]"#).expect_err("must fail");
        assert!(matches!(
            non_object,
            WireDecodeError::InvalidRequest(message) if message.contains("JSON object")
        ));
    }

    #[test]
    fn metadata_must_be_object_of_strings() {
        let metadata_not_object =
            decode_request_line(br#"{"id":"1","method":"ping","params":{},"metadata":1}"#)
                .expect_err("must fail");
        assert!(matches!(
            metadata_not_object,
            WireDecodeError::InvalidRequest(message) if message.contains("metadata")
        ));
    }

    #[test]
    fn encode_rejects_ambiguous_response_shapes() {
        let both_some = ResponseEnvelope {
            id: "1".to_string(),
            result: Some(json!({"ok": true})),
            error: Some(ErrorEnvelope {
                code: -32603,
                message: "internal".to_string(),
                data: None,
            }),
            metadata: Default::default(),
        };

        let both_none = ResponseEnvelope {
            id: "1".to_string(),
            result: None,
            error: None,
            metadata: Default::default(),
        };

        assert!(encode_response_line(&both_some).is_err());
        assert!(encode_response_line(&both_none).is_err());
    }
}
