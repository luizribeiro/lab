//! Minimal pilot usage: send a one-word prompt, print streamed events.
//!
//! Run with:
//!     cargo run --example greeting -- claude
//!     cargo run --example greeting -- codex
//!     cargo run --example greeting -- gemini
//!     cargo run --example greeting -- pi

use futures_util::StreamExt;
use pilot::{Claude, Codex, Event, Gemini, Pi, Session, TurnItem, TurnOptions};
use std::io::Write;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = std::env::args()
        .nth(1)
        .expect("usage: greeting <claude|codex|gemini|pi>");

    let workdir = std::env::current_dir()?;
    let mut session = match agent.as_str() {
        "claude" => Session::new(Claude::new(), workdir),
        "codex" => Session::new(Codex::new(), workdir),
        "gemini" => Session::new(Gemini::new(), workdir),
        "pi" => Session::new(Pi::new(), workdir),
        other => return Err(format!("unknown agent: {other}").into()),
    };
    println!("session id: {}", session.id());

    let mut opts = TurnOptions::default();
    opts.timeout = Some(Duration::from_secs(60));
    let mut stream = session.send("Say only the word: hello", opts).await?;

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
