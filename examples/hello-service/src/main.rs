use std::process;

use fittings::{FittingsError, RouterService, RunOutcome, SpawnRunner};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HelloParams {
    name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct HelloResult {
    message: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PingParams {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PingResult {
    ok: bool,
}

#[fittings::service]
trait HelloService {
    /// Greets the provided name
    async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError>;

    /// Health check
    async fn ping(&self, params: PingParams) -> Result<PingResult, FittingsError>;
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

fn run_normal_cli(args: &[String]) {
    let name = args.first().cloned().unwrap_or_else(|| "world".to_string());
    println!("Hello, {name}!");
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
        RunOutcome::Normal => run_normal_cli(&args),
        RunOutcome::Exit(code) => process::exit(code),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        hello_service_schema, into_hello_service_router, HelloParams, HelloService,
        HelloServiceImpl, PingParams,
    };
    use fittings::{FittingsError, MethodRouter};
    use serde_json::json;

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
