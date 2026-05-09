use std::process;
use std::time::Duration;

use mcpfit::Client;
use serde_json::json;
use tokio::time::timeout;

fn print_usage_and_exit() -> ! {
    eprintln!("usage: client <server-command> [--progress]");
    process::exit(2);
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let server_command = match args.next() {
        Some(value) => value,
        None => print_usage_and_exit(),
    };
    let mut with_progress = false;
    for arg in args {
        match arg.as_str() {
            "--progress" => with_progress = true,
            _ => print_usage_and_exit(),
        }
    }

    let exit_code = match run(&server_command, with_progress).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("client error: {error}");
            1
        }
    };
    process::exit(exit_code);
}

async fn run(server_command: &str, with_progress: bool) -> mcpfit::Result<()> {
    let client = Client::spawn(server_command).await?;

    println!("tools:");
    for tool in client.list_tools().await? {
        println!("  - {}", tool.name);
    }

    let response = client
        .call_tool("echo", json!({ "message": "hello from mcpfit client" }))
        .await?;

    println!("echo response:");
    print_text_content(&response);

    if with_progress {
        let mut handle = client
            .call_tool_with_progress("progress_demo", json!({}))
            .start()
            .await?;
        println!("progress events:");
        loop {
            match timeout(Duration::from_millis(500), handle.progress().recv()).await {
                Ok(Some(event)) => {
                    let total = event
                        .total
                        .map(|t| format!("/{t}"))
                        .unwrap_or_default();
                    let message = event
                        .message
                        .as_deref()
                        .map(|m| format!(" — {m}"))
                        .unwrap_or_default();
                    println!("  {}{}{}", event.progress, total, message);
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        let response = handle.await?;
        println!("progress_demo response:");
        print_text_content(&response);
    }

    Ok(())
}

fn print_text_content(response: &mcpfit::ToolResponse) {
    for content in &response.content {
        match content {
            mcpfit::ToolContent::Text { text, .. } => println!("  {text}"),
            other => println!("  {other:?}"),
        }
    }
}
