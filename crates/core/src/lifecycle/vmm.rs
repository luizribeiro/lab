//! capsa-vmm spawn path: builds the vmm sandbox policy, inherits
//! each guest socketpair fd into the sandboxed VMM, and finalizes
//! the launch spec with the kernel-assigned raw fd numbers before
//! exec.

use std::os::fd::OwnedFd;
use std::path::Path;

use anyhow::{ensure, Context, Result};
use capsa_sandbox::SandboxBuilder;
use capsa_spec::{encode_launch_spec_args, ResolvedNetworkInterface, VmmLaunchSpec};

use super::child::{self, ChildHandle};
use super::plan;

pub(super) fn spawn_vmm(spec: &VmmLaunchSpec, guest_fds: Vec<OwnedFd>) -> Result<ChildHandle> {
    ensure!(
        spec.resolved_interfaces.len() == guest_fds.len(),
        "vmm guest_fds count ({}) must match resolved_interfaces ({})",
        guest_fds.len(),
        spec.resolved_interfaces.len()
    );

    let binary = child::resolve_binary("CAPSA_VMM_PATH", "capsa-vmm")
        .context("failed to resolve VMM binary")?;

    let mut builder = vmm_sandbox_builder(spec, &binary);

    // Inherit each guest fd and record its kernel-assigned number
    // on the final spec. One-stage replacement for the older
    // "placeholder zeros then adapter overwrites" pattern.
    let mut resolved = Vec::with_capacity(guest_fds.len());
    for (guest_fd, interface) in guest_fds.into_iter().zip(&spec.resolved_interfaces) {
        let guest_raw = builder
            .inherit_fd(guest_fd)
            .context("failed to inherit vmm guest fd")?;
        resolved.push(ResolvedNetworkInterface {
            mac: interface.mac,
            guest_fd: guest_raw,
        });
    }

    let runtime_spec = VmmLaunchSpec {
        resolved_interfaces: resolved,
        ..spec.clone()
    };
    runtime_spec.validate().context("invalid vmm launch spec")?;

    let args = encode_launch_spec_args(&runtime_spec)?;
    child::spawn_sandboxed("vmm", &binary, builder, &args, false)
        .context("failed to spawn sandboxed VMM process")
}

fn vmm_sandbox_builder(spec: &VmmLaunchSpec, vmm_exe: &Path) -> SandboxBuilder {
    // The spec's paths were canonicalized in `plan::build_vmm_spec`,
    // so they match the path the macOS kernel resolves at open(2)
    // time and the sandbox policy doesn't need to canonicalize
    // again here. The vmm binary path comes from `resolve_binary`
    // which doesn't canonicalize, so it gets the canonical
    // treatment via `plan::canonical_or_unchanged`.
    let mut builder = capsa_sandbox::Sandbox::builder()
        .allow_network(false)
        .allow_kvm(true)
        .allow_interactive_tty(true)
        .read_only_path(plan::canonical_or_unchanged(vmm_exe));

    if let Some(root) = &spec.root {
        builder = builder.read_write_path(root.clone());
    }
    if let Some(kernel) = &spec.kernel {
        builder = builder.read_only_path(kernel.clone());
    }
    if let Some(initramfs) = &spec.initramfs {
        builder = builder.read_only_path(initramfs.clone());
    }

    builder
}
