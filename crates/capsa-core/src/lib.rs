use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

mod ffi;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub root: Option<PathBuf>,
    pub kernel: Option<PathBuf>,
    pub initramfs: Option<PathBuf>,
    pub kernel_cmdline: Option<String>,
    pub vcpus: u8,
    pub memory_mib: u32,
    pub verbosity: u8,
}

impl VmConfig {
    /// Start the VM in the sandboxed sidecar process.
    pub fn start(&self) -> Result<()> {
        let vmm_exe = resolve_vmm_binary()?;
        let spec = vm_sandbox_spec(self, &vmm_exe);

        let config_json = serde_json::to_string(self).context("failed to serialize VM config")?;
        let child_args = vec!["--vm-config-json".to_string(), config_json];

        let child =
            capsa_sandbox::spawn_sandboxed(&vmm_exe, &child_args, &spec).with_context(|| {
                format!(
                    "failed to spawn sandboxed VMM process: {}",
                    vmm_exe.display()
                )
            })?;

        let status = child.wait().context("failed to wait on sandboxed child")?;
        if status.success() {
            return Ok(());
        }

        anyhow::bail!("sandboxed VMM process exited with status {status}")
    }
}

pub fn start_vm(config: &VmConfig) -> Result<()> {
    ffi::init_logging(config.verbosity)?;

    let vm = ffi::KrunVm::new()?
        .configure(config.vcpus, config.memory_mib)?
        .configure_host_tty_console()?;

    if let Some(kernel) = &config.kernel {
        let kernel_cmdline = effective_kernel_cmdline(config);
        let vm = vm.set_kernel(
            kernel,
            config.initramfs.as_deref(),
            kernel_cmdline.as_deref(),
        )?;
        return vm.start_enter();
    }

    if let Some(root) = &config.root {
        return vm.set_root(root)?.start_enter();
    }

    anyhow::bail!("missing boot source: pass either --root <dir> or --kernel <path>")
}

fn vm_sandbox_spec(config: &VmConfig, vmm_exe: &std::path::Path) -> capsa_sandbox::SandboxSpec {
    let mut spec = capsa_sandbox::SandboxSpec::new().allow_network(true);

    spec.read_only_paths.push(vmm_exe.to_path_buf());

    if let Some(root) = &config.root {
        spec.read_write_paths.push(root.clone());
    }

    if let Some(kernel) = &config.kernel {
        spec.read_only_paths.push(kernel.clone());
    }

    if let Some(initramfs) = &config.initramfs {
        spec.read_only_paths.push(initramfs.clone());
    }

    spec
}

fn resolve_vmm_binary() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("CAPSA_VMM_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        let sibling = current_exe.with_file_name("capsa-vmm");
        if sibling.exists() {
            return Ok(sibling);
        }
    }

    if let Some(in_path) = find_in_path("capsa-vmm") {
        return Ok(in_path);
    }

    anyhow::bail!(
        "unable to locate capsa-vmm sidecar. Build/install it (e.g. `cargo build --bins`) and optionally set CAPSA_VMM_PATH"
    )
}

fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn effective_kernel_cmdline(config: &VmConfig) -> Option<String> {
    let defaults = match config.verbosity {
        0 => "quiet loglevel=0",
        1 => "",
        _ => "ignore_loglevel loglevel=7",
    };

    let user_cmdline = config.kernel_cmdline.as_deref().unwrap_or("").trim();
    let cmdline = match (defaults.is_empty(), user_cmdline.is_empty()) {
        (true, true) => String::new(),
        (false, true) => defaults.to_string(),
        (true, false) => user_cmdline.to_string(),
        (false, false) => format!("{defaults} {user_cmdline}"),
    };

    Some(cmdline)
}
