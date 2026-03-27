use fittings_core::{
    error::FittingsError,
    message::{Metadata, ServiceError},
};

use crate::types::{ErrorEnvelope, ResponseEnvelope};

const PARSE_ERROR_CODE: i32 = -32700;
const INVALID_REQUEST_CODE: i32 = -32600;
const METHOD_NOT_FOUND_CODE: i32 = -32601;
const INVALID_PARAMS_CODE: i32 = -32602;
const INTERNAL_ERROR_CODE: i32 = -32603;

pub fn to_error_envelope(id: String, err: FittingsError) -> ResponseEnvelope {
    let error = match err {
        FittingsError::ParseError(message) => ErrorEnvelope {
            code: PARSE_ERROR_CODE,
            message,
            data: None,
        },
        FittingsError::InvalidRequest(message) => ErrorEnvelope {
            code: INVALID_REQUEST_CODE,
            message,
            data: None,
        },
        FittingsError::MethodNotFound(message) => ErrorEnvelope {
            code: METHOD_NOT_FOUND_CODE,
            message,
            data: None,
        },
        FittingsError::InvalidParams(message) => ErrorEnvelope {
            code: INVALID_PARAMS_CODE,
            message,
            data: None,
        },
        FittingsError::Transport(message) | FittingsError::Internal(message) => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message,
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
        FittingsError::Service(ServiceError { message, .. }) => ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message,
            data: None,
        },
    };

    ResponseEnvelope::error(id, error, Metadata::default())
}

pub fn from_error_envelope(error: ErrorEnvelope) -> FittingsError {
    match error {
        ErrorEnvelope {
            code: PARSE_ERROR_CODE,
            message,
            ..
        } => FittingsError::parse_error(message),
        ErrorEnvelope {
            code: INVALID_REQUEST_CODE,
            message,
            ..
        } => FittingsError::invalid_request(message),
        ErrorEnvelope {
            code: METHOD_NOT_FOUND_CODE,
            message,
            ..
        } => FittingsError::method_not_found(message),
        ErrorEnvelope {
            code: INVALID_PARAMS_CODE,
            message,
            ..
        } => FittingsError::invalid_params(message),
        ErrorEnvelope {
            code: INTERNAL_ERROR_CODE,
            message,
            ..
        } => FittingsError::internal(message),
        ErrorEnvelope {
            code,
            message,
            data,
        } if ServiceError::is_valid_code_value(code) => FittingsError::service(ServiceError {
            code,
            message,
            data,
        }),
        ErrorEnvelope { message, .. } => FittingsError::internal(message),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use fittings_core::{error::FittingsError, message::ServiceError};

    use super::{from_error_envelope, to_error_envelope};
    use crate::types::ErrorEnvelope;

    #[test]
    fn maps_each_error_family_to_expected_code() {
        let parse = to_error_envelope("1".to_string(), FittingsError::parse_error("bad json"));
        assert_eq!(parse.error.expect("error").code, -32700);

        let invalid = to_error_envelope(
            "1".to_string(),
            FittingsError::invalid_request("bad request"),
        );
        assert_eq!(invalid.error.expect("error").code, -32600);

        let not_found =
            to_error_envelope("1".to_string(), FittingsError::method_not_found("missing"));
        assert_eq!(not_found.error.expect("error").code, -32601);

        let bad_params =
            to_error_envelope("1".to_string(), FittingsError::invalid_params("bad params"));
        assert_eq!(bad_params.error.expect("error").code, -32602);

        let internal = to_error_envelope("1".to_string(), FittingsError::internal("oops"));
        assert_eq!(internal.error.expect("error").code, -32603);

        let transport = to_error_envelope("1".to_string(), FittingsError::transport("pipe"));
        assert_eq!(transport.error.expect("error").code, -32603);
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
        assert_eq!(error.message, "domain error");
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
        assert!(matches!(parse, FittingsError::ParseError(message) if message == "bad json"));

        let invalid_request = from_error_envelope(ErrorEnvelope {
            code: -32600,
            message: "bad request".to_string(),
            data: None,
        });
        assert!(matches!(
            invalid_request,
            FittingsError::InvalidRequest(message) if message == "bad request"
        ));

        let method_not_found = from_error_envelope(ErrorEnvelope {
            code: -32601,
            message: "missing".to_string(),
            data: None,
        });
        assert!(matches!(
            method_not_found,
            FittingsError::MethodNotFound(message) if message == "missing"
        ));

        let invalid_params = from_error_envelope(ErrorEnvelope {
            code: -32602,
            message: "bad params".to_string(),
            data: None,
        });
        assert!(matches!(
            invalid_params,
            FittingsError::InvalidParams(message) if message == "bad params"
        ));

        let internal = from_error_envelope(ErrorEnvelope {
            code: -32603,
            message: "internal".to_string(),
            data: None,
        });
        assert!(matches!(internal, FittingsError::Internal(message) if message == "internal"));

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
            FittingsError::Internal(message) if message == "unknown negative"
        ));

        let unknown_positive = from_error_envelope(ErrorEnvelope {
            code: 1000,
            message: "unknown positive".to_string(),
            data: Some(json!({"ignored": true})),
        });
        assert!(matches!(
            unknown_positive,
            FittingsError::Internal(message) if message == "unknown positive"
        ));
    }
}
