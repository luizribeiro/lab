//! MCP service trait. Declares the JSON-RPC method surface implemented by
//! the mcpfit server.

use std::sync::Mutex;

use fittings::serde_json::Value;
use fittings::{FittingsError, Result};

use crate::protocol::{
    InitializeParams, InitializeResult, ServerCapabilities, ServerInfo, ToolsCallParams,
    ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
};
use crate::registry::ToolRegistry;
use crate::response::ToolResponse;

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
}

#[allow(dead_code)]
impl McpServiceImpl {
    pub(crate) fn new(server_info: ServerInfo, registry: ToolRegistry) -> Self {
        Self {
            server_info,
            registry,
            lifecycle: Mutex::new(SessionLifecycle::AwaitingInitialize),
        }
    }

    fn lifecycle_state(&self) -> SessionLifecycle {
        *self
            .lifecycle
            .lock()
            .expect("session lifecycle mutex should not be poisoned")
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

    async fn call_tool(&self, _params: ToolsCallParams) -> Result<ToolResponse> {
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
        Err(FittingsError::invalid_request(
            "tools/call not yet implemented",
        ))
    }

    async fn register_tool(&self, _params: ToolsRegisterParams) -> Result<ToolsRegisterResult> {
        Err(FittingsError::invalid_request(
            "tools/register not yet implemented",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{mcp_service_schema, McpService, McpServiceImpl, SessionLifecycle};
    use crate::protocol::{InitializeParams, ServerInfo, ToolsCallParams};
    use crate::registry::ToolRegistry;
    use crate::tool::Tool;
    use fittings::serde_json::{json, Value};

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
    async fn call_tool_passes_lifecycle_gate_when_running() {
        let svc = service("demo", "0.1.0");
        initialize(&svc).await;
        svc.initialized(json!({})).await.unwrap();
        let err = svc
            .call_tool(call_params("a"))
            .await
            .expect_err("tools/call dispatch is not yet implemented");
        assert!(err.to_string().contains("not yet implemented"));
        assert_eq!(svc.lifecycle_state(), SessionLifecycle::Running);
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
