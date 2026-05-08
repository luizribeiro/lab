//! MCP service trait. Declares the JSON-RPC method surface implemented by
//! the mcpfit server.

use std::sync::Mutex;

use fittings::serde_json::Value;
use fittings::{FittingsError, Result};

use crate::protocol::{
    InitializeParams, InitializeResult, ServerCapabilities, ServerInfo, ToolsCallParams,
    ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
};
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
    lifecycle: Mutex<SessionLifecycle>,
}

#[allow(dead_code)]
impl McpServiceImpl {
    pub(crate) fn new(server_info: ServerInfo) -> Self {
        Self {
            server_info,
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
        Err(FittingsError::invalid_request(
            "notifications/initialized lifecycle not yet implemented",
        ))
    }

    async fn list_tools(&self, _params: Value) -> Result<ToolsListResult> {
        Err(FittingsError::invalid_request(
            "tools/list not yet implemented",
        ))
    }

    async fn call_tool(&self, _params: ToolsCallParams) -> Result<ToolResponse> {
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
    use crate::protocol::{InitializeParams, ServerInfo};

    fn service(name: &str, version: &str) -> McpServiceImpl {
        McpServiceImpl::new(ServerInfo {
            name: name.into(),
            version: version.into(),
        })
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
}
