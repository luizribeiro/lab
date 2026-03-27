use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnMode {
    Normal,
    Schema,
    Serve { config_json: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpawnModeError {
    #[error("unsupported FITTINGS protocol version `{0}`; expected `1`")]
    UnsupportedVersion(String),
    #[error("{0}")]
    Usage(String),
}

pub fn detect_mode(
    env_fittings: Option<&str>,
    args: &[String],
) -> Result<SpawnMode, SpawnModeError> {
    match env_fittings {
        None | Some("") => return Ok(SpawnMode::Normal),
        Some("1") => {}
        Some(other) => return Err(SpawnModeError::UnsupportedVersion(other.to_string())),
    }

    let Some(command) = args.first() else {
        return Err(SpawnModeError::Usage(usage("missing command")));
    };

    match command.as_str() {
        "schema" => {
            if args.len() != 1 {
                return Err(SpawnModeError::Usage(usage(
                    "`schema` does not accept extra arguments",
                )));
            }
            Ok(SpawnMode::Schema)
        }
        "serve" => {
            if args.len() > 2 {
                return Err(SpawnModeError::Usage(usage(
                    "`serve` accepts at most one config JSON argument",
                )));
            }
            Ok(SpawnMode::Serve {
                config_json: args.get(1).cloned(),
            })
        }
        _ => Err(SpawnModeError::Usage(usage("unknown command"))),
    }
}

fn usage(reason: &str) -> String {
    format!("{reason}. Usage: FITTINGS=1 <bin> schema | FITTINGS=1 <bin> serve [<config-json>]")
}

#[cfg(test)]
mod tests {
    use super::{detect_mode, SpawnMode, SpawnModeError};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn mode_matrix_covers_normal_unsupported_and_valid_spawn_modes() {
        assert_eq!(detect_mode(None, &[]).expect("mode"), SpawnMode::Normal);
        assert_eq!(detect_mode(Some(""), &[]).expect("mode"), SpawnMode::Normal);

        let unsupported = detect_mode(Some("2"), &[]).expect_err("must fail");
        assert!(matches!(
            unsupported,
            SpawnModeError::UnsupportedVersion(version) if version == "2"
        ));

        assert_eq!(
            detect_mode(Some("1"), &args(&["schema"])).expect("schema mode should be detected"),
            SpawnMode::Schema
        );

        assert_eq!(
            detect_mode(Some("1"), &args(&["serve"])).expect("serve mode should be detected"),
            SpawnMode::Serve { config_json: None }
        );

        assert_eq!(
            detect_mode(Some("1"), &args(&["serve", "{\"log_level\":\"debug\"}"]))
                .expect("serve mode with config should be detected"),
            SpawnMode::Serve {
                config_json: Some("{\"log_level\":\"debug\"}".to_string())
            }
        );
    }

    #[test]
    fn arity_and_unknown_command_errors_are_usage_failures() {
        let missing = detect_mode(Some("1"), &[]).expect_err("must fail");
        assert!(
            matches!(missing, SpawnModeError::Usage(message) if message.contains("missing command"))
        );

        let schema_extra =
            detect_mode(Some("1"), &args(&["schema", "extra"])).expect_err("must fail");
        assert!(
            matches!(schema_extra, SpawnModeError::Usage(message) if message.contains("schema"))
        );

        let serve_extra =
            detect_mode(Some("1"), &args(&["serve", "{}", "extra"])).expect_err("must fail");
        assert!(
            matches!(serve_extra, SpawnModeError::Usage(message) if message.contains("at most one"))
        );

        let unknown = detect_mode(Some("1"), &args(&["wat"])).expect_err("must fail");
        assert!(
            matches!(unknown, SpawnModeError::Usage(message) if message.contains("unknown command"))
        );
    }
}
