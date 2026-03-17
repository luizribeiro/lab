use std::ffi::{c_char, CString};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;

use anyhow::{anyhow, bail, Context, Result};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

const KRUN_LOG_LEVEL_INFO: u32 = 3;
const KRUN_LOG_STYLE_AUTO: u32 = 0;
const KRUN_LOG_OPTION_NO_ENV: u32 = 1;
const KRUN_KERNEL_FORMAT_RAW: u32 = 0;
const VIRGLRENDERER_VENUS: u32 = 1 << 6;
const VIRGLRENDERER_NO_VIRGL: u32 = 1 << 7;

#[link(name = "krun-efi")]
unsafe extern "C" {
    fn krun_create_ctx() -> i32;
    fn krun_free_ctx(ctx_id: u32) -> i32;
    fn krun_init_log(target_fd: i32, level: u32, style: u32, options: u32) -> i32;
    fn krun_set_vm_config(ctx_id: u32, num_vcpus: u8, ram_mib: u32) -> i32;
    fn krun_set_gpu_options2(ctx_id: u32, virgl_flags: u32, shm_size: u64) -> i32;
    fn krun_set_root(ctx_id: u32, root_path: *const c_char) -> i32;
    fn krun_set_kernel(
        ctx_id: u32,
        kernel_path: *const c_char,
        kernel_format: u32,
        initramfs: *const c_char,
        cmdline: *const c_char,
    ) -> i32;
    fn krun_set_console_output(ctx_id: u32, filepath: *const c_char) -> i32;
    fn krun_get_shutdown_eventfd(ctx_id: u32) -> i32;
    fn krun_start_enter(ctx_id: u32) -> i32;
}

pub(crate) fn init_logging() -> Result<()> {
    // SAFETY: primitive scalar arguments to libkrun.
    check_rc(
        unsafe {
            krun_init_log(
                std::io::stderr().as_raw_fd(),
                KRUN_LOG_LEVEL_INFO,
                KRUN_LOG_STYLE_AUTO,
                KRUN_LOG_OPTION_NO_ENV,
            )
        },
        "failed to initialize libkrun logging",
    )
}

pub(crate) struct Created;
pub(crate) struct Configured;
pub(crate) struct BootConfigured;

pub(crate) struct KrunVm<State> {
    ctx: KrunContext,
    _state: PhantomData<State>,
}

struct KrunContext {
    id: u32,
}

impl Drop for KrunContext {
    fn drop(&mut self) {
        // SAFETY: id originates from successful krun_create_ctx.
        let rc = unsafe { krun_free_ctx(self.id) };
        if cfg!(debug_assertions) && rc < 0 {
            eprintln!(
                "warning: krun_free_ctx({}) failed: {}",
                self.id,
                os_error_from_neg_errno(rc)
            );
        }
    }
}

impl KrunVm<Created> {
    pub(crate) fn new() -> Result<Self> {
        // SAFETY: no args.
        let ctx_id = unsafe { krun_create_ctx() };
        if ctx_id < 0 {
            bail!(
                "failed to create libkrun context: {}",
                os_error_from_neg_errno(ctx_id)
            );
        }

        Ok(Self {
            ctx: KrunContext { id: ctx_id as u32 },
            _state: PhantomData,
        })
    }

    pub(crate) fn configure(self, vcpus: u8, memory_mib: u32) -> Result<KrunVm<Configured>> {
        // SAFETY: primitive scalar arguments.
        check_rc(
            unsafe { krun_set_vm_config(self.ctx.id, vcpus, memory_mib) },
            "failed to set VM config",
        )?;

        // Mirror krunkit defaults on macOS.
        let virgl_flags = VIRGLRENDERER_VENUS | VIRGLRENDERER_NO_VIRGL;
        let rounded_mem_gib = (u64::from(memory_mib) / 1024 + 1) * 1024;
        let vram = (63488u64.saturating_sub(rounded_mem_gib)) * 1024 * 1024;
        let _ = unsafe { krun_set_gpu_options2(self.ctx.id, virgl_flags, vram) };

        Ok(self.into_state())
    }
}

impl KrunVm<Configured> {
    pub(crate) fn set_console_output_stdout(self) -> Result<Self> {
        let output = CString::new("/dev/stdout").expect("static string without NUL");

        // SAFETY: pointer valid for duration of call.
        check_rc(
            unsafe { krun_set_console_output(self.ctx.id, output.as_ptr()) },
            "failed to set VM console output",
        )?;

        Ok(self)
    }

    pub(crate) fn set_kernel(
        self,
        kernel: &Path,
        initramfs: Option<&Path>,
        kernel_cmdline: Option<&str>,
    ) -> Result<KrunVm<BootConfigured>> {
        let kernel = path_to_cstring(kernel).context("kernel path contains NUL")?;
        let initramfs = initramfs
            .map(|path| path_to_cstring(path).context("initramfs path contains NUL"))
            .transpose()?;
        let initramfs_ptr = initramfs.as_ref().map_or(ptr::null(), |s| s.as_ptr());
        let cmdline =
            CString::new(kernel_cmdline.unwrap_or("")).context("kernel cmdline contains NUL")?;

        // SAFETY: pointers are valid for duration of call.
        check_rc(
            unsafe {
                krun_set_kernel(
                    self.ctx.id,
                    kernel.as_ptr(),
                    KRUN_KERNEL_FORMAT_RAW,
                    initramfs_ptr,
                    cmdline.as_ptr(),
                )
            },
            "failed to configure kernel",
        )?;

        Ok(self.into_state())
    }

    pub(crate) fn set_root(self, root: &Path) -> Result<KrunVm<BootConfigured>> {
        let root = path_to_cstring(root).context("root path contains NUL")?;

        // SAFETY: pointer valid for duration of call.
        check_rc(
            unsafe { krun_set_root(self.ctx.id, root.as_ptr()) },
            "failed to configure VM root",
        )?;

        Ok(self.into_state())
    }
}

impl KrunVm<BootConfigured> {
    pub(crate) fn start_enter(self) -> Result<()> {
        // SAFETY: optional preflight used by krunkit.
        let _ = unsafe { krun_get_shutdown_eventfd(self.ctx.id) };

        check_rc(
            unsafe { krun_start_enter(self.ctx.id) },
            "failed to start VM",
        )
    }
}

impl<State> KrunVm<State> {
    fn into_state<NextState>(self) -> KrunVm<NextState> {
        KrunVm {
            ctx: self.ctx,
            _state: PhantomData,
        }
    }
}

fn check_rc(rc: i32, context: &str) -> Result<()> {
    if rc < 0 {
        bail!("{context}: {}", os_error_from_neg_errno(rc));
    }
    Ok(())
}

#[cfg(unix)]
fn path_to_cstring(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_bytes()).map_err(|e| anyhow!(e))
}

#[cfg(not(unix))]
fn path_to_cstring(path: &Path) -> Result<CString> {
    let s = path
        .to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8 on this platform"))?;
    CString::new(s).map_err(|e| anyhow!(e))
}

fn os_error_from_neg_errno(rc: i32) -> std::io::Error {
    std::io::Error::from_raw_os_error(-rc)
}
