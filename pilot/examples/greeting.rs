//! Minimal pilot usage: send a one-word prompt, print streamed events.
//!
//! Run with:
//!     cargo run --example greeting -- claude
//!     cargo run --example greeting -- codex
//!     cargo run --example greeting -- gemini
//!     cargo run --example greeting -- pi

use futures_util::StreamExt;
use pilot::{Event, Session, TurnItem, TurnOptions};
use std::io::Write;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = std::env::args()
        .nth(1)
        .expect("usage: greeting <claude|codex|gemini|pi>");

    let driver = pilot::driver(&agent)?;
    let mut session = Session::new(driver, std::env::current_dir()?);
    println!("session id: {}", session.id());

    let mut stream = session
        .send(
            "Say only the word: hello",
            TurnOptions {
                timeout: Some(Duration::from_secs(60)),
                ..Default::default()
            },
        )
        .await?;

    while let Some(item) = stream.next().await {
        match item? {
            TurnItem::Event(Event::AssistantText { delta }) => {
                print!("{delta}");
                // Flush so streamed text appears live, not after the "[complete]" line.
                std::io::stdout().flush().ok();
            }
            TurnItem::Event(Event::Usage {
                input_tokens,
                output_tokens,
            }) => {
                eprintln!("\n[usage: {input_tokens} in / {output_tokens} out]");
            }
            TurnItem::Event(Event::ToolCall { name, .. }) => {
                eprintln!("\n[tool call: {name}]");
            }
            TurnItem::Complete(turn) => {
                eprintln!("\n[complete: {} events]", turn.events.len());
            }
            _ => {}
        }
    }
    println!();
    Ok(())
}
