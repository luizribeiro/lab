use std::process;

use mcpfit::Client;
use serde_json::json;

fn print_usage_and_exit() -> ! {
    eprintln!("usage: client <server-command>");
    process::exit(2);
}

#[tokio::main]
async fn main() {
    let server_command = match std::env::args().nth(1) {
        Some(value) => value,
        None => print_usage_and_exit(),
    };

    let exit_code = match run(&server_command).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("client error: {error}");
            1
        }
    };
    process::exit(exit_code);
}

async fn run(server_command: &str) -> mcpfit::Result<()> {
    let client = Client::spawn(server_command).await?;

    println!("tools:");
    for tool in client.list_tools().await? {
        println!("  - {}", tool.name);
    }

    let response = client
        .call_tool("echo", json!({ "message": "hello from mcpfit client" }))
        .await?;

    println!("echo response:");
    for content in &response.content {
        match content {
            mcpfit::ToolContent::Text { text, .. } => println!("  {text}"),
            other => println!("  {other:?}"),
        }
    }

    Ok(())
}
