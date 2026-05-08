use fittings::core::error::FittingsError;
use fittings::core::message::ServiceError;
use fittings::wire::error_map::{from_error_envelope, to_error_envelope};

struct ValidCase {
    name: &'static str,
    code: i32,
}

fn valid_cases() -> [ValidCase; 4] {
    [
        ValidCase {
            name: "positive",
            code: 42,
        },
        ValidCase {
            name: "server-band",
            code: -32_050,
        },
        ValidCase {
            name: "below-reserved",
            code: -40_000,
        },
        ValidCase {
            name: "above-reserved-negative",
            code: -31_999,
        },
    ]
}

#[test]
fn valid_service_codes_round_trip_without_marker_rewrite() {
    for case in valid_cases() {
        let original = ServiceError {
            code: case.code,
            message: format!("{} domain failure", case.name),
            data: None,
        };

        let envelope = to_error_envelope(
            "req-1".to_string(),
            FittingsError::service(original.clone()),
        )
        .error
        .unwrap_or_else(|| panic!("{}: error envelope missing", case.name));

        assert_eq!(
            envelope.code, case.code,
            "{}: outbound code preserved",
            case.name
        );
        assert_eq!(
            envelope.message, original.message,
            "{}: outbound message preserved",
            case.name
        );
        assert_eq!(
            envelope.data, original.data,
            "{}: outbound data preserved (no invalidServiceCode marker)",
            case.name
        );

        let decoded = from_error_envelope(envelope);
        match decoded {
            FittingsError::Service(roundtripped) => {
                assert_eq!(
                    roundtripped, original,
                    "{}: round-trip preserves ServiceError verbatim",
                    case.name
                );
            }
            other => panic!(
                "{}: expected Service variant after round-trip, got {:?}",
                case.name, other
            ),
        }
    }
}
