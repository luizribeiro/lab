//! Device attachments for [`VmBuilder::attach`](crate::VmBuilder::attach).
//!
//! Today the only attachable device is [`NetworkHandle`]; future
//! device types (disks, GPU passthrough, etc.) will plug in through
//! the same trait. The trait is **sealed** — third-party crates
//! cannot add new device types. This is deliberate: attachments
//! touch the vmm's fd table and sandbox policy, which we do not
//! want to expose as an unstable extension point.

use crate::network::NetworkHandle;

mod sealed {
    pub trait Sealed {}
}

/// Marker for types that can be passed to
/// [`VmBuilder::attach`](crate::VmBuilder::attach) /
/// [`VmBuilder::attach_with`](crate::VmBuilder::attach_with).
///
/// Sealed — the capsa crate is the only place that can implement
/// this. See the [module-level docs](self) for the rationale.
pub trait Attachable: sealed::Sealed {
    /// Per-attachment configuration (MAC overrides, port forwards,
    /// etc.). Must have a sensible default so bare
    /// [`attach`](crate::VmBuilder::attach) works without a closure.
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

/// A single TCP host→guest port mapping. Named fields because two
/// bare `u16`s in the same position are trivially swappable.
///
/// TCP-only for now; UDP forwards (and other protocols) will land
/// as additional methods on [`NetworkAttach`] rather than a
/// `protocol` field here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortForward {
    /// TCP port bound on the host's loopback interface.
    pub host: u16,
    /// TCP port on the guest that inbound connections are routed to.
    pub guest: u16,
}

/// Per-VM attachment configuration for a [`NetworkHandle`].
///
/// Received by the closure passed to
/// [`VmBuilder::attach_with`](crate::VmBuilder::attach_with); the
/// default has no MAC override (one is generated) and no port
/// forwards. Builder-style setters consume and return `self` so the
/// closure body can be written as a single `.forward(…).…` chain.
#[derive(Debug, Clone, Default)]
pub struct NetworkAttach {
    pub(crate) mac: Option<[u8; 6]>,
    pub(crate) port_forwards: Vec<(u16, u16)>,
}

impl NetworkAttach {
    /// Pin the guest-side MAC address. Must be a locally-administered
    /// unicast address (`0x02` in the first octet's low bits).
    /// When omitted, a unique MAC is generated per attachment.
    pub fn mac(mut self, mac: [u8; 6]) -> Self {
        self.mac = Some(mac);
        self
    }

    /// Forward a host TCP port to a guest TCP port. Call multiple
    /// times to forward multiple ports; order is preserved.
    ///
    /// ```
    /// use capsa::{NetworkAttach, PortForward};
    /// let attach = NetworkAttach::default()
    ///     .forward(PortForward { host: 8080, guest: 80 })
    ///     .forward(PortForward { host: 8443, guest: 443 });
    /// # drop(attach);
    /// ```
    pub fn forward(mut self, forward: PortForward) -> Self {
        self.port_forwards.push((forward.host, forward.guest));
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
            .forward(PortForward {
                host: 8080,
                guest: 80,
            })
            .forward(PortForward {
                host: 8443,
                guest: 443,
            });
        assert_eq!(attach.port_forwards, vec![(8080, 80), (8443, 443)]);
    }
}
