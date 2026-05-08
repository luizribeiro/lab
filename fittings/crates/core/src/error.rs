use serde_json::Value;
use thiserror::Error;

use crate::message::ServiceError;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum FittingsError {
    #[error("parse error: {message}")]
    Parse {
        message: String,
        data: Option<Value>,
    },
    #[error("invalid request: {message}")]
    InvalidRequest {
        message: String,
        data: Option<Value>,
    },
    #[error("method not found: {message}")]
    MethodNotFound {
        message: String,
        data: Option<Value>,
    },
    #[error("invalid params: {message}")]
    InvalidParams {
        message: String,
        data: Option<Value>,
    },
    #[error("service error: {0}")]
    Service(ServiceError),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("internal error: {message}")]
    Internal {
        message: String,
        data: Option<Value>,
    },
    #[error("handler panic: {message}")]
    Panic { message: String },
}

impl FittingsError {
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
            data: None,
        }
    }

    pub fn parse_error_with_data(message: impl Into<String>, data: Value) -> Self {
        Self::Parse {
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_request_with_data(message: impl Into<String>, data: Value) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::MethodNotFound {
            message: message.into(),
            data: None,
        }
    }

    pub fn method_not_found_with_data(message: impl Into<String>, data: Value) -> Self {
        Self::MethodNotFound {
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::InvalidParams {
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_params_with_data(message: impl Into<String>, data: Value) -> Self {
        Self::InvalidParams {
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn service(error: ServiceError) -> Self {
        Self::Service(error)
    }

    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            data: None,
        }
    }

    pub fn internal_with_data(message: impl Into<String>, data: Value) -> Self {
        Self::Internal {
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn panic(message: impl Into<String>) -> Self {
        Self::Panic {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::FittingsError;
    use crate::message::ServiceError;

    #[test]
    fn one_arg_constructors_default_data_to_none() {
        assert!(matches!(
            FittingsError::parse_error("bad json"),
            FittingsError::Parse { message, data: None } if message == "bad json"
        ));
        assert!(matches!(
            FittingsError::invalid_request("missing id"),
            FittingsError::InvalidRequest { message, data: None } if message == "missing id"
        ));
        assert!(matches!(
            FittingsError::method_not_found("unknown"),
            FittingsError::MethodNotFound { message, data: None } if message == "unknown"
        ));
        assert!(matches!(
            FittingsError::invalid_params("wrong type"),
            FittingsError::InvalidParams { message, data: None } if message == "wrong type"
        ));
        assert!(matches!(
            FittingsError::internal("oops"),
            FittingsError::Internal { message, data: None } if message == "oops"
        ));
        assert!(matches!(
            FittingsError::transport("broken pipe"),
            FittingsError::Transport(message) if message == "broken pipe"
        ));
    }

    #[test]
    fn panic_constructor_carries_message() {
        assert!(matches!(
            FittingsError::panic("kaboom"),
            FittingsError::Panic { message } if message == "kaboom"
        ));
    }

    #[test]
    fn service_constructor_wraps_inner_error() {
        let service_error = ServiceError {
            code: 42,
            message: "domain failure".to_string(),
            data: Some(json!({"context": "test"})),
        };

        assert!(matches!(
            FittingsError::service(service_error.clone()),
            FittingsError::Service(err) if err == service_error
        ));
    }
}
