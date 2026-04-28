//! capsa-vmm spawn path: builds the vmm sandbox policy, inherits
//! each guest socketpair fd into the sandboxed VMM, and produces
//! the launch spec with kernel-assigned raw fd numbers in one
//! pass — no placeholder values exist along the way.

use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use capsa_spec::{encode_launch_spec_args, ResolvedNetworkInterface, VmmLaunchSpec};
use lockin::SandboxBuilder;

use crate::config::VmConfig;

use super::child::{self, ChildHandle};
use super::plan::{self, VmmInterfaceBinding};

pub(super) fn spawn_vmm(
    config: &VmConfig,
    bindings: Vec<VmmInterfaceBinding>,
) -> Result<ChildHandle> {
    let binary = child::resolve_binary("CAPSA_VMM_PATH", "capsa-vmm")
        .context("failed to resolve VMM binary")?;

    let paths = VmmPaths::from_config(config);
    let builder = vmm_sandbox_builder(&paths, &binary);

    let mut resolved = Vec::with_capacity(bindings.len());
    let mut fds = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let guest_raw = binding.guest_fd.as_raw_fd();
        fds.push(binding.guest_fd);
        resolved.push(ResolvedNetworkInterface {
            mac: binding.mac,
            guest_fd: guest_raw,
        });
    }

    let spec = VmmLaunchSpec {
        kernel: paths.kernel,
        initramfs: paths.initramfs,
        kernel_cmdline: config.kernel_cmdline.clone(),
        vcpus: config.vcpus,
        memory_mib: config.memory_mib,
        resolved_interfaces: resolved,
    };
    spec.validate().context("invalid vmm launch spec")?;

    let args = encode_launch_spec_args(&spec)?;
    child::spawn_sandboxed("vmm", &binary, builder, fds, &args, false)
        .context("failed to spawn sandboxed VMM process")
}

/// Canonicalized VMM paths shared between the sandbox policy and
/// the launch spec; built once per `spawn_vmm` call. Private to
/// this file because no other site needs the post-symlink form.
struct VmmPaths {
    kernel: PathBuf,
    initramfs: Option<PathBuf>,
}

impl VmmPaths {
    fn from_config(config: &VmConfig) -> Self {
        Self {
            kernel: plan::canonical_or_unchanged(&config.kernel),
            initramfs: config
                .initramfs
                .as_deref()
                .map(plan::canonical_or_unchanged),
        }
    }
}

fn vmm_sandbox_builder(paths: &VmmPaths, vmm_exe: &Path) -> SandboxBuilder {
    let mut builder = lockin::Sandbox::builder()
        .network_deny()
        .allow_kvm(true)
        .allow_interactive_tty(true)
        .read_path(plan::canonical_or_unchanged(vmm_exe))
        // libkrun reads this to enumerate capabilities before dropping
        // privileges for the guest.
        .read_path(PathBuf::from("/proc/sys/kernel/cap_last_cap"))
        .read_path(paths.kernel.clone());
    builder = child::apply_syd_path(builder);
    builder = child::apply_library_dirs(builder);

    if let Some(initramfs) = &paths.initramfs {
        builder = builder.read_path(initramfs.clone());
    }

    builder
}
