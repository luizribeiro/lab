use fittings::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializeParams {
    pub protocol_version: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub server_info: ServerInfo,
}

#[fittings::service]
pub trait McpService {
    /// Minimal MCP-style initialize handshake endpoint for demonstration.
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult>;
}

pub struct McpServiceImpl;

impl McpService for McpServiceImpl {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            protocol_version: params.protocol_version,
            server_info: ServerInfo {
                name: "fittings-mcp-example".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_schema_is_available() {
        let schema = mcp_service_schema();
        assert_eq!(schema.name, "mcp-service");
    }

    #[tokio::test]
    async fn initialize_echoes_protocol_and_sets_server_info() {
        let service = McpServiceImpl;
        let response = service
            .initialize(InitializeParams {
                protocol_version: "2024-11-05".to_string(),
            })
            .await
            .expect("initialize should succeed");

        assert_eq!(response.protocol_version, "2024-11-05");
        assert_eq!(response.server_info.name, "fittings-mcp-example");
        assert_eq!(response.server_info.version, env!("CARGO_PKG_VERSION"));
    }
}
