//! MCP service trait. Declares the JSON-RPC method surface implemented by
//! the mcpfit server.

use fittings::serde_json::Value;
use fittings::Result;

use crate::protocol::{
    InitializeParams, InitializeResult, ToolsCallParams, ToolsListResult, ToolsRegisterParams,
    ToolsRegisterResult,
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

#[cfg(test)]
mod tests {
    use super::mcp_service_schema;

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
}
