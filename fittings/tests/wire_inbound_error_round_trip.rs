use fittings::core::error::FittingsError;
use fittings::wire::error_map::{from_error_envelope, to_error_envelope};
use fittings::wire::types::JsonRpcId;
use serde_json::{json, Value};

struct VariantCase {
    name: &'static str,
    err: FittingsError,
    expected_message: &'static str,
    expected_data: Value,
}

fn cases() -> [VariantCase; 5] {
    [
        VariantCase {
            name: "Parse",
            err: FittingsError::parse_error_with_data("bad json", json!({"line": 1})),
            expected_message: "bad json",
            expected_data: json!({"line": 1}),
        },
        VariantCase {
            name: "InvalidRequest",
            err: FittingsError::invalid_request_with_data("missing id", json!({"field": "id"})),
            expected_message: "missing id",
            expected_data: json!({"field": "id"}),
        },
        VariantCase {
            name: "MethodNotFound",
            err: FittingsError::method_not_found_with_method("math/double", "nope"),
            expected_message: "nope",
            expected_data: json!({"method": "math/double"}),
        },
        VariantCase {
            name: "InvalidParams",
            err: FittingsError::invalid_params_with_data(
                "wrong shape",
                json!({"detail": "expected u32"}),
            ),
            expected_message: "wrong shape",
            expected_data: json!({"detail": "expected u32"}),
        },
        VariantCase {
            name: "Internal",
            err: FittingsError::internal_with_data("kaboom", json!({"trace": "abc"})),
            expected_message: "kaboom",
            expected_data: json!({"trace": "abc"}),
        },
    ]
}

#[test]
fn predefined_variants_round_trip_byte_equal_through_to_then_from() {
    for case in cases() {
        let original = case.err.clone();
        let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), case.err);
        let error = envelope
            .error
            .clone()
            .unwrap_or_else(|| panic!("{}: error envelope must be present", case.name));

        assert_eq!(
            error.message, case.expected_message,
            "{}: outbound message preserved",
            case.name,
        );
        assert_eq!(
            error.data,
            Some(case.expected_data.clone()),
            "{}: outbound data preserved",
            case.name,
        );

        let decoded = from_error_envelope(error);
        assert_eq!(
            decoded, original,
            "{}: inbound decode reconstructs the same FittingsError variant",
            case.name,
        );
    }
}

#[test]
fn predefined_variants_round_trip_with_data_none() {
    let originals = [
        FittingsError::parse_error("bad json"),
        FittingsError::invalid_request("missing id"),
        FittingsError::method_not_found("nope"),
        FittingsError::invalid_params("wrong shape"),
        FittingsError::internal("kaboom"),
    ];

    for original in originals {
        let envelope = to_error_envelope(JsonRpcId::from("req-1".to_string()), original.clone());
        let error = envelope.error.expect("error envelope must be present");
        assert_eq!(error.data, None);
        let decoded = from_error_envelope(error);
        assert_eq!(decoded, original);
    }
}
