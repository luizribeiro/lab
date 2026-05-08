use fittings::core::error::FittingsError;
use fittings::wire::error_map::{from_error_envelope, to_error_envelope};
use fittings::wire::types::{ErrorEnvelope, JsonRpcId};

#[test]
fn from_error_envelope_never_decodes_to_cancelled() {
    let candidate_codes: [i32; 11] = [
        -32_700, -32_600, -32_601, -32_602, -32_603, -32_000, -32_099, -32_500, 1, 7, 1_000,
    ];

    for code in candidate_codes {
        let decoded = from_error_envelope(ErrorEnvelope {
            code,
            message: "anything".to_string(),
            data: None,
        });
        assert!(
            !matches!(decoded, FittingsError::Cancelled { .. }),
            "code {code} unexpectedly decoded to Cancelled",
        );
    }
}

#[test]
fn to_error_envelope_does_not_emit_a_dedicated_cancelled_marker() {
    let envelope = to_error_envelope(
        JsonRpcId::from("req-1".to_string()),
        FittingsError::cancelled(Some("client aborted".to_string())),
    );
    let error = envelope.error.expect("error envelope must be present");

    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "Internal error");
    let serialized = serde_json::to_string(&error.data).expect("data serializes");
    assert!(
        !serialized.contains("cancelled") && !serialized.contains("Cancelled"),
        "envelope data leaked a cancelled marker: {serialized}",
    );
}
