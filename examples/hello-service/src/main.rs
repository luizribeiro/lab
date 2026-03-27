use std::process;

use async_trait::async_trait;
use fittings::{
    FittingsError, MethodRouter, RouterService, RunOutcome, ServiceSchema, SpawnRunner,
};
use serde_json::{json, Value};

struct HelloRouter;

#[async_trait]
impl MethodRouter for HelloRouter {
    async fn route(
        &self,
        method: &str,
        params: Value,
        _metadata: fittings::Metadata,
    ) -> Result<Value, FittingsError> {
        match method {
            "hello" => hello_result(params),
            "ping" => ping_result(params),
            _ => Err(FittingsError::method_not_found(method.to_string())),
        }
    }
}

fn hello_result(params: Value) -> Result<Value, FittingsError> {
    let Some(obj) = params.as_object() else {
        return Err(FittingsError::invalid_params(
            "`hello` expects an object parameter",
        ));
    };

    let Some(name) = obj.get("name").and_then(Value::as_str) else {
        return Err(FittingsError::invalid_params(
            "`hello.name` must be a string",
        ));
    };

    Ok(json!({"message": format!("Hello, {name}!")}))
}

fn ping_result(params: Value) -> Result<Value, FittingsError> {
    let Some(obj) = params.as_object() else {
        return Err(FittingsError::invalid_params(
            "`ping` expects an object parameter",
        ));
    };

    if !obj.is_empty() {
        return Err(FittingsError::invalid_params(
            "`ping` does not accept parameters",
        ));
    }

    Ok(json!({"ok": true}))
}

fn service_schema() -> ServiceSchema {
    ServiceSchema {
        name: "hello-service".to_string(),
        methods: vec![
            fittings::MethodSchema {
                name: "hello".to_string(),
                description: Some("Greets the provided name".to_string()),
                params_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    },
                    "required": ["name"],
                    "additionalProperties": false
                })),
                result_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"],
                    "additionalProperties": false
                })),
            },
            fittings::MethodSchema {
                name: "ping".to_string(),
                description: Some("Health check".to_string()),
                params_schema: Some(json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                })),
                result_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "ok": {"type": "boolean"}
                    },
                    "required": ["ok"],
                    "additionalProperties": false
                })),
            },
        ],
        config_schema: Some(json!({
            "type": "object",
            "properties": {
                "log_level": {
                    "type": "string",
                    "enum": ["trace", "debug", "info", "warn", "error"]
                }
            },
            "additionalProperties": false
        })),
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

    let runner = SpawnRunner::new(service_schema());
    let outcome = runner
        .run_with_stdio_service(env_fittings.as_deref(), &args, |_config| {
            RouterService::new(HelloRouter)
        })
        .await;

    match outcome {
        RunOutcome::Normal => run_normal_cli(&args),
        RunOutcome::Exit(code) => process::exit(code),
    }
}

#[cfg(test)]
mod tests {
    use super::{hello_result, ping_result, service_schema, HelloRouter};
    use fittings::{FittingsError, MethodRouter};
    use serde_json::json;

    #[test]
    fn hello_handler_validates_and_formats_message() {
        let result = hello_result(json!({"name": "Ada"})).expect("hello should succeed");
        assert_eq!(result, json!({"message": "Hello, Ada!"}));

        let missing_name = hello_result(json!({})).expect_err("missing name should fail");
        assert!(matches!(missing_name, FittingsError::InvalidParams(_)));

        let wrong_type = hello_result(json!({"name": 7})).expect_err("wrong name type should fail");
        assert!(matches!(wrong_type, FittingsError::InvalidParams(_)));
    }

    #[test]
    fn ping_handler_requires_empty_object() {
        let result = ping_result(json!({})).expect("ping should succeed");
        assert_eq!(result, json!({"ok": true}));

        let bad = ping_result(json!({"x": 1})).expect_err("extra params should fail");
        assert!(matches!(bad, FittingsError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn router_maps_unknown_method_to_method_not_found() {
        let router = HelloRouter;
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
        let schema = service_schema();
        assert_eq!(schema.name, "hello-service");
        assert_eq!(schema.methods.len(), 2);

        let config_schema = schema.config_schema.expect("config schema should exist");
        assert_eq!(config_schema["properties"]["log_level"]["type"], "string");
    }
}
