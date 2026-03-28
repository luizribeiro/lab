use std::process;

use mcp::{McpService, McpServiceImpl};

mod mcp;

#[tokio::main]
async fn main() {
    process::exit(McpServiceImpl.main().await);
}
