use capsa_core::VmNetworkInterfaceConfig;

use crate::network::Network;

mod sealed {
    pub trait Sealed {}
}

pub trait Attachable: sealed::Sealed {
    type Attachment: Default;
}

#[doc(hidden)]
pub mod __private {
    use super::Attachable;
    use capsa_core::VmNetworkInterfaceConfig;

    pub trait AttachApply: Attachable {
        fn apply(&self, attachment: Self::Attachment, ctx: &mut AttachCtx);
    }

    #[derive(Debug, Default)]
    pub struct AttachCtx {
        pub interfaces: Vec<VmNetworkInterfaceConfig>,
    }
}

pub(crate) use __private::{AttachApply, AttachCtx};

#[derive(Debug, Clone, Default)]
pub struct NetworkAttach {
    pub(crate) mac: Option<[u8; 6]>,
    pub(crate) port_forwards: Vec<(u16, u16)>,
}

impl NetworkAttach {
    pub fn mac(mut self, mac: [u8; 6]) -> Self {
        self.mac = Some(mac);
        self
    }

    pub fn forward_tcp(mut self, host_port: u16, guest_port: u16) -> Self {
        self.port_forwards.push((host_port, guest_port));
        self
    }
}

impl sealed::Sealed for Network {}

impl Attachable for Network {
    type Attachment = NetworkAttach;
}

impl __private::AttachApply for Network {
    fn apply(&self, attachment: NetworkAttach, ctx: &mut __private::AttachCtx) {
        ctx.interfaces.push(VmNetworkInterfaceConfig {
            mac: attachment.mac,
            policy: Some(self.policy.clone()),
            port_forwards: attachment.port_forwards,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capsa_core::PolicyAction;

    #[test]
    fn network_attach_defaults_are_empty() {
        let attach = NetworkAttach::default();
        assert_eq!(attach.mac, None);
        assert!(attach.port_forwards.is_empty());
    }

    #[test]
    fn network_attach_sets_mac() {
        let attach = NetworkAttach::default().mac([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]);
        assert_eq!(attach.mac, Some([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]));
    }

    #[test]
    fn network_attach_accumulates_forwards_in_order() {
        let attach = NetworkAttach::default()
            .forward_tcp(8080, 80)
            .forward_tcp(8443, 443);
        assert_eq!(attach.port_forwards, vec![(8080, 80), (8443, 443)]);
    }

    #[test]
    fn network_apply_pushes_interface_with_policy() {
        let network = Network::builder()
            .allow_host("api.example.com")
            .build()
            .expect("network should build");

        let mut ctx = AttachCtx::default();
        let attachment = NetworkAttach::default()
            .mac([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee])
            .forward_tcp(8080, 80);

        network.apply(attachment, &mut ctx);

        assert_eq!(ctx.interfaces.len(), 1);
        let iface = &ctx.interfaces[0];
        assert_eq!(iface.mac, Some([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]));
        assert_eq!(iface.port_forwards, vec![(8080, 80)]);
        let policy = iface.policy.as_ref().expect("policy should be set");
        assert_eq!(policy.default_action, PolicyAction::Deny);
        assert_eq!(policy.rules.len(), 1);
    }

    #[test]
    fn network_apply_with_default_attachment_leaves_mac_and_forwards_unset() {
        let network = Network::builder()
            .allow_all_hosts()
            .build()
            .expect("network should build");

        let mut ctx = AttachCtx::default();
        network.apply(NetworkAttach::default(), &mut ctx);

        let iface = &ctx.interfaces[0];
        assert_eq!(iface.mac, None);
        assert!(iface.port_forwards.is_empty());
        assert_eq!(
            iface.policy.as_ref().unwrap().default_action,
            PolicyAction::Allow
        );
    }
}
