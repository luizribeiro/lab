use fittings::FittingsError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct HelloParams {
    name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct HelloResult {
    message: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PingParams {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PingResult {
    ok: bool,
}

#[allow(dead_code)]
#[fittings::service]
trait GreetingService {
    /// Greets a person by name.
    #[fittings::method(name = "tools/hello")]
    async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError>;

    async fn ping(&self, params: PingParams) -> Result<PingResult, FittingsError>;
}

#[test]
fn generated_schema_uses_expected_service_name_and_methods() {
    let schema = greeting_service_schema();

    assert_eq!(schema.name, "greeting-service");
    assert_eq!(schema.methods.len(), 2);
    assert_eq!(schema.methods[0].name, "tools/hello");
    assert_eq!(
        schema.methods[0].description.as_deref(),
        Some("Greets a person by name.")
    );
    assert_eq!(schema.methods[1].name, "ping");
    assert_eq!(schema.methods[1].description, None);
}

#[test]
fn generated_schema_includes_object_json_schemas_for_params_and_results() {
    let schema = greeting_service_schema();

    for method in schema.methods {
        let params_schema = method
            .params_schema
            .expect("params schema should be present");
        let result_schema = method
            .result_schema
            .expect("result schema should be present");

        assert!(
            params_schema.is_object(),
            "params schema for method `{}` must be a JSON object",
            method.name
        );
        assert!(
            result_schema.is_object(),
            "result schema for method `{}` must be a JSON object",
            method.name
        );
    }
}

#[test]
fn generated_schema_json_shape_is_stable() {
    let schema = greeting_service_schema();
    let value = fittings::serde_json::to_value(schema).expect("schema should serialize");

    assert_eq!(value["name"], "greeting-service");
    assert_eq!(value["methods"][0]["name"], "tools/hello");
    assert_eq!(
        value["methods"][0]["description"],
        "Greets a person by name."
    );
    assert_eq!(value["methods"][0]["params_schema"]["type"], "object");
    assert_eq!(value["methods"][0]["result_schema"]["type"], "object");
    assert_eq!(value["methods"][1]["name"], "ping");
    assert!(value["methods"][1].get("description").is_none());
    assert_eq!(
        value["config_schema"]["properties"]["log_level"]["enum"],
        fittings::serde_json::json!(["trace", "debug", "info", "warn", "error"])
    );
    assert_eq!(value["config_schema"]["additionalProperties"], false);
}
