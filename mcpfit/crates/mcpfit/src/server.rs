//! Top-level server builder.

use crate::protocol::ServerInfo;
use crate::registry::ToolRegistry;
use crate::tool::{Tool, ToolSpec};
use crate::Result;

pub const MCP_CANCELLATION_METHOD: &str = "notifications/cancelled";
pub const MCP_CANCELLATION_REQUEST_ID_FIELD: &str = "requestId";

/// Names the JSON-RPC method and `params` field that the transport reads to
/// cancel an in-flight request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationConfig {
    pub method: &'static str,
    pub request_id_field: &'static str,
}

impl CancellationConfig {
    pub const fn mcp() -> Self {
        Self {
            method: MCP_CANCELLATION_METHOD,
            request_id_field: MCP_CANCELLATION_REQUEST_ID_FIELD,
        }
    }
}

impl Default for CancellationConfig {
    fn default() -> Self {
        Self::mcp()
    }
}

/// Builder for an MCP server. Owns the canonical [`ServerInfo`] returned in
/// `initialize` responses and the [`ToolRegistry`] populated via [`Server::tool`].
pub struct Server {
    info: ServerInfo,
    registry: ToolRegistry,
}

impl Server {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            info: ServerInfo {
                name: name.into(),
                version: version.into(),
            },
            registry: ToolRegistry::new(),
        }
    }

    /// Registers a tool. Panics on duplicate names — duplicate registration is
    /// a programmer error in static server construction. Use [`Server::try_tool`]
    /// when registering tools whose names are not known at compile time.
    pub fn tool(self, tool: impl IntoTool) -> Self {
        self.try_tool(tool).expect("tool registration failed")
    }

    /// Returns `Err` on duplicate tool name instead of panicking.
    pub fn try_tool(mut self, tool: impl IntoTool) -> Result<Self> {
        self.registry.register(tool.into_tool())?;
        Ok(self)
    }

    pub fn server_info(&self) -> &ServerInfo {
        &self.info
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}

/// Conversion into a [`Tool`] for registration on a [`Server`].
pub trait IntoTool {
    fn into_tool(self) -> Tool;
}

impl IntoTool for Tool {
    fn into_tool(self) -> Tool {
        self
    }
}

impl IntoTool for ToolSpec {
    fn into_tool(self) -> Tool {
        self.build()
    }
}

impl IntoTool for &ToolSpec {
    fn into_tool(self) -> Tool {
        self.build()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CancellationConfig, Server, MCP_CANCELLATION_METHOD, MCP_CANCELLATION_REQUEST_ID_FIELD,
    };
    use crate::tool::{Tool, ToolSpec};

    #[test]
    fn cancellation_constants_match_mcp_wire_strings() {
        assert_eq!(MCP_CANCELLATION_METHOD, "notifications/cancelled");
        assert_eq!(MCP_CANCELLATION_REQUEST_ID_FIELD, "requestId");
        let config = CancellationConfig::default();
        assert_eq!(config.method, MCP_CANCELLATION_METHOD);
        assert_eq!(config.request_id_field, MCP_CANCELLATION_REQUEST_ID_FIELD);
    }

    #[test]
    fn new_records_server_info() {
        let server = Server::new("demo", "1.2.3");
        assert_eq!(server.server_info().name, "demo");
        assert_eq!(server.server_info().version, "1.2.3");
        assert!(server.registry().is_empty());
    }

    #[test]
    fn tool_registers_tool_value() {
        let server = Server::new("demo", "0.1.0").tool(Tool::new("a"));
        assert!(server.registry().contains("a"));
        assert_eq!(server.registry().len(), 1);
    }

    #[test]
    fn tool_registers_tool_spec_by_reference() {
        const SPEC: ToolSpec = ToolSpec::new("from_spec", || Tool::new("from_spec"));
        let server = Server::new("demo", "0.1.0").tool(&SPEC);
        assert!(server.registry().contains("from_spec"));
    }

    #[test]
    fn tool_registers_owned_tool_spec() {
        let spec = ToolSpec::new("owned_spec", || Tool::new("owned_spec"));
        let server = Server::new("demo", "0.1.0").tool(spec);
        assert!(server.registry().contains("owned_spec"));
    }

    #[test]
    fn try_tool_returns_error_on_duplicate() {
        let server = Server::new("demo", "0.1.0").tool(Tool::new("dup"));
        match server.try_tool(Tool::new("dup")) {
            Err(crate::McpfitError::InvalidRequest(_)) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected duplicate registration to fail"),
        }
    }

    #[test]
    #[should_panic(expected = "tool registration failed")]
    fn tool_panics_on_duplicate() {
        let _ = Server::new("demo", "0.1.0")
            .tool(Tool::new("dup"))
            .tool(Tool::new("dup"));
    }
}
