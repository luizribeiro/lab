//! MCP service trait. Declares the JSON-RPC method surface implemented by
//! the mcpfit server.

use std::sync::{Arc, Mutex};

use fittings::serde_json::{self, Value};
use fittings::{FittingsError, Result};

use crate::context::{extract_progress_token, Cx, ProgressSink};
use crate::error::McpfitError;
use crate::protocol::{
    InitializeParams, InitializeResult, ProgressNotificationParams, ServerCapabilities, ServerInfo,
    ToolsCallParams, ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
};
use crate::registry::ToolRegistry;
use crate::response::ToolResponse;

#[derive(Debug, Clone, PartialEq)]
pub struct ServerNotification {
    pub method: String,
    pub params: Value,
}

fn to_fittings_error(err: McpfitError) -> FittingsError {
    match err {
        McpfitError::InvalidRequest(m) => FittingsError::invalid_request(m),
        McpfitError::MethodNotFound(m) => FittingsError::method_not_found(m),
        McpfitError::InvalidParams(m) => FittingsError::invalid_params(m),
        McpfitError::Cancelled => FittingsError::invalid_request("cancelled"),
        McpfitError::Internal(m) => FittingsError::internal(m),
    }
}

#[fittings::service]
pub trait McpService {
    #[fittings::method(name = "initialize")]
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult>;

    #[fittings::method(name = "notifications/initialized")]
    async fn initialized(&self, params: Value) -> Result<Value>;

    #[fittings::method(name = "tools/list")]
    async fn list_tools(&self, params: Value) -> Result<ToolsListResult>;

    #[fittings::method(name = "tools/call")]
    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolResponse>;

    /// Disabled by default; gated by `Server::allow_runtime_registration()`.
    #[fittings::method(name = "tools/register")]
    async fn register_tool(&self, params: ToolsRegisterParams) -> Result<ToolsRegisterResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum SessionLifecycle {
    AwaitingInitialize,
    AwaitingInitializedNotification,
    Running,
}

#[allow(dead_code)]
pub(crate) struct McpServiceImpl {
    server_info: ServerInfo,
    registry: ToolRegistry,
    lifecycle: Mutex<SessionLifecycle>,
    pending_notifications: Arc<Mutex<Vec<ServerNotification>>>,
}

#[allow(dead_code)]
impl McpServiceImpl {
    pub(crate) fn new(server_info: ServerInfo, registry: ToolRegistry) -> Self {
        Self {
            server_info,
            registry,
            lifecycle: Mutex::new(SessionLifecycle::AwaitingInitialize),
            pending_notifications: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn lifecycle_state(&self) -> SessionLifecycle {
        *self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned")
    }

    pub(crate) fn drain_notifications(&self) -> Vec<ServerNotification> {
        self.pending_notifications
            .lock()
            .expect("pending notifications mutex should not be poisoned")
            .drain(..)
            .collect()
    }

    fn cx_for_call(&self, meta: Option<&Value>) -> Cx {
        let Some(token) = extract_progress_token(meta) else {
            return Cx::default();
        };
        let pending = Arc::clone(&self.pending_notifications);
        let sink: ProgressSink = Arc::new(move |params: ProgressNotificationParams| {
            let serialized = serde_json::to_value(&params)
                .expect("progress notification params should serialize");
            pending
                .lock()
                .expect("pending notifications mutex should not be poisoned")
                .push(ServerNotification {
                    method: "notifications/progress".to_string(),
                    params: serialized,
                });
        });
        Cx::with_progress(token, sink)
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
            capabilities: ServerCapabilities::default(),
            server_info: self.server_info.clone(),
        })
    }

    async fn initialized(&self, _params: Value) -> Result<Value> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned");
        match *lifecycle {
            SessionLifecycle::AwaitingInitialize => {
                return Err(FittingsError::invalid_request(
                    "notifications/initialized received before initialize",
                ));
            }
            SessionLifecycle::AwaitingInitializedNotification | SessionLifecycle::Running => {
                *lifecycle = SessionLifecycle::Running;
            }
        }
        Ok(Value::Null)
    }

    async fn list_tools(&self, _params: Value) -> Result<ToolsListResult> {
        if self.lifecycle_state() == SessionLifecycle::AwaitingInitialize {
            return Err(FittingsError::invalid_request(
                "tools/list received before initialize",
            ));
        }
        Ok(ToolsListResult {
            tools: self.registry.list(),
        })
    }

    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolResponse> {
        match self.lifecycle_state() {
            SessionLifecycle::AwaitingInitialize => {
                return Err(FittingsError::invalid_request(
                    "tools/call received before initialize",
                ));
            }
            SessionLifecycle::AwaitingInitializedNotification => {
                return Err(FittingsError::invalid_request(
                    "tools/call received before notifications/initialized",
                ));
            }
            SessionLifecycle::Running => {}
        }
        let cx = self.cx_for_call(params.meta.as_ref());
        self.registry
            .call(&params.name, params.arguments, cx)
            .await
            .map_err(to_fittings_error)
    }

    async fn register_tool(&self, _params: ToolsRegisterParams) -> Result<ToolsRegisterResult> {
        Err(FittingsError::method_not_found(
            "tools/register is disabled; enable runtime registration on the server to use it",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{mcp_service_schema, McpService, McpServiceImpl, SessionLifecycle};
    use crate::error::McpfitError;
    use crate::protocol::{InitializeParams, ServerInfo, ToolsCallParams, ToolsRegisterParams};
    use crate::registry::ToolRegistry;
    use crate::response::ToolResponse;
    use crate::tool::Tool;
    use fittings::serde_json::{json, Value};
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(JsonSchema, Deserialize)]
    struct AddArgs {
        a: f64,
        b: f64,
    }

    fn call_params(name: &str) -> ToolsCallParams {
        ToolsCallParams {
            name: name.into(),
            arguments: json!({}),
            meta: None,
        }
    }

    fn service(name: &str, version: &str) -> McpServiceImpl {
        service_with_registry(name, version, ToolRegistry::new())
    }

    fn service_with_add_tool() -> McpServiceImpl {
        let mut registry = ToolRegistry::new();
        registry
            .register(Tool::new("add").input::<AddArgs>().handler(
                |args: AddArgs, _cx| async move { Ok::<_, McpfitError>(args.a + args.b) },
            ))
            .unwrap();
        service_with_registry("demo", "0.1.0", registry)
    }

    fn service_with_registry(name: &str, version: &str, registry: ToolRegistry) -> McpServiceImpl {
        McpServiceImpl::new(
            ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            registry,
        )
    }

    async fn initialize(svc: &McpServiceImpl) {
        svc.initialize(InitializeParams {
            protocol_version: "2025-01-01".into(),
            client_info: None,
            capabilities: None,
        })
        .await
        .expect("initialize should succeed");
    }

    #[test]
    fn schema_exposes_expected_methods() {
        let schema = mcp_service_schema();
        let names: Vec<&str> = schema.methods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/call",
                "tools/register",
            ]
        );
    }

    #[test]
    fn new_starts_awaiting_initialize() {
        let svc = service("demo", "0.1.0");
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::AwaitingInitialize);
    }

    #[tokio::test]
    async fn initialize_transitions_to_awaiting_initialized_notification() {
        let svc = service("demo", "1.2.3");
        let result = svc
            .initialize(InitializeParams {
                protocol_version: "2025-01-01".into(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");

        assert_eq!(result.protocol_version, "2025-01-01");
        assert_eq!(result.server_info.name, "demo");
        assert_eq!(result.server_info.version, "1.2.3");
        assert_eq!(
            svc.lifecycle_state(),
            SessionLifecycle::AwaitingInitializedNotification
        );
    }

    #[tokio::test]
    async fn initialized_transitions_to_running() {
        let svc = service("demo", "0.1.0");
        svc.initialize(InitializeParams {
            protocol_version: "2025-01-01".into(),
            client_info: None,
            capabilities: None,
        })
        .await
        .expect("initialize should succeed");

        let result = svc
            .initialized(json!({}))
            .await
            .expect("initialized should succeed after initialize");

        assert_eq!(result, Value::Null);
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::Running);
    }

    #[tokio::test]
    async fn initialized_before_initialize_is_rejected() {
        let svc = service("demo", "0.1.0");
        let err = svc
            .initialized(json!({}))
            .await
            .expect_err("initialized before initialize should be rejected");
        assert!(err.to_string().contains("before initialize"));
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::AwaitingInitialize);
    }

    #[tokio::test]
    async fn list_tools_before_initialize_is_rejected() {
        let svc = service("demo", "0.1.0");
        let err = svc
            .list_tools(json!({}))
            .await
            .expect_err("tools/list before initialize should be rejected");
        assert!(err.to_string().contains("before initialize"));
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::AwaitingInitialize);
    }

    #[tokio::test]
    async fn list_tools_is_allowed_before_initialized_notification() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Tool::new("a").description("alpha"))
            .unwrap();
        let svc = service_with_registry("demo", "0.1.0", registry);
        initialize(&svc).await;
        assert_eq!(
            svc.lifecycle_state(),
            SessionLifecycle::AwaitingInitializedNotification
        );

        let result = svc
            .list_tools(json!({}))
            .await
            .expect("tools/list should be lenient after initialize");
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].name, "a");
    }

    #[tokio::test]
    async fn call_tool_before_initialize_is_rejected() {
        let svc = service("demo", "0.1.0");
        let err = svc
            .call_tool(call_params("a"))
            .await
            .expect_err("tools/call before initialize should be rejected");
        assert!(err.to_string().contains("before initialize"));
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::AwaitingInitialize);
    }

    #[tokio::test]
    async fn call_tool_before_initialized_notification_is_rejected() {
        let svc = service("demo", "0.1.0");
        initialize(&svc).await;
        let err = svc
            .call_tool(call_params("a"))
            .await
            .expect_err("tools/call before initialized notification should be rejected");
        assert!(err
            .to_string()
            .contains("before notifications/initialized"));
        assert_eq!(
            svc.lifecycle_state(),
            SessionLifecycle::AwaitingInitializedNotification
        );
    }

    #[tokio::test]
    async fn call_tool_dispatches_to_registered_tool() {
        let svc = service_with_add_tool();
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();

        let response = svc
            .call_tool(ToolsCallParams {
                name: "add".into(),
                arguments: json!({"a": 2.0, "b": 3.0}),
                meta: None,
            })
            .await
            .expect("dispatch should succeed");
        assert_eq!(response, ToolResponse::success("5"));
    }

    #[tokio::test]
    async fn call_tool_unknown_tool_returns_method_not_found() {
        let svc = service("demo", "0.1.0");
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        let err = svc
            .call_tool(call_params("missing"))
            .await
            .expect_err("unknown tool should map to method-not-found");
        assert!(err.to_string().contains("missing"));
        assert!(err.to_string().contains("method not found"));
    }

    #[tokio::test]
    async fn call_tool_propagates_invalid_params_from_handler() {
        let svc = service_with_add_tool();
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        let err = svc
            .call_tool(ToolsCallParams {
                name: "add".into(),
                arguments: json!({"a": "nope", "b": 1.0}),
                meta: None,
            })
            .await
            .expect_err("bad args should error");
        assert!(err.to_string().contains("invalid params"));
    }

    #[tokio::test]
    async fn call_tool_threads_progress_token_into_cx() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Tool::new("ping").handler(|_args: Value, cx| async move {
                cx.progress(0.5).total(1.0).message("half").emit();
                Ok::<_, McpfitError>("ok".to_string())
            }))
            .unwrap();
        let svc = service_with_registry("demo", "0.1.0", registry);
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();

        svc.call_tool(ToolsCallParams {
            name: "ping".into(),
            arguments: json!({}),
            meta: Some(json!({"progressToken": "tok"})),
        })
        .await
        .expect("dispatch should succeed");

        let notifications = svc.drain_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, "notifications/progress");
        assert_eq!(
            notifications[0].params,
            json!({
                "progressToken": "tok",
                "progress": 0.5,
                "total": 1.0,
                "message": "half",
            })
        );
    }

    #[tokio::test]
    async fn call_tool_without_progress_token_emits_no_notifications() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Tool::new("ping").handler(|_args: Value, cx| async move {
                cx.progress(0.5).emit();
                Ok::<_, McpfitError>("ok".to_string())
            }))
            .unwrap();
        let svc = service_with_registry("demo", "0.1.0", registry);
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();

        svc.call_tool(ToolsCallParams {
            name: "ping".into(),
            arguments: json!({}),
            meta: None,
        })
        .await
        .expect("dispatch should succeed");

        assert!(svc.drain_notifications().is_empty());
    }

    fn register_params(name: &str) -> ToolsRegisterParams {
        ToolsRegisterParams {
            name: name.into(),
            description: None,
            response_text: "ok".into(),
        }
    }

    fn assert_register_disabled(err: fittings::FittingsError) {
        let msg = err.to_string();
        assert!(msg.contains("method not found"), "unexpected: {msg}");
        assert!(msg.contains("disabled"), "unexpected: {msg}");
    }

    #[tokio::test]
    async fn register_tool_is_rejected_before_initialize() {
        let svc = service("demo", "0.1.0");
        let err = svc
            .register_tool(register_params("late"))
            .await
            .expect_err("tools/register should be disabled by default");
        assert_register_disabled(err);
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::AwaitingInitialize);
    }

    #[tokio::test]
    async fn register_tool_is_rejected_when_running() {
        let svc = service("demo", "0.1.0");
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        let err = svc
            .register_tool(register_params("late"))
            .await
            .expect_err("tools/register should be disabled by default");
        assert_register_disabled(err);
        assert_eq!(svc.registry.list().len(), 0);
    }

    #[tokio::test]
    async fn initialized_is_idempotent_when_running() {
        let svc = service("demo", "0.1.0");
        svc.initialize(InitializeParams {
            protocol_version: "2025-01-01".into(),
            client_info: None,
            capabilities: None,
        })
        .await
        .unwrap();
        svc.initialized(json!({})).await.unwrap();
        svc.initialized(json!({}))
            .await
            .expect("repeat initialized should be tolerated");
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::Running);
    }
}
