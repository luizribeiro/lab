//! Macro-first MCP server framework. See `mcpfit/plans/m0.md`.

mod content;
mod error;
mod response;

pub use content::{EmbeddedResource, ToolContent};
pub use error::McpfitError;
pub use response::ToolResponse;

pub type Result<T, E = McpfitError> = std::result::Result<T, E>;
