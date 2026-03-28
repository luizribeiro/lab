use serde_json::{Map, Value};
use thiserror::Error;

use crate::types::{JsonRpcId, JsonRpcVersion, RequestEnvelope, ResponseEnvelope, JSONRPC_VERSION};

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

    parse_jsonrpc(object)?;
    let id = get_required_id(object, "id")?;
    let method = get_required_string(object, "method")?;
    let params = get_optional_params(object)?;

    Ok(RequestEnvelope {
        jsonrpc: JsonRpcVersion,
        id,
        method,
        params,
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

fn parse_jsonrpc(object: &Map<String, Value>) -> Result<(), WireDecodeError> {
    let version = get_required_string(object, "jsonrpc")?;
    if version != JSONRPC_VERSION {
        return Err(WireDecodeError::InvalidRequest(format!(
            "field `jsonrpc` must be \"{JSONRPC_VERSION}\""
        )));
    }

    Ok(())
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

fn get_required_id(object: &Map<String, Value>, key: &str) -> Result<JsonRpcId, WireDecodeError> {
    let value = object.get(key).ok_or_else(|| {
        WireDecodeError::InvalidRequest(format!("missing required field `{key}`"))
    })?;

    match value {
        Value::String(value) => Ok(JsonRpcId::String(value.clone())),
        Value::Number(value) => Ok(JsonRpcId::Number(value.clone())),
        Value::Null => Ok(JsonRpcId::Null),
        _ => Err(WireDecodeError::InvalidRequest(format!(
            "field `{key}` must be a string, number, or null"
        ))),
    }
}

fn get_optional_params(object: &Map<String, Value>) -> Result<Option<Value>, WireDecodeError> {
    let Some(params) = object.get("params") else {
        return Ok(None);
    };

    if !params.is_object() && !params.is_array() {
        return Err(WireDecodeError::InvalidRequest(
            "field `params` must be an object or array when present".into(),
        ));
    }

    Ok(Some(params.clone()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{decode_request_line, encode_response_line, WireDecodeError};
    use crate::types::{ErrorEnvelope, JsonRpcId, ResponseEnvelope};

    #[test]
    fn valid_line_decode_and_response_encode_roundtrip() {
        let line = br#"{"jsonrpc":"2.0","id":"1","method":"ping","params":{"x":1}}"#;
        let decoded = decode_request_line(line).expect("line should decode");

        assert_eq!(decoded.id, JsonRpcId::from("1"));
        assert_eq!(decoded.method, "ping");
        assert_eq!(decoded.params, Some(json!({"x": 1})));

        let response = ResponseEnvelope::success(decoded.id.clone(), json!({"ok": true}));
        let encoded = encode_response_line(&response).expect("response should encode");

        assert!(encoded.ends_with(b"\n"));
        let encoded_json: serde_json::Value = serde_json::from_slice(&encoded[..encoded.len() - 1])
            .expect("encoded JSON should parse");
        assert_eq!(
            encoded_json,
            json!({"jsonrpc":"2.0","id":"1","result":{"ok":true}})
        );
    }

    #[test]
    fn request_id_accepts_string_number_and_null() {
        let with_string = decode_request_line(br#"{"jsonrpc":"2.0","id":"req-1","method":"ping"}"#)
            .expect("string id should decode");
        assert_eq!(with_string.id, JsonRpcId::from("req-1"));

        let with_number = decode_request_line(br#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#)
            .expect("number id should decode");
        assert_eq!(with_number.id, JsonRpcId::from(7_i64));

        let with_null = decode_request_line(br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#)
            .expect("null id should decode");
        assert_eq!(with_null.id, JsonRpcId::Null);
    }

    #[test]
    fn params_is_optional_and_accepts_object_or_array() {
        let without_params = decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"ping"}"#)
            .expect("request without params should decode");
        assert!(without_params.params.is_none());

        let with_object =
            decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"ping","params":{"x":1}}"#)
                .expect("request with object params should decode");
        assert_eq!(with_object.params, Some(json!({"x": 1})));

        let with_array =
            decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"ping","params":[1,2,3]}"#)
                .expect("request with array params should decode");
        assert_eq!(with_array.params, Some(json!([1, 2, 3])));
    }

    #[test]
    fn malformed_json_returns_parse_error() {
        let error = decode_request_line(br#"{"jsonrpc": "2.0", "id": "1""#).expect_err("must fail");

        assert!(matches!(error, WireDecodeError::Parse(_)));
    }

    #[test]
    fn missing_or_invalid_fields_return_invalid_request() {
        let missing_jsonrpc =
            decode_request_line(br#"{"id":"1","method":"ping"}"#).expect_err("must fail");
        assert!(matches!(
            missing_jsonrpc,
            WireDecodeError::InvalidRequest(message) if message.contains("missing required field `jsonrpc`")
        ));

        let wrong_jsonrpc = decode_request_line(br#"{"jsonrpc":"1.0","id":"1","method":"ping"}"#)
            .expect_err("must fail");
        assert!(matches!(
            wrong_jsonrpc,
            WireDecodeError::InvalidRequest(message) if message.contains("field `jsonrpc`")
        ));

        let invalid_id = decode_request_line(br#"{"jsonrpc":"2.0","id":{},"method":"ping"}"#)
            .expect_err("must fail");
        assert!(matches!(
            invalid_id,
            WireDecodeError::InvalidRequest(message) if message.contains("field `id`")
        ));

        let invalid_params =
            decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"ping","params":1}"#)
                .expect_err("must fail");
        assert!(matches!(
            invalid_params,
            WireDecodeError::InvalidRequest(message) if message.contains("field `params`")
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
    fn encode_rejects_ambiguous_response_shapes() {
        let both_some = ResponseEnvelope {
            jsonrpc: crate::types::JsonRpcVersion,
            id: JsonRpcId::from("1"),
            result: Some(json!({"ok": true})),
            error: Some(ErrorEnvelope {
                code: -32603,
                message: "internal".to_string(),
                data: None,
            }),
        };

        let both_none = ResponseEnvelope {
            jsonrpc: crate::types::JsonRpcVersion,
            id: JsonRpcId::from("1"),
            result: None,
            error: None,
        };

        assert!(encode_response_line(&both_some).is_err());
        assert!(encode_response_line(&both_none).is_err());
    }
}
