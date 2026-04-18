use crate::network::NetworkHandle;

mod sealed {
    pub trait Sealed {}
}

pub trait Attachable: sealed::Sealed {
    type Attachment: Default;
}

#[doc(hidden)]
pub mod __private {
    use super::Attachable;
    use crate::network::NetworkHandle;

    pub trait AttachApply: Attachable {
        fn apply(&self, attachment: Self::Attachment, ctx: &mut AttachCtx);
    }

    #[derive(Default)]
    pub struct AttachCtx {
        pub attachments: Vec<NetworkAttachment>,
    }

    pub struct NetworkAttachment {
        pub handle: NetworkHandle,
        pub attach: super::NetworkAttach,
    }
}

pub(crate) use __private::{AttachApply, AttachCtx, NetworkAttachment};

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

impl sealed::Sealed for NetworkHandle {}

impl Attachable for NetworkHandle {
    type Attachment = NetworkAttach;
}

impl __private::AttachApply for NetworkHandle {
    fn apply(&self, attachment: NetworkAttach, ctx: &mut __private::AttachCtx) {
        ctx.attachments.push(NetworkAttachment {
            handle: self.clone(),
            attach: attachment,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
