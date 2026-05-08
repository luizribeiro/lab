use mcpfit::{Cx, Result, Server, tool};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize)]
struct NapArgs {
    millis: u64,
}

/// Sleeps then returns.
#[tool]
async fn nap(args: NapArgs, cx: Cx) -> Result<String> {
    cx.check_cancelled()?;
    Ok(format!("slept {}ms", args.millis))
}

fn main() {
    let _ = Server::new("demo", "0.1.0").tool(nap::TOOL);
    assert_eq!(nap::TOOL.name(), "nap");
    assert_eq!(nap::TOOL.description(), Some("Sleeps then returns."));
}
