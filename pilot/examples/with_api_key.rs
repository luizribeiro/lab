//! Explicit API key auth via `Auth::ApiKey(SecretString)`. Demonstrated
//! with the Claude driver; the Gemini and Pi drivers follow the same
//! pattern with their respective `*Config` types.
//!
//! Run with:
//!     PILOT_AGENT_KEY=sk-... cargo run --example with_api_key

use futures_util::StreamExt;
use pilot::{Auth, Claude, ClaudeConfig, Driver, Session, TurnItem, TurnOptions};
use secrecy::SecretString;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key =
        std::env::var("PILOT_AGENT_KEY").expect("set PILOT_AGENT_KEY to your provider API key");

    let driver: Arc<dyn Driver> = Arc::new(Claude::with_config(ClaudeConfig {
        auth: Auth::ApiKey(SecretString::from(key)),
        ..Default::default()
    }));

    let mut session = Session::new(driver, std::env::current_dir()?);
    let mut stream = session
        .send(
            "Say only the word: hi",
            TurnOptions {
                timeout: Some(Duration::from_secs(60)),
                ..Default::default()
            },
        )
        .await?;

    while let Some(item) = stream.next().await {
        if let TurnItem::Complete(turn) = item? {
            println!("complete: {} events", turn.events.len());
        }
    }
    Ok(())
}
