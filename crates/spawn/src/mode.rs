use thiserror::Error;

const DEFAULT_TCP_ADDRESS: &str = "127.0.0.1:7000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeTransport {
    Stdio,
    Tcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeOptions {
    pub transport: ServeTransport,
    pub address: String,
    pub once: bool,
    pub config_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnMode {
    Schema,
    Serve(ServeOptions),
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
        None | Some("") | Some("1") => {}
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
        "serve" => parse_serve_args(&args[1..]).map(SpawnMode::Serve),
        _ => Err(SpawnModeError::Usage(usage("unknown command"))),
    }
}

fn parse_serve_args(args: &[String]) -> Result<ServeOptions, SpawnModeError> {
    let mut transport = ServeTransport::Stdio;
    let mut address = DEFAULT_TCP_ADDRESS.to_string();
    let mut config_json: Option<String> = None;
    let mut explicit_address = false;
    let mut once = false;

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--transport" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(SpawnModeError::Usage(usage(
                        "`--transport` requires a value",
                    )));
                };

                transport = match value.as_str() {
                    "stdio" => ServeTransport::Stdio,
                    "tcp" => ServeTransport::Tcp,
                    _ => {
                        return Err(SpawnModeError::Usage(usage(
                            "`--transport` must be `stdio` or `tcp`",
                        )));
                    }
                };
            }
            "--addr" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(SpawnModeError::Usage(usage("`--addr` requires a value")));
                };
                explicit_address = true;
                address = value.clone();
            }
            "--config" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(SpawnModeError::Usage(usage("`--config` requires a value")));
                };
                if config_json.is_some() {
                    return Err(SpawnModeError::Usage(usage(
                        "config may be provided only once",
                    )));
                }
                config_json = Some(value.clone());
            }
            "--once" => {
                once = true;
            }
            other if other.starts_with('-') => {
                return Err(SpawnModeError::Usage(usage(
                    format!("unknown option `{other}`").as_str(),
                )));
            }
            value => {
                if config_json.is_some() {
                    return Err(SpawnModeError::Usage(usage(
                        "`serve` accepts at most one positional config JSON argument",
                    )));
                }
                config_json = Some(value.to_string());
            }
        }
        index += 1;
    }

    if explicit_address && !matches!(transport, ServeTransport::Tcp) {
        return Err(SpawnModeError::Usage(usage(
            "`--addr` is only valid with `--transport tcp`",
        )));
    }

    if once && !matches!(transport, ServeTransport::Tcp) {
        return Err(SpawnModeError::Usage(usage(
            "`--once` is only valid with `--transport tcp`",
        )));
    }

    Ok(ServeOptions {
        transport,
        address,
        once,
        config_json,
    })
}

fn usage(reason: &str) -> String {
    format!(
        "{reason}. Usage: <bin> schema | <bin> serve [--transport stdio|tcp] [--addr <host:port>] [--once] [--config <json>]"
    )
}

#[cfg(test)]
mod tests {
    use super::{detect_mode, ServeOptions, ServeTransport, SpawnMode, SpawnModeError};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn mode_matrix_covers_schema_and_serve_without_requiring_fittings_env() {
        assert_eq!(
            detect_mode(None, &args(&["schema"])).expect("schema mode should be detected"),
            SpawnMode::Schema
        );

        assert_eq!(
            detect_mode(Some(""), &args(&["serve"])).expect("serve mode should be detected"),
            SpawnMode::Serve(ServeOptions {
                transport: ServeTransport::Stdio,
                address: "127.0.0.1:7000".to_string(),
                once: false,
                config_json: None,
            })
        );

        assert_eq!(
            detect_mode(Some("1"), &args(&["serve", "{\"log_level\":\"debug\"}"]))
                .expect("serve mode with positional config should be detected"),
            SpawnMode::Serve(ServeOptions {
                transport: ServeTransport::Stdio,
                address: "127.0.0.1:7000".to_string(),
                once: false,
                config_json: Some("{\"log_level\":\"debug\"}".to_string()),
            })
        );

        assert_eq!(
            detect_mode(
                Some("1"),
                &args(&[
                    "serve",
                    "--transport",
                    "tcp",
                    "--addr",
                    "127.0.0.1:8123",
                    "--config",
                    "{\"log_level\":\"info\"}"
                ])
            )
            .expect("tcp serve options should parse"),
            SpawnMode::Serve(ServeOptions {
                transport: ServeTransport::Tcp,
                address: "127.0.0.1:8123".to_string(),
                once: false,
                config_json: Some("{\"log_level\":\"info\"}".to_string()),
            })
        );
    }

    #[test]
    fn unsupported_fittings_version_fails() {
        let unsupported = detect_mode(Some("2"), &args(&["schema"])).expect_err("must fail");
        assert!(matches!(
            unsupported,
            SpawnModeError::UnsupportedVersion(version) if version == "2"
        ));
    }

    #[test]
    fn invalid_inputs_are_usage_failures() {
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
            matches!(serve_extra, SpawnModeError::Usage(message) if message.contains("at most one positional config"))
        );

        let addr_without_tcp = detect_mode(Some("1"), &args(&["serve", "--addr", "127.0.0.1:1"]))
            .expect_err("must fail");
        assert!(
            matches!(addr_without_tcp, SpawnModeError::Usage(message) if message.contains("only valid with"))
        );

        let once_without_tcp =
            detect_mode(Some("1"), &args(&["serve", "--once"])).expect_err("must fail");
        assert!(
            matches!(once_without_tcp, SpawnModeError::Usage(message) if message.contains("only valid with"))
        );
    }
}
