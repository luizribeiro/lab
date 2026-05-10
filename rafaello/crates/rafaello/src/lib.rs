//! `rafaello` library: CLI surface and shared types for the `rfl` binary.
//!
//! This module currently exposes stubs that later commits flesh out; the lib
//! target exists so integration tests under `rafaello/tests/` can call into
//! [`resolve_tui_path`] and friends directly.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rfl", version, about = "rafaello — minimal coding agent")]
pub struct RflChatCli {
    #[command(subcommand)]
    pub command: RflChatCommand,
}

#[derive(Debug, Subcommand)]
pub enum RflChatCommand {
    /// Start an interactive chat session (stub; implementation lands later).
    Chat,
}

#[derive(Debug, thiserror::Error)]
pub enum RflChatError {
    #[error("rfl-tui binary not found at {0}")]
    TuiNotFound(PathBuf),
}

pub fn resolve_tui_path() -> Result<PathBuf, RflChatError> {
    Err(RflChatError::TuiNotFound(PathBuf::from("rfl-tui")))
}

pub fn run_cli() -> ExitCode {
    let _cli = RflChatCli::parse();
    ExitCode::SUCCESS
}
