use std::path::PathBuf;

/// How the VM boots.
///
/// Two mutually exclusive modes:
/// - [`Boot::root`] — boot from an existing rootfs or disk image (the
///   vmm picks which based on whether the path is a directory or a
///   file). Use this for VM-shaped workloads.
/// - [`Boot::kernel`] — supply a kernel image directly and optionally
///   an initramfs and cmdline. Use this for minimal / custom boots.
///
/// Required up front by [`Vm::builder`](crate::Vm::builder) so a VM
/// cannot be constructed without a boot source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boot {
    pub(crate) kind: BootKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BootKind {
    Root(PathBuf),
    Kernel {
        kernel: PathBuf,
        initramfs: Option<PathBuf>,
        cmdline: Option<String>,
    },
}

impl Boot {
    /// Boot from an existing root. `path` may be either a directory
    /// (treated as a rootfs, mounted read-write) or a regular file
    /// (treated as a disk image). The vmm picks the mode at start
    /// time.
    pub fn root(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: BootKind::Root(path.into()),
        }
    }

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
    fn root_boot_captures_path() {
        let boot = Boot::root("/var/lib/capsa/rootfs");
        assert_eq!(
            boot.kind,
            BootKind::Root(PathBuf::from("/var/lib/capsa/rootfs"))
        );
    }

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
        let BootKind::Kernel { cmdline, .. } = boot.kind else {
            panic!("expected kernel boot");
        };
        assert_eq!(cmdline.as_deref(), Some("second"));
    }
}
