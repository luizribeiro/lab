use std::process;
use std::time::Duration;

use mcpfit::{tool, Cx, Result, Server, Structured, StructuredObject};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

#[derive(JsonSchema, Deserialize)]
pub struct EchoArgs {
    pub message: String,
}

#[derive(JsonSchema, Deserialize)]
pub struct AddArgs {
    pub a: f64,
    pub b: f64,
}

#[derive(JsonSchema, Deserialize)]
pub struct WaitArgs {
    pub duration_ms: u64,
}

#[derive(JsonSchema, Deserialize)]
pub struct ProgressArgs {
    pub steps: u32,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema, StructuredObject)]
pub struct AddOut {
    pub a: f64,
    pub b: f64,
    pub sum: f64,
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

/// Adds two numbers and returns both text and structured details.
#[tool]
pub async fn add_with_details(args: AddArgs) -> Result<Structured<AddOut>> {
    let sum = args.a + args.b;
    let text = format!("{} + {} = {}", args.a, args.b, sum);
    Ok(Structured::new(AddOut {
        a: args.a,
        b: args.b,
        sum,
    })
    .with_text(text))
}

/// Sleeps for the requested duration, honouring cancellation between steps.
#[tool]
pub async fn wait(args: WaitArgs, cx: Cx) -> Result<String> {
    const STEP_MS: u64 = 25;
    let mut remaining = args.duration_ms;
    while remaining > 0 {
        cx.check_cancelled()?;
        let step = remaining.min(STEP_MS);
        sleep(Duration::from_millis(step)).await;
        remaining -= step;
    }
    cx.check_cancelled()?;
    Ok(format!("waited {} ms", args.duration_ms))
}

/// Reports progress for the requested number of steps, then returns.
#[tool]
pub async fn progress_demo(args: ProgressArgs, cx: Cx) -> Result<String> {
    let total = f64::from(args.steps);
    for step in 1..=args.steps {
        cx.progress(f64::from(step))
            .total(total)
            .message(format!("step {step} of {}", args.steps))
            .emit();
    }
    Ok(format!("completed {} steps", args.steps))
}

pub fn build_server() -> Server {
    Server::new("mcp-server", env!("CARGO_PKG_VERSION"))
        .tool(echo::TOOL)
        .tool(add::TOOL)
        .tool(add_with_details::TOOL)
        .tool(wait::TOOL)
        .tool(progress_demo::TOOL)
}

#[tokio::main]
async fn main() {
    let exit_code = match build_server().run_entrypoint().await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("mcp-server serve error: {error}");
            1
        }
    };
    process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::build_server;
    use fittings::serde_json::json;
    use mcpfit::{Cx, ToolContent, ToolResponse};

    #[test]
    fn mcpfit_server_lists_echo_and_add() {
        let server = build_server();
        let names: Vec<String> = server
            .registry()
            .list()
            .into_iter()
            .map(|info| info.name)
            .collect();
        assert_eq!(
            names,
            vec!["add", "add_with_details", "echo", "progress_demo", "wait"]
        );
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
    async fn mcpfit_add_with_details_returns_structured_response() {
        let server = build_server();
        let response = server
            .registry()
            .call(
                "add_with_details",
                json!({"a": 2, "b": 3}),
                Cx::default(),
            )
            .await
            .expect("add_with_details should succeed");
        assert_eq!(
            response,
            ToolResponse {
                content: vec![ToolContent::text("2 + 3 = 5")],
                structured_content: Some(json!({"a": 2.0, "b": 3.0, "sum": 5.0})),
                is_error: false,
            }
        );
    }

    #[tokio::test]
    async fn mcpfit_wait_tool_returns_waited_text() {
        let server = build_server();
        let response = server
            .registry()
            .call("wait", json!({"duration_ms": 10}), Cx::default())
            .await
            .expect("wait should succeed");
        assert_eq!(response, ToolResponse::success("waited 10 ms"));
    }

    #[tokio::test]
    async fn mcpfit_progress_demo_returns_completion_text() {
        let server = build_server();
        let response = server
            .registry()
            .call("progress_demo", json!({"steps": 3}), Cx::default())
            .await
            .expect("progress_demo should succeed");
        assert_eq!(response, ToolResponse::success("completed 3 steps"));
    }
}
