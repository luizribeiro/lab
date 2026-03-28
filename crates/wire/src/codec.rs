use serde_json::{Map, Value};
use thiserror::Error;

use crate::types::{
    ErrorEnvelope, JsonRpcId, JsonRpcVersion, RequestEnvelope, ResponseEnvelope, JSONRPC_VERSION,
};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WireDecodeError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("invalid request: {message}")]
    InvalidRequest {
        message: String,
        id: Option<JsonRpcId>,
    },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum WireEncodeError {
    #[error("encode error: {0}")]
    Encode(String),
}

const ALLOWED_REQUEST_FIELDS: [&str; 4] = ["jsonrpc", "id", "method", "params"];
const ALLOWED_RESPONSE_FIELDS: [&str; 4] = ["jsonrpc", "id", "result", "error"];
const ALLOWED_ERROR_FIELDS: [&str; 3] = ["code", "message", "data"];

fn invalid_request(message: impl Into<String>, id: Option<JsonRpcId>) -> WireDecodeError {
    WireDecodeError::InvalidRequest {
        message: message.into(),
        id,
    }
}

pub fn decode_request_line(line: &[u8]) -> Result<RequestEnvelope, WireDecodeError> {
    let value: Value =
        serde_json::from_slice(line).map_err(|error| WireDecodeError::Parse(error.to_string()))?;

    let object = value
        .as_object()
        .ok_or_else(|| invalid_request("request must be a JSON object", None))?;

    let response_id = extract_valid_id(object.get("id"));

    validate_request_fields(object, &response_id)?;
    parse_jsonrpc(object, &response_id)?;
    let id = get_optional_id(object, "id")?;
    let method = get_required_string(object, "method", &response_id)?;
    validate_method_name(&method, &response_id)?;
    let params = get_optional_params(object, &response_id)?;

    Ok(RequestEnvelope {
        jsonrpc: JsonRpcVersion,
        id,
        method,
        params,
    })
}

pub fn decode_response_line(line: &[u8]) -> Result<ResponseEnvelope, WireDecodeError> {
    let value: Value =
        serde_json::from_slice(line).map_err(|error| WireDecodeError::Parse(error.to_string()))?;

    let object = value
        .as_object()
        .ok_or_else(|| invalid_request("response must be a JSON object", None))?;

    let response_id = extract_valid_id(object.get("id"));

    validate_response_fields(object, &response_id)?;
    parse_jsonrpc(object, &response_id)?;
    let id = get_required_id(object, "id", &response_id)?;

    let result = object.get("result").cloned();
    let error = match object.get("error") {
        Some(value) => Some(parse_error_envelope(value, &Some(id.clone()))?),
        None => None,
    };

    if result.is_some() == error.is_some() {
        return Err(invalid_request(
            "response must contain exactly one of `result` or `error`",
            Some(id),
        ));
    }

    Ok(ResponseEnvelope {
        jsonrpc: JsonRpcVersion,
        id,
        result,
        error,
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

fn extract_valid_id(value: Option<&Value>) -> Option<JsonRpcId> {
    match value {
        Some(Value::String(value)) => Some(JsonRpcId::String(value.clone())),
        Some(Value::Number(value)) if value.is_i64() || value.is_u64() => {
            Some(JsonRpcId::Number(value.clone()))
        }
        Some(Value::Null) => Some(JsonRpcId::Null),
        _ => None,
    }
}

fn validate_request_fields(
    object: &Map<String, Value>,
    response_id: &Option<JsonRpcId>,
) -> Result<(), WireDecodeError> {
    for field in object.keys() {
        if !ALLOWED_REQUEST_FIELDS.contains(&field.as_str()) {
            return Err(invalid_request(
                format!("unexpected field `{field}`"),
                response_id.clone(),
            ));
        }
    }

    Ok(())
}

fn validate_response_fields(
    object: &Map<String, Value>,
    response_id: &Option<JsonRpcId>,
) -> Result<(), WireDecodeError> {
    for field in object.keys() {
        if !ALLOWED_RESPONSE_FIELDS.contains(&field.as_str()) {
            return Err(invalid_request(
                format!("unexpected field `{field}`"),
                response_id.clone(),
            ));
        }
    }

    Ok(())
}

fn parse_jsonrpc(
    object: &Map<String, Value>,
    response_id: &Option<JsonRpcId>,
) -> Result<(), WireDecodeError> {
    let version = get_required_string(object, "jsonrpc", response_id)?;
    if version != JSONRPC_VERSION {
        return Err(invalid_request(
            format!("field `jsonrpc` must be \"{JSONRPC_VERSION}\""),
            response_id.clone(),
        ));
    }

    Ok(())
}

fn validate_method_name(
    method: &str,
    response_id: &Option<JsonRpcId>,
) -> Result<(), WireDecodeError> {
    if method.starts_with("rpc.") {
        return Err(invalid_request(
            "method names starting with `rpc.` are reserved",
            response_id.clone(),
        ));
    }

    Ok(())
}

fn get_required_string(
    object: &Map<String, Value>,
    key: &str,
    response_id: &Option<JsonRpcId>,
) -> Result<String, WireDecodeError> {
    let value = object.get(key).ok_or_else(|| {
        invalid_request(
            format!("missing required field `{key}`"),
            response_id.clone(),
        )
    })?;

    value.as_str().map(str::to_owned).ok_or_else(|| {
        invalid_request(
            format!("field `{key}` must be a string"),
            response_id.clone(),
        )
    })
}

fn get_optional_id(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<JsonRpcId>, WireDecodeError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };

    parse_id_value(value, key, &None).map(Some)
}

fn get_required_id(
    object: &Map<String, Value>,
    key: &str,
    response_id: &Option<JsonRpcId>,
) -> Result<JsonRpcId, WireDecodeError> {
    let value = object.get(key).ok_or_else(|| {
        invalid_request(
            format!("missing required field `{key}`"),
            response_id.clone(),
        )
    })?;

    parse_id_value(value, key, response_id)
}

fn parse_id_value(
    value: &Value,
    key: &str,
    response_id: &Option<JsonRpcId>,
) -> Result<JsonRpcId, WireDecodeError> {
    match value {
        Value::String(value) => Ok(JsonRpcId::String(value.clone())),
        Value::Number(value) if value.is_i64() || value.is_u64() => {
            Ok(JsonRpcId::Number(value.clone()))
        }
        Value::Number(_) => Err(invalid_request(
            format!("field `{key}` number must be an integer"),
            response_id.clone(),
        )),
        Value::Null => Ok(JsonRpcId::Null),
        _ => Err(invalid_request(
            format!("field `{key}` must be a string, integer number, or null"),
            response_id.clone(),
        )),
    }
}

fn get_optional_params(
    object: &Map<String, Value>,
    response_id: &Option<JsonRpcId>,
) -> Result<Option<Value>, WireDecodeError> {
    let Some(params) = object.get("params") else {
        return Ok(None);
    };

    if !params.is_object() && !params.is_array() {
        return Err(invalid_request(
            "field `params` must be an object or array when present",
            response_id.clone(),
        ));
    }

    Ok(Some(params.clone()))
}

fn parse_error_envelope(
    value: &Value,
    response_id: &Option<JsonRpcId>,
) -> Result<ErrorEnvelope, WireDecodeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_request("field `error` must be an object", response_id.clone()))?;

    for field in object.keys() {
        if !ALLOWED_ERROR_FIELDS.contains(&field.as_str()) {
            return Err(invalid_request(
                format!("unexpected field `error.{field}`"),
                response_id.clone(),
            ));
        }
    }

    let code_value = object.get("code").ok_or_else(|| {
        invalid_request("missing required field `error.code`", response_id.clone())
    })?;

    let Some(code_i64) = code_value.as_i64() else {
        return Err(invalid_request(
            "field `error.code` must be an integer",
            response_id.clone(),
        ));
    };

    let code = i32::try_from(code_i64).map_err(|_| {
        invalid_request(
            "field `error.code` must fit in signed 32-bit integer range",
            response_id.clone(),
        )
    })?;

    let message_value = object.get("message").ok_or_else(|| {
        invalid_request(
            "missing required field `error.message`",
            response_id.clone(),
        )
    })?;

    let message = message_value.as_str().ok_or_else(|| {
        invalid_request(
            "field `error.message` must be a string",
            response_id.clone(),
        )
    })?;

    let data = object.get("data").cloned();

    Ok(ErrorEnvelope {
        code,
        message: message.to_owned(),
        data,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{decode_request_line, decode_response_line, encode_response_line, WireDecodeError};
    use crate::types::{ErrorEnvelope, JsonRpcId, ResponseEnvelope};

    #[test]
    fn valid_line_decode_and_response_encode_roundtrip() {
        let line = br#"{"jsonrpc":"2.0","id":"1","method":"ping","params":{"x":1}}"#;
        let decoded = decode_request_line(line).expect("line should decode");

        assert_eq!(decoded.id, Some(JsonRpcId::from("1")));
        assert_eq!(decoded.method, "ping");
        assert_eq!(decoded.params, Some(json!({"x": 1})));

        let response = ResponseEnvelope::success(
            decoded.id.clone().expect("request id should be present"),
            json!({"ok": true}),
        );
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
        assert_eq!(with_string.id, Some(JsonRpcId::from("req-1")));

        let with_number = decode_request_line(br#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#)
            .expect("number id should decode");
        assert_eq!(with_number.id, Some(JsonRpcId::from(7_i64)));

        let with_null = decode_request_line(br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#)
            .expect("null id should decode");
        assert_eq!(with_null.id, Some(JsonRpcId::Null));
    }

    #[test]
    fn request_without_id_decodes_as_notification() {
        let notification = decode_request_line(br#"{"jsonrpc":"2.0","method":"ping"}"#)
            .expect("notification should decode");

        assert!(notification.id.is_none());
        assert_eq!(notification.method, "ping");
    }

    #[test]
    fn rejects_fractional_number_ids() {
        let invalid_id = decode_request_line(br#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#)
            .expect_err("must fail");

        assert!(matches!(
            invalid_id,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `id` number must be an integer") && id.is_none()
        ));
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
        let malformed =
            decode_request_line(br#"{"jsonrpc": "2.0", "id": "1""#).expect_err("must fail");
        assert!(matches!(malformed, WireDecodeError::Parse(_)));

        let empty = decode_request_line(b"\n").expect_err("must fail");
        assert!(matches!(empty, WireDecodeError::Parse(_)));
    }

    #[test]
    fn missing_or_invalid_fields_return_invalid_request() {
        let missing_jsonrpc =
            decode_request_line(br#"{"id":"1","method":"ping"}"#).expect_err("must fail");
        assert!(matches!(
            missing_jsonrpc,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("missing required field `jsonrpc`") && id == Some(JsonRpcId::from("1"))
        ));

        let wrong_jsonrpc = decode_request_line(br#"{"jsonrpc":"1.0","id":"1","method":"ping"}"#)
            .expect_err("must fail");
        assert!(matches!(
            wrong_jsonrpc,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `jsonrpc`") && id == Some(JsonRpcId::from("1"))
        ));

        let invalid_id = decode_request_line(br#"{"jsonrpc":"2.0","id":{},"method":"ping"}"#)
            .expect_err("must fail");
        assert!(matches!(
            invalid_id,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `id`") && id.is_none()
        ));

        let invalid_params =
            decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"ping","params":1}"#)
                .expect_err("must fail");
        assert!(matches!(
            invalid_params,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `params`") && id == Some(JsonRpcId::from("1"))
        ));
    }

    #[test]
    fn non_object_input_returns_invalid_request() {
        let non_object = decode_request_line(br#"[]"#).expect_err("must fail");
        assert!(matches!(
            non_object,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("JSON object") && id.is_none()
        ));
    }

    #[test]
    fn reserved_method_prefix_and_unknown_fields_are_rejected() {
        let reserved = decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"rpc.ping"}"#)
            .expect_err("must fail");
        assert!(matches!(
            reserved,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("method names starting with `rpc.` are reserved")
                    && id == Some(JsonRpcId::from("1"))
        ));

        let unknown_field =
            decode_request_line(br#"{"jsonrpc":"2.0","id":"1","method":"ping","extra":true}"#)
                .expect_err("must fail");
        assert!(matches!(
            unknown_field,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("unexpected field `extra`") && id == Some(JsonRpcId::from("1"))
        ));
    }

    #[test]
    fn response_decode_accepts_string_number_and_null_ids() {
        let string_id = decode_response_line(br#"{"jsonrpc":"2.0","id":"res-1","result":{}}"#)
            .expect("string id response should decode");
        assert_eq!(string_id.id, JsonRpcId::from("res-1"));

        let number_id = decode_response_line(br#"{"jsonrpc":"2.0","id":42,"result":true}"#)
            .expect("number id response should decode");
        assert_eq!(number_id.id, JsonRpcId::from(42_i64));

        let null_id = decode_response_line(
            br#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}}"#,
        )
        .expect("null id response should decode");
        assert_eq!(null_id.id, JsonRpcId::Null);
    }

    #[test]
    fn response_decode_enforces_jsonrpc_shape_and_fields() {
        let wrong_jsonrpc = decode_response_line(br#"{"jsonrpc":"1.0","id":"1","result":1}"#)
            .expect_err("must fail");
        assert!(matches!(
            wrong_jsonrpc,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `jsonrpc`") && id == Some(JsonRpcId::from("1"))
        ));

        let unknown_field =
            decode_response_line(br#"{"jsonrpc":"2.0","id":"1","result":1,"extra":true}"#)
                .expect_err("must fail");
        assert!(matches!(
            unknown_field,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("unexpected field `extra`") && id == Some(JsonRpcId::from("1"))
        ));

        let invalid_id = decode_response_line(br#"{"jsonrpc":"2.0","id":{},"result":1}"#)
            .expect_err("must fail");
        assert!(matches!(
            invalid_id,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `id`") && id.is_none()
        ));
    }

    #[test]
    fn response_decode_rejects_ambiguous_or_invalid_error_payloads() {
        let both = decode_response_line(
            br#"{"jsonrpc":"2.0","id":"1","result":{},"error":{"code":-32603,"message":"Internal error"}}"#,
        )
        .expect_err("must fail");
        assert!(matches!(
            both,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("exactly one of `result` or `error`") && id == Some(JsonRpcId::from("1"))
        ));

        let neither =
            decode_response_line(br#"{"jsonrpc":"2.0","id":"1"}"#).expect_err("must fail");
        assert!(matches!(
            neither,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("exactly one of `result` or `error`") && id == Some(JsonRpcId::from("1"))
        ));

        let bad_error = decode_response_line(
            br#"{"jsonrpc":"2.0","id":"1","error":{"code":"oops","message":"bad"}}"#,
        )
        .expect_err("must fail");
        assert!(matches!(
            bad_error,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `error.code` must be an integer") && id == Some(JsonRpcId::from("1"))
        ));

        let bad_error_field = decode_response_line(
            br#"{"jsonrpc":"2.0","id":"1","error":{"code":-32603,"message":"bad","extra":true}}"#,
        )
        .expect_err("must fail");
        assert!(matches!(
            bad_error_field,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("unexpected field `error.extra`") && id == Some(JsonRpcId::from("1"))
        ));

        let out_of_range_error_code = decode_response_line(
            br#"{"jsonrpc":"2.0","id":"1","error":{"code":2147483648,"message":"bad"}}"#,
        )
        .expect_err("must fail");
        assert!(matches!(
            out_of_range_error_code,
            WireDecodeError::InvalidRequest { message, id }
                if message.contains("field `error.code` must fit in signed 32-bit integer range")
                    && id == Some(JsonRpcId::from("1"))
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
