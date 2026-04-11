use fittings_core::{error::FittingsError, message::ServiceError};

use crate::types::{ErrorEnvelope, JsonRpcId, ResponseEnvelope};

const PARSE_ERROR_CODE: i32 = -32700;
const INVALID_REQUEST_CODE: i32 = -32600;
const METHOD_NOT_FOUND_CODE: i32 = -32601;
const INVALID_PARAMS_CODE: i32 = -32602;
const INTERNAL_ERROR_CODE: i32 = -32603;

const PARSE_ERROR_MESSAGE: &str = "Parse error";
const INVALID_REQUEST_MESSAGE: &str = "Invalid Request";
const METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";
const INVALID_PARAMS_MESSAGE: &str = "Invalid params";
const INTERNAL_ERROR_MESSAGE: &str = "Internal error";

pub fn to_error_envelope(id: impl Into<JsonRpcId>, err: FittingsError) -> ResponseEnvelope {
    let id = id.into();
    let error = match err {
        FittingsError::ParseError(_) => ErrorEnvelope {
            code: PARSE_ERROR_CODE,
            message: PARSE_ERROR_MESSAGE.to_string(),
            data: None,
        },
        FittingsError::InvalidRequest(_) => ErrorEnvelope {
            code: INVALID_REQUEST_CODE,
            message: INVALID_REQUEST_MESSAGE.to_string(),
            data: None,
        },
        FittingsError::MethodNotFound(_) => ErrorEnvelope {
            code: METHOD_NOT_FOUND_CODE,
            message: METHOD_NOT_FOUND_MESSAGE.to_string(),
            data: None,
        },
        FittingsError::InvalidParams(_) => ErrorEnvelope {
            code: INVALID_PARAMS_CODE,
            message: INVALID_PARAMS_MESSAGE.to_string(),
            data: None,
        },
        FittingsError::Transport(_) | FittingsError::Internal(_) => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message: INTERNAL_ERROR_MESSAGE.to_string(),
            data: None,
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
            ..
        } => FittingsError::parse_error(PARSE_ERROR_MESSAGE),
        ErrorEnvelope {
            code: INVALID_REQUEST_CODE,
            ..
        } => FittingsError::invalid_request(INVALID_REQUEST_MESSAGE),
        ErrorEnvelope {
            code: METHOD_NOT_FOUND_CODE,
            ..
        } => FittingsError::method_not_found(METHOD_NOT_FOUND_MESSAGE),
        ErrorEnvelope {
            code: INVALID_PARAMS_CODE,
            ..
        } => FittingsError::invalid_params(INVALID_PARAMS_MESSAGE),
        ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            ..
        } => FittingsError::internal(INTERNAL_ERROR_MESSAGE),
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
        assert_eq!(parse_error.message, "Parse error");

        let invalid = to_error_envelope(
            "1".to_string(),
            FittingsError::invalid_request("bad request"),
        );
        let invalid_error = invalid.error.expect("error");
        assert_eq!(invalid_error.code, -32600);
        assert_eq!(invalid_error.message, "Invalid Request");

        let not_found =
            to_error_envelope("1".to_string(), FittingsError::method_not_found("missing"));
        let not_found_error = not_found.error.expect("error");
        assert_eq!(not_found_error.code, -32601);
        assert_eq!(not_found_error.message, "Method not found");

        let bad_params =
            to_error_envelope("1".to_string(), FittingsError::invalid_params("bad params"));
        let bad_params_error = bad_params.error.expect("error");
        assert_eq!(bad_params_error.code, -32602);
        assert_eq!(bad_params_error.message, "Invalid params");

        let internal = to_error_envelope("1".to_string(), FittingsError::internal("oops"));
        let internal_error = internal.error.expect("error");
        assert_eq!(internal_error.code, -32603);
        assert_eq!(internal_error.message, "Internal error");

        let transport = to_error_envelope("1".to_string(), FittingsError::transport("pipe"));
        let transport_error = transport.error.expect("error");
        assert_eq!(transport_error.code, -32603);
        assert_eq!(transport_error.message, "Internal error");
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
            data: Some(json!({"ignored": true})),
        });
        assert!(matches!(parse, FittingsError::ParseError(message) if message == "Parse error"));

        let invalid_request = from_error_envelope(ErrorEnvelope {
            code: -32600,
            message: "bad request".to_string(),
            data: None,
        });
        assert!(matches!(
            invalid_request,
            FittingsError::InvalidRequest(message) if message == "Invalid Request"
        ));

        let method_not_found = from_error_envelope(ErrorEnvelope {
            code: -32601,
            message: "missing".to_string(),
            data: None,
        });
        assert!(matches!(
            method_not_found,
            FittingsError::MethodNotFound(message) if message == "Method not found"
        ));

        let invalid_params = from_error_envelope(ErrorEnvelope {
            code: -32602,
            message: "bad params".to_string(),
            data: None,
        });
        assert!(matches!(
            invalid_params,
            FittingsError::InvalidParams(message) if message == "Invalid params"
        ));

        let internal = from_error_envelope(ErrorEnvelope {
            code: -32603,
            message: "internal".to_string(),
            data: None,
        });
        assert!(
            matches!(internal, FittingsError::Internal(message) if message == "Internal error")
        );

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
            FittingsError::Internal(message) if message == "Internal error"
        ));

        let unknown_positive = from_error_envelope(ErrorEnvelope {
            code: 1000,
            message: "unknown positive".to_string(),
            data: Some(json!({"ignored": true})),
        });
        assert!(matches!(
            unknown_positive,
            FittingsError::Internal(message) if message == "Internal error"
        ));
    }
}
