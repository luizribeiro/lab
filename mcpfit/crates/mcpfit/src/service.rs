//! MCP service trait. Declares the JSON-RPC method surface implemented by
//! the mcpfit server.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use fittings::serde_json::{self, Value};
use fittings::{FittingsError, Result};

use crate::context::{extract_progress_token, Cx, ProgressSink};
use crate::error::McpfitError;
use crate::protocol::{
    InitializeParams, InitializeResult, ProgressNotificationParams, ServerCapabilities, ServerInfo,
    ToolsCallParams, ToolsCapability, ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
};
use crate::registry::ToolRegistry;
use crate::response::ToolResponse;
use crate::tool::Tool;

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
    registry: Mutex<ToolRegistry>,
    lifecycle: Mutex<SessionLifecycle>,
    pending_notifications: Arc<Mutex<Vec<ServerNotification>>>,
    allow_runtime_registration: bool,
    allow_dynamic_tools: bool,
    progress_enabled: AtomicBool,
}

#[allow(dead_code)]
impl McpServiceImpl {
    pub(crate) fn new(
        server_info: ServerInfo,
        registry: ToolRegistry,
        allow_runtime_registration: bool,
        allow_dynamic_tools: bool,
    ) -> Self {
        Self {
            server_info,
            registry: Mutex::new(registry),
            lifecycle: Mutex::new(SessionLifecycle::AwaitingInitialize),
            pending_notifications: Arc::new(Mutex::new(Vec::new())),
            allow_runtime_registration,
            allow_dynamic_tools,
            progress_enabled: AtomicBool::new(false),
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

    fn cx_for_call_with_cancellation(
        &self,
        meta: Option<&Value>,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Cx {
        let token = if self.progress_enabled.load(Ordering::Acquire) {
            extract_progress_token(meta)
        } else {
            None
        };
        match (token, cancellation) {
            (Some(token), Some(cancel)) => {
                Cx::with_progress_and_cancellation(token, self.progress_sink(), cancel)
            }
            (Some(token), None) => Cx::with_progress(token, self.progress_sink()),
            (None, Some(cancel)) => Cx::with_external_cancellation(cancel),
            (None, None) => Cx::default(),
        }
    }

    fn progress_sink(&self) -> ProgressSink {
        let pending = Arc::clone(&self.pending_notifications);
        Arc::new(move |params: ProgressNotificationParams| {
            let serialized = serde_json::to_value(&params)
                .expect("progress notification params should serialize");
            pending
                .lock()
                .expect("pending notifications mutex should not be poisoned")
                .push(ServerNotification {
                    method: "notifications/progress".to_string(),
                    params: serialized,
                });
        })
    }

    pub(crate) async fn dispatch(
        &self,
        method: &str,
        params: Value,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<Value> {
        match method {
            "initialize" => {
                let p: InitializeParams = serde_json::from_value(params)
                    .map_err(|e| FittingsError::invalid_params(e.to_string()))?;
                if client_progress_enabled(p.capabilities.as_ref()) {
                    self.progress_enabled.store(true, Ordering::Release);
                }
                let result = self.initialize(p).await?;
                serde_json::to_value(result)
                    .map_err(|e| FittingsError::internal(e.to_string()))
            }
            "notifications/initialized" => self.initialized(params).await,
            "tools/list" => {
                let result = self.list_tools(params).await?;
                serde_json::to_value(result)
                    .map_err(|e| FittingsError::internal(e.to_string()))
            }
            "tools/call" => {
                let p: ToolsCallParams = serde_json::from_value(params)
                    .map_err(|e| FittingsError::invalid_params(e.to_string()))?;
                let result = self.call_tool_with_cancellation(p, cancellation).await?;
                serde_json::to_value(result)
                    .map_err(|e| FittingsError::internal(e.to_string()))
            }
            "tools/register" => {
                let p: ToolsRegisterParams = serde_json::from_value(params)
                    .map_err(|e| FittingsError::invalid_params(e.to_string()))?;
                let result = self.register_tool(p).await?;
                serde_json::to_value(result)
                    .map_err(|e| FittingsError::internal(e.to_string()))
            }
            other => Err(FittingsError::method_not_found(format!(
                "unknown method: {other}"
            ))),
        }
    }

    async fn call_tool_with_cancellation(
        &self,
        params: ToolsCallParams,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<ToolResponse> {
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
        let cx = self.cx_for_call_with_cancellation(params.meta.as_ref(), cancellation);
        let handler = self
            .registry
            .lock()
            .expect("registry mutex should not be poisoned")
            .handler_for(&params.name);
        match handler {
            Some(handler) => handler(params.arguments, cx).await.map_err(to_fittings_error),
            None => Err(FittingsError::method_not_found(format!(
                "unknown tool: {}",
                params.name
            ))),
        }
    }
}

fn client_progress_enabled(capabilities: Option<&Value>) -> bool {
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
        })
        .unwrap_or(false)
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

        let capabilities = if self.allow_runtime_registration || self.allow_dynamic_tools {
            ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
            }
        } else {
            ServerCapabilities::default()
        };

        Ok(InitializeResult {
            protocol_version: params.protocol_version,
            capabilities,
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
        let tools = self
            .registry
            .lock()
            .expect("registry mutex should not be poisoned")
            .list();
        Ok(ToolsListResult { tools })
    }

    async fn call_tool(&self, params: ToolsCallParams) -> Result<ToolResponse> {
        self.call_tool_with_cancellation(params, None).await
    }

    async fn register_tool(&self, params: ToolsRegisterParams) -> Result<ToolsRegisterResult> {
        if !self.allow_runtime_registration {
            return Err(FittingsError::method_not_found(
                "tools/register is disabled; enable runtime registration on the server to use it",
            ));
        }
        if self.lifecycle_state() != SessionLifecycle::Running {
            return Err(FittingsError::invalid_request(
                "tools/register requires an initialized session",
            ));
        }
        if params.name.trim().is_empty() {
            return Err(FittingsError::invalid_params(
                "tools/register requires a non-empty `name`",
            ));
        }
        let tool = build_runtime_tool(&params);
        let info = tool.to_info();
        self.registry
            .lock()
            .expect("registry mutex should not be poisoned")
            .register(tool)
            .map_err(to_fittings_error)?;
        self.pending_notifications
            .lock()
            .expect("pending notifications mutex should not be poisoned")
            .push(ServerNotification {
                method: "notifications/tools/list_changed".to_string(),
                params: Value::Object(Default::default()),
            });
        Ok(ToolsRegisterResult { tool: info })
    }
}

fn build_runtime_tool(params: &ToolsRegisterParams) -> Tool {
    let response_text = params.response_text.clone();
    let mut tool = Tool::new(params.name.clone()).handler(move |_args: Value, _cx: Cx| {
        let text = response_text.clone();
        async move { Ok::<_, McpfitError>(text) }
    });
    if let Some(desc) = &params.description {
        tool = tool.description(desc.clone());
    }
    tool
}

#[cfg(test)]
mod tests {
    use super::{mcp_service_schema, McpService, McpServiceImpl, Ordering, SessionLifecycle};
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
        service_with_options(name, version, registry, false)
    }

    fn service_with_options(
        name: &str,
        version: &str,
        registry: ToolRegistry,
        allow_runtime_registration: bool,
    ) -> McpServiceImpl {
        service_with_flags(name, version, registry, allow_runtime_registration, false)
    }

    fn service_with_flags(
        name: &str,
        version: &str,
        registry: ToolRegistry,
        allow_runtime_registration: bool,
        allow_dynamic_tools: bool,
    ) -> McpServiceImpl {
        McpServiceImpl::new(
            ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            registry,
            allow_runtime_registration,
            allow_dynamic_tools,
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
        svc.progress_enabled.store(true, Ordering::Release);
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
    async fn initialize_does_not_advertise_list_changed_by_default() {
        let svc = service("demo", "0.1.0");
        let result = svc
            .initialize(InitializeParams {
                protocol_version: "2025-01-01".into(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");

        assert!(result.capabilities.tools.is_none());
    }

    #[tokio::test]
    async fn initialize_advertises_list_changed_when_runtime_registration_enabled() {
        let svc = service_with_options("demo", "0.1.0", ToolRegistry::new(), true);
        let result = svc
            .initialize(InitializeParams {
                protocol_version: "2025-01-01".into(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");

        let tools = result
            .capabilities
            .tools
            .expect("tools capability should be advertised");
        assert_eq!(tools.list_changed, Some(true));
    }

    #[tokio::test]
    async fn initialize_advertises_list_changed_for_dynamic_tools_knob() {
        let svc = service_with_flags("demo", "0.1.0", ToolRegistry::new(), false, true);
        let result = svc
            .initialize(InitializeParams {
                protocol_version: "2025-01-01".into(),
                client_info: None,
                capabilities: None,
            })
            .await
            .expect("initialize should succeed");

        let tools = result
            .capabilities
            .tools
            .expect("tools capability should be advertised");
        assert_eq!(tools.list_changed, Some(true));
    }

    #[tokio::test]
    async fn dynamic_tools_knob_does_not_expose_register_method() {
        let svc = service_with_flags("demo", "0.1.0", ToolRegistry::new(), false, true);
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        let err = svc
            .register_tool(register_params("ping"))
            .await
            .expect_err("dynamic-tools knob must not enable client tools/register");
        assert_register_disabled(err);
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
        assert_eq!(svc.registry.lock().unwrap().list().len(), 0);
    }

    #[tokio::test]
    async fn register_tool_when_enabled_requires_running_state() {
        let svc = service_with_options("demo", "0.1.0", ToolRegistry::new(), true);
        let err = svc
            .register_tool(register_params("late"))
            .await
            .expect_err("tools/register before initialize should be rejected");
        assert!(err.to_string().contains("invalid request"));
        assert!(err.to_string().contains("requires an initialized session"));
        assert_eq!(svc.registry.lock().unwrap().list().len(), 0);

        initialize(&svc).await;
        let err = svc
            .register_tool(register_params("late"))
            .await
            .expect_err("tools/register before initialized notification should be rejected");
        assert!(err
            .to_string()
            .contains("requires an initialized session"));
    }

    #[tokio::test]
    async fn register_tool_when_enabled_adds_callable_tool() {
        let svc = service_with_options("demo", "0.1.0", ToolRegistry::new(), true);
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();

        let result = svc
            .register_tool(ToolsRegisterParams {
                name: "ping".into(),
                description: Some("static responder".into()),
                response_text: "pong".into(),
            })
            .await
            .expect("registration should succeed when enabled and running");
        assert_eq!(result.tool.name, "ping");
        assert_eq!(result.tool.description.as_deref(), Some("static responder"));

        let list = svc
            .list_tools(json!({}))
            .await
            .expect("tools/list should include the new tool");
        assert_eq!(list.tools.len(), 1);
        assert_eq!(list.tools[0].name, "ping");

        let response = svc
            .call_tool(call_params("ping"))
            .await
            .expect("registered tool should be callable");
        assert_eq!(response, ToolResponse::success("pong"));
    }

    #[tokio::test]
    async fn register_tool_emits_list_changed_notification_on_success() {
        let svc = service_with_options("demo", "0.1.0", ToolRegistry::new(), true);
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        svc.register_tool(register_params("ping"))
            .await
            .expect("registration should succeed");

        let notifications = svc.drain_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, "notifications/tools/list_changed");
        assert_eq!(notifications[0].params, json!({}));
    }

    #[tokio::test]
    async fn register_tool_does_not_notify_when_rejected() {
        let svc = service_with_options("demo", "0.1.0", ToolRegistry::new(), true);
        let _ = svc.register_tool(register_params("late")).await;
        assert!(svc.drain_notifications().is_empty());

        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        svc.register_tool(register_params("ping")).await.unwrap();
        let _ = svc.drain_notifications();
        let _ = svc.register_tool(register_params("ping")).await;
        assert!(svc.drain_notifications().is_empty());
    }

    #[tokio::test]
    async fn register_tool_rejects_duplicates_when_enabled() {
        let svc = service_with_options("demo", "0.1.0", ToolRegistry::new(), true);
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        svc.register_tool(register_params("ping")).await.unwrap();
        let err = svc
            .register_tool(register_params("ping"))
            .await
            .expect_err("duplicate registration should fail");
        assert!(err.to_string().contains("invalid request"));
        assert!(err.to_string().contains("ping"));
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
