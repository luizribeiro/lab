use fittings::core::error::FittingsError;
use serde_json::{json, Value};

struct VariantCase {
    name: &'static str,
    without: FittingsError,
    with: FittingsError,
    expected_message: &'static str,
    expected_data: Value,
}

fn cases() -> [VariantCase; 5] {
    [
        VariantCase {
            name: "Parse",
            without: FittingsError::parse_error("bad json"),
            with: FittingsError::parse_error_with_data("bad json", json!({"line": 1})),
            expected_message: "bad json",
            expected_data: json!({"line": 1}),
        },
        VariantCase {
            name: "InvalidRequest",
            without: FittingsError::invalid_request("missing id"),
            with: FittingsError::invalid_request_with_data("missing id", json!({"field": "id"})),
            expected_message: "missing id",
            expected_data: json!({"field": "id"}),
        },
        VariantCase {
            name: "MethodNotFound",
            without: FittingsError::method_not_found("nope"),
            with: FittingsError::method_not_found_with_data(
                "nope",
                json!({"method": "math/double"}),
            ),
            expected_message: "nope",
            expected_data: json!({"method": "math/double"}),
        },
        VariantCase {
            name: "InvalidParams",
            without: FittingsError::invalid_params("wrong shape"),
            with: FittingsError::invalid_params_with_data(
                "wrong shape",
                json!({"detail": "expected u32"}),
            ),
            expected_message: "wrong shape",
            expected_data: json!({"detail": "expected u32"}),
        },
        VariantCase {
            name: "Internal",
            without: FittingsError::internal("kaboom"),
            with: FittingsError::internal_with_data("kaboom", json!({"trace": "..."})),
            expected_message: "kaboom",
            expected_data: json!({"trace": "..."}),
        },
    ]
}

fn read(error: &FittingsError) -> (&str, Option<&Value>) {
    match error {
        FittingsError::Parse { message, data }
        | FittingsError::InvalidRequest { message, data }
        | FittingsError::MethodNotFound { message, data }
        | FittingsError::InvalidParams { message, data }
        | FittingsError::Internal { message, data } => (message.as_str(), data.as_ref()),
        other => panic!("expected predefined variant, got {other:?}"),
    }
}

#[test]
fn predefined_variants_carry_message_and_data() {
    for case in cases() {
        let (msg_without, data_without) = read(&case.without);
        assert_eq!(
            msg_without, case.expected_message,
            "{}: message via single-arg constructor",
            case.name,
        );
        assert!(
            data_without.is_none(),
            "{}: single-arg constructor must default data to None",
            case.name,
        );

        let (msg_with, data_with) = read(&case.with);
        assert_eq!(
            msg_with, case.expected_message,
            "{}: message via data-bearing constructor",
            case.name,
        );
        assert_eq!(
            data_with,
            Some(&case.expected_data),
            "{}: data preserved via data-bearing constructor",
            case.name,
        );
    }
}

#[test]
fn panic_variant_carries_message_only() {
    let err = FittingsError::panic("boom");
    match err {
        FittingsError::Panic { message } => assert_eq!(message, "boom"),
        other => panic!("expected Panic variant, got {other:?}"),
    }

    let direct = FittingsError::Panic {
        message: "direct".to_string(),
    };
    match direct {
        FittingsError::Panic { message } => assert_eq!(message, "direct"),
        _ => panic!("direct construction must produce Panic variant"),
    }
}
