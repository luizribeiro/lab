//! Macro-first MCP server framework. See `mcpfit/plans/m0.md`.

mod content;
mod error;
mod protocol;
mod response;
mod structured;

pub use content::{EmbeddedResource, ToolContent};
pub use error::McpfitError;
pub use protocol::{
    ClientInfo, InitializeParams, InitializeResult, ServerCapabilities, ServerInfo, ToolInfo,
    ToolsCallParams, ToolsCapability, ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
};
pub use response::{IntoToolResponse, ToolResponse};
pub use structured::{Structured, StructuredObject};

pub type Result<T, E = McpfitError> = std::result::Result<T, E>;
