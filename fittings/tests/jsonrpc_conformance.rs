use fittings::wire::{codec::decode_response_line, types::JsonRpcId};
use fittings::{decode_request_line, WireDecodeError};

#[test]
fn strict_request_envelope_rejects_non_conformant_shapes() {
    let unknown_field = decode_request_line(
        br#"{"jsonrpc":"2.0","id":"req-1","method":"hello","params":{},"metadata":{"trace":"x"}}"#,
    )
    .expect_err("unknown fields must be rejected");
    assert!(matches!(
        unknown_field,
        WireDecodeError::InvalidRequest { message, id }
            if message.contains("unexpected field `metadata`")
                && id == Some(JsonRpcId::from("req-1"))
    ));

    let reserved_method =
        decode_request_line(br#"{"jsonrpc":"2.0","id":"req-2","method":"rpc.ping","params":{}}"#)
            .expect_err("reserved rpc.* methods must be rejected");
    assert!(matches!(
        reserved_method,
        WireDecodeError::InvalidRequest { message, id }
            if message.contains("method names starting with `rpc.` are reserved")
                && id == Some(JsonRpcId::from("req-2"))
    ));
}

#[test]
fn strict_response_envelope_requires_exactly_one_of_result_or_error() {
    let ambiguous = decode_response_line(
        br#"{"jsonrpc":"2.0","id":"res-1","result":{},"error":{"code":-32603,"message":"Internal error"}}"#,
    )
    .expect_err("response with both result and error must be rejected");
    assert!(matches!(
        ambiguous,
        WireDecodeError::InvalidRequest { message, id }
            if message.contains("exactly one of `result` or `error`")
                && id == Some(JsonRpcId::from("res-1"))
    ));

    let missing = decode_response_line(br#"{"jsonrpc":"2.0","id":"res-2"}"#)
        .expect_err("response with neither result nor error must be rejected");
    assert!(matches!(
        missing,
        WireDecodeError::InvalidRequest { message, id }
            if message.contains("exactly one of `result` or `error`")
                && id == Some(JsonRpcId::from("res-2"))
    ));
}

#[test]
fn typed_ids_are_preserved_for_request_and_response_envelopes() {
    let string_id = decode_request_line(br#"{"jsonrpc":"2.0","id":"s-1","method":"hello"}"#)
        .expect("string id should decode");
    assert_eq!(string_id.id, Some(JsonRpcId::from("s-1")));

    let number_id = decode_request_line(br#"{"jsonrpc":"2.0","id":11,"method":"hello"}"#)
        .expect("number id should decode");
    assert_eq!(number_id.id, Some(JsonRpcId::from(11_i64)));

    let null_id = decode_response_line(
        br#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}}"#,
    )
    .expect("null id response should decode");
    assert_eq!(null_id.id, JsonRpcId::Null);
}
