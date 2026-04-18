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

#[derive(Debug)]
pub struct StartError {
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl StartError {
    pub(crate) fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to start VM: {}", self.source)
    }
}

impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Debug)]
pub struct RuntimeError {
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl RuntimeError {
    pub(crate) fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VM runtime error: {}", self.source)
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

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

    #[test]
    fn start_error_preserves_source() {
        use std::error::Error;

        let cause = std::io::Error::new(std::io::ErrorKind::NotFound, "binary missing");
        let err = StartError::new(cause);

        let msg = err.to_string();
        assert!(msg.contains("binary missing"), "unexpected: {msg}");
        assert!(err.source().is_some(), "source should be set");
    }

    #[test]
    fn runtime_error_preserves_source() {
        use std::error::Error;

        let cause = std::io::Error::other("reaper bailed");
        let err = RuntimeError::new(cause);

        let msg = err.to_string();
        assert!(msg.contains("reaper bailed"), "unexpected: {msg}");
        assert!(err.source().is_some(), "source should be set");
    }
}
