use fittings::core::error::FittingsError;
use fittings::core::message::ServiceError;
use fittings::wire::error_map::to_error_envelope;

struct InvalidCase {
    name: &'static str,
    code: i32,
}

fn invalid_cases() -> [InvalidCase; 3] {
    [
        InvalidCase {
            name: "zero",
            code: 0,
        },
        InvalidCase {
            name: "reserved-parse-error",
            code: -32_700,
        },
        InvalidCase {
            name: "predefined-method-not-found",
            code: -32_601,
        },
    ]
}

#[test]
fn invalid_service_codes_emit_invalid_service_code_marker() {
    for case in invalid_cases() {
        let err = FittingsError::service(ServiceError {
            code: case.code,
            message: format!("{} should be rejected", case.name),
            data: None,
        });

        let envelope = to_error_envelope("req-1".to_string(), err)
            .error
            .unwrap_or_else(|| panic!("{}: error envelope missing", case.name));

        assert_eq!(
            envelope.code, -32_603,
            "{}: invalid service code falls back to Internal",
            case.name
        );

        let data = envelope
            .data
            .unwrap_or_else(|| panic!("{}: data must carry the marker", case.name));
        let kind = data
            .get("fittingsKind")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{}: data.fittingsKind missing or not a string", case.name));

        assert_eq!(
            kind, "invalidServiceCode",
            "{}: marker identifies invalid service code",
            case.name
        );
    }
}
