use mcpfit::{Result, Server, tool};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize)]
struct AddArgs {
    a: f64,
    b: f64,
}

/// Adds two numbers.
#[tool]
async fn add(args: AddArgs) -> Result<f64> {
    Ok(args.a + args.b)
}

fn main() {
    let _ = Server::new("demo", "0.1.0").tool(add::TOOL);
    assert_eq!(add::TOOL.name(), "add");
    assert_eq!(add::TOOL.description(), Some("Adds two numbers."));
}
