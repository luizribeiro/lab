use fittings::core::error::FittingsError;
use fittings::wire::error_map::{from_error_envelope, to_error_envelope};
use fittings::wire::types::JsonRpcId;
use serde_json::{json, Value};

struct Case {
    name: &'static str,
    err: FittingsError,
    expected_wire_data: Option<Value>,
    expected_decoded_method: Option<&'static str>,
    expected_decoded_data: Option<Value>,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "(a) None round-trip with no data",
            err: FittingsError::method_not_found("nope"),
            expected_wire_data: None,
            expected_decoded_method: None,
            expected_decoded_data: None,
        },
        Case {
            name: "(b) Some(name) synthesised and recovered",
            err: FittingsError::method_not_found_with_method("math/double", "no such method"),
            expected_wire_data: Some(json!({"method": "math/double"})),
            expected_decoded_method: Some("math/double"),
            expected_decoded_data: None,
        },
        Case {
            name: "(c) caller-supplied data: synthesised method overwrites conflicting key, others preserved",
            err: {
                let mut err = FittingsError::method_not_found_with_method(
                    "math/double",
                    "no such method",
                );
                if let FittingsError::MethodNotFound { data, .. } = &mut err {
                    *data = Some(json!({"method": "stale", "hint": "register first"}));
                }
                err
            },
            expected_wire_data: Some(json!({"method": "math/double", "hint": "register first"})),
            expected_decoded_method: Some("math/double"),
            expected_decoded_data: Some(json!({"hint": "register first"})),
        },
        Case {
            name: "(c2) None encode preserves opaque data verbatim",
            err: FittingsError::method_not_found_with_data(
                "nope",
                json!({"hint": "register first"}),
            ),
            expected_wire_data: Some(json!({"hint": "register first"})),
            expected_decoded_method: None,
            expected_decoded_data: Some(json!({"hint": "register first"})),
        },
    ]
}

#[test]
fn typed_method_round_trips_through_wire() {
    for case in cases() {
        let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), case.err);
        let error = envelope
            .error
            .clone()
            .unwrap_or_else(|| panic!("[{}] envelope must carry error", case.name));

        assert_eq!(
            error.code, -32601,
            "[{}] code must be MethodNotFound",
            case.name
        );
        assert_eq!(
            error.data, case.expected_wire_data,
            "[{}] wire data shape",
            case.name
        );

        let decoded = from_error_envelope(error);
        match decoded {
            FittingsError::MethodNotFound {
                method,
                message: _,
                data,
            } => {
                assert_eq!(
                    method.as_deref(),
                    case.expected_decoded_method,
                    "[{}] typed method recovered",
                    case.name
                );
                assert_eq!(data, case.expected_decoded_data, "[{}] data", case.name);
            }
            other => panic!("[{}] expected MethodNotFound, got {other:?}", case.name),
        }
    }
}

#[test]
fn one_arg_constructor_sets_method_to_none() {
    let err = FittingsError::method_not_found("nope");
    match err {
        FittingsError::MethodNotFound {
            method: None,
            message,
            data: None,
        } => assert_eq!(message, "nope"),
        other => panic!("expected method=None, got {other:?}"),
    }
}

#[test]
fn with_method_constructor_sets_typed_method() {
    let err = FittingsError::method_not_found_with_method("math/double", "no such method");
    match err {
        FittingsError::MethodNotFound {
            method: Some(method),
            message,
            data: None,
        } => {
            assert_eq!(method, "math/double");
            assert_eq!(message, "no such method");
        }
        other => panic!("expected method=Some, got {other:?}"),
    }
}
