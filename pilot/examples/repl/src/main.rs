//! Interactive REPL example for the pilot crate, built on ratatui's
//! inline viewport so the prompt stays visible while turns stream and
//! the terminal's native scrollback keeps working.
//!
//! Run with:
//!     cargo run -p pilot-repl -- --agent claude
//!     cargo run -p pilot-repl -- --agent claude --resume 6e7c…

mod app;
mod composer;
mod markdown;
mod transcript;
mod turn;
mod ui;

use std::panic;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use uuid::Uuid;

use crate::app::{AgentKind, App};

#[derive(Parser)]
#[command(
    name = "pilot-repl",
    about = "Interactive REPL example for the pilot crate"
)]
struct Args {
    /// Which agent CLI to drive
    #[arg(long, value_enum, default_value_t = AgentKindArg::Claude)]
    agent: AgentKindArg,

    /// Resume a previous session by UUID. The transcript at
    /// ~/.pilot/transcripts/<agent>-<uuid>.jsonl is replayed before
    /// the prompt is mounted.
    #[arg(long)]
    resume: Option<Uuid>,
}

#[derive(Copy, Clone, ValueEnum)]
enum AgentKindArg {
    Claude,
    Codex,
    Gemini,
    Pi,
}

impl From<AgentKindArg> for AgentKind {
    fn from(a: AgentKindArg) -> Self {
        match a {
            AgentKindArg::Claude => AgentKind::Claude,
            AgentKindArg::Codex => AgentKind::Codex,
            AgentKindArg::Gemini => AgentKind::Gemini,
            AgentKindArg::Pi => AgentKind::Pi,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let agent: AgentKind = args.agent.into();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));

    // Restore terminal state on panic so we don't leave the user's tty wedged.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut terminal = app::make_terminal(app::VIEWPORT_HEIGHT)?;

    let mut app = App::new(agent, &cwd, args.resume);
    app.boot(&mut terminal)?;
    let result = app.run(&mut terminal).await;

    // Best-effort cleanup: clear the viewport and disable raw mode so the
    // user's shell prompt comes back clean below our final output.
    let _ = terminal.clear();
    let _ = disable_raw_mode();
    app.print_resume_hint();

    result.map_err(Into::into)
}
