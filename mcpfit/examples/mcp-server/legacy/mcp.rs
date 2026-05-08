use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fittings::serde_json::{json, Value};
use fittings::{FittingsError, Result, ServiceContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use fittings::core::context::{DroppedNotifications, PeerHandle};
#[cfg(test)]
use tokio_util::sync::CancellationToken;

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
#[serde(rename_all = "camelCase")]
pub struct ToolsRegisterParams {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub response_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolsRegisterResult {
    pub tool: Tool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressNotificationParams {
    pub progress_token: Value,
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    #[serde(default)]
    pub is_error: bool,
}

impl ToolsCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            structured_content: None,
            is_error: false,
        }
    }

    pub fn text_with_structured(
        text: impl Into<String>,
        structured_content: Value,
    ) -> Result<Self> {
        if !structured_content.is_object() {
            return Err(FittingsError::invalid_params(
                "`structuredContent` must be a JSON object",
            ));
        }

        Ok(Self {
            content: vec![ToolContent::Text { text: text.into() }],
            structured_content: Some(structured_content),
            is_error: false,
        })
    }
}

/// Per-tool-call context handed to a registered tool handler. Wraps the
/// handler's `ServiceContext` (so handlers can observe cancellation via
/// `ctx.cancelled()` / `ctx.is_cancelled()` and emit `notifications/progress`
/// via `ctx.notify`) plus the optional progress token negotiated for the call.
#[derive(Clone)]
pub struct ToolCallContext {
    ctx: ServiceContext,
    progress_token: Option<Value>,
}

impl ToolCallContext {
    pub fn from_service_context(ctx: ServiceContext) -> Self {
        Self {
            ctx,
            progress_token: None,
        }
    }

    pub fn detached() -> Self {
        Self::from_service_context(ServiceContext::detached())
    }

    pub fn with_progress_token(mut self, progress_token: Option<Value>) -> Self {
        self.progress_token = progress_token;
        self
    }

    pub fn is_cancelled(&self) -> bool {
        self.ctx.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.ctx.cancelled().await
    }

    pub fn emit_progress(&self, progress: f64, total: Option<f64>, message: Option<String>) {
        let Some(token) = &self.progress_token else {
            return;
        };
        let params = fittings::serde_json::to_value(ProgressNotificationParams {
            progress_token: token.clone(),
            progress,
            total,
            message,
        })
        .expect("progress notification params should serialize");
        let _ = self.ctx.notify("notifications/progress", params);
    }

    #[cfg(test)]
    fn detached_cancelled() -> Self {
        let token = CancellationToken::new();
        token.cancel();
        let (tx, rx) = fittings::tokio::sync::mpsc::channel(1);
        std::mem::forget(rx);
        let peer = PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new());
        Self::from_service_context(ServiceContext::new(None, token, peer))
    }
}

impl Default for ToolCallContext {
    fn default() -> Self {
        Self::detached()
    }
}

type ToolHandler = Arc<dyn Fn(Value, &ToolCallContext) -> Result<ToolsCallResult> + Send + Sync>;

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
        F: Fn(Value, &ToolCallContext) -> Result<ToolsCallResult> + Send + Sync + 'static,
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
                handler: Arc::new(handler),
            },
        );

        Ok(())
    }

    pub fn register_static_text_tool(
        &mut self,
        name: impl Into<String>,
        description: Option<String>,
        response_text: impl Into<String>,
    ) -> Result<Tool> {
        let name = name.into();
        let response_text = response_text.into();
        let description = description.unwrap_or_else(|| "Runtime registered tool".to_string());

        self.register(
            name.clone(),
            description,
            json!({
                "type": "object",
                "additionalProperties": false
            }),
            move |_arguments, _context| Ok(ToolsCallResult::text(response_text.clone())),
        )?;

        let tool = self
            .tools
            .get(&name)
            .map(|entry| entry.tool.clone())
            .expect("newly registered tool should exist");

        Ok(tool)
    }

    pub fn list(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|entry| entry.tool.clone())
            .collect()
    }

    fn handler_for(&self, name: &str) -> Result<ToolHandler> {
        self.tools
            .get(name)
            .map(|entry| Arc::clone(&entry.handler))
            .ok_or_else(|| FittingsError::method_not_found(format!("unknown tool `{name}`")))
    }

    #[cfg(test)]
    pub fn execute(
        &self,
        params: ToolsCallParams,
        context: &ToolCallContext,
    ) -> Result<ToolsCallResult> {
        if !params.arguments.is_object() {
            return Err(FittingsError::invalid_params(
                "`arguments` must be a JSON object",
            ));
        }

        let handler = self.handler_for(&params.name)?;
        handler(params.arguments, context)
    }
}

#[fittings::service]
pub trait McpService {
    /// Minimal MCP initialize handshake (stdio-oriented baseline).
    #[fittings::method(name = "initialize")]
    async fn initialize(
        &self,
        ctx: ServiceContext,
        params: InitializeParams,
    ) -> Result<InitializeResult>;

    /// Client notification sent after successful initialize handshake.
    #[fittings::method(name = "notifications/initialized")]
    async fn initialized(&self, ctx: ServiceContext, params: Value) -> Result<Value>;

    /// Returns the tools exposed by this process.
    #[fittings::method(name = "tools/list")]
    async fn list_tools(&self, ctx: ServiceContext, params: Value) -> Result<ToolsListResult>;

    /// Executes a named tool with JSON arguments.
    #[fittings::method(name = "tools/call")]
    async fn call_tool(
        &self,
        ctx: ServiceContext,
        params: ToolsCallParams,
    ) -> Result<ToolsCallResult>;

    /// Registers a simple runtime tool and notifies active clients that tools/list changed.
    #[fittings::method(name = "tools/register")]
    async fn register_tool(
        &self,
        ctx: ServiceContext,
        params: ToolsRegisterParams,
    ) -> Result<ToolsRegisterResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionLifecycle {
    AwaitingInitialize,
    AwaitingInitializedNotification,
    Running,
}

pub struct McpServiceImpl {
    registry: Mutex<ToolRegistry>,
    lifecycle: Mutex<SessionLifecycle>,
    list_changed_enabled: bool,
    progress_notifications_enabled: Mutex<bool>,
}

impl McpServiceImpl {
    pub fn new(registry: ToolRegistry) -> Self {
        Self {
            registry: Mutex::new(registry),
            lifecycle: Mutex::new(SessionLifecycle::AwaitingInitialize),
            list_changed_enabled: false,
            progress_notifications_enabled: Mutex::new(false),
        }
    }

    pub fn with_tools_list_changed(mut self, enabled: bool) -> Self {
        self.list_changed_enabled = enabled;
        self
    }

    fn set_progress_notifications_enabled(&self, enabled: bool) {
        let mut state = self
            .progress_notifications_enabled
            .lock()
            .expect("progress notification capability mutex should not be poisoned");
        *state = enabled;
    }

    fn progress_notifications_enabled(&self) -> bool {
        *self
            .progress_notifications_enabled
            .lock()
            .expect("progress notification capability mutex should not be poisoned")
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
    async fn initialize(
        &self,
        _ctx: ServiceContext,
        params: InitializeParams,
    ) -> Result<InitializeResult> {
        let progress_enabled = client_supports_progress_notifications(params.capabilities.as_ref());
        self.set_progress_notifications_enabled(progress_enabled);

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
                    list_changed: self.list_changed_enabled.then_some(true),
                }),
            },
            server_info: ServerInfo {
                name: "fittings-mcp-example".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }

    async fn initialized(&self, _ctx: ServiceContext, _params: Value) -> Result<Value> {
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

    async fn list_tools(&self, _ctx: ServiceContext, _params: Value) -> Result<ToolsListResult> {
        let registry = self
            .registry
            .lock()
            .expect("tool registry mutex should not be poisoned");

        Ok(ToolsListResult {
            tools: registry.list(),
        })
    }

    async fn call_tool(
        &self,
        ctx: ServiceContext,
        params: ToolsCallParams,
    ) -> Result<ToolsCallResult> {
        if !params.arguments.is_object() {
            return Err(FittingsError::invalid_params(
                "`arguments` must be a JSON object",
            ));
        }

        let progress_token = if self.progress_notifications_enabled() {
            extract_progress_token(params.meta.as_ref())
        } else {
            None
        };
        let context =
            ToolCallContext::from_service_context(ctx).with_progress_token(progress_token);

        let handler = {
            let registry = self
                .registry
                .lock()
                .expect("tool registry mutex should not be poisoned");
            registry.handler_for(&params.name)?
        };

        handler(params.arguments, &context)
    }

    async fn register_tool(
        &self,
        ctx: ServiceContext,
        params: ToolsRegisterParams,
    ) -> Result<ToolsRegisterResult> {
        if params.name.trim().is_empty() {
            return Err(FittingsError::invalid_params("`name` must not be empty"));
        }

        let tool = {
            let mut registry = self
                .registry
                .lock()
                .expect("tool registry mutex should not be poisoned");
            registry.register_static_text_tool(
                params.name,
                params.description,
                params.response_text,
            )?
        };

        if self.list_changed_enabled {
            let _ = ctx.notify("notifications/tools/list_changed", json!({}));
        }

        Ok(ToolsRegisterResult { tool })
    }
}

fn client_supports_progress_notifications(capabilities: Option<&Value>) -> bool {
    let Some(capabilities) = capabilities else {
        return false;
    };

    capabilities
        .get("experimental")
        .and_then(Value::as_object)
        .and_then(|experimental| {
            experimental
                .get("progressNotifications")
                .and_then(Value::as_bool)
                .or_else(|| {
                    experimental
                        .get("notifications/progress")
                        .and_then(Value::as_bool)
                })
        })
        .unwrap_or(false)
}

fn extract_progress_token(meta: Option<&Value>) -> Option<Value> {
    let meta = meta?.as_object()?;
    let token = meta.get("progressToken")?;
    if token.is_string() || token.is_number() {
        Some(token.clone())
    } else {
        None
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
                "tools/call",
                "tools/register"
            ]
        );
    }

    #[test]
    fn initialize_result_serializes_as_camel_case() {
        let result = InitializeResult {
            protocol_version: "2025-01-01".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
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
        assert_eq!(encoded["capabilities"]["tools"]["listChanged"], true);
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
    fn tools_call_result_supports_text_and_structured_content() {
        let result = ToolsCallResult::text_with_structured(
            "2 + 3 = 5",
            json!({
                "a": 2,
                "b": 3,
                "sum": 5
            }),
        )
        .expect("tools/call result with structured content should be valid");

        let encoded = fittings::serde_json::to_value(result)
            .expect("tools/call result should serialize to JSON");

        assert_eq!(encoded["content"][0]["type"], "text");
        assert_eq!(encoded["content"][0]["text"], "2 + 3 = 5");
        assert_eq!(encoded["structuredContent"]["sum"], 5);
        assert_eq!(encoded["isError"], false);
    }

    #[test]
    fn tools_call_result_rejects_non_object_structured_content() {
        let result = ToolsCallResult::text_with_structured("invalid", json!([1, 2, 3]));

        assert!(matches!(result, Err(FittingsError::InvalidParams { .. })));
    }

    #[test]
    fn registry_lists_tools_and_executes_tool_handler() {
        let mut registry = ToolRegistry::new();
        registry
            .register(
                "sum",
                "Sums two numbers",
                json!({"type": "object"}),
                |arguments, _context| {
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
            .execute(
                ToolsCallParams {
                    name: "sum".to_string(),
                    arguments: json!({"a": 2, "b": 3}),
                    meta: None,
                },
                &ToolCallContext::default(),
            )
            .expect("tool execution should succeed");

        assert_eq!(result.content.len(), 1);
        assert!(matches!(
            &result.content[0],
            ToolContent::Text { text } if text == "5"
        ));
    }

    #[test]
    fn tool_handlers_can_observe_cancellation_context() {
        let mut registry = ToolRegistry::new();
        registry
            .register(
                "cancel-aware",
                "Observes cancellation",
                json!({"type": "object"}),
                |_arguments, context| {
                    if context.is_cancelled() {
                        return Ok(ToolsCallResult::text("cancelled"));
                    }
                    Ok(ToolsCallResult::text("running"))
                },
            )
            .expect("register should succeed");

        let context = ToolCallContext::detached_cancelled();

        let result = registry
            .execute(
                ToolsCallParams {
                    name: "cancel-aware".to_string(),
                    arguments: json!({}),
                    meta: None,
                },
                &context,
            )
            .expect("tool execution should succeed");

        assert!(matches!(
            &result.content[0],
            ToolContent::Text { text } if text == "cancelled"
        ));
    }

    #[test]
    fn registry_reports_unknown_tool_and_bad_arguments() {
        let registry = ToolRegistry::new();

        let unknown = registry.execute(
            ToolsCallParams {
                name: "missing".to_string(),
                arguments: json!({}),
                meta: None,
            },
            &ToolCallContext::default(),
        );
        assert!(matches!(unknown, Err(FittingsError::MethodNotFound { .. })));

        let mut registry = ToolRegistry::new();
        registry
            .register(
                "noop",
                "No-op tool",
                json!({"type": "object"}),
                |_, _context| Ok(ToolsCallResult::text("ok")),
            )
            .expect("register should succeed");

        let invalid = registry.execute(
            ToolsCallParams {
                name: "noop".to_string(),
                arguments: json!([1, 2, 3]),
                meta: None,
            },
            &ToolCallContext::default(),
        );
        assert!(matches!(invalid, Err(FittingsError::InvalidParams { .. })));
    }

    #[test]
    fn registry_rejects_duplicate_tool_names() {
        let mut registry = ToolRegistry::new();
        registry
            .register("echo", "Echo", json!({"type": "object"}), |_, _context| {
                Ok(ToolsCallResult::text("one"))
            })
            .expect("first registration should succeed");

        let duplicate = registry.register(
            "echo",
            "Echo again",
            json!({"type": "object"}),
            |_, _context| Ok(ToolsCallResult::text("two")),
        );

        assert!(matches!(
            duplicate,
            Err(FittingsError::InvalidRequest { .. })
        ));
    }

    #[tokio::test]
    async fn default_service_supports_initialize_and_empty_tool_list() {
        let service = McpServiceImpl::default();

        let initialize = service
            .initialize(
                ServiceContext::detached(),
                InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                    client_info: None,
                    capabilities: None,
                },
            )
            .await
            .expect("initialize should succeed");
        assert_eq!(initialize.protocol_version, "2024-11-05");
        assert!(initialize.capabilities.tools.is_some());
        assert_eq!(
            initialize
                .capabilities
                .tools
                .expect("tools capability should be present")
                .list_changed,
            None
        );

        let listed = service
            .list_tools(
                ServiceContext::detached(),
                fittings::serde_json::Value::Null,
            )
            .await
            .expect("tools/list should succeed");
        assert!(listed.tools.is_empty());

        let called = service
            .call_tool(
                ServiceContext::detached(),
                ToolsCallParams {
                    name: "echo".to_string(),
                    arguments: json!({"message": "hello"}),
                    meta: None,
                },
            )
            .await;

        assert!(matches!(called, Err(FittingsError::MethodNotFound { .. })));
    }

    #[tokio::test]
    async fn initialized_before_initialize_is_invalid_request() {
        let service = McpServiceImpl::default();

        let result = service
            .initialized(ServiceContext::detached(), Value::Null)
            .await;

        assert!(matches!(result, Err(FittingsError::InvalidRequest { .. })));
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
            .initialize(
                ServiceContext::detached(),
                InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                    client_info: None,
                    capabilities: None,
                },
            )
            .await
            .expect("initialize should succeed");
        assert_eq!(
            service.lifecycle_state(),
            SessionLifecycle::AwaitingInitializedNotification
        );

        service
            .initialized(ServiceContext::detached(), Value::Null)
            .await
            .expect("initialized notification should be accepted");
        assert_eq!(service.lifecycle_state(), SessionLifecycle::Running);
    }

    #[tokio::test]
    async fn runtime_tool_registration_succeeds_in_each_lifecycle_state() {
        let service = McpServiceImpl::default().with_tools_list_changed(true);

        service
            .register_tool(
                ServiceContext::detached(),
                ToolsRegisterParams {
                    name: "runtime".to_string(),
                    description: None,
                    response_text: "ok".to_string(),
                },
            )
            .await
            .expect("register before initialize should succeed");

        service
            .initialize(
                ServiceContext::detached(),
                InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                    client_info: None,
                    capabilities: None,
                },
            )
            .await
            .expect("initialize should succeed");
        service
            .initialized(ServiceContext::detached(), Value::Null)
            .await
            .expect("initialized should succeed");

        let registered = service
            .register_tool(
                ServiceContext::detached(),
                ToolsRegisterParams {
                    name: "runtime-two".to_string(),
                    description: Some("runtime".to_string()),
                    response_text: "ok".to_string(),
                },
            )
            .await
            .expect("register while running should succeed");
        assert_eq!(registered.tool.name, "runtime-two");
    }

    #[tokio::test]
    async fn runtime_tool_registration_rejects_invalid_or_duplicate_names() {
        let service = McpServiceImpl::default().with_tools_list_changed(true);

        service
            .initialize(
                ServiceContext::detached(),
                InitializeParams {
                    protocol_version: "2024-11-05".to_string(),
                    client_info: None,
                    capabilities: None,
                },
            )
            .await
            .expect("initialize should succeed");
        service
            .initialized(ServiceContext::detached(), Value::Null)
            .await
            .expect("initialized should succeed");

        let empty_name = service
            .register_tool(
                ServiceContext::detached(),
                ToolsRegisterParams {
                    name: "   ".to_string(),
                    description: None,
                    response_text: "ignored".to_string(),
                },
            )
            .await;
        assert!(matches!(
            empty_name,
            Err(FittingsError::InvalidParams { .. })
        ));

        service
            .register_tool(
                ServiceContext::detached(),
                ToolsRegisterParams {
                    name: "runtime".to_string(),
                    description: None,
                    response_text: "ok".to_string(),
                },
            )
            .await
            .expect("first registration should succeed");

        let duplicate = service
            .register_tool(
                ServiceContext::detached(),
                ToolsRegisterParams {
                    name: "runtime".to_string(),
                    description: None,
                    response_text: "another".to_string(),
                },
            )
            .await;
        assert!(matches!(
            duplicate,
            Err(FittingsError::InvalidRequest { .. })
        ));

        let listed = service
            .list_tools(ServiceContext::detached(), Value::Null)
            .await
            .expect("tools/list should succeed");
        assert_eq!(listed.tools.len(), 1);
        assert_eq!(listed.tools[0].name, "runtime");
    }
}
