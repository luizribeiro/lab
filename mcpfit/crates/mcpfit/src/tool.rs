//! Builder for tool definitions.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::context::Cx;
use crate::error::McpfitError;
use crate::response::{IntoToolResponse, ToolResponse};
use crate::schema::schema_for;
use crate::Result;

pub(crate) type HandlerFuture = Pin<Box<dyn Future<Output = Result<ToolResponse>> + Send>>;
pub(crate) type BoxedHandler = Arc<dyn Fn(Value, Cx) -> HandlerFuture + Send + Sync>;

/// Builder for a single MCP tool.
pub struct Tool {
    name: String,
    description: Option<String>,
    input_schema: Option<Value>,
    handler: Option<BoxedHandler>,
}

impl Tool {
    /// Starts a new tool builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: None,
            handler: None,
        }
    }

    /// Sets the human-readable description advertised to clients.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn input<T: JsonSchema>(mut self) -> Self {
        self.input_schema = Some(schema_for::<T>());
        self
    }

    /// Mutually exclusive with [`Tool::input`] and [`Tool::input_with_schema`];
    /// the most recent call wins.
    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Reserves `T` as the Rust type used for argument deserialization once
    /// handlers are wired up, while advertising the supplied hand-tuned schema.
    pub fn input_with_schema<T>(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description_str(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn input_schema_value(&self) -> Option<&Value> {
        self.input_schema.as_ref()
    }

    pub fn handler<F, Fut, A, R>(mut self, f: F) -> Self
    where
        F: Fn(A, Cx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
        A: DeserializeOwned + Send + 'static,
        R: IntoToolResponse + Send + 'static,
    {
        self.handler = Some(Arc::new(move |args, cx| {
            match serde_json::from_value::<A>(args) {
                Ok(typed) => {
                    let fut = f(typed, cx);
                    Box::pin(async move { fut.await.map(IntoToolResponse::into_tool_response) })
                }
                Err(e) => {
                    let err = McpfitError::invalid_params(format!("invalid arguments: {e}"));
                    Box::pin(async move { Err(err) })
                }
            }
        }));
        self
    }

    /// Invokes the stored handler. Returns `Internal` when no handler has been
    /// configured.
    pub async fn call(&self, args: Value, cx: Cx) -> Result<ToolResponse> {
        match &self.handler {
            Some(handler) => handler(args, cx).await,
            None => Err(McpfitError::internal(format!(
                "tool {} has no handler",
                self.name
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Tool;
    use crate::context::Cx;
    use crate::error::McpfitError;
    use crate::response::ToolResponse;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(JsonSchema, Deserialize)]
    #[allow(dead_code)]
    struct AddArgs {
        a: f64,
        b: f64,
    }

    #[test]
    fn new_stores_name_without_description() {
        let tool = Tool::new("add");
        assert_eq!(tool.name(), "add");
        assert_eq!(tool.description_str(), None);
        assert!(tool.input_schema_value().is_none());
    }

    #[test]
    fn description_sets_value() {
        let tool = Tool::new("add").description("Adds two numbers");
        assert_eq!(tool.description_str(), Some("Adds two numbers"));
    }

    #[test]
    fn description_overrides_previous_value() {
        let tool = Tool::new("add")
            .description("first")
            .description("second");
        assert_eq!(tool.description_str(), Some("second"));
    }

    #[test]
    fn name_accepts_string_and_str() {
        let from_str = Tool::new("a");
        let from_string = Tool::new(String::from("a"));
        assert_eq!(from_str.name(), from_string.name());
    }

    #[test]
    fn input_overrides_previous_schema() {
        #[derive(JsonSchema)]
        #[allow(dead_code)]
        struct Other {
            x: i32,
        }

        let tool = Tool::new("t").input::<AddArgs>().input::<Other>();
        let props = tool.input_schema_value().unwrap()["properties"]
            .as_object()
            .unwrap();
        assert!(props.contains_key("x"));
        assert!(!props.contains_key("a"));
    }

    #[test]
    fn input_schema_sets_hand_written_schema() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": { "a": { "type": "number", "description": "first" } },
            "required": ["a"],
        });
        let tool = Tool::new("t").input_schema(raw.clone());
        assert_eq!(tool.input_schema_value(), Some(&raw));
    }

    #[test]
    fn input_schema_overrides_previous_typed_schema() {
        let raw = serde_json::json!({"type": "object"});
        let tool = Tool::new("t").input::<AddArgs>().input_schema(raw.clone());
        assert_eq!(tool.input_schema_value(), Some(&raw));
    }

    #[test]
    fn input_with_schema_stores_hand_tuned_schema() {
        let raw = serde_json::json!({
            "type": "object",
            "properties": { "a": { "type": "number", "description": "tuned" } },
        });
        let tool = Tool::new("t").input_with_schema::<AddArgs>(raw.clone());
        assert_eq!(tool.input_schema_value(), Some(&raw));
    }

    #[test]
    fn typed_input_overrides_hand_written_schema() {
        let raw = serde_json::json!({"type": "object"});
        let tool = Tool::new("t").input_schema(raw).input::<AddArgs>();
        let props = tool.input_schema_value().unwrap()["properties"]
            .as_object()
            .unwrap();
        assert!(props.contains_key("a"));
    }

    #[tokio::test]
    async fn call_runs_stored_handler_with_args() {
        let tool = Tool::new("echo").handler(|args: serde_json::Value, _cx| async move {
            Ok::<_, McpfitError>(args["msg"].as_str().expect("msg key").to_string())
        });
        let response = tool
            .call(json!({"msg": "hi"}), Cx::default())
            .await
            .expect("handler ok");
        assert_eq!(response, ToolResponse::success("hi"));
    }

    #[tokio::test]
    async fn call_propagates_handler_errors() {
        let tool = Tool::new("boom").handler(|_args: serde_json::Value, _cx| async move {
            Err::<String, _>(McpfitError::invalid_params("bad"))
        });
        let err = tool.call(json!({}), Cx::default()).await.unwrap_err();
        assert!(matches!(err, McpfitError::InvalidParams(m) if m == "bad"));
    }

    #[tokio::test]
    async fn call_returns_internal_when_no_handler() {
        let tool = Tool::new("noop");
        let err = tool.call(json!({}), Cx::default()).await.unwrap_err();
        assert!(matches!(err, McpfitError::Internal(m) if m.contains("noop")));
    }

    #[tokio::test]
    async fn call_deserializes_typed_args() {
        let tool = Tool::new("add").handler(|args: AddArgs, _cx| async move {
            Ok::<_, McpfitError>(args.a + args.b)
        });
        let response = tool
            .call(json!({"a": 2.0, "b": 3.0}), Cx::default())
            .await
            .expect("handler ok");
        assert_eq!(response, ToolResponse::success("5"));
    }

    #[tokio::test]
    async fn call_returns_invalid_params_for_bad_args() {
        let tool = Tool::new("add").handler(|args: AddArgs, _cx| async move {
            Ok::<_, McpfitError>(args.a + args.b)
        });
        let err = tool
            .call(json!({"a": "not a number", "b": 3.0}), Cx::default())
            .await
            .unwrap_err();
        assert!(matches!(err, McpfitError::InvalidParams(m) if m.contains("invalid arguments")));
    }

    #[tokio::test]
    async fn call_returns_invalid_params_for_missing_field() {
        let tool = Tool::new("add").handler(|args: AddArgs, _cx| async move {
            Ok::<_, McpfitError>(args.a + args.b)
        });
        let err = tool
            .call(json!({"a": 1.0}), Cx::default())
            .await
            .unwrap_err();
        assert!(matches!(err, McpfitError::InvalidParams(_)));
    }

    #[tokio::test]
    async fn call_supports_unit_args() {
        let tool = Tool::new("ping")
            .handler(|_args: (), _cx| async move { Ok::<_, McpfitError>("pong".to_string()) });
        let response = tool
            .call(json!(null), Cx::default())
            .await
            .expect("handler ok");
        assert_eq!(response, ToolResponse::success("pong"));
    }

    #[test]
    fn input_preserves_name_and_description() {
        let tool = Tool::new("add")
            .description("Adds two numbers")
            .input::<AddArgs>();
        assert_eq!(tool.name(), "add");
        assert_eq!(tool.description_str(), Some("Adds two numbers"));
        assert!(tool.input_schema_value().is_some());
    }
}
