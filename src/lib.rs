use std::path::PathBuf;

use anyhow::Result;

mod ffi;

pub struct VmConfig {
    pub root: Option<PathBuf>,
    pub kernel: Option<PathBuf>,
    pub initramfs: Option<PathBuf>,
    pub kernel_cmdline: Option<String>,
    pub vcpus: u8,
    pub memory_mib: u32,
    pub verbosity: u8,
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
