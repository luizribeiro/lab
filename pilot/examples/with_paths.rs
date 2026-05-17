//! Per-driver workspace overrides + per-turn options.
//!
//! Demonstrates two knobs you reach for when running pilot inside a
//! larger system (CI, test harness, sandboxed eval):
//!
//!   * `ClaudeConfig.additional_dirs` grants the agent read access to extra
//!     workspace directories beyond `Session::workdir`.
//!   * `TurnOptions.timeout` caps wall-clock per turn — the stream yields
//!     `Err(Error::Timeout(...))` if the agent stalls past the deadline.
//!
//! The third knob, `AgentPaths::config_home`, is shown commented-out
//! inside `main` because pointing `CLAUDE_CONFIG_DIR` at a fresh dir means
//! claude has no credentials there. See the inline comment for how to
//! enable it.
//!
//! Run with:
//!     cargo run --example with_paths

use futures_util::StreamExt;
use pilot::{Claude, ClaudeConfig, Driver, Event, Session, TurnItem, TurnOptions};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // An "extra" directory the agent can read but `Session::workdir`
    // doesn't include. Real use: grant the agent access to a vendor
    // directory, a generated artifact path, etc.
    let extra_dir = tempfile::tempdir()?;
    std::fs::write(
        extra_dir.path().join("README"),
        "hello from pilot example\n",
    )?;

    // Per-run isolated claude config dir is COMMENTED OUT below because
    // pointing `CLAUDE_CONFIG_DIR` at a fresh empty directory means claude
    // has no auth credentials there. To try it: set ANTHROPIC_API_KEY in
    // your environment (so `Auth::Ambient` picks it up inside the new
    // config home), or switch to `Auth::ApiKey(SecretString::from(...))`,
    // then uncomment the lines below.
    //
    //     let config_dir = tempfile::tempdir()?;
    //     paths: pilot::AgentPaths { config_home: Some(config_dir.path().to_path_buf()) },

    let mut config = ClaudeConfig::default();
    config.additional_dirs = vec![extra_dir.path().to_path_buf()];
    let driver: Arc<dyn Driver> = Arc::new(Claude::with_config(config));

    let mut session = Session::new(driver, std::env::current_dir()?);

    let prompt = format!(
        "Read the file at {}/README and tell me its contents in one short line.",
        extra_dir.path().display()
    );
    let mut opts = TurnOptions::default();
    opts.timeout = Some(Duration::from_secs(90));
    let mut stream = session.send(&prompt, opts).await?;

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
