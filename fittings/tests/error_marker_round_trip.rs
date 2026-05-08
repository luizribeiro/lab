use fittings::core::error::FittingsError;
use fittings::wire::error_map::{from_error_envelope, to_error_envelope};
use fittings::wire::types::JsonRpcId;
use serde_json::json;

#[test]
fn transport_marker_round_trips_via_fittings_kind() {
    let original = FittingsError::transport("broken pipe");
    let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), original.clone());
    let error = envelope.error.expect("error envelope must be present");

    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "transport");
    assert_eq!(
        error.data,
        Some(json!({"fittingsKind": "transport", "detail": "broken pipe"})),
    );

    let decoded = from_error_envelope(error);
    assert_eq!(decoded, original);
}

#[test]
fn panic_marker_round_trips_via_fittings_kind() {
    let original = FittingsError::panic("kaboom");
    let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), original.clone());
    let error = envelope.error.expect("error envelope must be present");

    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "internal error");
    assert_eq!(
        error.data,
        Some(json!({"fittingsKind": "panic", "detail": "kaboom"})),
    );

    let decoded = from_error_envelope(error);
    assert_eq!(decoded, original);
}

#[test]
fn internal_without_marker_decodes_as_internal() {
    let envelope = to_error_envelope(
        JsonRpcId::from("req-1".to_string()),
        FittingsError::internal_with_data("oops", json!({"trace": "abc"})),
    );
    let error = envelope.error.expect("error envelope must be present");

    let decoded = from_error_envelope(error);
    assert!(matches!(
        decoded,
        FittingsError::Internal { message, data }
            if message == "oops" && data == Some(json!({"trace": "abc"}))
    ));
}
