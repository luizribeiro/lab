use mcpfit::{tool, Result, Server};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize)]
pub struct EchoArgs {
    pub message: String,
}

#[derive(JsonSchema, Deserialize)]
pub struct AddArgs {
    pub a: f64,
    pub b: f64,
}

/// Echoes the provided message.
#[tool]
pub async fn echo(args: EchoArgs) -> Result<String> {
    Ok(args.message)
}

/// Adds two numbers and returns their sum.
#[tool]
pub async fn add(args: AddArgs) -> Result<f64> {
    Ok(args.a + args.b)
}

pub fn build_server() -> Server {
    Server::new("mcp-server", env!("CARGO_PKG_VERSION"))
        .tool(echo::TOOL)
        .tool(add::TOOL)
}

#[cfg(test)]
mod tests {
    use super::build_server;
    use fittings::serde_json::json;
    use mcpfit::{Cx, ToolResponse};

    #[test]
    fn mcpfit_server_lists_echo_and_add() {
        let server = build_server();
        let names: Vec<String> = server
            .registry()
            .list()
            .into_iter()
            .map(|info| info.name)
            .collect();
        assert_eq!(names, vec!["add", "echo"]);
    }

    #[tokio::test]
    async fn mcpfit_add_tool_returns_sum_text() {
        let server = build_server();
        let response = server
            .registry()
            .call("add", json!({"a": 2, "b": 3}), Cx::default())
            .await
            .expect("add should succeed");
        assert_eq!(response, ToolResponse::success("5"));
    }

    #[tokio::test]
    async fn mcpfit_echo_tool_returns_message_text() {
        let server = build_server();
        let response = server
            .registry()
            .call("echo", json!({"message": "hi"}), Cx::default())
            .await
            .expect("echo should succeed");
        assert_eq!(response, ToolResponse::success("hi"));
    }
}
