use fittings_wire::{
    codec::{decode_request_line, encode_response_line, WireDecodeError, WireEncodeError},
    types::{ErrorEnvelope, JsonRpcId, RequestEnvelope, ResponseEnvelope},
};
use serde_json::Value;

pub fn request_envelope(id: impl Into<JsonRpcId>, method: &str, params: Value) -> RequestEnvelope {
    RequestEnvelope::new(id, method, Some(params))
}

pub fn request_line(id: impl Into<JsonRpcId>, method: &str, params: Value) -> Vec<u8> {
    let envelope = request_envelope(id, method, params);
    let mut bytes = serde_json::to_vec(&envelope).expect("request fixture should serialize");
    bytes.push(b'\n');
    bytes
}

pub fn success_response_envelope(id: impl Into<JsonRpcId>, result: Value) -> ResponseEnvelope {
    ResponseEnvelope::success(id, result)
}

pub fn error_response_envelope(
    id: impl Into<JsonRpcId>,
    code: i32,
    message: &str,
) -> ResponseEnvelope {
    ResponseEnvelope::error(
        id,
        ErrorEnvelope {
            code,
            message: message.to_string(),
            data: None,
        },
    )
}

pub fn success_response_line(
    id: impl Into<JsonRpcId>,
    result: Value,
) -> Result<Vec<u8>, WireEncodeError> {
    encode_response_line(&success_response_envelope(id, result))
}

pub fn error_response_line(
    id: impl Into<JsonRpcId>,
    code: i32,
    message: &str,
) -> Result<Vec<u8>, WireEncodeError> {
    encode_response_line(&error_response_envelope(id, code, message))
}

pub fn parse_request_fixture(line: &[u8]) -> Result<RequestEnvelope, WireDecodeError> {
    decode_request_line(line)
}

pub fn parse_response_fixture(line: &[u8]) -> Result<ResponseEnvelope, serde_json::Error> {
    serde_json::from_slice(line)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        error_response_line, parse_request_fixture, parse_response_fixture, request_line,
        success_response_line,
    };

    #[test]
    fn fixture_parsers_produce_expected_envelopes() {
        let request = request_line("req-1", "hello", json!({"name": "Ada"}));
        let parsed_request = parse_request_fixture(&request).expect("request fixture should parse");
        assert_eq!(parsed_request.id, "req-1");
        assert_eq!(parsed_request.method, "hello");
        assert_eq!(parsed_request.params, Some(json!({"name": "Ada"})));

        let success = success_response_line("req-1", json!({"message": "Hi Ada"}))
            .expect("success response should encode");
        let parsed_success = parse_response_fixture(&success).expect("success should parse");
        assert_eq!(parsed_success.id, "req-1");
        assert_eq!(parsed_success.result, Some(json!({"message": "Hi Ada"})));
        assert!(parsed_success.error.is_none());

        let error = error_response_line("req-2", -32601, "method not found")
            .expect("error response should encode");
        let parsed_error = parse_response_fixture(&error).expect("error should parse");
        assert_eq!(parsed_error.id, "req-2");
        assert!(parsed_error.result.is_none());
        assert_eq!(parsed_error.error.expect("error envelope").code, -32601);
    }
}
