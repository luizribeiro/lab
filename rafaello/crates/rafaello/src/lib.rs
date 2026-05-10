//! `rafaello` library: CLI surface and shared types for the `rfl` binary.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use rafaello_core::error::{BrokerError, FrontendSpawnError};
use rafaello_core::session::SessionError;

#[derive(Debug, Parser)]
#[command(name = "rfl", version, about = "rafaello — minimal coding agent")]
pub struct RflChatCli {
    #[command(subcommand)]
    pub command: RflChatCommand,
}

#[derive(Debug, Subcommand)]
pub enum RflChatCommand {
    /// Start an interactive chat session.
    Chat {
        /// Project root directory. Defaults to the current working directory.
        project_root: Option<PathBuf>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RflChatError {
    #[error("project root invalid at {path:?}: {source}")]
    ProjectRootInvalid {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("rfl-tui path unresolved (tried RFL_TUI_PATH env, then sibling of current_exe)")]
    TuiPathUnresolved,
    #[error("frontend exited before ready: {reason}")]
    FrontendExitedBeforeReady { reason: String },
    #[error("frontend ready timeout")]
    FrontendReadyTimeout,
    #[error("frontend exited abnormally: {reason}")]
    FrontendExitedAbnormally { reason: String },
    #[error("session: {0}")]
    Session(#[from] Box<SessionError>),
    #[error("frontend spawn: {0}")]
    Spawn(#[from] Box<FrontendSpawnError>),
    #[error("broker: {0}")]
    Broker(#[from] Box<BrokerError>),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve the path to the `rfl-tui` binary.
///
/// Two-level lookup:
/// 1. `RFL_TUI_PATH` env-var override (used by tests and non-installed targets).
/// 2. Sibling of `current_exe()` — the canonical installed-binary location.
pub fn resolve_tui_path(
    env: &dyn Fn(&str) -> Option<OsString>,
    current_exe: &Path,
) -> Result<PathBuf, RflChatError> {
    if let Some(value) = env("RFL_TUI_PATH") {
        return Ok(PathBuf::from(value));
    }
    if let Some(parent) = current_exe.parent() {
        let sibling = parent.join("rfl-tui");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    Err(RflChatError::TuiPathUnresolved)
}

pub fn run_cli() -> ExitCode {
    let _cli = RflChatCli::parse();
    ExitCode::SUCCESS
}
