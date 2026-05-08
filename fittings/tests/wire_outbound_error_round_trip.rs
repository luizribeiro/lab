use fittings::core::error::FittingsError;
use fittings::wire::error_map::to_error_envelope;
use fittings::wire::types::JsonRpcId;
use serde_json::{json, Value};

struct VariantCase {
    name: &'static str,
    err: FittingsError,
    expected_code: i32,
    expected_message: &'static str,
    expected_data: Value,
}

fn cases() -> [VariantCase; 5] {
    [
        VariantCase {
            name: "Parse",
            err: FittingsError::parse_error_with_data("bad json", json!({"line": 1})),
            expected_code: -32700,
            expected_message: "bad json",
            expected_data: json!({"line": 1}),
        },
        VariantCase {
            name: "InvalidRequest",
            err: FittingsError::invalid_request_with_data("missing id", json!({"field": "id"})),
            expected_code: -32600,
            expected_message: "missing id",
            expected_data: json!({"field": "id"}),
        },
        VariantCase {
            name: "MethodNotFound",
            err: FittingsError::method_not_found_with_data(
                "nope",
                json!({"method": "math/double"}),
            ),
            expected_code: -32601,
            expected_message: "nope",
            expected_data: json!({"method": "math/double"}),
        },
        VariantCase {
            name: "InvalidParams",
            err: FittingsError::invalid_params_with_data(
                "wrong shape",
                json!({"detail": "expected u32"}),
            ),
            expected_code: -32602,
            expected_message: "wrong shape",
            expected_data: json!({"detail": "expected u32"}),
        },
        VariantCase {
            name: "Internal",
            err: FittingsError::internal_with_data("kaboom", json!({"trace": "abc"})),
            expected_code: -32603,
            expected_message: "kaboom",
            expected_data: json!({"trace": "abc"}),
        },
    ]
}

#[test]
fn outbound_envelope_preserves_message_and_data_for_predefined_variants() {
    for case in cases() {
        let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), case.err);
        let error = envelope
            .error
            .as_ref()
            .unwrap_or_else(|| panic!("{}: error envelope must be present", case.name));

        assert_eq!(error.code, case.expected_code, "{}: code", case.name);
        assert_eq!(
            error.message, case.expected_message,
            "{}: typed message preserved",
            case.name,
        );
        assert_eq!(
            error.data,
            Some(case.expected_data.clone()),
            "{}: data byte-equal",
            case.name,
        );
    }
}

#[test]
fn outbound_envelope_omits_data_when_none_for_predefined_variants() {
    let cases: [(FittingsError, i32, &str); 5] = [
        (FittingsError::parse_error("bad json"), -32700, "bad json"),
        (
            FittingsError::invalid_request("missing id"),
            -32600,
            "missing id",
        ),
        (FittingsError::method_not_found("nope"), -32601, "nope"),
        (
            FittingsError::invalid_params("wrong shape"),
            -32602,
            "wrong shape",
        ),
        (FittingsError::internal("kaboom"), -32603, "kaboom"),
    ];

    for (err, expected_code, expected_message) in cases {
        let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), err);
        let error = envelope.error.expect("error envelope must be present");
        assert_eq!(error.code, expected_code);
        assert_eq!(error.message, expected_message);
        assert_eq!(error.data, None);
    }
}
