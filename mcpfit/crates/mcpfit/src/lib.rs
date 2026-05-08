//! Macro-first MCP server framework. See `mcpfit/plans/m0.md`.

mod error;

pub use error::McpfitError;

pub type Result<T, E = McpfitError> = std::result::Result<T, E>;
