use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::{SandboxSpec, SandboxedChild};

/// Linux sandbox backend.
///
/// Until `syd` integration lands, this is a compatibility fallback that
/// launches the target process directly (without sandbox enforcement).
pub fn spawn_with_syd(
    program: &Path,
    args: &[String],
    _spec: &SandboxSpec,
) -> Result<SandboxedChild> {
    eprintln!(
        "warning: Linux sandbox backend is not implemented yet; running capsa-vmm without sandbox"
    );

    let child = Command::new(program)
        .args(args)
        .spawn()
        .with_context(|| format!("failed to spawn {}", program.display()))?;

    Ok(SandboxedChild::new(child, vec![]))
}
