//! VM lifecycle: brings up a `capsa-vmm` child for a VM whose
//! interfaces have already been attached to a caller-owned
//! `NetworkProcesses` (or a VM that runs without networking).
//!
//! Submodule layout:
//!
//! - `orchestrate`: `VmProcesses` (vmm supervision) + `VmAttachment`.
//! - `network`: `NetworkProcesses` — owns a `capsa-netd` child and
//!   its control socket.
//! - `netd`: shared netd helpers (sandbox builder, readiness wait).
//! - `vmm`: capsa-vmm spawn + sandbox policy.
//! - `plan`: config → spec shaping (path canonicalization, fd
//!   binding).
//! - `child`: generic child-process primitives (`ChildHandle`,
//!   `spawn_sandboxed`, reaper, signal teardown).
//!
//! On platforms other than Linux or macOS this module is empty; the
//! public types resolve to stubs that error out at use time.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod child;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod control_client;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod netd;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod network;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod orchestrate;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod plan;
#[cfg(test)]
mod test_helpers;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod vmm;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use network::NetworkProcesses;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use orchestrate::{VmAttachment, VmProcesses};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub struct VmProcesses {
    _private: (),
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl VmProcesses {
    pub fn wait(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("capsa VM launch is only supported on Linux and macOS")
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
