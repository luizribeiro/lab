use std::process;
use std::thread;
use std::time::Duration;

use fittings::serde_json::{json, Value};
use fittings::{FittingsError, Result};
use mcp::{McpServiceImpl, ToolRegistry, ToolsCallResult};

mod mcp;
mod mcpfit_example;

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
        |arguments, _context| {
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
        |arguments, _context| {
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

fn register_add_with_details_tool(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(
        "add_with_details",
        "Adds two numbers and returns both text and structured details",
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "number" },
                "b": { "type": "number" }
            },
            "required": ["a", "b"],
            "additionalProperties": false
        }),
        |arguments, _context| {
            let a = arguments
                .get("a")
                .and_then(Value::as_f64)
                .ok_or_else(|| FittingsError::invalid_params("`arguments.a` must be a number"))?;
            let b = arguments
                .get("b")
                .and_then(Value::as_f64)
                .ok_or_else(|| FittingsError::invalid_params("`arguments.b` must be a number"))?;
            let sum = a + b;

            ToolsCallResult::text_with_structured(
                format!("{a} + {b} = {sum}"),
                json!({
                    "a": a,
                    "b": b,
                    "sum": sum
                }),
            )
        },
    )
}

fn register_long_running_demo_tool(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(
        "long_running_demo",
        "Long-running tool that supports notifications/cancelled",
        json!({
            "type": "object",
            "additionalProperties": false
        }),
        |_arguments, context| {
            for _ in 0..60 {
                if context.is_cancelled() {
                    return Err(FittingsError::invalid_request("tool call cancelled"));
                }
                thread::sleep(Duration::from_millis(50));
            }

            Ok(ToolsCallResult::text("long running completed"))
        },
    )
}

fn register_progress_demo_tool(registry: &mut ToolRegistry) -> Result<()> {
    registry.register(
        "progress_demo",
        "Long-running tool that emits notifications/progress when requested",
        json!({
            "type": "object",
            "additionalProperties": false
        }),
        |_arguments, context| {
            let total_steps = 3;
            for step in 1..=total_steps {
                if context.is_cancelled() {
                    return Err(FittingsError::invalid_request("tool call cancelled"));
                }

                context.emit_progress(
                    step as f64,
                    Some(total_steps as f64),
                    Some(format!("progress step {step}/{total_steps}")),
                );
                thread::sleep(Duration::from_millis(50));
            }

            Ok(ToolsCallResult::text("progress demo completed"))
        },
    )
}

fn build_service() -> McpServiceImpl {
    let mut registry = ToolRegistry::new();
    register_echo_tool(&mut registry).expect("example tool registration should succeed");
    register_add_tool(&mut registry).expect("example tool registration should succeed");
    register_add_with_details_tool(&mut registry)
        .expect("example tool registration should succeed");
    register_long_running_demo_tool(&mut registry)
        .expect("example tool registration should succeed");
    register_progress_demo_tool(&mut registry).expect("example tool registration should succeed");
    McpServiceImpl::new(registry)
}

#[tokio::main]
async fn main() {
    let exit_code = match mcpfit_example::build_server().run_entrypoint().await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mcp-server serve error: {error}");
            1
        }
    };
    process::exit(exit_code);
}
