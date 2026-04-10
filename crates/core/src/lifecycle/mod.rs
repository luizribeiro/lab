//! VM lifecycle: orchestrates a single VM launch from validation
//! through teardown. Spawns the network daemon (`capsa-netd`) and
//! the VMM child (`capsa-vmm`) under their respective sandbox
//! policies, waits for either to exit, and tears the other down via
//! RAII.
//!
//! Submodule layout:
//!
//! - `orchestrate`: high-level flow (no-network vs network paths).
//! - `netd`: capsa-netd spawn + readiness wait + sandbox policy.
//! - `vmm`: capsa-vmm spawn + sandbox policy.
//! - `plan`: config → spec shaping (mac generation, path
//!   canonicalization, launch spec construction).
//! - `child`: generic child-process primitives (`ChildHandle`,
//!   `spawn_sandboxed`, reaper, `wait_either`, signal teardown).
//!   Used by `netd` and `vmm` via `super::child`; the only file in
//!   `lifecycle/` that knows nothing about VMs.
//!
//! On unsupported platforms (anything other than Linux or macOS),
//! `VmConfig::start` returns a clear error so downstream crates
//! still compile.

use anyhow::Result;

use crate::config::VmConfig;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod child;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod netd;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod orchestrate;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod plan;
#[cfg(test)]
mod test_helpers;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod vmm;

impl VmConfig {
    /// Start the VM. On Linux and macOS this spawns the daemon
    /// children and blocks until one of them exits. On other
    /// platforms it returns an error immediately because the VM
    /// launch path relies on libkrun, which only supports KVM
    /// (Linux) and HVF (macOS) backends.
    pub fn start(&self) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            orchestrate::run(self)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = self;
            anyhow::bail!("capsa VM launch is only supported on Linux and macOS")
        }
    }
}
