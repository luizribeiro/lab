use capsa_core::VmConfig;

use crate::attach::{AttachApply, AttachCtx, Attachable};
use crate::boot::{Boot, BootKind};
use crate::error::BuildError;

#[derive(Debug, Clone)]
pub struct Vm {
    pub(crate) config: VmConfig,
}

impl Vm {
    #[doc(hidden)]
    pub fn __into_core_config(self) -> VmConfig {
        self.config
    }

    pub fn builder(boot: impl Into<Boot>) -> VmBuilder {
        VmBuilder {
            boot: boot.into(),
            vcpus: 1,
            memory_mib: 512,
            verbosity: 0,
            ctx: AttachCtx::default(),
        }
    }
}

#[derive(Debug)]
pub struct VmBuilder {
    boot: Boot,
    vcpus: u8,
    memory_mib: u32,
    verbosity: u8,
    ctx: AttachCtx,
}

impl VmBuilder {
    pub fn vcpus(mut self, n: u8) -> Self {
        self.vcpus = n;
        self
    }

    pub fn memory_mib(mut self, mib: u32) -> Self {
        self.memory_mib = mib;
        self
    }

    pub fn verbosity(mut self, level: u8) -> Self {
        self.verbosity = level;
        self
    }

    pub fn attach<D>(mut self, device: &D) -> Self
    where
        D: Attachable + AttachApply,
    {
        device.apply(D::Attachment::default(), &mut self.ctx);
        self
    }

    pub fn attach_with<D, F>(mut self, device: &D, configure: F) -> Self
    where
        D: Attachable + AttachApply,
        F: FnOnce(D::Attachment) -> D::Attachment,
    {
        let attachment = configure(D::Attachment::default());
        device.apply(attachment, &mut self.ctx);
        self
    }

    pub fn build(self) -> Result<Vm, BuildError> {
        let (root, kernel, initramfs, kernel_cmdline) = match self.boot.kind {
            BootKind::Root(path) => (Some(path), None, None, None),
            BootKind::Kernel {
                kernel,
                initramfs,
                cmdline,
            } => (None, Some(kernel), initramfs, cmdline),
        };

        let config = VmConfig {
            root,
            kernel,
            initramfs,
            kernel_cmdline,
            vcpus: self.vcpus,
            memory_mib: self.memory_mib,
            verbosity: self.verbosity,
            interfaces: self.ctx.interfaces,
        };

        config
            .validate()
            .map_err(|e| BuildError::InvalidConfig(e.to_string()))?;

        Ok(Vm { config })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::Boot;
    use crate::network::Network;
    use capsa_core::PolicyAction;
    use std::path::PathBuf;

    #[test]
    fn builder_applies_defaults() {
        let vm = Vm::builder(Boot::root("/var/lib/capsa/rootfs"))
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.vcpus, 1);
        assert_eq!(vm.config.memory_mib, 512);
        assert_eq!(vm.config.verbosity, 0);
        assert!(vm.config.interfaces.is_empty());
    }

    #[test]
    fn root_boot_lowers_to_root_path() {
        let vm = Vm::builder(Boot::root("/var/lib/capsa/rootfs"))
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.root, Some(PathBuf::from("/var/lib/capsa/rootfs")));
        assert_eq!(vm.config.kernel, None);
        assert_eq!(vm.config.initramfs, None);
        assert_eq!(vm.config.kernel_cmdline, None);
    }

    #[test]
    fn kernel_boot_lowers_all_boot_fields() {
        let vm = Vm::builder(
            Boot::kernel("/boot/vmlinuz")
                .initramfs("/boot/initramfs.cpio")
                .cmdline("console=hvc0"),
        )
        .build()
        .expect("build should succeed");

        assert_eq!(vm.config.root, None);
        assert_eq!(vm.config.kernel, Some(PathBuf::from("/boot/vmlinuz")));
        assert_eq!(
            vm.config.initramfs,
            Some(PathBuf::from("/boot/initramfs.cpio"))
        );
        assert_eq!(vm.config.kernel_cmdline.as_deref(), Some("console=hvc0"));
    }

    #[test]
    fn resource_setters_lower_to_config() {
        let vm = Vm::builder(Boot::root("/rootfs"))
            .vcpus(4)
            .memory_mib(2048)
            .verbosity(2)
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.vcpus, 4);
        assert_eq!(vm.config.memory_mib, 2048);
        assert_eq!(vm.config.verbosity, 2);
    }

    #[test]
    fn attach_adds_default_interface() {
        let network = Network::builder()
            .allow_host("api.example.com")
            .build()
            .expect("network should build");

        let vm = Vm::builder(Boot::root("/rootfs"))
            .attach(&network)
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.interfaces.len(), 1);
        let iface = &vm.config.interfaces[0];
        assert_eq!(iface.mac, None);
        assert!(iface.port_forwards.is_empty());
        let policy = iface.policy.as_ref().expect("policy should be set");
        assert_eq!(policy.default_action, PolicyAction::Deny);
        assert_eq!(policy.rules.len(), 1);
    }

    #[test]
    fn attach_with_applies_mac_and_forwards() {
        let network = Network::builder()
            .allow_all_hosts()
            .build()
            .expect("network should build");

        let vm = Vm::builder(Boot::root("/rootfs"))
            .attach_with(&network, |a| {
                a.mac([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee])
                    .forward_tcp(8080, 80)
            })
            .build()
            .expect("build should succeed");

        let iface = &vm.config.interfaces[0];
        assert_eq!(iface.mac, Some([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]));
        assert_eq!(iface.port_forwards, vec![(8080, 80)]);
    }

    #[test]
    fn build_rejects_multiple_attachments_for_now() {
        let net_a = Network::builder().build().expect("network should build");
        let net_b = Network::builder().build().expect("network should build");

        let err = Vm::builder(Boot::root("/rootfs"))
            .attach(&net_a)
            .attach(&net_b)
            .build()
            .expect_err("multi-interface should be rejected");

        match err {
            BuildError::InvalidConfig(msg) => {
                assert!(
                    msg.contains("multiple network interfaces"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected InvalidConfig, got {other:?}"),
        }
    }

    #[test]
    fn builder_accepts_kernel_boot_without_initramfs_or_cmdline() {
        let vm = Vm::builder(Boot::kernel("/boot/vmlinuz"))
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.kernel, Some(PathBuf::from("/boot/vmlinuz")));
        assert_eq!(vm.config.initramfs, None);
        assert_eq!(vm.config.kernel_cmdline, None);
    }
}
