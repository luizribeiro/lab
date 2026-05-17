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
    // Per-run temp dirs so concurrent runs don't share state.
    let config_dir = tempfile::tempdir()?;
    let extra_dir = tempfile::tempdir()?;
    std::fs::write(
        extra_dir.path().join("README"),
        "hello from pilot example\n",
    )?;

    let driver: Arc<dyn Driver> = Arc::new(Claude::with_config(ClaudeConfig {
        paths: AgentPaths {
            config_home: Some(config_dir.path().to_path_buf()),
        },
        additional_dirs: vec![extra_dir.path().to_path_buf()],
        ..Default::default()
    }));

    let mut session = Session::new(driver, std::env::current_dir()?);

    // 3. Per-turn timeout cap.
    let prompt = format!(
        "Read the file at {}/README and tell me its contents in one short line.",
        extra_dir.path().display()
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
