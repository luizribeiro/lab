use fittings::{
    core::message::{JsonRpcId, Request},
    decode_request_line,
};
use serde_json::Value;

fn wire_to_core_request(frame: &[u8]) -> Request {
    let envelope = decode_request_line(frame).expect("frame should decode");
    Request {
        id: envelope.id,
        method: envelope.method,
        params: envelope.params.unwrap_or(Value::Null),
        metadata: Default::default(),
    }
}

#[test]
fn missing_id_decodes_as_notification() {
    let frame = br#"{"jsonrpc":"2.0","method":"ping","params":{}}"#;
    let request = wire_to_core_request(frame);

    assert!(request.id.is_none(), "missing wire id must yield None");
    assert_eq!(request.method, "ping");
}

#[test]
fn explicit_null_id_decodes_as_request_with_null_id() {
    let frame = br#"{"jsonrpc":"2.0","id":null,"method":"ping","params":{}}"#;
    let request = wire_to_core_request(frame);

    assert_eq!(
        request.id,
        Some(JsonRpcId::Null),
        "explicit `id: null` must yield Some(JsonRpcId::Null), distinguishing it from a notification"
    );
}

#[test]
fn typed_id_decodes_to_matching_variant() {
    let string_request =
        wire_to_core_request(br#"{"jsonrpc":"2.0","id":"req-1","method":"ping","params":{}}"#);
    assert_eq!(string_request.id, Some(JsonRpcId::from("req-1")));

    let number_request =
        wire_to_core_request(br#"{"jsonrpc":"2.0","id":42,"method":"ping","params":{}}"#);
    assert_eq!(number_request.id, Some(JsonRpcId::from(42_i64)));
}
