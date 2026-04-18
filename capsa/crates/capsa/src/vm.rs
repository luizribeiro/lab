use std::os::fd::OwnedFd;
use std::os::unix::net::UnixDatagram;
use std::sync::atomic::{AtomicU64, Ordering};

use capsa_core::{VmAttachment, VmConfig, VmProcesses};

use crate::attach::{AttachApply, AttachCtx, Attachable, NetworkAttachment};
use crate::boot::{Boot, BootKind};
use crate::error::{BuildError, RuntimeError, StartError};
use crate::network::NetworkHandle;

pub struct Vm {
    pub(crate) config: VmConfig,
    pub(crate) attachments: Vec<NetworkAttachment>,
}

impl std::fmt::Debug for Vm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vm")
            .field("config", &self.config)
            .field("attachments", &self.attachments.len())
            .finish()
    }
}

impl Vm {
    pub fn builder(boot: impl Into<Boot>) -> VmBuilder {
        VmBuilder {
            boot: boot.into(),
            vcpus: 1,
            memory_mib: 512,
            verbosity: 0,
            ctx: AttachCtx::default(),
        }
    }

    /// Spawn the VM: allocate a host/guest socketpair per attachment,
    /// send `AddInterface` to each attached network daemon, and
    /// launch the vmm process. The returned [`VmHandle`] keeps clones
    /// of every attached [`NetworkHandle`] alive so the network
    /// daemons outlive the VM.
    pub fn start(self) -> Result<VmHandle, StartError> {
        let Self {
            config,
            attachments,
        } = self;

        let mut vm_attachments = Vec::with_capacity(attachments.len());
        let mut network_handles = Vec::with_capacity(attachments.len());

        for attachment in attachments.iter() {
            let (host_sock, guest_sock) = UnixDatagram::pair().map_err(StartError::Socketpair)?;
            let host_fd: OwnedFd = host_sock.into();
            let guest_fd: OwnedFd = guest_sock.into();
            let mac = attachment.attach.mac.unwrap_or_else(generate_mac);
            attachment
                .handle
                .inner
                .attach(mac, attachment.attach.port_forwards.clone(), &host_fd)
                .map_err(|e| StartError::Attach(e.into()))?;
            vm_attachments.push(VmAttachment { mac, guest_fd });
        }

        for attachment in attachments {
            network_handles.push(attachment.handle);
        }

        let inner = VmProcesses::spawn_with_attachments(&config, vm_attachments)
            .map_err(|e| StartError::VmSpawn(e.into()))?;

        Ok(VmHandle {
            inner,
            _network_handles: network_handles,
        })
    }

    /// Blocking convenience: start the VM and wait for it to exit.
    /// See [`VmHandle::wait`] for the return semantics.
    pub fn run(self) -> Result<VmExit, RuntimeError> {
        self.start().map_err(RuntimeError::Start)?.wait()
    }
}

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
        };

        Ok(Vm {
            config,
            attachments: self.ctx.attachments,
        })
    }
}

/// A handle to a running VM. Single-owner: dropping `VmHandle`
/// SIGKILLs the vmm child, so the type is intentionally not `Clone`.
/// If you need to observe the VM from multiple places, share the
/// handle behind an `Arc<Mutex<…>>` or pass `&mut self` methods
/// through a supervisor.
///
/// Attached networks outlive the VM only as long as the caller holds
/// their own [`NetworkHandle`] clones.
pub struct VmHandle {
    inner: VmProcesses,
    // Held across the VM's lifetime so the network daemons it
    // attached to stay alive until the VM is dropped.
    _network_handles: Vec<NetworkHandle>,
}

impl std::fmt::Debug for VmHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmHandle")
            .field("networks", &self._network_handles.len())
            .finish()
    }
}

impl VmHandle {
    /// SIGKILL the vmm child immediately. Safe to call after the VM
    /// has exited on its own (becomes a no-op). Does not tear down
    /// attached networks — those are owned by the caller's
    /// [`NetworkHandle`] clones.
    pub fn kill(&mut self) -> Result<(), RuntimeError> {
        self.inner.kill().map_err(RuntimeError::Kill)
    }

    /// Block until the VM exits. Returns the guest's exit status;
    /// a non-zero exit is not an error — the caller decides how to
    /// interpret it via [`VmExit::success`] / [`VmExit::code`].
    pub fn wait(mut self) -> Result<VmExit, RuntimeError> {
        self.inner
            .wait()
            .map(VmExit::from)
            .map_err(|e| RuntimeError::Wait(e.into()))
    }
}

/// The exit status of a VM. Thin wrapper around
/// [`std::process::ExitStatus`] with convenience accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmExit(std::process::ExitStatus);

impl VmExit {
    /// Whether the VM exited with status code 0.
    pub fn success(&self) -> bool {
        self.0.success()
    }

    /// The exit code if the VM exited normally. Returns `None` if
    /// the VM was terminated by a signal.
    pub fn code(&self) -> Option<i32> {
        self.0.code()
    }

    /// The signal number that terminated the VM, if any.
    pub fn signal(&self) -> Option<i32> {
        use std::os::unix::process::ExitStatusExt;
        self.0.signal()
    }

    /// The underlying [`std::process::ExitStatus`].
    pub fn as_exit_status(&self) -> std::process::ExitStatus {
        self.0
    }
}

impl std::fmt::Display for VmExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl From<std::process::ExitStatus> for VmExit {
    fn from(status: std::process::ExitStatus) -> Self {
        Self(status)
    }
}

/// Monotonic counter for process-local MAC uniqueness. Combined with
/// the pid below it guarantees distinct MACs for every call from this
/// process; cross-process distinctness is best-effort via pid.
static MAC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn generate_mac() -> [u8; 6] {
    let counter = MAC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id() as u64;

    // Layout: locally-administered unicast prefix 0x02, then three
    // pid-derived bytes, then two counter-derived bytes. Two VMs in
    // the same process get different counter bytes; VMs across
    // processes get different pid bytes.
    [
        0x02,
        ((pid >> 16) & 0xff) as u8,
        ((pid >> 8) & 0xff) as u8,
        (pid & 0xff) as u8,
        ((counter >> 8) & 0xff) as u8,
        (counter & 0xff) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn builder_applies_defaults() {
        let vm = Vm::builder(Boot::root("/var/lib/capsa/rootfs"))
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.vcpus, 1);
        assert_eq!(vm.config.memory_mib, 512);
        assert_eq!(vm.config.verbosity, 0);
        assert!(vm.attachments.is_empty());
    }

    #[test]
    fn root_boot_lowers_to_root_path() {
        let vm = Vm::builder(Boot::root("/var/lib/capsa/rootfs"))
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.root, Some(PathBuf::from("/var/lib/capsa/rootfs")));
        assert_eq!(vm.config.kernel, None);
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
    fn builder_accepts_kernel_boot_without_initramfs_or_cmdline() {
        let vm = Vm::builder(Boot::kernel("/boot/vmlinuz"))
            .build()
            .expect("build should succeed");

        assert_eq!(vm.config.kernel, Some(PathBuf::from("/boot/vmlinuz")));
        assert_eq!(vm.config.initramfs, None);
        assert_eq!(vm.config.kernel_cmdline, None);
    }

    #[test]
    fn generated_macs_are_nonzero_and_locally_administered() {
        let mac = generate_mac();
        assert_ne!(mac, [0u8; 6]);
        assert_eq!(mac[0] & 0x02, 0x02, "locally administered bit set");
        assert_eq!(mac[0] & 0x01, 0x00, "multicast bit clear");
    }

    #[test]
    fn generated_macs_are_unique_across_back_to_back_calls() {
        let a = generate_mac();
        let b = generate_mac();
        assert_ne!(a, b);
    }

    #[test]
    fn generated_macs_are_unique_across_many_calls() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1024 {
            assert!(seen.insert(generate_mac()), "MAC collision within same run");
        }
    }
}
