//! Orb is a terminal workspace for agentic coding sessions, built on
//! pilot and ratatui's inline viewport.
//!
//! Run with:
//!     cargo run -- --agent claude
//!     cargo run -- --agent claude --resume 6e7c...

mod agent;
mod app;
mod commands;
mod composer;
mod help_modal;
mod markdown;
mod modal;
mod transcript;
mod turn;
mod ui;
mod utils;

use std::io;
use std::panic;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use crossterm::event::{DisableFocusChange, EnableFocusChange};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::app::App;

#[derive(Parser)]
#[command(name = "orb", about = "Terminal workspace for agentic coding sessions")]
struct Args {
    /// Which agent CLI to drive
    #[arg(long, value_enum, default_value_t = AgentKindArg::Claude)]
    agent: AgentKindArg,

    /// Resume a previous session by UUID. The transcript at
    /// ~/.orb/transcripts/<agent>-<uuid>.jsonl is replayed before
    /// the prompt is mounted.
    #[arg(long)]
    resume: Option<Uuid>,

    /// Model id to pass to the agent CLI. Overrides the per-agent
    /// default. For `pi`, defaults to whatever pi's own config picks.
    #[arg(long)]
    model: Option<String>,
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
        let _ = execute!(io::stdout(), DisableFocusChange);
        let _ = disable_raw_mode();
        original_hook(info);
    }));

    enable_raw_mode()?;
    execute!(io::stdout(), EnableFocusChange)?;
    let mut terminal = app::make_terminal(app::VIEWPORT_HEIGHT)?;

    let mut app = App::new(agent, &cwd, args.resume, args.model);
    app.boot(&mut terminal)?;
    let result = app.run(&mut terminal).await;

    // Best-effort cleanup: clear the viewport and disable raw mode so the
    // user's shell prompt comes back clean below our final output.
    let _ = terminal.clear();
    let _ = execute!(io::stdout(), DisableFocusChange);
    let _ = disable_raw_mode();
    app.print_resume_hint();

    result.map_err(Into::into)
}
