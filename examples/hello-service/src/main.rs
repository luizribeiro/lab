use std::process;

use clap::{error::ErrorKind, Parser};
use fittings::{accept_one, FittingsError, RouterService, RunOutcome, Server, SpawnRunner};
use hello_api::{
    hello_service_schema, into_hello_service_router, HelloParams, HelloResult, HelloService,
    PingParams, PingResult,
};
use tokio::net::TcpListener;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:7000";

#[derive(Debug, Parser)]
#[command(
    name = "hello-service",
    about = "Hello service. Normal mode runs a single-connection TCP server.",
    after_help = "Set FITTINGS=1 to use schema/serve stdio mode."
)]
struct NormalCli {
    #[arg(long = "bind", default_value = DEFAULT_BIND_ADDRESS)]
    bind_address: String,
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

async fn run_normal_mode(args: &[String]) -> Result<(), String> {
    let argv = std::iter::once("hello-service".to_string()).chain(args.iter().cloned());
    let cli = match NormalCli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            return Ok(());
        }
        Err(error) => return Err(error.to_string()),
    };

    let listener = TcpListener::bind(&cli.bind_address)
        .await
        .map_err(|error| format!("failed to bind {}: {error}", cli.bind_address))?;
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
    use clap::Parser;

    use super::{HelloServiceImpl, NormalCli, DEFAULT_BIND_ADDRESS};
    use fittings::{FittingsError, MethodRouter};
    use hello_api::{
        hello_service_schema, into_hello_service_router, HelloParams, HelloService, PingParams,
    };
    use serde_json::json;

    #[test]
    fn normal_cli_defaults_bind_address() {
        let cli = NormalCli::parse_from(["hello-service"]);
        assert_eq!(cli.bind_address, DEFAULT_BIND_ADDRESS);
    }

    #[test]
    fn normal_cli_accepts_bind_address() {
        let cli = NormalCli::parse_from(["hello-service", "--bind", "127.0.0.1:0"]);
        assert_eq!(cli.bind_address, "127.0.0.1:0");
    }

    #[test]
    fn normal_cli_rejects_unknown_flag() {
        let err = NormalCli::try_parse_from(["hello-service", "--unknown"]).expect_err("fail");
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
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
