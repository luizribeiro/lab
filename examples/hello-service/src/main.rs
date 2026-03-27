use std::process;

use fittings::{accept_one, FittingsError, RouterService, RunOutcome, Server, SpawnRunner};
use hello_api::{
    hello_service_schema, into_hello_service_router, HelloParams, HelloResult, HelloService,
    PingParams, PingResult,
};
use tokio::net::TcpListener;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:7000";
const NORMAL_USAGE: &str = "Usage: hello-service [--bind <addr>]\n\
Runs a single-connection TCP server in normal mode.\n\
Default bind address: 127.0.0.1:7000\n\
Set FITTINGS=1 to use schema/serve stdio mode.";

#[derive(Debug, PartialEq, Eq)]
enum NormalArgs {
    Run { bind_address: String },
    Help,
}

struct HelloServiceImpl;

impl HelloService for HelloServiceImpl {
    async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError> {
        Ok(HelloResult {
            message: format!("Hello, {}!", params.name),
        })
    }

    async fn ping(&self, _params: PingParams) -> Result<PingResult, FittingsError> {
        Ok(PingResult { ok: true })
    }
}

fn parse_normal_args(args: &[String]) -> Result<NormalArgs, String> {
    let mut bind_address = DEFAULT_BIND_ADDRESS.to_string();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--bind" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("--bind requires an address\n{NORMAL_USAGE}"))?;
                bind_address = value.clone();
            }
            "-h" | "--help" => return Ok(NormalArgs::Help),
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag: {flag}\n{NORMAL_USAGE}"));
            }
            value => {
                return Err(format!("unexpected argument: {value}\n{NORMAL_USAGE}"));
            }
        }
    }

    Ok(NormalArgs::Run { bind_address })
}

async fn run_normal_mode(args: &[String]) -> Result<(), String> {
    let parsed = parse_normal_args(args)?;
    let bind_address = match parsed {
        NormalArgs::Help => {
            println!("{NORMAL_USAGE}");
            return Ok(());
        }
        NormalArgs::Run { bind_address } => bind_address,
    };

    let listener = TcpListener::bind(&bind_address)
        .await
        .map_err(|error| format!("failed to bind {bind_address}: {error}"))?;
    println!(
        "hello-service listening on {} (single connection)",
        listener
            .local_addr()
            .map_err(|error| format!("failed to read local address: {error}"))?
    );

    let transport = accept_one(&listener, 1_048_576)
        .await
        .map_err(|error| format!("failed to accept connection: {error}"))?;
    let service = RouterService::new(into_hello_service_router(HelloServiceImpl));
    let server = Server::new(service, transport);

    server
        .serve()
        .await
        .map_err(|error| format!("server failed: {error}"))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env_fittings = std::env::var("FITTINGS").ok();

    let runner = SpawnRunner::new(hello_service_schema());
    let outcome = runner
        .run_with_stdio_service(env_fittings.as_deref(), &args, |_config| {
            RouterService::new(into_hello_service_router(HelloServiceImpl))
        })
        .await;

    match outcome {
        RunOutcome::Normal => {
            if let Err(error) = run_normal_mode(&args).await {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        RunOutcome::Exit(code) => process::exit(code),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_normal_args, HelloServiceImpl, NormalArgs, DEFAULT_BIND_ADDRESS, NORMAL_USAGE,
    };
    use fittings::{FittingsError, MethodRouter};
    use hello_api::{
        hello_service_schema, into_hello_service_router, HelloParams, HelloService, PingParams,
    };
    use serde_json::json;

    #[test]
    fn parse_normal_args_defaults_bind_address() {
        let parsed = parse_normal_args(&[]).expect("args should parse");
        assert!(matches!(
            parsed,
            NormalArgs::Run { bind_address } if bind_address == DEFAULT_BIND_ADDRESS
        ));
    }

    #[test]
    fn parse_normal_args_accepts_bind_address() {
        let parsed = parse_normal_args(&["--bind".to_string(), "127.0.0.1:0".to_string()])
            .expect("args should parse");

        assert!(matches!(
            parsed,
            NormalArgs::Run { bind_address } if bind_address == "127.0.0.1:0"
        ));
    }

    #[test]
    fn parse_normal_args_supports_help() {
        let parsed = parse_normal_args(&["--help".to_string()]).expect("help should parse");
        assert!(matches!(parsed, NormalArgs::Help));
    }

    #[test]
    fn parse_normal_args_rejects_missing_bind_value_with_usage() {
        let error = parse_normal_args(&["--bind".to_string()]).expect_err("should fail");
        assert!(error.contains("--bind requires an address"));
        assert!(error.contains(NORMAL_USAGE));
    }

    #[test]
    fn parse_normal_args_rejects_unknown_flag_with_usage() {
        let error = parse_normal_args(&["--unknown".to_string()]).expect_err("should fail");
        assert!(error.contains("unknown flag"));
        assert!(error.contains(NORMAL_USAGE));
    }

    #[test]
    fn parse_normal_args_rejects_unexpected_positional_argument() {
        let error = parse_normal_args(&["Ada".to_string()]).expect_err("should fail");
        assert!(error.contains("unexpected argument"));
        assert!(error.contains(NORMAL_USAGE));
    }

    #[tokio::test]
    async fn hello_handler_formats_message() {
        let service = HelloServiceImpl;
        let result = service
            .hello(HelloParams {
                name: "Ada".to_string(),
            })
            .await
            .expect("hello should succeed");

        assert_eq!(result.message, "Hello, Ada!");
    }

    #[tokio::test]
    async fn generated_router_maps_invalid_hello_params_to_invalid_params() {
        let router = into_hello_service_router(HelloServiceImpl);

        let missing_name = router
            .route("hello", json!({}), fittings::Metadata::default())
            .await
            .expect_err("missing name should fail");
        assert!(matches!(missing_name, FittingsError::InvalidParams(_)));

        let wrong_type = router
            .route("hello", json!({"name": 7}), fittings::Metadata::default())
            .await
            .expect_err("wrong name type should fail");
        assert!(matches!(wrong_type, FittingsError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn ping_handler_requires_empty_object() {
        let service = HelloServiceImpl;
        let result = service
            .ping(PingParams {})
            .await
            .expect("ping should succeed");
        assert!(result.ok);

        let router = into_hello_service_router(HelloServiceImpl);
        let ok = router
            .route("ping", json!({}), fittings::Metadata::default())
            .await
            .expect("empty params should succeed");
        assert_eq!(ok, json!({"ok": true}));

        let bad = router
            .route("ping", json!({"x": 1}), fittings::Metadata::default())
            .await
            .expect_err("extra params should fail");
        assert!(matches!(bad, FittingsError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn router_maps_unknown_method_to_method_not_found() {
        let router = into_hello_service_router(HelloServiceImpl);
        let error = router
            .route("unknown", json!({}), fittings::Metadata::default())
            .await
            .expect_err("unknown method should fail");

        assert!(matches!(
            error,
            FittingsError::MethodNotFound(message) if message == "unknown"
        ));
    }

    #[test]
    fn schema_includes_methods_and_log_level_config() {
        let schema = hello_service_schema();
        assert_eq!(schema.name, "hello-service");
        assert_eq!(schema.methods.len(), 2);
        assert_eq!(schema.methods[0].name, "hello");
        assert_eq!(
            schema.methods[0].description.as_deref(),
            Some("Greets the provided name")
        );
        assert_eq!(schema.methods[1].name, "ping");
        assert_eq!(
            schema.methods[1].description.as_deref(),
            Some("Health check")
        );

        let config_schema = schema.config_schema.expect("config schema should exist");
        assert_eq!(config_schema["properties"]["log_level"]["type"], "string");
        assert_eq!(
            config_schema["properties"]["log_level"]["enum"],
            json!(["trace", "debug", "info", "warn", "error"])
        );
        assert_eq!(config_schema["additionalProperties"], false);
    }
}
