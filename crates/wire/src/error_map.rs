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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use fittings_core::{error::FittingsError, message::ServiceError};

    use super::to_error_envelope;

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
}
