use std::path::Path;

use anyhow::Result;

use super::{SandboxSpec, SandboxedChild};

/// TODO: Linux backend via `syd`.
///
/// Planned approach:
/// 1. Map `SandboxSpec` into a generated `syd` policy.
/// 2. Spawn `syd` as the supervisor process and exec target program under it.
/// 3. Add integration tests that validate filesystem and network denials.
pub fn spawn_with_syd(
    _program: &Path,
    _args: &[String],
    _spec: &SandboxSpec,
) -> Result<SandboxedChild> {
    anyhow::bail!("Linux sandbox backend is not implemented yet (TODO: syd integration)")
}
