use fittings_core::{error::FittingsError, message::ServiceError};
use serde_json::{json, Value};

use crate::types::{ErrorEnvelope, JsonRpcId, ResponseEnvelope};

const PARSE_ERROR_CODE: i32 = -32700;
const INVALID_REQUEST_CODE: i32 = -32600;
const METHOD_NOT_FOUND_CODE: i32 = -32601;
const INVALID_PARAMS_CODE: i32 = -32602;
const INTERNAL_ERROR_CODE: i32 = -32603;

const TRANSPORT_MESSAGE: &str = "transport";
const PANIC_MESSAGE: &str = "internal error";
const INTERNAL_ERROR_MESSAGE: &str = "Internal error";

const FITTINGS_KIND_KEY: &str = "fittingsKind";
const FITTINGS_KIND_TRANSPORT: &str = "transport";
const FITTINGS_KIND_PANIC: &str = "panic";
const FITTINGS_DETAIL_KEY: &str = "detail";

pub fn to_error_envelope(id: impl Into<JsonRpcId>, err: FittingsError) -> ResponseEnvelope {
    let id = id.into();
    let error = match err {
        FittingsError::Parse { message, data } => ErrorEnvelope {
            code: PARSE_ERROR_CODE,
            message,
            data,
        },
        FittingsError::InvalidRequest { message, data } => ErrorEnvelope {
            code: INVALID_REQUEST_CODE,
            message,
            data,
        },
        FittingsError::MethodNotFound { message, data } => ErrorEnvelope {
            code: METHOD_NOT_FOUND_CODE,
            message,
            data,
        },
        FittingsError::InvalidParams { message, data } => ErrorEnvelope {
            code: INVALID_PARAMS_CODE,
            message,
            data,
        },
        FittingsError::Internal { message, data } => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message,
            data,
        },
        FittingsError::Transport(detail) => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message: TRANSPORT_MESSAGE.to_string(),
            data: Some(json!({
                FITTINGS_KIND_KEY: FITTINGS_KIND_TRANSPORT,
                FITTINGS_DETAIL_KEY: detail,
            })),
        },
        FittingsError::Panic { message } => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message: PANIC_MESSAGE.to_string(),
            data: Some(json!({
                FITTINGS_KIND_KEY: FITTINGS_KIND_PANIC,
                FITTINGS_DETAIL_KEY: message,
            })),
        },
        FittingsError::Service(ServiceError {
            code,
            message,
            data,
        }) if ServiceError::is_valid_code_value(code) => ErrorEnvelope {
            code,
            message,
            data,
        },
        FittingsError::Service(_) => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message: INTERNAL_ERROR_MESSAGE.to_string(),
            data: None,
        },
    };

    ResponseEnvelope::error(id, error)
}

pub fn from_error_envelope(error: ErrorEnvelope) -> FittingsError {
    match error {
        ErrorEnvelope {
            code: PARSE_ERROR_CODE,
            message,
            data,
        } => FittingsError::Parse { message, data },
        ErrorEnvelope {
            code: INVALID_REQUEST_CODE,
            message,
            data,
        } => FittingsError::InvalidRequest { message, data },
        ErrorEnvelope {
            code: METHOD_NOT_FOUND_CODE,
            message,
            data,
        } => FittingsError::MethodNotFound { message, data },
        ErrorEnvelope {
            code: INVALID_PARAMS_CODE,
            message,
            data,
        } => FittingsError::InvalidParams { message, data },
        ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message,
            data,
        } => match fittings_kind(data.as_ref()) {
            Some(FITTINGS_KIND_TRANSPORT) => {
                FittingsError::Transport(extract_detail(data.as_ref(), &message))
            }
            Some(FITTINGS_KIND_PANIC) => FittingsError::Panic {
                message: extract_detail(data.as_ref(), &message),
            },
            _ => FittingsError::Internal { message, data },
        },
        ErrorEnvelope {
            code,
            message,
            data,
        } if ServiceError::is_valid_code_value(code) => FittingsError::service(ServiceError {
            code,
            message,
            data,
        }),
        ErrorEnvelope { .. } => FittingsError::internal(INTERNAL_ERROR_MESSAGE),
    }
}

fn fittings_kind(data: Option<&Value>) -> Option<&str> {
    data?.get(FITTINGS_KIND_KEY)?.as_str()
}

fn extract_detail(data: Option<&Value>, fallback_message: &str) -> String {
    data.and_then(|d| d.get(FITTINGS_DETAIL_KEY))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback_message.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use fittings_core::{error::FittingsError, message::ServiceError};

    use super::{from_error_envelope, to_error_envelope};
    use crate::types::ErrorEnvelope;

    #[test]
    fn maps_each_error_family_to_expected_code_and_message() {
        let parse = to_error_envelope("1".to_string(), FittingsError::parse_error("bad json"));
        let parse_error = parse.error.expect("error");
        assert_eq!(parse_error.code, -32700);
        assert_eq!(parse_error.message, "bad json");

        let invalid = to_error_envelope(
            "1".to_string(),
            FittingsError::invalid_request("bad request"),
        );
        let invalid_error = invalid.error.expect("error");
        assert_eq!(invalid_error.code, -32600);
        assert_eq!(invalid_error.message, "bad request");

        let not_found =
            to_error_envelope("1".to_string(), FittingsError::method_not_found("missing"));
        let not_found_error = not_found.error.expect("error");
        assert_eq!(not_found_error.code, -32601);
        assert_eq!(not_found_error.message, "missing");

        let bad_params =
            to_error_envelope("1".to_string(), FittingsError::invalid_params("bad params"));
        let bad_params_error = bad_params.error.expect("error");
        assert_eq!(bad_params_error.code, -32602);
        assert_eq!(bad_params_error.message, "bad params");

        let internal = to_error_envelope("1".to_string(), FittingsError::internal("oops"));
        let internal_error = internal.error.expect("error");
        assert_eq!(internal_error.code, -32603);
        assert_eq!(internal_error.message, "oops");

        let transport = to_error_envelope("1".to_string(), FittingsError::transport("pipe"));
        let transport_error = transport.error.expect("error");
        assert_eq!(transport_error.code, -32603);
        assert_eq!(transport_error.message, "transport");
        assert_eq!(
            transport_error.data,
            Some(json!({"fittingsKind": "transport", "detail": "pipe"})),
        );

        let panic = to_error_envelope("1".to_string(), FittingsError::panic("boom"));
        let panic_error = panic.error.expect("error");
        assert_eq!(panic_error.code, -32603);
        assert_eq!(panic_error.message, "internal error");
        assert_eq!(
            panic_error.data,
            Some(json!({"fittingsKind": "panic", "detail": "boom"})),
        );
    }

    #[test]
    fn maps_out_of_range_service_code_to_internal_error() {
        let err = FittingsError::service(ServiceError {
            code: 1_000,
            message: "domain error".to_string(),
            data: Some(json!({"detail": "x"})),
        });

        let mapped = to_error_envelope("request-1".to_string(), err);
        let error = mapped.error.expect("error envelope should exist");

        assert_eq!(error.code, -32603);
        assert_eq!(error.message, "Internal error");
        assert_eq!(error.data, None);
    }

    #[test]
    fn keeps_valid_service_code_and_data() {
        let err = FittingsError::service(ServiceError {
            code: 7,
            message: "domain error".to_string(),
            data: Some(json!({"detail": "x"})),
        });

        let mapped = to_error_envelope("request-1".to_string(), err);
        let error = mapped.error.expect("error envelope should exist");

        assert_eq!(error.code, 7);
        assert_eq!(error.message, "domain error");
        assert_eq!(error.data, Some(json!({"detail": "x"})));
    }

    #[test]
    fn reverse_mapping_table_is_deterministic() {
        let parse = from_error_envelope(ErrorEnvelope {
            code: -32700,
            message: "bad json".to_string(),
            data: Some(json!({"line": 1})),
        });
        assert!(matches!(
            parse,
            FittingsError::Parse { message, data }
                if message == "bad json" && data == Some(json!({"line": 1}))
        ));

        let invalid_request = from_error_envelope(ErrorEnvelope {
            code: -32600,
            message: "bad request".to_string(),
            data: None,
        });
        assert!(matches!(
            invalid_request,
            FittingsError::InvalidRequest { message, data: None } if message == "bad request"
        ));

        let method_not_found = from_error_envelope(ErrorEnvelope {
            code: -32601,
            message: "missing".to_string(),
            data: Some(json!({"method": "x"})),
        });
        assert!(matches!(
            method_not_found,
            FittingsError::MethodNotFound { message, data }
                if message == "missing" && data == Some(json!({"method": "x"}))
        ));

        let invalid_params = from_error_envelope(ErrorEnvelope {
            code: -32602,
            message: "bad params".to_string(),
            data: None,
        });
        assert!(matches!(
            invalid_params,
            FittingsError::InvalidParams { message, data: None } if message == "bad params"
        ));

        let internal = from_error_envelope(ErrorEnvelope {
            code: -32603,
            message: "kaboom".to_string(),
            data: Some(json!({"trace": "abc"})),
        });
        assert!(matches!(
            internal,
            FittingsError::Internal { message, data }
                if message == "kaboom" && data == Some(json!({"trace": "abc"}))
        ));

        let service = from_error_envelope(ErrorEnvelope {
            code: 42,
            message: "domain".to_string(),
            data: Some(json!({"detail": "x"})),
        });
        assert!(matches!(
            service,
            FittingsError::Service(ServiceError { code: 42, message, data })
                if message == "domain" && data == Some(json!({"detail": "x"}))
        ));

        let unknown_negative = from_error_envelope(ErrorEnvelope {
            code: -32000,
            message: "unknown negative".to_string(),
            data: None,
        });
        assert!(matches!(
            unknown_negative,
            FittingsError::Internal { message, .. } if message == "Internal error"
        ));

        let unknown_positive = from_error_envelope(ErrorEnvelope {
            code: 1000,
            message: "unknown positive".to_string(),
            data: Some(json!({"ignored": true})),
        });
        assert!(matches!(
            unknown_positive,
            FittingsError::Internal { message, .. } if message == "Internal error"
        ));
    }
}
