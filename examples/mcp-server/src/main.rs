use std::process;

use fittings::serde_json::{json, Value};
use fittings::{FittingsError, Result};
use mcp::{McpService, McpServiceImpl, ToolRegistry, ToolsCallResult};

mod mcp;

fn register_echo_tool(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(
        "echo",
        "Echoes the provided message",
        json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"],
            "additionalProperties": false
        }),
        |arguments| {
            let message = arguments
                .get("message")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    FittingsError::invalid_params("`arguments.message` must be a string")
                })?;

            Ok(ToolsCallResult::text(message))
        },
    )
}

fn register_add_tool(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(
        "add",
        "Adds two numbers and returns their sum",
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "number" },
                "b": { "type": "number" }
            },
            "required": ["a", "b"],
            "additionalProperties": false
        }),
        |arguments| {
            let a = arguments
                .get("a")
                .and_then(Value::as_f64)
                .ok_or_else(|| FittingsError::invalid_params("`arguments.a` must be a number"))?;
            let b = arguments
                .get("b")
                .and_then(Value::as_f64)
                .ok_or_else(|| FittingsError::invalid_params("`arguments.b` must be a number"))?;

            Ok(ToolsCallResult::text((a + b).to_string()))
        },
    )
}

fn build_service() -> McpServiceImpl {
    let mut registry = ToolRegistry::new();
    register_echo_tool(&mut registry).expect("example tool registration should succeed");
    register_add_tool(&mut registry).expect("example tool registration should succeed");
    McpServiceImpl::new(registry)
}

#[tokio::main]
async fn main() {
    process::exit(build_service().main().await);
}

#[cfg(test)]
mod tests {
    use super::build_service;
    use crate::mcp::{McpService, ToolContent, ToolsCallParams};
    use fittings::serde_json::json;
    use fittings::FittingsError;

    #[tokio::test]
    async fn example_binary_service_registers_echo_and_add_tools() {
        let service = build_service();

        let listed = service
            .list_tools(fittings::serde_json::Value::Null)
            .await
            .expect("tools/list should succeed");

        let tool_names: Vec<_> = listed.tools.iter().map(|tool| tool.name.as_str()).collect();
        assert_eq!(tool_names, vec!["add", "echo"]);
    }

    #[tokio::test]
    async fn add_tool_returns_sum_text() {
        let service = build_service();

        let called = service
            .call_tool(ToolsCallParams {
                name: "add".to_string(),
                arguments: json!({"a": 2, "b": 3}),
            })
            .await
            .expect("tools/call should succeed");

        assert!(matches!(
            &called.content[0],
            ToolContent::Text { text } if text == "5"
        ));
    }

    #[tokio::test]
    async fn add_tool_rejects_invalid_arguments() {
        let service = build_service();

        let invalid = service
            .call_tool(ToolsCallParams {
                name: "add".to_string(),
                arguments: json!({"a": "x", "b": 1}),
            })
            .await;

        assert!(matches!(invalid, Err(FittingsError::InvalidParams(_))));
    }
}
