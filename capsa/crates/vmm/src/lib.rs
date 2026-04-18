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

    let vm = vm.set_kernel(
        &spec.kernel,
        spec.initramfs.as_deref(),
        spec.kernel_cmdline.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::network_call_params;
    use capsa_spec::ResolvedNetworkInterface;

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
}
