//! VM-backend policy enforcement: maps intercepted guest packets to a
//! hostname (via the DNS cache populated by the gateway's DNS proxy)
//! and runs the [`outpost::NetworkPolicy`] matcher on it.
//!
//! The declarative policy vocabulary (`NetworkPolicy`, `DomainPattern`,
//! `PolicyAction`, `PolicyRule`, `MatchCriteria`) lives in the
//! `outpost` crate and is re-exported from [`crate`] for consumers
//! that want to express policy without also pulling in VM internals.

use crate::dns::DnsCache;
use crate::frame::parse::parse_ipv4_frame;

use outpost::{MatchCriteria, NetworkPolicy, PolicyAction};
use smoltcp::wire::{IpProtocol, TcpPacket, UdpPacket};
use std::net::Ipv4Addr;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocol {
    Tcp,
    Udp,
    Icmp,
    Other(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketInfo {
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub protocol: TransportProtocol,
    pub dst_port: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyResult {
    Allow,
    Deny,
    Log,
}

pub struct PolicyChecker {
    default_action: PolicyResult,
    rules: Vec<CompiledRule>,
    dns_cache: Arc<RwLock<DnsCache>>,
}

struct CompiledRule {
    action: PolicyResult,
    matcher: CompiledMatcher,
}

enum CompiledMatcher {
    Any,
    Domain(outpost::DomainPattern),
    All(Vec<CompiledMatcher>),
}

impl PolicyChecker {
    pub fn new(policy: NetworkPolicy, dns_cache: Arc<RwLock<DnsCache>>) -> Self {
        Self {
            default_action: policy.default_action.into(),
            rules: policy
                .rules
                .iter()
                .map(|rule| CompiledRule {
                    action: rule.action.into(),
                    matcher: CompiledMatcher::from(&rule.criteria),
                })
                .collect(),
            dns_cache,
        }
    }

    pub fn check(&self, info: &PacketInfo) -> PolicyResult {
        let cache = self
            .dns_cache
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for rule in &self.rules {
            if rule.matcher.matches(info, &cache) {
                if rule.action == PolicyResult::Log {
                    tracing::info!(
                        "Policy log: {:?} {} -> {}:{}",
                        info.protocol,
                        info.src_ip,
                        info.dst_ip,
                        info.dst_port.unwrap_or(0)
                    );
                    continue;
                }

                return rule.action;
            }
        }

        self.default_action
    }

    pub fn extract_packet_info(frame: &[u8]) -> Option<PacketInfo> {
        let (_eth_frame, ip_packet) = parse_ipv4_frame(frame)?;

        let src_ip: Ipv4Addr = ip_packet.src_addr();
        let dst_ip: Ipv4Addr = ip_packet.dst_addr();

        let (protocol, dst_port) = match ip_packet.next_header() {
            IpProtocol::Tcp => {
                let tcp = TcpPacket::new_checked(ip_packet.payload()).ok()?;
                (TransportProtocol::Tcp, Some(tcp.dst_port()))
            }
            IpProtocol::Udp => {
                let udp = UdpPacket::new_checked(ip_packet.payload()).ok()?;
                (TransportProtocol::Udp, Some(udp.dst_port()))
            }
            IpProtocol::Icmp => (TransportProtocol::Icmp, None),
            other => (TransportProtocol::Other(other.into()), None),
        };

        Some(PacketInfo {
            src_ip,
            dst_ip,
            protocol,
            dst_port,
        })
    }
}

impl CompiledMatcher {
    fn matches(&self, info: &PacketInfo, dns_cache: &DnsCache) -> bool {
        match self {
            CompiledMatcher::Any => true,
            CompiledMatcher::Domain(pattern) => dns_cache
                .lookup(info.dst_ip)
                .is_some_and(|domain| pattern.matches(domain)),
            CompiledMatcher::All(inner) => {
                inner.iter().all(|matcher| matcher.matches(info, dns_cache))
            }
        }
    }
}

impl From<&MatchCriteria> for CompiledMatcher {
    fn from(value: &MatchCriteria) -> Self {
        match value {
            MatchCriteria::Any => Self::Any,
            MatchCriteria::Domain(pattern) => Self::Domain(pattern.clone()),
            MatchCriteria::All(inner) => Self::All(inner.iter().map(Self::from).collect()),
        }
    }
}

impl From<PolicyAction> for PolicyResult {
    fn from(value: PolicyAction) -> Self {
        match value {
            PolicyAction::Allow => PolicyResult::Allow,
            PolicyAction::Deny => PolicyResult::Deny,
            PolicyAction::Log => PolicyResult::Log,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outpost::{DomainPattern, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule};
    use smoltcp::phy::ChecksumCapabilities;
    use smoltcp::wire::{
        EthernetAddress, EthernetFrame, EthernetProtocol, EthernetRepr, IpProtocol, Ipv4Repr,
    };
    use std::sync::{Arc, RwLock};
    use std::time::Duration;

    fn dns_cache_with(ip: Ipv4Addr, domain: &str) -> Arc<RwLock<DnsCache>> {
        let cache = Arc::new(RwLock::new(DnsCache::new()));
        cache
            .write()
            .unwrap()
            .insert(ip, domain.to_string(), Duration::from_secs(300));
        cache
    }

    fn packet_info(dst_ip: Ipv4Addr) -> PacketInfo {
        PacketInfo {
            src_ip: Ipv4Addr::new(10, 0, 2, 15),
            dst_ip,
            protocol: TransportProtocol::Tcp,
            dst_port: Some(443),
        }
    }

    fn build_ipv4_frame(protocol: IpProtocol, payload: &[u8]) -> Vec<u8> {
        let total_len = 14 + 20 + payload.len();
        let mut frame = vec![0u8; total_len];

        let eth_repr = EthernetRepr {
            src_addr: EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]),
            dst_addr: EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x02]),
            ethertype: EthernetProtocol::Ipv4,
        };

        let mut eth_frame = EthernetFrame::new_unchecked(&mut frame);
        eth_repr.emit(&mut eth_frame);

        let ip_repr = Ipv4Repr {
            src_addr: smoltcp::wire::Ipv4Address::new(10, 0, 2, 15),
            dst_addr: smoltcp::wire::Ipv4Address::new(93, 184, 216, 34),
            next_header: protocol,
            payload_len: payload.len(),
            hop_limit: 64,
        };

        let mut ip_packet = smoltcp::wire::Ipv4Packet::new_unchecked(&mut frame[14..]);
        ip_repr.emit(&mut ip_packet, &ChecksumCapabilities::default());
        ip_packet.payload_mut().copy_from_slice(payload);

        frame
    }

    fn build_tcp_payload(dst_port: u16) -> Vec<u8> {
        let mut payload = vec![0u8; 20];
        payload[0..2].copy_from_slice(&12345u16.to_be_bytes());
        payload[2..4].copy_from_slice(&dst_port.to_be_bytes());
        payload[12] = 5 << 4; // header length = 20 bytes
        payload
    }

    fn build_udp_payload(dst_port: u16) -> Vec<u8> {
        let mut payload = vec![0u8; 8];
        payload[0..2].copy_from_slice(&53u16.to_be_bytes());
        payload[2..4].copy_from_slice(&dst_port.to_be_bytes());
        payload[4..6].copy_from_slice(&(8u16).to_be_bytes());
        payload
    }

    #[test]
    fn checker_uses_default_action_when_no_rule_matches() {
        let ip = Ipv4Addr::new(93, 184, 216, 34);
        let cache = dns_cache_with(ip, "example.com");

        let deny = PolicyChecker::new(NetworkPolicy::deny_all(), cache.clone());
        assert_eq!(deny.check(&packet_info(ip)), PolicyResult::Deny);

        let allow = PolicyChecker::new(NetworkPolicy::allow_all(), cache);
        assert_eq!(allow.check(&packet_info(ip)), PolicyResult::Allow);
    }

    #[test]
    fn checker_log_is_non_terminal_and_domain_miss_denies() {
        let ip = Ipv4Addr::new(93, 184, 216, 34);
        let cache = dns_cache_with(ip, "api.example.com");

        let allow_pattern = DomainPattern::parse("*.example.com").unwrap();
        let miss_pattern = DomainPattern::parse("*.internal.example").unwrap();

        let policy = NetworkPolicy {
            default_action: PolicyAction::Deny,
            rules: vec![
                PolicyRule {
                    action: PolicyAction::Log,
                    criteria: MatchCriteria::Any,
                },
                PolicyRule {
                    action: PolicyAction::Allow,
                    criteria: MatchCriteria::All(vec![
                        MatchCriteria::Any,
                        MatchCriteria::Domain(allow_pattern),
                    ]),
                },
                PolicyRule {
                    action: PolicyAction::Allow,
                    criteria: MatchCriteria::Domain(miss_pattern),
                },
            ],
        };

        let checker = PolicyChecker::new(policy, cache.clone());
        assert_eq!(checker.check(&packet_info(ip)), PolicyResult::Allow);

        let unknown_ip = Ipv4Addr::new(1, 1, 1, 1);
        assert_eq!(checker.check(&packet_info(unknown_ip)), PolicyResult::Deny);
    }

    #[test]
    fn extract_packet_info_parses_tcp_udp_icmp_and_invalid_frames() {
        let tcp_frame = build_ipv4_frame(IpProtocol::Tcp, &build_tcp_payload(443));
        let tcp = PolicyChecker::extract_packet_info(&tcp_frame).unwrap();
        assert_eq!(tcp.protocol, TransportProtocol::Tcp);
        assert_eq!(tcp.dst_port, Some(443));

        let udp_frame = build_ipv4_frame(IpProtocol::Udp, &build_udp_payload(5353));
        let udp = PolicyChecker::extract_packet_info(&udp_frame).unwrap();
        assert_eq!(udp.protocol, TransportProtocol::Udp);
        assert_eq!(udp.dst_port, Some(5353));

        let icmp_frame = build_ipv4_frame(IpProtocol::Icmp, &[0u8; 8]);
        let icmp = PolicyChecker::extract_packet_info(&icmp_frame).unwrap();
        assert_eq!(icmp.protocol, TransportProtocol::Icmp);
        assert_eq!(icmp.dst_port, None);

        let other_frame = build_ipv4_frame(IpProtocol::Unknown(99), &[]);
        let other = PolicyChecker::extract_packet_info(&other_frame).unwrap();
        assert_eq!(other.protocol, TransportProtocol::Other(99));

        let non_ipv4 = {
            let mut frame = vec![0u8; 14];
            let eth_repr = EthernetRepr {
                src_addr: EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]),
                dst_addr: EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x02]),
                ethertype: EthernetProtocol::Arp,
            };
            let mut eth = EthernetFrame::new_unchecked(&mut frame);
            eth_repr.emit(&mut eth);
            frame
        };

        assert!(PolicyChecker::extract_packet_info(&non_ipv4).is_none());
        assert!(PolicyChecker::extract_packet_info(&tcp_frame[..16]).is_none());
    }
}
