use anyhow::Result;

use capsa_spec::{ResolvedNetworkInterface, VmmLaunchSpec};

mod boot;
mod libkrun;

pub fn start_vm(spec: &VmmLaunchSpec) -> Result<()> {
    spec.validate()?;

    libkrun::init_logging()?;

    let vm = libkrun::KrunVm::new()?
        .configure(spec.vcpus, spec.memory_mib)?
        .configure_host_tty_console()?;

    let vm = configure_network(vm, &spec.resolved_interfaces)?;

    let kernel_cmdline = effective_kernel_cmdline(spec);
    let vm = vm.set_kernel(
        &spec.kernel,
        spec.initramfs.as_deref(),
        kernel_cmdline.as_deref(),
    )?;
    vm.start_enter()
}

fn configure_network(
    mut vm: libkrun::KrunVm<libkrun::Configured>,
    interfaces: &[ResolvedNetworkInterface],
) -> Result<libkrun::KrunVm<libkrun::Configured>> {
    for (fd, mut mac, features, flags) in network_call_params(interfaces) {
        vm = vm.add_network_unixstream(fd, &mut mac, features, flags)?;
    }

    Ok(vm)
}

fn network_call_params(interfaces: &[ResolvedNetworkInterface]) -> Vec<(i32, [u8; 6], u32, u32)> {
    interfaces
        .iter()
        .map(|iface| (iface.guest_fd, iface.mac, libkrun::compat_net_features(), 0))
        .collect()
}

fn effective_kernel_cmdline(spec: &VmmLaunchSpec) -> Option<String> {
    let mut cmdline = KernelCmdline::new();

    if spec.verbosity == 0 {
        cmdline.push_segment("quiet loglevel=0");
    }

    if spec.verbosity > 0 {
        cmdline.push_segment("capsa_init_verbose=1");
    }

    if let Some(user_cmdline) = spec.kernel_cmdline.as_deref() {
        cmdline.push_segment(user_cmdline);
    }

    Some(cmdline.render())
}

#[derive(Debug, Default)]
struct KernelCmdline {
    segments: Vec<String>,
}

impl KernelCmdline {
    fn new() -> Self {
        Self::default()
    }

    fn push_segment(&mut self, segment: &str) {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            return;
        }
        self.segments.push(trimmed.to_string());
    }

    fn render(self) -> String {
        self.segments.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::{effective_kernel_cmdline, network_call_params};
    use capsa_spec::{ResolvedNetworkInterface, VmmLaunchSpec};

    fn base_spec() -> VmmLaunchSpec {
        VmmLaunchSpec {
            kernel: "/boot/vmlinuz".into(),
            initramfs: None,
            kernel_cmdline: None,
            vcpus: 1,
            memory_mib: 512,
            verbosity: 0,
            resolved_interfaces: vec![],
        }
    }

    #[test]
    fn network_call_params_is_empty_for_empty_interfaces() {
        let observed = network_call_params(&[]);
        assert!(observed.is_empty());
    }

    #[test]
    fn network_call_params_matches_single_interface_values() {
        let interfaces = vec![ResolvedNetworkInterface {
            mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
            guest_fd: 42,
        }];

        let observed = network_call_params(&interfaces);

        assert_eq!(
            observed,
            vec![(
                42,
                [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                crate::libkrun::compat_net_features(),
                0,
            )]
        );
    }

    #[test]
    fn effective_kernel_cmdline_is_quiet_by_default() {
        let spec = base_spec();
        let cmdline = effective_kernel_cmdline(&spec).expect("cmdline should exist");
        assert!(cmdline.contains("quiet loglevel=0"));
        assert!(!cmdline.contains("ignore_loglevel"));
        assert!(!cmdline.contains("capsa_init_verbose=1"));
    }

    #[test]
    fn effective_kernel_cmdline_maps_verbosity_levels() {
        let mut spec = base_spec();

        spec.verbosity = 1;
        let cmdline = effective_kernel_cmdline(&spec).expect("cmdline should exist");
        assert!(cmdline.contains("capsa_init_verbose=1"));
        assert!(!cmdline.contains("ignore_loglevel loglevel=7"));

        spec.verbosity = 2;
        let cmdline = effective_kernel_cmdline(&spec).expect("cmdline should exist");
        assert!(cmdline.contains("capsa_init_verbose=1"));
        assert!(!cmdline.contains("ignore_loglevel loglevel=7"));
    }
}
