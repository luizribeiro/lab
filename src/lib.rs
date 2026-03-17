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
}

pub fn start_vm(config: &VmConfig) -> Result<()> {
    ffi::init_logging()?;

    let vm = ffi::KrunVm::new()?
        .configure(config.vcpus, config.memory_mib)?
        .set_console_output_stdout()?;

    if let Some(kernel) = &config.kernel {
        let vm = vm.set_kernel(
            kernel,
            config.initramfs.as_deref(),
            config.kernel_cmdline.as_deref(),
        )?;
        return vm.start_enter();
    }

    if let Some(root) = &config.root {
        return vm.set_root(root)?.start_enter();
    }

    anyhow::bail!("missing boot source: pass either --root <dir> or --kernel <path>")
}
