//! Path overrides + per-turn options.
//!
//! This example shows the knobs you reach for when running pilot inside a
//! larger system (CI, test harness, sandboxed eval):
//!
//!   * `AgentPaths::config_home` isolates the claude CLI's config/state to
//!     a dedicated directory instead of `~/.claude` — useful when you don't
//!     want pilot runs to share session history with your interactive claude.
//!   * `ClaudeConfig.additional_dirs` grants the agent read access to extra
//!     workspace directories beyond `Session::workdir`.
//!   * `TurnOptions.timeout` caps wall-clock per turn — the stream yields
//!     `Err(Error::Timeout(...))` if the agent stalls past the deadline.
//!
//! Run with:
//!     cargo run --example with_paths

use futures_util::StreamExt;
use pilot::{AgentPaths, Claude, ClaudeConfig, Driver, Event, Session, TurnItem, TurnOptions};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Isolated claude config dir — no shared state with `~/.claude`.
    let config_dir = std::env::temp_dir().join("pilot-example-claude-home");
    std::fs::create_dir_all(&config_dir)?;

    // 2. An "extra" directory the agent can read but `Session::workdir`
    //    doesn't include. Real use: grant the agent access to a vendor
    //    directory, a generated artifact path, etc.
    let extra_dir = std::env::temp_dir().join("pilot-example-extra-dir");
    std::fs::create_dir_all(&extra_dir)?;
    // Seed a file so there's something to find.
    std::fs::write(extra_dir.join("README"), "hello from pilot example\n")?;

    let driver: Arc<dyn Driver> = Arc::new(Claude::with_config(ClaudeConfig {
        paths: AgentPaths {
            config_home: Some(config_dir),
        },
        additional_dirs: vec![extra_dir.clone()],
        ..Default::default()
    }));

    let mut session = Session::new(driver, std::env::current_dir()?);

    // 3. Per-turn timeout cap.
    let prompt = format!(
        "Read the file at {}/README and tell me its contents in one short line.",
        extra_dir.display()
    );
    let mut stream = session
        .send(
            &prompt,
            TurnOptions {
                timeout: Some(Duration::from_secs(90)),
                ..Default::default()
            },
        )
        .await?;

    while let Some(item) = stream.next().await {
        match item? {
            TurnItem::Event(Event::AssistantText { delta }) => {
                print!("{delta}");
                std::io::stdout().flush().ok();
            }
            TurnItem::Event(Event::ToolCall { name, .. }) => {
                eprintln!("\n[tool: {name}]");
            }
            TurnItem::Complete(_) => println!(),
            _ => {}
        }
    }
    Ok(())
}
