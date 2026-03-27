use std::process;

use fittings::ProcessConnector;
use hello_api::{HelloParams, HelloServiceClient};

const DEFAULT_SERVICE_BIN: &str = "hello-service";
const USAGE: &str = "Usage: hello-client [--service-bin <path>] [name]";

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    service_bin: Option<String>,
    name: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ParseArgs {
    Run(Cli),
    Help,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<ParseArgs, String> {
    let mut service_bin = None;
    let mut name = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--service-bin" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--service-bin requires a value".to_string())?;
                if service_bin.is_some() {
                    return Err(format!("--service-bin may only be provided once\n{USAGE}"));
                }
                service_bin = Some(value);
            }
            "-h" | "--help" => return Ok(ParseArgs::Help),
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag: {flag}\n{USAGE}"));
            }
            value => {
                if name.is_some() {
                    return Err(format!("unexpected argument: {value}\n{USAGE}"));
                }
                name = Some(value.to_string());
            }
        }
    }

    Ok(ParseArgs::Run(Cli {
        service_bin,
        name: name.unwrap_or_else(|| "world".to_string()),
    }))
}

fn resolve_service_bin(service_bin_arg: Option<String>, service_bin_env: Option<String>) -> String {
    service_bin_arg
        .or(service_bin_env)
        .unwrap_or_else(|| DEFAULT_SERVICE_BIN.to_string())
}

async fn request_hello_message(
    service_bin: String,
    name: String,
) -> Result<String, fittings::FittingsError> {
    let client = HelloServiceClient::connect(ProcessConnector::new(&service_bin)).await?;
    let result = client.hello(HelloParams { name }).await?;
    Ok(result.message)
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(ParseArgs::Run(cli)) => cli,
        Ok(ParseArgs::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(message) => {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into());
        }
    };

    let service_bin = resolve_service_bin(cli.service_bin, std::env::var("HELLO_SERVICE_BIN").ok());
    let message = request_hello_message(service_bin, cli.name).await?;

    println!("{message}");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        parse_args, request_hello_message, resolve_service_bin, Cli, ParseArgs, DEFAULT_SERVICE_BIN,
    };

    #[test]
    fn parse_args_accepts_service_bin_and_name() {
        let parsed = parse_args(vec![
            "--service-bin".to_string(),
            "/tmp/hello-service".to_string(),
            "Ada".to_string(),
        ])
        .expect("args should parse");

        assert!(matches!(
            parsed,
            ParseArgs::Run(Cli {
                service_bin: Some(path),
                name,
            }) if path == "/tmp/hello-service" && name == "Ada"
        ));
    }

    #[test]
    fn parse_args_defaults_name_to_world() {
        let parsed = parse_args(Vec::<String>::new()).expect("args should parse");
        assert!(matches!(
            parsed,
            ParseArgs::Run(Cli {
                service_bin: None,
                name,
            }) if name == "world"
        ));
    }

    #[test]
    fn parse_args_rejects_missing_service_bin_value() {
        let error = parse_args(vec!["--service-bin".to_string()]).expect_err("should fail");
        assert!(error.contains("requires a value"));
    }

    #[test]
    fn parse_args_rejects_duplicate_service_bin_flags() {
        let error = parse_args(vec![
            "--service-bin".to_string(),
            "a".to_string(),
            "--service-bin".to_string(),
            "b".to_string(),
        ])
        .expect_err("should fail");
        assert!(error.contains("only be provided once"));
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let error = parse_args(vec!["--unknown".to_string()]).expect_err("should fail");
        assert!(error.contains("unknown flag"));
    }

    #[test]
    fn parse_args_rejects_unexpected_second_positional_argument() {
        let error =
            parse_args(vec!["Ada".to_string(), "Grace".to_string()]).expect_err("should fail");
        assert!(error.contains("unexpected argument"));
    }

    #[test]
    fn parse_args_supports_help() {
        let long = parse_args(vec!["--help".to_string()]).expect("help should parse");
        assert!(matches!(long, ParseArgs::Help));

        let short = parse_args(vec!["-h".to_string()]).expect("help should parse");
        assert!(matches!(short, ParseArgs::Help));
    }

    #[test]
    fn resolve_service_bin_prefers_cli_arg_then_env_then_default() {
        let from_cli = resolve_service_bin(
            Some("/tmp/from-cli".to_string()),
            Some("/tmp/from-env".to_string()),
        );
        assert_eq!(from_cli, "/tmp/from-cli");

        let from_env = resolve_service_bin(None, Some("/tmp/from-env".to_string()));
        assert_eq!(from_env, "/tmp/from-env");

        let from_default = resolve_service_bin(None, None);
        assert_eq!(from_default, DEFAULT_SERVICE_BIN);
    }

    #[cfg(unix)]
    fn unique_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "hello-client-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    fn write_executable_script(path: &Path, content: &str) {
        fs::write(path, content).expect("write script fixture");
        let mut perms = fs::metadata(path)
            .expect("read script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set executable permissions");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn request_hello_message_uses_connector_based_client_flow() {
        let script_path = unique_path("echo-server.py");
        write_executable_script(
            &script_path,
            r#"#!/usr/bin/env python3
import json
import sys

line = sys.stdin.readline()
req = json.loads(line)
name = req["params"]["name"]
resp = {"id": req["id"], "result": {"message": f"Hello, {name}!"}, "metadata": {}}
sys.stdout.write(json.dumps(resp) + "\n")
sys.stdout.flush()
"#,
        );

        let message = request_hello_message(
            script_path.to_string_lossy().into_owned(),
            "Ada".to_string(),
        )
        .await
        .expect("request should succeed");
        assert_eq!(message, "Hello, Ada!");

        fs::remove_file(script_path).expect("remove script fixture");
    }
}
