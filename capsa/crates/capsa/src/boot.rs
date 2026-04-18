use std::path::PathBuf;

/// How the VM boots.
///
/// Today there is one mode: direct-kernel boot via [`Boot::kernel`],
/// which supplies a kernel image plus an optional initramfs and
/// kernel command line. The type is shaped as an enum wrapper so
/// future boot modes (disk image, EFI) can land as new variants
/// without breaking existing callers.
///
/// Required up front by [`Vm::builder`](crate::Vm::builder) so a VM
/// cannot be constructed without a boot source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boot {
    pub(crate) kind: BootKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BootKind {
    Kernel {
        kernel: PathBuf,
        initramfs: Option<PathBuf>,
        cmdline: Option<String>,
    },
}

impl Boot {
    /// Start a [`KernelBoot`] builder for direct-kernel boot. Chain
    /// `.initramfs(...)` and `.cmdline(...)` as needed; the result
    /// implicitly converts to `Boot` via `Into` so
    /// [`Vm::builder`](crate::Vm::builder) accepts it directly.
    pub fn kernel(path: impl Into<PathBuf>) -> KernelBoot {
        KernelBoot {
            kernel: path.into(),
            initramfs: None,
            cmdline: None,
        }
    }
}

/// Builder for direct-kernel boot. Created by [`Boot::kernel`].
///
/// Converts to [`Boot`] implicitly, so it can be passed straight to
/// [`Vm::builder`](crate::Vm::builder):
///
/// ```no_run
/// use capsa::{Boot, Vm};
/// Vm::builder(Boot::kernel("/boot/vmlinuz").cmdline("console=hvc0"))
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelBoot {
    kernel: PathBuf,
    initramfs: Option<PathBuf>,
    cmdline: Option<String>,
}

impl KernelBoot {
    /// Optional initramfs/initrd image loaded before kernel hand-off.
    pub fn initramfs(mut self, path: impl Into<PathBuf>) -> Self {
        self.initramfs = Some(path.into());
        self
    }

    /// Kernel command line passed to the guest. Later calls replace
    /// earlier ones (not appended), so compose the full string in
    /// one go.
    pub fn cmdline(mut self, cmdline: impl Into<String>) -> Self {
        self.cmdline = Some(cmdline.into());
        self
    }
}

impl From<KernelBoot> for Boot {
    fn from(k: KernelBoot) -> Self {
        Self {
            kind: BootKind::Kernel {
                kernel: k.kernel,
                initramfs: k.initramfs,
                cmdline: k.cmdline,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn kernel_boot_without_initramfs_or_cmdline() {
        let boot: Boot = Boot::kernel("/boot/vmlinuz").into();
        assert_eq!(
            boot.kind,
            BootKind::Kernel {
                kernel: PathBuf::from("/boot/vmlinuz"),
                initramfs: None,
                cmdline: None,
            }
        );
    }

    #[test]
    fn kernel_boot_with_initramfs_and_cmdline() {
        let boot: Boot = Boot::kernel("/boot/vmlinuz")
            .initramfs("/boot/initramfs.cpio")
            .cmdline("console=hvc0")
            .into();
        assert_eq!(
            boot.kind,
            BootKind::Kernel {
                kernel: PathBuf::from("/boot/vmlinuz"),
                initramfs: Some(PathBuf::from("/boot/initramfs.cpio")),
                cmdline: Some("console=hvc0".to_string()),
            }
        );
    }

    #[test]
    fn kernel_boot_accepts_path_types() {
        let _: Boot = Boot::kernel(Path::new("/boot/vmlinuz"))
            .initramfs(PathBuf::from("/boot/initramfs.cpio"))
            .into();
    }

    #[test]
    fn kernel_boot_last_cmdline_wins() {
        let boot: Boot = Boot::kernel("/boot/vmlinuz")
            .cmdline("first")
            .cmdline("second")
            .into();
        let BootKind::Kernel { cmdline, .. } = boot.kind;
        assert_eq!(cmdline.as_deref(), Some("second"));
    }
}
