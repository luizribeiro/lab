//! Tokio flavor of the [`Sandbox::command`](crate::Sandbox::command) factory.
//!
//! Enabled by the `tokio` cargo feature. Exposes a free function that mirrors
//! [`Sandbox::command`] but returns a [`tokio::process::Command`] so async
//! callers can `.spawn()` it on the tokio runtime without going through the
//! synchronous wrapper.

use std::path::Path;

use crate::Sandbox;

/// Builds a [`tokio::process::Command`] that runs `program` inside `sandbox`.
///
/// The returned command is fully configurable (`args`, `env`, `stdin`/
/// `stdout`/`stderr`, `current_dir`, `kill_on_drop`, ...) exactly like any
/// other tokio command. The caller must keep `sandbox` alive until the
/// spawned child exits.
pub fn command(sandbox: &Sandbox, program: &Path) -> tokio::process::Command {
    tokio::process::Command::from(sandbox.command(program))
}
