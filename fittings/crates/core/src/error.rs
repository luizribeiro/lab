use thiserror::Error;

use crate::message::ServiceError;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum FittingsError {
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("method not found: {0}")]
    MethodNotFound(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("service error: {0}")]
    Service(ServiceError),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl FittingsError {
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError(message.into())
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::MethodNotFound(message.into())
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::InvalidParams(message.into())
    }

    pub fn service(error: ServiceError) -> Self {
        Self::Service(error)
    }

    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::FittingsError;
    use crate::message::ServiceError;

    #[test]
    fn error_constructors_create_expected_variants() {
        assert!(matches!(
            FittingsError::parse_error("bad json"),
            FittingsError::ParseError(message) if message == "bad json"
        ));
        assert!(matches!(
            FittingsError::invalid_request("missing id"),
            FittingsError::InvalidRequest(message) if message == "missing id"
        ));
        assert!(matches!(
            FittingsError::method_not_found("unknown"),
            FittingsError::MethodNotFound(message) if message == "unknown"
        ));
        assert!(matches!(
            FittingsError::invalid_params("wrong type"),
            FittingsError::InvalidParams(message) if message == "wrong type"
        ));
        assert!(matches!(
            FittingsError::transport("broken pipe"),
            FittingsError::Transport(message) if message == "broken pipe"
        ));
        assert!(matches!(
            FittingsError::internal("panic"),
            FittingsError::Internal(message) if message == "panic"
        ));

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
