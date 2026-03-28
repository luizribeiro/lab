use std::collections::BTreeMap;
use std::sync::Mutex;

use fittings::serde_json::{json, Value};
use fittings::{FittingsError, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
}
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCallParams {
    pub name: String,
    #[serde(default = "empty_object")]
    pub arguments: Value,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCallResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolsCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: false,
        }
    }
}

type ToolHandler = Box<dyn Fn(Value) -> Result<ToolsCallResult> + Send + Sync>;

struct RegisteredTool {
    tool: Tool,
    handler: ToolHandler,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Value) -> Result<ToolsCallResult> + Send + Sync + 'static,
    {
        let name = name.into();
        if self.tools.contains_key(&name) {
            return Err(FittingsError::invalid_request(format!(
                "tool `{name}` is already registered"
            )));
        }

        self.tools.insert(
            name.clone(),
            RegisteredTool {
                tool: Tool {
                    name,
                    description: Some(description.into()),
                    input_schema,
                },
                handler: Box::new(handler),
            },
        );

        Ok(())
    }

    pub fn list(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|entry| entry.tool.clone())
            .collect()
    }

    pub fn execute(&self, params: ToolsCallParams) -> Result<ToolsCallResult> {
        let entry = self.tools.get(&params.name).ok_or_else(|| {
            FittingsError::method_not_found(format!("unknown tool `{}`", params.name))
        })?;

        if !params.arguments.is_object() {
            return Err(FittingsError::invalid_params(
                "`arguments` must be a JSON object",
            ));
        }

        (entry.handler)(params.arguments)
    }
}

#[fittings::service]
pub trait McpService {
    /// Minimal MCP initialize handshake (stdio-oriented baseline).
    #[fittings::method(name = "initialize")]
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult>;

    /// Client notification sent after successful initialize handshake.
    #[fittings::method(name = "notifications/initialized")]
    async fn initialized(&self, params: Value) -> Result<Value>;

    /// Returns the tools exposed by this process.
    #[fittings::method(name = "tools/list")]
    async fn list_tools(&self, params: Value) -> Result<ToolsListResult>;

    /// Executes a named tool with JSON arguments.
    #[fittings::method(name = "tools/call")]
    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolsCallResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionLifecycle {
    AwaitingInitialize,
    AwaitingInitializedNotification,
    Running,
}

pub struct McpServiceImpl {
    registry: ToolRegistry,
    lifecycle: Mutex<SessionLifecycle>,
}

impl McpServiceImpl {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry,
            lifecycle: Mutex::new(SessionLifecycle::AwaitingInitialize),
        }
    }

    #[cfg(test)]
    fn lifecycle_state(&self) -> SessionLifecycle {
        *self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned")
    }
}

impl Default for McpServiceImpl {
    fn default() -> Self {
        Self::new(ToolRegistry::new())
    }
}

impl McpService for McpServiceImpl {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        {
            let mut lifecycle = self
                .lifecycle
                .lock()
                .expect("session lifecycle mutex should not be poisoned");
            *lifecycle = SessionLifecycle::AwaitingInitializedNotification;
        }

        Ok(InitializeResult {
            protocol_version: params.protocol_version,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
            },
            server_info: ServerInfo {
                name: "fittings-mcp-example".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }

    async fn initialized(&self, _params: Value) -> Result<Value> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned");

        match *lifecycle {
            SessionLifecycle::AwaitingInitialize => Err(FittingsError::invalid_request(
                "received notifications/initialized before initialize",
            )),
            SessionLifecycle::AwaitingInitializedNotification => {
                *lifecycle = SessionLifecycle::Running;
                Ok(Value::Null)
            }
            SessionLifecycle::Running => Ok(Value::Null),
        }
    }

    async fn list_tools(&self, _params: Value) -> Result<ToolsListResult> {
        Ok(ToolsListResult {
            tools: self.registry.list(),
        })
    }

    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolsCallResult> {
        self.registry.execute(params)
    }
}

fn empty_object() -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_schema_contains_mcp_method_names() {
        let schema = mcp_service_schema();
        let method_names: Vec<_> = schema
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect();

        assert_eq!(schema.name, "mcp-service");
        assert_eq!(
            method_names,
            vec![
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/call"
            ]
        );
    }

    #[test]
    fn initialize_result_serializes_as_camel_case() {
        let result = InitializeResult {
            protocol_version: "2025-01-01".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
            },
            server_info: ServerInfo {
                name: "server".to_string(),
                version: "1.2.3".to_string(),
            },
        };

        let encoded =
            fittings::serde_json::to_value(&result).expect("initialize result should serialize");

        assert_eq!(encoded["protocolVersion"], "2025-01-01");
        assert_eq!(encoded["serverInfo"]["name"], "server");
        assert_eq!(encoded["capabilities"]["tools"]["listChanged"], false);
    }

    #[test]
    fn tools_call_params_defaults_arguments_to_object() {
        let params: ToolsCallParams = fittings::serde_json::from_value(json!({
            "name": "echo"
        }))
        .expect("params should deserialize");

        assert_eq!(params.name, "echo");
        assert_eq!(params.arguments, json!({}));
    }

    #[test]
    fn registry_lists_tools_and_executes_tool_handler() {
        let mut registry = ToolRegistry::new();
        registry
            .register(
                "sum",
                "Sums two numbers",
                json!({"type": "object"}),
                |arguments| {
                    let a = arguments["a"].as_i64().ok_or_else(|| {
                        FittingsError::invalid_params("`arguments.a` must be an integer")
                    })?;
                    let b = arguments["b"].as_i64().ok_or_else(|| {
                        FittingsError::invalid_params("`arguments.b` must be an integer")
                    })?;
                    Ok(ToolsCallResult::text((a + b).to_string()))
                },
            )
            .expect("register should succeed");

        let listed = registry.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "sum");

        let result = registry
            .execute(ToolsCallParams {
                name: "sum".to_string(),
                arguments: json!({"a": 2, "b": 3}),
                meta: None,
            })
            .expect("tool execution should succeed");

        assert_eq!(result.content.len(), 1);
        assert!(matches!(
            &result.content[0],
            ToolContent::Text { text } if text == "5"
        ));
    }

    #[test]
    fn registry_reports_unknown_tool_and_bad_arguments() {
        let registry = ToolRegistry::new();

        let unknown = registry.execute(ToolsCallParams {
            name: "missing".to_string(),
            arguments: json!({}),
            meta: None,
        });
        assert!(matches!(unknown, Err(FittingsError::MethodNotFound(_))));

        let mut registry = ToolRegistry::new();
        registry
            .register("noop", "No-op tool", json!({"type": "object"}), |_| {
                Ok(ToolsCallResult::text("ok"))
            })
            .expect("register should succeed");

        let invalid = registry.execute(ToolsCallParams {
            name: "noop".to_string(),
            arguments: json!([1, 2, 3]),
            meta: None,
        });
        assert!(matches!(invalid, Err(FittingsError::InvalidParams(_))));
    }

    #[test]
    fn registry_rejects_duplicate_tool_names() {
        let mut registry = ToolRegistry::new();
        registry
            .register("echo", "Echo", json!({"type": "object"}), |_| {
                Ok(ToolsCallResult::text("one"))
            })
            .expect("first registration should succeed");

        let duplicate = registry.register("echo", "Echo again", json!({"type": "object"}), |_| {
            Ok(ToolsCallResult::text("two"))
        });

        assert!(matches!(duplicate, Err(FittingsError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn default_service_supports_initialize_and_empty_tool_list() {
        let service = McpServiceImpl::default();

        let initialize = service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");
        assert_eq!(initialize.protocol_version, "2024-11-05");

        let listed = service
            .list_tools(fittings::serde_json::Value::Null)
            .await
            .expect("tools/list should succeed");
        assert!(listed.tools.is_empty());

        let called = service
            .call_tool(ToolsCallParams {
                name: "echo".to_string(),
                arguments: json!({"message": "hello"}),
                meta: None,
            })
            .await;

        assert!(matches!(called, Err(FittingsError::MethodNotFound(_))));
    }

    #[tokio::test]
    async fn initialized_before_initialize_is_invalid_request() {
        let service = McpServiceImpl::default();

        let result = service.initialized(Value::Null).await;

        assert!(matches!(result, Err(FittingsError::InvalidRequest(_))));
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitialize,
            "state should remain unchanged"
        );
    }

    #[tokio::test]
    async fn initialize_and_initialized_notification_advance_session_lifecycle() {
        let service = McpServiceImpl::default();
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitialize
        );

        service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitializedNotification
        );

        service
            .initialized(Value::Null)
            .await
            .expect("initialized notification should be accepted");
        assert_eq!(service.lifecycle_state(), SessionLifecycle::Running);
    }
}
