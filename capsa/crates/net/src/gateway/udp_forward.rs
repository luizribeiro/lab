//! Inbound UDP port forwarding. The mirror of the outbound UDP NAT
//! in `nat/udp.rs`: netd binds a host `UdpSocket` per configured
//! forward and pumps datagrams in via [`UdpPortForwardRequest`]; the
//! gateway allocates a per-source-addr smoltcp UDP socket on an
//! ephemeral gateway-side port, sends the datagram to the guest, and
//! routes replies back through the originating host socket.
//!
//! Single-source flow tracking: each distinct `(host_src_addr)` gets
//! its own ephemeral gateway port. Idle flows are dropped after
//! `UDP_FORWARD_IDLE_TIMEOUT` and the total number of concurrent
//! flows is capped by `MAX_UDP_FORWARD_BINDINGS` to avoid resource
//! exhaustion via source-spoofed datagrams.

use std::collections::HashMap;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smoltcp::iface::{SocketHandle, SocketSet};
use smoltcp::socket::udp::{self, PacketBuffer, PacketMetadata};
use smoltcp::wire::{IpAddress, IpEndpoint};
use tokio::net::UdpSocket;

/// A datagram received by a netd host `UdpSocket`, queued for
/// delivery to the guest. Carries the originating host source
/// address and the host socket itself so the gateway can route
/// replies back via `host_socket.send_to(reply, host_src)`.
pub struct UdpPortForwardRequest {
    pub data: Vec<u8>,
    pub host_src: SocketAddrV4,
    pub host_socket: Arc<UdpSocket>,
    pub guest_ip: std::net::Ipv4Addr,
    pub guest_port: u16,
}

pub(super) const UDP_FORWARD_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_UDP_FORWARD_BINDINGS: usize = 256;
const EPHEMERAL_PORT_START: u16 = 40_000;
const EPHEMERAL_PORT_END: u16 = 49_999;
const UDP_FORWARD_BUF_META: usize = 4;
const UDP_FORWARD_BUF_BYTES: usize = 1500 * 4;

struct UdpInboundFlow {
    smoltcp_handle: SocketHandle,
    host_socket: Arc<UdpSocket>,
    last_activity: Instant,
}

pub(super) struct UdpPortForwardTable {
    flows: HashMap<SocketAddrV4, UdpInboundFlow>,
    next_ephemeral_port: u16,
}

impl UdpPortForwardTable {
    pub(super) fn new() -> Self {
        Self {
            flows: HashMap::new(),
            next_ephemeral_port: EPHEMERAL_PORT_START,
        }
    }

    /// Handle an inbound datagram. Looks up the NAT entry for
    /// `host_src`, creating one if needed, and enqueues the payload
    /// on the smoltcp UDP socket for delivery to the guest.
    pub(super) fn handle_ingress(
        &mut self,
        sockets: &mut SocketSet<'static>,
        req: UdpPortForwardRequest,
    ) {
        let key = req.host_src;

        let flow = match self.flows.get_mut(&key) {
            Some(existing) => {
                existing.last_activity = Instant::now();
                existing
            }
            None => {
                if self.flows.len() >= MAX_UDP_FORWARD_BINDINGS {
                    tracing::warn!(
                        host_src = %key,
                        "UDP forward: binding limit reached ({}), dropping",
                        MAX_UDP_FORWARD_BINDINGS,
                    );
                    return;
                }
                let Some(handle) = self.create_flow_socket(sockets) else {
                    tracing::warn!(
                        host_src = %key,
                        "UDP forward: could not allocate smoltcp socket",
                    );
                    return;
                };
                self.flows.insert(
                    key,
                    UdpInboundFlow {
                        smoltcp_handle: handle,
                        host_socket: req.host_socket.clone(),
                        last_activity: Instant::now(),
                    },
                );
                self.flows.get_mut(&key).expect("just inserted")
            }
        };

        let socket = sockets.get_mut::<udp::Socket>(flow.smoltcp_handle);
        let dest = IpEndpoint::new(IpAddress::Ipv4(req.guest_ip), req.guest_port);
        if let Err(err) = socket.send_slice(&req.data, dest) {
            tracing::warn!(
                guest_ip = %req.guest_ip,
                guest_port = req.guest_port,
                "UDP forward: send_slice to guest failed: {:?}",
                err,
            );
        }
    }

    /// Poll each flow's smoltcp socket for guest replies, forwarding
    /// them back to the original host source via the stored host
    /// `UdpSocket`. Must be called once per gateway poll cycle.
    pub(super) fn poll_replies(&mut self, sockets: &mut SocketSet<'static>) {
        let now = Instant::now();
        for (host_src, flow) in self.flows.iter_mut() {
            let socket = sockets.get_mut::<udp::Socket>(flow.smoltcp_handle);
            let mut drained_any = false;
            while let Ok((data, _endpoint)) = socket.recv() {
                drained_any = true;
                let bytes = data.to_vec();
                let host_sock = flow.host_socket.clone();
                let target = *host_src;
                tokio::spawn(async move {
                    if let Err(err) = host_sock.send_to(&bytes, target).await {
                        tracing::warn!(
                            host_src = %target,
                            "UDP forward: reply send_to failed: {}",
                            err,
                        );
                    }
                });
            }
            if drained_any {
                flow.last_activity = now;
            }
        }
    }

    /// Remove flows whose last activity exceeded the timeout. Also
    /// removes the associated smoltcp socket from the socket set.
    pub(super) fn cleanup_expired(&mut self, sockets: &mut SocketSet<'static>) {
        let now = Instant::now();
        self.flows.retain(|_, flow| {
            let keep =
                now.saturating_duration_since(flow.last_activity) <= UDP_FORWARD_IDLE_TIMEOUT;
            if !keep {
                sockets.remove(flow.smoltcp_handle);
            }
            keep
        });
    }

    fn next_port(&mut self) -> u16 {
        let port = self.next_ephemeral_port;
        self.next_ephemeral_port = if port >= EPHEMERAL_PORT_END {
            EPHEMERAL_PORT_START
        } else {
            port + 1
        };
        port
    }

    #[cfg(test)]
    pub(super) fn flow_count(&self) -> usize {
        self.flows.len()
    }

    #[cfg(test)]
    pub(super) fn backdate_flow_for_test(&mut self, host_src: SocketAddrV4, age: Duration) {
        if let Some(flow) = self.flows.get_mut(&host_src) {
            flow.last_activity = Instant::now() - age;
        }
    }

    fn create_flow_socket(&mut self, sockets: &mut SocketSet<'static>) -> Option<SocketHandle> {
        let total_range = (EPHEMERAL_PORT_END - EPHEMERAL_PORT_START + 1) as usize;
        for _ in 0..total_range {
            let port = self.next_port();
            let rx_buf = PacketBuffer::new(
                vec![PacketMetadata::EMPTY; UDP_FORWARD_BUF_META],
                vec![0u8; UDP_FORWARD_BUF_BYTES],
            );
            let tx_buf = PacketBuffer::new(
                vec![PacketMetadata::EMPTY; UDP_FORWARD_BUF_META],
                vec![0u8; UDP_FORWARD_BUF_BYTES],
            );
            let mut socket = udp::Socket::new(rx_buf, tx_buf);
            if socket.bind(port).is_ok() {
                return Some(sockets.add(socket));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn next_port_wraps_around_end_of_range() {
        let mut table = UdpPortForwardTable::new();
        table.next_ephemeral_port = EPHEMERAL_PORT_END;
        assert_eq!(table.next_port(), EPHEMERAL_PORT_END);
        assert_eq!(table.next_port(), EPHEMERAL_PORT_START);
    }

    #[test]
    fn next_port_advances_within_range() {
        let mut table = UdpPortForwardTable::new();
        let first = table.next_port();
        let second = table.next_port();
        assert_eq!(first, EPHEMERAL_PORT_START);
        assert_eq!(second, EPHEMERAL_PORT_START + 1);
    }

    async fn dummy_host_socket() -> Arc<UdpSocket> {
        Arc::new(
            UdpSocket::bind("127.0.0.1:0")
                .await
                .expect("bind host test socket"),
        )
    }

    fn make_request(
        host_src_port: u16,
        host_socket: Arc<UdpSocket>,
        guest_port: u16,
    ) -> UdpPortForwardRequest {
        UdpPortForwardRequest {
            data: b"hello guest".to_vec(),
            host_src: SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), host_src_port),
            host_socket,
            guest_ip: Ipv4Addr::new(10, 0, 2, 15),
            guest_port,
        }
    }

    #[tokio::test]
    async fn handle_ingress_creates_flow_on_first_datagram() {
        let mut table = UdpPortForwardTable::new();
        let mut sockets = SocketSet::new(vec![]);
        let sock = dummy_host_socket().await;

        assert_eq!(table.flow_count(), 0);
        table.handle_ingress(&mut sockets, make_request(40_001, sock, 53));
        assert_eq!(table.flow_count(), 1);
    }

    #[tokio::test]
    async fn handle_ingress_reuses_flow_for_same_source() {
        let mut table = UdpPortForwardTable::new();
        let mut sockets = SocketSet::new(vec![]);
        let sock = dummy_host_socket().await;

        table.handle_ingress(&mut sockets, make_request(40_001, sock.clone(), 53));
        table.handle_ingress(&mut sockets, make_request(40_001, sock, 53));
        assert_eq!(table.flow_count(), 1);
    }

    #[tokio::test]
    async fn handle_ingress_allocates_distinct_flows_per_source() {
        let mut table = UdpPortForwardTable::new();
        let mut sockets = SocketSet::new(vec![]);
        let sock = dummy_host_socket().await;

        table.handle_ingress(&mut sockets, make_request(40_001, sock.clone(), 53));
        table.handle_ingress(&mut sockets, make_request(40_002, sock, 53));
        assert_eq!(table.flow_count(), 2);
    }

    #[tokio::test]
    async fn cleanup_expired_removes_stale_flows() {
        let mut table = UdpPortForwardTable::new();
        let mut sockets = SocketSet::new(vec![]);
        let sock = dummy_host_socket().await;

        let req = make_request(40_001, sock, 53);
        let key = req.host_src;
        table.handle_ingress(&mut sockets, req);
        assert_eq!(table.flow_count(), 1);

        table.backdate_flow_for_test(key, UDP_FORWARD_IDLE_TIMEOUT + Duration::from_secs(1));
        table.cleanup_expired(&mut sockets);
        assert_eq!(table.flow_count(), 0);
    }

    #[tokio::test]
    async fn cleanup_expired_keeps_fresh_flows() {
        let mut table = UdpPortForwardTable::new();
        let mut sockets = SocketSet::new(vec![]);
        let sock = dummy_host_socket().await;

        table.handle_ingress(&mut sockets, make_request(40_001, sock, 53));
        table.cleanup_expired(&mut sockets);
        assert_eq!(table.flow_count(), 1);
    }
}
