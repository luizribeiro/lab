//! Two turns on the same session. The second turn is automatically
//! dispatched through `Driver::resume_command()` (for drivers that need
//! distinct first-vs-resume flags).
//!
//! Run with:
//!     cargo run --example multi_turn -- claude
//!     cargo run --example multi_turn -- codex
//!     cargo run --example multi_turn -- gemini
//!     cargo run --example multi_turn -- pi

use futures_util::StreamExt;
use pilot::{Claude, Codex, Driver, Event, Gemini, Pi, Session, TurnItem, TurnOptions};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

async fn drain(session: &mut Session, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- turn: {prompt}");
    let mut opts = TurnOptions::default();
    opts.timeout = Some(Duration::from_secs(60));
    let mut stream = session.send(prompt, opts).await?;
    while let Some(item) = stream.next().await {
        match item? {
            TurnItem::Event(Event::AssistantText { delta }) => {
                print!("{delta}");
                std::io::stdout().flush().ok();
            }
            TurnItem::Complete(_) => println!(),
            _ => {}
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let agent = std::env::args()
        .nth(1)
        .expect("usage: multi_turn <claude|codex|gemini|pi>");
    let driver: Arc<dyn Driver> = match agent.as_str() {
        "claude" => Arc::new(Claude::new()),
        "codex" => Arc::new(Codex::new()),
        "gemini" => Arc::new(Gemini::new()),
        "pi" => Arc::new(Pi::new()),
        other => return Err(format!("unknown agent: {other}").into()),
    };
    let mut session = Session::new(driver, std::env::current_dir()?);

    drain(&mut session, "Pick a fruit.").await?;
    drain(&mut session, "Now name a color that goes with it.").await?;

    Ok(())
}
