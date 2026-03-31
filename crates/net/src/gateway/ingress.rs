use crate::frame::craft::craft_tcp_rst;
use crate::frame::parse::parse_ipv4_frame;
use crate::policy::{PolicyChecker, PolicyResult};

use super::config::{is_ip_external, GatewayStackConfig};
use super::dns::DnsQueryInfo;
use super::outbound_buffer::OutboundFrameBuffer;
use super::GatewayStack;

use std::net::{Ipv4Addr, SocketAddrV4};

use super::tcp::{new_smoltcp_tcp_socket, InitiateResult};

use smoltcp::wire::{
    ArpOperation, ArpPacket, EthernetAddress, EthernetFrame, EthernetProtocol, IpAddress,
    IpListenEndpoint, IpProtocol, Ipv4Packet, TcpPacket, UdpPacket,
};

struct ExternalTcpInfo {
    guest_src: SocketAddrV4,
    remote_dst: SocketAddrV4,
    is_syn: bool,
}

enum FrameClassification {
    DnsQuery(DnsQueryInfo),
    ExternalTcp(ExternalTcpInfo),
    ExternalOther,
    ForGateway,
    Ignored,
}

impl FrameClassification {
    fn is_policy_subject(&self) -> bool {
        matches!(self, Self::ExternalTcp(_) | Self::ExternalOther)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyVerdict {
    NotApplicable,
    Allow,
    Deny,
}

fn classify_frame(frame: &[u8], config: &GatewayStackConfig) -> FrameClassification {
    let Some((eth, ip)) = parse_ipv4_frame(frame) else {
        unreachable!("caller ensures frame is IPv4");
    };

    let dst_ip: Ipv4Addr = ip.dst_addr();
    let src_ip: Ipv4Addr = ip.src_addr();
    let src_mac = eth.src_addr();
    let gateway_ip = config.gateway_ip;
    let gateway_mac = EthernetAddress(config.gateway_mac);

    if is_dns_query_to_gateway(&ip, dst_ip, gateway_ip) {
        if let Some(info) = parse_dns_query(&ip, src_mac, gateway_ip) {
            return FrameClassification::DnsQuery(info);
        }
    }

    if is_ip_external(dst_ip, gateway_ip, config.subnet_prefix) {
        if ip.next_header() == IpProtocol::Tcp {
            if let Ok(tcp) = TcpPacket::new_checked(ip.payload()) {
                return FrameClassification::ExternalTcp(ExternalTcpInfo {
                    guest_src: SocketAddrV4::new(src_ip, tcp.src_port()),
                    remote_dst: SocketAddrV4::new(dst_ip, tcp.dst_port()),
                    is_syn: tcp.syn() && !tcp.ack(),
                });
            }
        }
        return FrameClassification::ExternalOther;
    }

    if is_mac_for_gateway(eth.dst_addr(), gateway_mac) {
        return FrameClassification::ForGateway;
    }

    FrameClassification::Ignored
}

fn evaluate_policy(
    classification: &FrameClassification,
    frame: &[u8],
    checker: Option<&PolicyChecker>,
) -> PolicyVerdict {
    if !classification.is_policy_subject() {
        return PolicyVerdict::NotApplicable;
    }

    let Some(checker) = checker else {
        return PolicyVerdict::Allow;
    };

    let Some(info) = PolicyChecker::extract_packet_info(frame) else {
        tracing::debug!(
            "Policy skipped for outbound frame with unparseable transport metadata; allowing"
        );
        return PolicyVerdict::Allow;
    };

    match checker.check(&info) {
        PolicyResult::Deny => {
            tracing::debug!(
                "Policy denied: {:?} {} -> {}:{}",
                info.protocol,
                info.src_ip,
                info.dst_ip,
                info.dst_port.unwrap_or(0)
            );
            PolicyVerdict::Deny
        }
        PolicyResult::Allow | PolicyResult::Log => PolicyVerdict::Allow,
    }
}

fn is_dns_query_to_gateway(
    ip_packet: &Ipv4Packet<&[u8]>,
    dst_ip: Ipv4Addr,
    gateway_ip: Ipv4Addr,
) -> bool {
    dst_ip == gateway_ip
        && ip_packet.next_header() == IpProtocol::Udp
        && UdpPacket::new_checked(ip_packet.payload())
            .map(|udp| udp.dst_port() == 53)
            .unwrap_or(false)
}

fn parse_dns_query(
    ip_packet: &Ipv4Packet<&[u8]>,
    guest_mac: EthernetAddress,
    gateway_ip: Ipv4Addr,
) -> Option<DnsQueryInfo> {
    if ip_packet.dst_addr() != gateway_ip {
        return None;
    }

    if ip_packet.next_header() != IpProtocol::Udp {
        return None;
    }

    let udp_packet = UdpPacket::new_checked(ip_packet.payload()).ok()?;

    if udp_packet.dst_port() != 53 {
        return None;
    }

    Some(DnsQueryInfo {
        guest_mac,
        guest_ip: ip_packet.src_addr(),
        guest_port: udp_packet.src_port(),
        query_bytes: udp_packet.payload().to_vec(),
    })
}

impl GatewayStack {
    /// Handle a frame received from the guest.
    ///
    /// Queues any response frames to `outbound` instead of blocking,
    /// ensuring the main loop can continue draining incoming frames.
    pub(super) async fn handle_guest_frame(
        &mut self,
        frame: Vec<u8>,
        outbound: &mut OutboundFrameBuffer,
    ) {
        if parse_ipv4_frame(&frame).is_none() {
            // Non-IPv4 (e.g. ARP). With any_ip enabled, smoltcp responds to
            // ARP for ANY IP. Only feed ARP requests targeting the gateway IP.
            if is_arp_request_for(&frame, self.config.gateway_ip) {
                self.device.queue_rx_frame(frame);
            }
            return;
        }

        let classification = classify_frame(&frame, &self.config);

        if evaluate_policy(&classification, &frame, self.policy_checker.as_ref())
            == PolicyVerdict::Deny
        {
            if let Some(rst_frame) = craft_rst_for_denied_tcp_syn(&frame, self.config.gateway_mac) {
                outbound.push_logged(rst_frame, "RST");
            }
            return;
        }

        match classification {
            FrameClassification::DnsQuery(query_info) => {
                self.dns.dispatch_query(query_info);
            }
            FrameClassification::ExternalTcp(tcp_info) => {
                self.handle_external_tcp(frame, tcp_info);
            }
            FrameClassification::ExternalOther => {
                self.nat.process_frame(&frame).await;
            }
            FrameClassification::ForGateway => {
                // Feed gateway-destined or broadcast frames to smoltcp (ARP, ICMP
                // to gateway, DHCP). With any_ip enabled, smoltcp would otherwise
                // consume unicast frames meant for other VMs on the same subnet.
                self.device.queue_rx_frame(frame);
            }
            FrameClassification::Ignored => {}
        }
    }

    fn handle_external_tcp(&mut self, frame: Vec<u8>, info: ExternalTcpInfo) {
        if info.is_syn {
            let mut socket = new_smoltcp_tcp_socket();

            let endpoint = IpListenEndpoint {
                addr: Some(IpAddress::Ipv4(*info.remote_dst.ip())),
                port: info.remote_dst.port(),
            };

            if socket.listen(endpoint).is_err() {
                tracing::debug!("TCP manager: failed to listen on {}", info.remote_dst);
                return;
            }

            match self.tcp_manager.initiate(
                socket,
                info.remote_dst,
                info.guest_src,
                &mut self.sockets,
            ) {
                InitiateResult::Created(handle) => {
                    tracing::debug!(
                        dst = %info.remote_dst,
                        handle = ?handle,
                        "TCP manager: new outbound connection via smoltcp"
                    );
                }
                InitiateResult::DuplicateFlow => {
                    // Existing smoltcp socket needs the retransmitted SYN to
                    // complete its handshake — fall through to queue_rx_frame.
                }
                InitiateResult::RejectedLimit => {
                    tracing::warn!(
                        dst = %info.remote_dst,
                        "TCP manager: connection limit reached, dropping SYN"
                    );
                    return;
                }
            }
        }

        // Feed frame to smoltcp (SYN or data/ACK/FIN/RST for existing connections)
        self.device.queue_rx_frame(frame);
    }
}

fn craft_rst_for_denied_tcp_syn(frame: &[u8], gateway_mac: [u8; 6]) -> Option<Vec<u8>> {
    let (eth_frame, ip_packet, tcp_packet) = denied_tcp_syn_components(frame)?;

    let guest_addr = SocketAddrV4::new(ip_packet.src_addr(), tcp_packet.src_port());
    let remote_addr = SocketAddrV4::new(ip_packet.dst_addr(), tcp_packet.dst_port());
    let guest_seq = tcp_packet.seq_number().0 as u32;

    Some(craft_tcp_rst(
        remote_addr,
        guest_addr,
        0,
        guest_seq.wrapping_add(1),
        EthernetAddress(gateway_mac),
        eth_frame.src_addr(),
    ))
}

#[allow(clippy::type_complexity)]
fn denied_tcp_syn_components(
    frame: &[u8],
) -> Option<(EthernetFrame<&[u8]>, Ipv4Packet<&[u8]>, TcpPacket<&[u8]>)> {
    let (eth_frame, ip_packet) = parse_ipv4_frame(frame)?;
    if ip_packet.next_header() != IpProtocol::Tcp {
        return None;
    }

    let tcp_packet = TcpPacket::new_checked(ip_packet.payload()).ok()?;
    if !tcp_packet.syn() || tcp_packet.ack() {
        return None;
    }

    Some((eth_frame, ip_packet, tcp_packet))
}

/// Check if a raw Ethernet frame is an ARP request targeting `gateway_ip`.
///
/// With smoltcp's `any_ip` mode, it responds to ARP for ANY IP address.
/// We only forward ARP requests for the gateway to smoltcp so that
/// VM-to-VM ARP resolves correctly.
fn is_arp_request_for(frame: &[u8], gateway_ip: Ipv4Addr) -> bool {
    let Ok(eth) = EthernetFrame::new_checked(frame) else {
        return false;
    };
    if eth.ethertype() != EthernetProtocol::Arp {
        return false;
    }
    let Ok(arp) = ArpPacket::new_checked(eth.payload()) else {
        return false;
    };
    if arp.operation() != ArpOperation::Request {
        return false;
    }
    let addr = arp.target_protocol_addr();
    if addr.len() != 4 {
        return false;
    }
    Ipv4Addr::new(addr[0], addr[1], addr[2], addr[3]) == gateway_ip
}

/// Check if the Ethernet destination MAC indicates the frame is for the gateway.
fn is_mac_for_gateway(dst_mac: EthernetAddress, gateway_mac: EthernetAddress) -> bool {
    dst_mac == gateway_mac || dst_mac.is_broadcast() || dst_mac.is_multicast()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dns::DnsCache;
    use crate::frame::{frame_channel, EthernetFrameIO, FrameReader, FrameWriter};
    use crate::policy::NetworkPolicy;
    use smoltcp::phy::ChecksumCapabilities;
    use smoltcp::wire::{ArpRepr, EthernetRepr, Ipv4Repr, UdpRepr};
    use std::io;
    use std::sync::{Arc, RwLock};

    const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 2);
    const GUEST_IP: Ipv4Addr = Ipv4Addr::new(10, 0, 2, 15);
    const EXTERNAL_IP: Ipv4Addr = Ipv4Addr::new(93, 184, 216, 34);

    fn gateway_mac() -> EthernetAddress {
        EthernetAddress([0x52, 0x54, 0x00, 0x00, 0x00, 0x01])
    }

    fn guest_mac() -> EthernetAddress {
        EthernetAddress([0x52, 0x54, 0x00, 0x00, 0x00, 0x02])
    }

    fn checker(policy: NetworkPolicy) -> PolicyChecker {
        PolicyChecker::new(policy, Arc::new(RwLock::new(DnsCache::new())))
    }

    struct TestFrameIo;
    struct TestReader;
    struct TestWriter;

    impl EthernetFrameIO for TestFrameIo {
        type ReadHalf = TestReader;
        type WriteHalf = TestWriter;

        fn split(self) -> (Self::ReadHalf, Self::WriteHalf) {
            (TestReader, TestWriter)
        }
    }

    impl FrameReader for TestReader {
        async fn recv_frame(&mut self) -> io::Result<Vec<u8>> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "test reader closed",
            ))
        }
    }

    impl FrameWriter for TestWriter {
        async fn send_frame(&mut self, _frame: &[u8]) -> io::Result<()> {
            Ok(())
        }
    }

    async fn new_stack(policy: Option<NetworkPolicy>) -> GatewayStack {
        let mut cfg = default_config();
        cfg.policy = policy;
        GatewayStack::new(TestFrameIo, cfg).await
    }

    fn outbound_buffer() -> OutboundFrameBuffer {
        let (tx, _rx) = frame_channel(16);
        OutboundFrameBuffer::new(tx, 16, 16)
    }

    fn build_ipv4_frame(
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        src_mac: EthernetAddress,
        dst_mac: EthernetAddress,
        protocol: IpProtocol,
        payload: &[u8],
    ) -> Vec<u8> {
        let total_len = 14 + 20 + payload.len();
        let mut frame = vec![0u8; total_len];

        let eth_repr = EthernetRepr {
            src_addr: src_mac,
            dst_addr: dst_mac,
            ethertype: EthernetProtocol::Ipv4,
        };
        let mut eth_frame = EthernetFrame::new_unchecked(&mut frame);
        eth_repr.emit(&mut eth_frame);

        let ip_repr = Ipv4Repr {
            src_addr: src_ip,
            dst_addr: dst_ip,
            next_header: protocol,
            payload_len: payload.len(),
            hop_limit: 64,
        };

        let mut ip_packet = Ipv4Packet::new_unchecked(&mut frame[14..]);
        ip_repr.emit(&mut ip_packet, &ChecksumCapabilities::default());
        ip_packet.payload_mut().copy_from_slice(payload);

        frame
    }

    fn build_tcp_segment(
        src_port: u16,
        dst_port: u16,
        seq: u32,
        ack: Option<u32>,
        syn: bool,
        ack_flag: bool,
    ) -> Vec<u8> {
        let mut tcp = vec![0u8; 20];
        tcp[0..2].copy_from_slice(&src_port.to_be_bytes());
        tcp[2..4].copy_from_slice(&dst_port.to_be_bytes());
        tcp[4..8].copy_from_slice(&seq.to_be_bytes());
        tcp[8..12].copy_from_slice(&ack.unwrap_or(0).to_be_bytes());
        tcp[12] = 5 << 4; // 20-byte header

        let mut flags = 0u8;
        if syn {
            flags |= 0x02;
        }
        if ack_flag {
            flags |= 0x10;
        }
        tcp[13] = flags;
        tcp[14..16].copy_from_slice(&64240u16.to_be_bytes());
        tcp
    }

    fn src_ip_bytes() -> [u8; 4] {
        GUEST_IP.octets()
    }

    fn dst_ip_bytes() -> [u8; 4] {
        EXTERNAL_IP.octets()
    }

    fn build_udp_segment(src_port: u16, dst_port: u16, payload: &[u8]) -> Vec<u8> {
        let mut udp = vec![0u8; 8 + payload.len()];
        let repr = UdpRepr { src_port, dst_port };
        let src = smoltcp::wire::IpAddress::Ipv4(smoltcp::wire::Ipv4Address::from(src_ip_bytes()));
        let dst = smoltcp::wire::IpAddress::Ipv4(smoltcp::wire::Ipv4Address::from(dst_ip_bytes()));
        let mut pkt = UdpPacket::new_unchecked(&mut udp);
        repr.emit(
            &mut pkt,
            &src,
            &dst,
            payload.len(),
            |buf| buf.copy_from_slice(payload),
            &ChecksumCapabilities::default(),
        );
        udp
    }

    fn build_arp_request(sender_ip: Ipv4Addr, target_ip: Ipv4Addr) -> Vec<u8> {
        let repr = ArpRepr::EthernetIpv4 {
            operation: ArpOperation::Request,
            source_hardware_addr: guest_mac(),
            source_protocol_addr: smoltcp::wire::Ipv4Address::from_octets(sender_ip.octets()),
            target_hardware_addr: EthernetAddress([0xff; 6]),
            target_protocol_addr: smoltcp::wire::Ipv4Address::from_octets(target_ip.octets()),
        };

        let arp_len = repr.buffer_len();
        let mut buf = vec![0u8; 14 + arp_len];
        let mut eth = EthernetFrame::new_unchecked(&mut buf);
        eth.set_dst_addr(EthernetAddress([0xff; 6]));
        eth.set_src_addr(guest_mac());
        eth.set_ethertype(EthernetProtocol::Arp);

        let mut arp = ArpPacket::new_unchecked(eth.payload_mut());
        repr.emit(&mut arp);
        buf
    }

    fn default_config() -> GatewayStackConfig {
        GatewayStackConfig {
            gateway_ip: GATEWAY_IP,
            subnet_prefix: 24,
            dhcp_range_start: Ipv4Addr::new(10, 0, 2, 15),
            dhcp_range_end: Ipv4Addr::new(10, 0, 2, 254),
            gateway_mac: gateway_mac().0,
            policy: None,
            port_forwards: vec![],
        }
    }

    #[test]
    fn external_frames_are_policy_subject() {
        let config = default_config();

        let tcp_syn = build_tcp_segment(40000, 443, 1234, None, true, false);
        let tcp_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &tcp_syn,
        );
        let tcp_class = classify_frame(&tcp_frame, &config);
        assert!(tcp_class.is_policy_subject());

        let udp_payload = build_udp_segment(40000, 53, b"dns");
        let udp_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Udp,
            &udp_payload,
        );
        let udp_class = classify_frame(&udp_frame, &config);
        assert!(udp_class.is_policy_subject());
    }

    #[test]
    fn dns_and_dhcp_frames_are_not_policy_subject() {
        let config = default_config();

        let dns_payload = build_udp_segment(40000, 53, b"query");
        let dns_frame = build_ipv4_frame(
            GUEST_IP,
            GATEWAY_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Udp,
            &dns_payload,
        );
        let dns_class = classify_frame(&dns_frame, &config);
        assert!(!dns_class.is_policy_subject());

        let dhcp_payload = build_udp_segment(68, 67, b"dhcp");
        let dhcp_frame = build_ipv4_frame(
            GUEST_IP,
            GATEWAY_IP,
            guest_mac(),
            EthernetAddress([0xff; 6]),
            IpProtocol::Udp,
            &dhcp_payload,
        );
        let dhcp_class = classify_frame(&dhcp_frame, &config);
        assert!(!dhcp_class.is_policy_subject());
    }

    #[test]
    fn deny_policy_blocks_external_udp_and_icmp() {
        let config = default_config();
        let checker = checker(NetworkPolicy::deny_all());

        let udp_payload = build_udp_segment(40000, 443, b"hello");
        let udp_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Udp,
            &udp_payload,
        );
        let udp_class = classify_frame(&udp_frame, &config);
        assert_eq!(
            evaluate_policy(&udp_class, &udp_frame, Some(&checker)),
            PolicyVerdict::Deny
        );

        let icmp_echo = [8u8, 0, 0, 0, 0, 1, 0, 1];
        let icmp_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Icmp,
            &icmp_echo,
        );
        let icmp_class = classify_frame(&icmp_frame, &config);
        assert_eq!(
            evaluate_policy(&icmp_class, &icmp_frame, Some(&checker)),
            PolicyVerdict::Deny
        );
    }

    #[test]
    fn allow_policy_keeps_external_path_open() {
        let config = default_config();
        let checker = checker(NetworkPolicy::allow_all());

        let tcp_syn = build_tcp_segment(40000, 443, 555, None, true, false);
        let tcp_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &tcp_syn,
        );
        let tcp_class = classify_frame(&tcp_frame, &config);
        assert_eq!(
            evaluate_policy(&tcp_class, &tcp_frame, Some(&checker)),
            PolicyVerdict::Allow
        );
    }

    #[test]
    fn malformed_external_frame_treated_like_no_policy_when_allow_all() {
        let config = default_config();
        let malformed_tcp = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &[1, 2, 3, 4],
        );
        let class = classify_frame(&malformed_tcp, &config);
        assert!(matches!(class, FrameClassification::ExternalOther));

        let allow_all = checker(NetworkPolicy::allow_all());

        assert_eq!(
            evaluate_policy(&class, &malformed_tcp, None),
            PolicyVerdict::Allow
        );
        assert_eq!(
            evaluate_policy(&class, &malformed_tcp, Some(&allow_all)),
            PolicyVerdict::Allow
        );
    }

    #[test]
    fn deny_policy_does_not_touch_gateway_control_plane() {
        let config = default_config();
        let checker = checker(NetworkPolicy::deny_all());

        let dhcp_payload = build_udp_segment(68, 67, b"dhcp");
        let dhcp_frame = build_ipv4_frame(
            GUEST_IP,
            GATEWAY_IP,
            guest_mac(),
            EthernetAddress([0xff; 6]),
            IpProtocol::Udp,
            &dhcp_payload,
        );
        let dhcp_class = classify_frame(&dhcp_frame, &config);
        assert_eq!(
            evaluate_policy(&dhcp_class, &dhcp_frame, Some(&checker)),
            PolicyVerdict::NotApplicable
        );
    }

    #[test]
    fn rst_is_emitted_only_for_denied_syn_without_ack() {
        let syn = build_tcp_segment(40000, 443, 1000, None, true, false);
        let syn_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &syn,
        );
        assert!(craft_rst_for_denied_tcp_syn(&syn_frame, gateway_mac().0).is_some());

        let syn_ack = build_tcp_segment(40000, 443, 1000, Some(2000), true, true);
        let syn_ack_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &syn_ack,
        );
        assert!(craft_rst_for_denied_tcp_syn(&syn_ack_frame, gateway_mac().0).is_none());

        let ack = build_tcp_segment(40000, 443, 1000, Some(2000), false, true);
        let ack_frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &ack,
        );
        assert!(craft_rst_for_denied_tcp_syn(&ack_frame, gateway_mac().0).is_none());
    }

    #[test]
    fn denied_tcp_rst_targets_guest_and_acks_syn() {
        let syn = build_tcp_segment(45678, 443, 12345, None, true, false);
        let frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &syn,
        );

        let rst = craft_rst_for_denied_tcp_syn(&frame, gateway_mac().0).expect("rst expected");

        let (eth, ip) = parse_ipv4_frame(&rst).expect("valid ipv4 rst");
        let tcp = TcpPacket::new_checked(ip.payload()).expect("valid tcp rst");

        assert_eq!(eth.src_addr(), gateway_mac());
        assert_eq!(eth.dst_addr(), guest_mac());
        assert_eq!(ip.src_addr(), EXTERNAL_IP);
        assert_eq!(ip.dst_addr(), GUEST_IP);
        assert!(tcp.rst());
        assert_eq!(tcp.ack_number().0 as u32, 12346);
    }

    #[tokio::test]
    async fn denied_tcp_syn_in_handle_guest_frame_queues_rst_and_drops_ingress() {
        let mut stack = new_stack(Some(NetworkPolicy::deny_all())).await;
        let mut outbound = outbound_buffer();

        let syn = build_tcp_segment(45678, 443, 12345, None, true, false);
        let frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &syn,
        );

        stack.handle_guest_frame(frame, &mut outbound).await;

        assert!(!stack.device.has_pending_rx());
        assert_eq!(outbound.pending_len(), 1);
    }

    #[tokio::test]
    async fn allow_all_tcp_syn_reaches_smoltcp_path() {
        let mut stack = new_stack(Some(NetworkPolicy::allow_all())).await;
        let mut outbound = outbound_buffer();

        let syn = build_tcp_segment(45678, 443, 12345, None, true, false);
        let frame = build_ipv4_frame(
            GUEST_IP,
            EXTERNAL_IP,
            guest_mac(),
            gateway_mac(),
            IpProtocol::Tcp,
            &syn,
        );

        stack.handle_guest_frame(frame, &mut outbound).await;

        assert!(stack.device.has_pending_rx());
        assert_eq!(outbound.pending_len(), 0);
    }

    #[tokio::test]
    async fn deny_policy_still_allows_dhcp_frames_to_gateway() {
        let mut stack = new_stack(Some(NetworkPolicy::deny_all())).await;
        let mut outbound = outbound_buffer();

        let dhcp_payload = build_udp_segment(68, 67, b"dhcp");
        let frame = build_ipv4_frame(
            GUEST_IP,
            GATEWAY_IP,
            guest_mac(),
            EthernetAddress([0xff; 6]),
            IpProtocol::Udp,
            &dhcp_payload,
        );

        stack.handle_guest_frame(frame, &mut outbound).await;

        assert!(stack.device.has_pending_rx());
        assert_eq!(outbound.pending_len(), 0);
    }

    #[test]
    fn arp_request_for_gateway_accepted() {
        let frame = build_arp_request(GUEST_IP, GATEWAY_IP);
        assert!(is_arp_request_for(&frame, GATEWAY_IP));
    }

    #[test]
    fn arp_request_for_other_vm_rejected() {
        let frame = build_arp_request(GUEST_IP, Ipv4Addr::new(10, 0, 2, 20));
        assert!(!is_arp_request_for(&frame, GATEWAY_IP));
    }

    #[test]
    fn arp_truncated_frame_rejected() {
        assert!(!is_arp_request_for(&[0u8; 10], GATEWAY_IP));
    }

    #[test]
    fn mac_for_gateway_unicast_to_gateway() {
        assert!(is_mac_for_gateway(gateway_mac(), gateway_mac()));
    }

    #[test]
    fn mac_for_gateway_unicast_to_other_vm_rejected() {
        assert!(!is_mac_for_gateway(guest_mac(), gateway_mac()));
    }
}
