use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum McpfitError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("method not found: {0}")]
    MethodNotFound(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("cancelled")]
    Cancelled,
    #[error("internal error: {0}")]
    Internal(String),
}

impl McpfitError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::MethodNotFound(message.into())
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::InvalidParams(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::McpfitError;

    #[test]
    fn constructors_create_expected_variants() {
        assert!(matches!(
            McpfitError::invalid_request("a"),
            McpfitError::InvalidRequest(m) if m == "a"
        ));
        assert!(matches!(
            McpfitError::method_not_found("b"),
            McpfitError::MethodNotFound(m) if m == "b"
        ));
        assert!(matches!(
            McpfitError::invalid_params("c"),
            McpfitError::InvalidParams(m) if m == "c"
        ));
        assert!(matches!(
            McpfitError::internal("d"),
            McpfitError::Internal(m) if m == "d"
        ));
    }

    #[test]
    fn cancelled_display_matches_spec() {
        assert_eq!(McpfitError::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn display_is_non_empty_for_message_variants() {
        assert!(!McpfitError::invalid_params("x").to_string().is_empty());
    }
}
