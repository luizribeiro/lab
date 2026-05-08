//! Macro-first MCP server framework. See `mcpfit/plans/m0.md`.

mod content;
mod context;
mod error;
mod protocol;
mod response;
mod schema;
mod structured;
mod tool;

pub use content::{EmbeddedResource, ToolContent};
pub use context::Cx;
pub use error::McpfitError;
pub use protocol::{
    ClientInfo, InitializeParams, InitializeResult, ServerCapabilities, ServerInfo, ToolInfo,
    ToolsCallParams, ToolsCapability, ToolsListResult, ToolsRegisterParams, ToolsRegisterResult,
};
pub use response::{IntoToolResponse, ToolResponse};
pub use structured::{Structured, StructuredObject};
pub use tool::{Tool, ToolSpec};

pub type Result<T, E = McpfitError> = std::result::Result<T, E>;
