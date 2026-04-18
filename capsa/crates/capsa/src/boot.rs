use std::path::PathBuf;

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
    pub fn root(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: BootKind::Root(path.into()),
        }
    }

    pub fn kernel(path: impl Into<PathBuf>) -> KernelBoot {
        KernelBoot {
            kernel: path.into(),
            initramfs: None,
            cmdline: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelBoot {
    kernel: PathBuf,
    initramfs: Option<PathBuf>,
    cmdline: Option<String>,
}

impl KernelBoot {
    pub fn initramfs(mut self, path: impl Into<PathBuf>) -> Self {
        self.initramfs = Some(path.into());
        self
    }

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
