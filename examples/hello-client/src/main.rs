use std::process;

use fittings::TcpConnector;
use hello_api::{HelloParams, HelloServiceClient};

const DEFAULT_SERVICE_ADDRESS: &str = "127.0.0.1:7000";
const SERVICE_ADDRESS_ENV: &str = "HELLO_SERVICE_ADDR";
const USAGE: &str = "Usage: hello-client [--addr <host:port>] [name]\n\
Defaults: addr=127.0.0.1:7000, name=world";

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    service_address: Option<String>,
    name: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ParseArgs {
    Run(Cli),
    Help,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<ParseArgs, String> {
    let mut service_address = None;
    let mut name = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("--addr requires a value\n{USAGE}"))?;
                if service_address.is_some() {
                    return Err(format!("--addr may only be provided once\n{USAGE}"));
                }
                service_address = Some(value);
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
        service_address,
        name: name.unwrap_or_else(|| "world".to_string()),
    }))
}

fn resolve_service_address(
    service_address_arg: Option<String>,
    service_address_env: Option<String>,
) -> String {
    service_address_arg
        .or(service_address_env)
        .unwrap_or_else(|| DEFAULT_SERVICE_ADDRESS.to_string())
}

fn resolve_service_address_from_lookup(
    service_address_arg: Option<String>,
    env_lookup: impl FnOnce(&str) -> Option<String>,
) -> String {
    resolve_service_address(service_address_arg, env_lookup(SERVICE_ADDRESS_ENV))
}

async fn request_hello_message(
    service_address: String,
    name: String,
) -> Result<String, fittings::FittingsError> {
    let client = HelloServiceClient::connect(TcpConnector::new(service_address)).await?;
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

    let service_address =
        resolve_service_address_from_lookup(cli.service_address, |key| std::env::var(key).ok());
    let message = request_hello_message(service_address, cli.name).await?;

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
    use fittings::{accept_one, FittingsError, RouterService, Server};
    use hello_api::{
        into_hello_service_router, HelloParams, HelloResult, HelloService, PingParams, PingResult,
    };
    use tokio::{
        net::TcpListener,
        time::{timeout, Duration},
    };

    use super::{
        parse_args, request_hello_message, resolve_service_address,
        resolve_service_address_from_lookup, Cli, ParseArgs, DEFAULT_SERVICE_ADDRESS,
        SERVICE_ADDRESS_ENV,
    };

    #[test]
    fn parse_args_accepts_addr_and_name() {
        let parsed = parse_args(vec![
            "--addr".to_string(),
            "127.0.0.1:9000".to_string(),
            "Ada".to_string(),
        ])
        .expect("args should parse");

        assert!(matches!(
            parsed,
            ParseArgs::Run(Cli {
                service_address: Some(address),
                name,
            }) if address == "127.0.0.1:9000" && name == "Ada"
        ));
    }

    #[test]
    fn parse_args_defaults_name_to_world() {
        let parsed = parse_args(Vec::<String>::new()).expect("args should parse");
        assert!(matches!(
            parsed,
            ParseArgs::Run(Cli {
                service_address: None,
                name,
            }) if name == "world"
        ));
    }

    #[test]
    fn parse_args_rejects_missing_addr_value() {
        let error = parse_args(vec!["--addr".to_string()]).expect_err("should fail");
        assert!(error.contains("requires a value"));
    }

    #[test]
    fn parse_args_rejects_duplicate_addr_flags() {
        let error = parse_args(vec![
            "--addr".to_string(),
            "127.0.0.1:1".to_string(),
            "--addr".to_string(),
            "127.0.0.1:2".to_string(),
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
    fn resolve_service_address_prefers_cli_arg_then_env_then_default() {
        let from_cli = resolve_service_address(
            Some("127.0.0.1:9000".to_string()),
            Some("127.0.0.1:9001".to_string()),
        );
        assert_eq!(from_cli, "127.0.0.1:9000");

        let from_env = resolve_service_address(None, Some("127.0.0.1:9001".to_string()));
        assert_eq!(from_env, "127.0.0.1:9001");

        let from_default = resolve_service_address(None, None);
        assert_eq!(from_default, DEFAULT_SERVICE_ADDRESS);
    }

    #[test]
    fn resolve_service_address_from_lookup_uses_expected_env_var_name() {
        let resolved = resolve_service_address_from_lookup(None, |key| {
            assert_eq!(key, SERVICE_ADDRESS_ENV);
            Some("127.0.0.1:9002".to_string())
        });

        assert_eq!(resolved, "127.0.0.1:9002");
    }

    struct TestHelloService;

    impl HelloService for TestHelloService {
        async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError> {
            Ok(HelloResult {
                message: format!("Hello, {}!", params.name),
            })
        }

        async fn ping(&self, _params: PingParams) -> Result<PingResult, FittingsError> {
            Ok(PingResult { ok: true })
        }
    }

    #[tokio::test]
    async fn request_hello_message_uses_tcp_connector_based_client_flow() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener
            .local_addr()
            .expect("read listener address")
            .to_string();

        let server_task = tokio::spawn(async move {
            let transport = accept_one(&listener, 1_048_576)
                .await
                .expect("accept connection");
            let service = RouterService::new(into_hello_service_router(TestHelloService));
            Server::new(service, transport)
                .serve()
                .await
                .expect("serve request");
        });

        let message = request_hello_message(address, "Ada".to_string())
            .await
            .expect("request should succeed");
        assert_eq!(message, "Hello, Ada!");

        timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task should finish in time")
            .expect("server task should finish");
    }

    #[tokio::test]
    async fn request_hello_message_returns_error_when_address_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let unreachable_address = listener
            .local_addr()
            .expect("read listener address")
            .to_string();
        drop(listener);

        let error = request_hello_message(unreachable_address, "Ada".to_string())
            .await
            .expect_err("request should fail");
        assert!(matches!(error, FittingsError::Transport(_)));
    }
}
