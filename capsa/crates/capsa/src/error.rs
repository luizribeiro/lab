use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    InvalidHostPattern { pattern: String, reason: String },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHostPattern { pattern, reason } => {
                write!(f, "invalid host pattern '{pattern}': {reason}")
            }
        }
    }
}

impl std::error::Error for BuildError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_host_pattern_display_includes_pattern_and_reason() {
        let err = BuildError::InvalidHostPattern {
            pattern: "*example.com".into(),
            reason: "wildcard host pattern must use only a leading '*.' prefix".into(),
        };

        let msg = err.to_string();
        assert!(
            msg.contains("*example.com"),
            "message missing pattern: {msg}"
        );
        assert!(msg.contains("wildcard"), "message missing reason: {msg}");
    }

    #[test]
    fn build_error_implements_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}

        let err = BuildError::InvalidHostPattern {
            pattern: "x".into(),
            reason: "y".into(),
        };
        assert_error(&err);
    }
}
