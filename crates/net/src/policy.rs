use crate::dns::DnsCache;
use crate::frame::parse::parse_ipv4_frame;

use smoltcp::wire::{IpProtocol, TcpPacket, UdpPacket};
use std::fmt;
use std::net::Ipv4Addr;
use std::sync::{Arc, RwLock};

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPattern {
    Exact(String),
    Wildcard(String),
}

impl DomainPattern {
    pub fn parse(pattern: &str) -> Result<Self, DomainPatternParseError> {
        let normalized = normalize_host_pattern(pattern)?;

        if normalized.starts_with('*') {
            if !normalized.starts_with("*.") {
                return Err(DomainPatternParseError::MalformedWildcard);
            }

            let suffix = normalized
                .strip_prefix("*.")
                .ok_or(DomainPatternParseError::MalformedWildcard)?;
            if suffix.contains('*') {
                return Err(DomainPatternParseError::MalformedWildcard);
            }
            validate_hostname(suffix)?;
            return Ok(Self::Wildcard(suffix.to_string()));
        }

        validate_hostname(&normalized)?;
        Ok(Self::Exact(normalized))
    }

    pub fn matches(&self, domain: &str) -> bool {
        let normalized = normalize_domain_for_match(domain);
        let Ok(domain) = normalized else {
            return false;
        };

        match self {
            DomainPattern::Exact(expected) => &domain == expected,
            DomainPattern::Wildcard(suffix) => {
                domain.len() > suffix.len()
                    && domain.ends_with(suffix)
                    && domain
                        .as_bytes()
                        .get(domain.len().saturating_sub(suffix.len() + 1))
                        == Some(&b'.')
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainPatternParseError {
    Empty,
    GlobalWildcardNotAllowed,
    MalformedWildcard,
    DomainTooLong,
    EmptyLabel,
    LabelTooLong,
    InvalidCharacter(char),
}

impl fmt::Display for DomainPatternParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "host pattern cannot be empty"),
            Self::GlobalWildcardNotAllowed => {
                write!(
                    f,
                    "'*' is not a domain pattern; use it only in allow-host policy lists"
                )
            }
            Self::MalformedWildcard => write!(
                f,
                "wildcard host pattern must use only a leading '*.' prefix (e.g. *.example.com)"
            ),
            Self::DomainTooLong => {
                write!(f, "hostname exceeds 253 characters")
            }
            Self::EmptyLabel => write!(f, "hostname contains an empty label"),
            Self::LabelTooLong => write!(f, "hostname label exceeds 63 characters"),
            Self::InvalidCharacter(ch) => {
                write!(f, "hostname contains invalid character '{ch}'")
            }
        }
    }
}

impl std::error::Error for DomainPatternParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAction {
    Allow,
    Deny,
    Log,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchCriteria {
    Any,
    Domain(DomainPattern),
    All(Vec<MatchCriteria>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRule {
    pub action: PolicyAction,
    pub criteria: MatchCriteria,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPolicy {
    pub default_action: PolicyAction,
    pub rules: Vec<PolicyRule>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self::allow_all()
    }
}

impl NetworkPolicy {
    pub fn deny_all() -> Self {
        Self {
            default_action: PolicyAction::Deny,
            rules: Vec::new(),
        }
    }

    pub fn allow_all() -> Self {
        Self {
            default_action: PolicyAction::Allow,
            rules: Vec::new(),
        }
    }

    pub fn allow_domain(mut self, pattern: DomainPattern) -> Self {
        self.rules.push(PolicyRule {
            action: PolicyAction::Allow,
            criteria: MatchCriteria::Domain(pattern),
        });
        self
    }

    pub fn from_allowed_hosts<'a>(
        hosts: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, DomainPatternParseError> {
        let mut has_global_wildcard = false;
        let mut patterns = Vec::new();

        for raw in hosts {
            let normalized = normalize_host_pattern(raw)?;
            if normalized == "*" {
                has_global_wildcard = true;
                continue;
            }

            patterns.push(DomainPattern::parse(&normalized)?);
        }

        if has_global_wildcard {
            return Ok(Self::allow_all());
        }

        let mut policy = Self::deny_all();
        for pattern in patterns {
            policy = policy.allow_domain(pattern);
        }
        Ok(policy)
    }
}

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
    Domain(DomainPattern),
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

fn normalize_host_pattern(pattern: &str) -> Result<String, DomainPatternParseError> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return Err(DomainPatternParseError::Empty);
    }

    let lowered = trimmed.to_ascii_lowercase();
    let without_dot = lowered.strip_suffix('.').unwrap_or(&lowered);

    if without_dot.is_empty() {
        return Err(DomainPatternParseError::Empty);
    }

    if without_dot == "*" {
        return Ok(without_dot.to_string());
    }

    if without_dot.contains('*') && !without_dot.starts_with("*.") {
        return Err(DomainPatternParseError::MalformedWildcard);
    }

    Ok(without_dot.to_string())
}

fn normalize_domain_for_match(domain: &str) -> Result<String, DomainPatternParseError> {
    let normalized = normalize_host_pattern(domain)?;
    if normalized == "*" {
        return Err(DomainPatternParseError::GlobalWildcardNotAllowed);
    }
    validate_hostname(&normalized)?;
    Ok(normalized)
}

fn validate_hostname(hostname: &str) -> Result<(), DomainPatternParseError> {
    if hostname == "*" {
        return Err(DomainPatternParseError::GlobalWildcardNotAllowed);
    }

    if hostname.len() > MAX_DOMAIN_LEN {
        return Err(DomainPatternParseError::DomainTooLong);
    }

    for label in hostname.split('.') {
        if label.is_empty() {
            return Err(DomainPatternParseError::EmptyLabel);
        }
        if label.len() > MAX_LABEL_LEN {
            return Err(DomainPatternParseError::LabelTooLong);
        }
        for ch in label.chars() {
            if !(ch.is_ascii_alphanumeric() || ch == '-') {
                return Err(DomainPatternParseError::InvalidCharacter(ch));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn parse_exact_pattern() {
        let parsed = DomainPattern::parse("api.example.com").unwrap();
        assert_eq!(parsed, DomainPattern::Exact("api.example.com".to_string()));
    }

    #[test]
    fn parse_wildcard_pattern() {
        let parsed = DomainPattern::parse("*.example.com").unwrap();
        assert_eq!(parsed, DomainPattern::Wildcard("example.com".to_string()));
    }

    #[test]
    fn wildcard_matches_subdomain_only() {
        let pattern = DomainPattern::parse("*.example.com").unwrap();

        assert!(pattern.matches("api.example.com"));
        assert!(pattern.matches("deep.api.example.com"));
        assert!(!pattern.matches("example.com"));
    }

    #[test]
    fn parse_rejects_malformed_wildcards() {
        assert!(matches!(
            DomainPattern::parse("*example.com"),
            Err(DomainPatternParseError::MalformedWildcard)
        ));
        assert!(matches!(
            DomainPattern::parse("foo.*.com"),
            Err(DomainPatternParseError::MalformedWildcard)
        ));
        assert!(matches!(
            DomainPattern::parse("*."),
            Err(DomainPatternParseError::MalformedWildcard)
        ));
        assert!(matches!(
            DomainPattern::parse("*.*.example.com"),
            Err(DomainPatternParseError::MalformedWildcard)
        ));
    }

    #[test]
    fn parse_normalizes_input() {
        let parsed = DomainPattern::parse("  API.Example.COM.  ").unwrap();
        assert_eq!(parsed, DomainPattern::Exact("api.example.com".to_string()));
    }

    #[test]
    fn parse_rejects_label_length_over_63() {
        let long_label = "a".repeat(64);
        let host = format!("{long_label}.example.com");
        assert!(matches!(
            DomainPattern::parse(&host),
            Err(DomainPatternParseError::LabelTooLong)
        ));
    }

    #[test]
    fn parse_rejects_total_length_over_253() {
        let long_domain = format!("{}.com", "a".repeat(250));
        assert!(matches!(
            DomainPattern::parse(&long_domain),
            Err(DomainPatternParseError::DomainTooLong)
        ));
    }

    #[test]
    fn from_allowed_hosts_star_returns_allow_all() {
        let policy =
            NetworkPolicy::from_allowed_hosts(["example.com", "*", "*.internal"].iter().copied())
                .unwrap();

        assert_eq!(policy.default_action, PolicyAction::Allow);
        assert!(policy.rules.is_empty());
    }

    #[test]
    fn from_allowed_hosts_builds_deny_default_with_allow_rules() {
        let policy =
            NetworkPolicy::from_allowed_hosts(["example.com", "*.example.org"].iter().copied())
                .unwrap();

        assert_eq!(policy.default_action, PolicyAction::Deny);
        assert_eq!(policy.rules.len(), 2);
        assert!(matches!(
            policy.rules[0].criteria,
            MatchCriteria::Domain(DomainPattern::Exact(_))
        ));
        assert!(matches!(
            policy.rules[1].criteria,
            MatchCriteria::Domain(DomainPattern::Wildcard(_))
        ));
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
