use std::process::ExitStatus;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to spawn agent process")]
    Spawn(#[source] std::io::Error),
    #[error("agent exited with {status}: {stderr}")]
    Exit { status: ExitStatus, stderr: String },
    #[error("failed to parse JSON line: {line}")]
    Parse {
        line: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("driver {driver} could not parse event: {source}")]
    DriverParse {
        driver: &'static str,
        value: serde_json::Value,
        #[source]
        source: ParseError,
    },
    #[error("turn cancelled")]
    Cancelled,
    #[error("turn timed out after {0:?}")]
    Timeout(Duration),
    #[error("I/O error")]
    Io(#[source] std::io::Error),
    /// A `Session::send` call was rejected because a previous turn is still
    /// in flight on the same session.
    #[error("session is already executing a turn")]
    Busy,
    /// A configuration option was set on a driver that does not support it.
    #[error("driver {driver}: option {option} is not supported by this CLI")]
    UnsupportedOption {
        driver: &'static str,
        option: &'static str,
    },
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid type for field {field}: expected {expected}")]
    InvalidFieldType {
        field: &'static str,
        expected: &'static str,
    },
    #[error("{0}")]
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;
    use std::io;

    #[test]
    fn error_variants_display() {
        let cases: Vec<Error> = vec![
            Error::Spawn(io::Error::other("x")),
            Error::Exit {
                status: exit_status(),
                stderr: "boom".into(),
            },
            Error::Parse {
                line: "{".into(),
                source: serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
            },
            Error::DriverParse {
                driver: "claude",
                value: serde_json::json!({}),
                source: ParseError::Custom("nope".into()),
            },
            Error::Cancelled,
            Error::Timeout(Duration::from_secs(1)),
            Error::Io(io::Error::other("x")),
        ];
        let prefixes = [
            "failed to spawn agent process",
            "agent exited with",
            "failed to parse JSON line",
            "driver claude could not parse event",
            "turn cancelled",
            "turn timed out after",
            "I/O error",
        ];
        for (e, prefix) in cases.iter().zip(prefixes.iter()) {
            assert!(format!("{e}").contains(prefix), "{e}");
        }
    }

    #[test]
    fn unsupported_option_displays() {
        let e = Error::UnsupportedOption {
            driver: "x",
            option: "foo",
        };
        assert!(format!("{e}").contains("driver x"));
        assert!(format!("{e}").contains("option foo"));
    }

    #[test]
    fn busy_variant_displays() {
        let e = Error::Busy;
        assert!(format!("{e}").contains("already executing"));
    }

    #[test]
    fn parse_error_variants_display() {
        assert!(format!("{}", ParseError::MissingField("id")).contains("missing field: id"));
        assert!(
            format!(
                "{}",
                ParseError::InvalidFieldType {
                    field: "kind",
                    expected: "string"
                }
            )
            .contains("invalid type for field kind: expected string")
        );
        assert!(format!("{}", ParseError::Custom("oops".into())).contains("oops"));
    }

    #[test]
    fn parse_preserves_source() {
        let e = Error::Parse {
            line: "{".into(),
            source: serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
        };
        assert!(e.source().is_some());
    }

    #[test]
    fn driver_parse_preserves_typed_source() {
        let e = Error::DriverParse {
            driver: "test",
            value: serde_json::json!({"x": 1}),
            source: ParseError::MissingField("y"),
        };
        let src = e.source().expect("source set");
        assert!(src.downcast_ref::<ParseError>().is_some());
    }

    #[test]
    fn spawn_preserves_source() {
        let e = Error::Spawn(io::Error::other("x"));
        assert!(e.source().is_some());
    }

    fn exit_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
        #[cfg(not(unix))]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
    }
}
