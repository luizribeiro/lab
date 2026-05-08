//! Macro-first MCP server framework. See `mcpfit/plans/m0.md`.

extern crate self as mcpfit;

pub use mcpfit_macros::StructuredObject;

mod content;
mod context;
mod error;
mod protocol;
mod registry;
mod response;
mod schema;
mod server;
mod service;
mod structured;
mod tool;

pub use content::{EmbeddedResource, ToolContent};
pub use context::Cx;
pub use error::McpfitError;
pub use protocol::{
    ClientInfo, InitializeParams, InitializeResult, ProgressNotificationParams,
    ServerCapabilities, ServerInfo, ToolInfo, ToolsCallParams, ToolsCapability, ToolsListResult,
    ToolsRegisterParams, ToolsRegisterResult,
};
pub use registry::ToolRegistry;
pub use response::{IntoToolResponse, ToolResponse};
pub use server::{
    CancellationConfig, IntoTool, Server, MCP_CANCELLATION_METHOD,
    MCP_CANCELLATION_REQUEST_ID_FIELD,
};
pub use service::McpService;
pub use structured::{Structured, StructuredObject};
pub use tool::{Tool, ToolSpec};

pub type Result<T, E = McpfitError> = std::result::Result<T, E>;
