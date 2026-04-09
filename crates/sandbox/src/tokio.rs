//! Tokio flavor of the [`SandboxBuilder::build`](crate::SandboxBuilder::build) factory.
//!
//! Enabled by the `tokio` cargo feature. Exposes a free function that
//! mirrors [`SandboxBuilder::build`] but returns a [`tokio::process::Command`]
//! so async callers can `.spawn()` it on the tokio runtime without going
//! through the synchronous wrapper.

use std::path::Path;

use anyhow::Result;

use crate::{Sandbox, SandboxBuilder};

/// Consumes `builder` and returns a `(tokio::process::Command, Sandbox)`
/// pair ready to spawn `program` on the tokio runtime.
///
/// The sandbox must be kept alive until the spawned child exits so its
/// private tmp directory is not removed out from under the child.
pub fn build(
    builder: SandboxBuilder,
    program: &Path,
) -> Result<(tokio::process::Command, Sandbox)> {
    let (command, sandbox) = builder.build(program)?;
    Ok((tokio::process::Command::from(command), sandbox))
}
