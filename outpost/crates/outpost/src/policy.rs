//! Host-allowlist policy vocabulary.
//!
//! Declarative types only: `DomainPattern` for host patterns,
//! `NetworkPolicy` for a full policy, plus its constituent
//! [`PolicyRule`] / [`MatchCriteria`] / [`PolicyAction`]. Runtime
//! enforcement (packet-filtering in capsa-vmnet, CONNECT handling in
//! outpost-proxy) lives in each backend and calls into
//! [`NetworkPolicy::matches_host`] to map a hostname to a verdict.

use serde::{Deserialize, Serialize};
use std::fmt;

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

    fn as_host_pattern(&self) -> String {
        match self {
            DomainPattern::Exact(host) => host.clone(),
            DomainPattern::Wildcard(suffix) => format!("*.{suffix}"),
        }
    }
}

impl Serialize for DomainPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.as_host_pattern())
    }
}

impl<'de> Deserialize<'de> for DomainPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        DomainPattern::parse(&raw).map_err(serde::de::Error::custom)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyAction {
    Allow,
    Deny,
    Log,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchCriteria {
    Any,
    Domain(DomainPattern),
    All(Vec<MatchCriteria>),
}

impl MatchCriteria {
    /// Match against a hostname string. Backends that enforce policy
    /// at a layer where hostnames are directly on the wire (e.g. an
    /// HTTP CONNECT proxy reading the target authority) call this.
    /// Backends enforcing on IPs (capsa-vmnet's packet filter) need a
    /// separate IP→domain resolution step before invoking an
    /// equivalent matcher.
    pub fn matches_host(&self, host: &str) -> bool {
        match self {
            MatchCriteria::Any => true,
            MatchCriteria::Domain(pattern) => pattern.matches(host),
            MatchCriteria::All(inner) => inner.iter().all(|c| c.matches_host(host)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub action: PolicyAction,
    pub criteria: MatchCriteria,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    /// Build a policy from a list of allowed hostnames.
    ///
    /// Accepts exact hostnames (`api.example.com`), subdomain wildcards
    /// (`*.example.com`), and the global wildcard (`*`), which yields an
    /// [`Self::allow_all`] policy.
    ///
    /// # Trust model
    ///
    /// `allow_hosts` is a *hostname* allowlist, not an *address*
    /// allowlist. The policy admits a connection when its target
    /// hostname matches a listed pattern; the address it resolves to is
    /// not constrained.
    ///
    /// - DNS resolution happens per request, so the set of reachable
    ///   IPs can shift between requests if records change (DNS
    ///   rebinding). Allowing a domain implicitly trusts whoever
    ///   controls its authoritative DNS.
    /// - Hostnames that resolve to loopback (`127.0.0.0/8`, `::1`),
    ///   RFC1918 private ranges, link-local (`169.254.0.0/16`,
    ///   `fe80::/10`), or cloud-metadata addresses (e.g.
    ///   `169.254.169.254`) give the sandboxed program reach into the
    ///   host network. That is sometimes intentional (a sandboxed tool
    ///   talking to a local dev server, or scoped corp-internal
    ///   access); the deny-by-default mode does not grant it, so be
    ///   sure it is what you want.
    /// - Wildcards like `*.example.com` extend trust to everyone who
    ///   controls subdomains of the parent. Avoid wildcards on domains
    ///   you do not own or fully control.
    ///
    /// The caller owns the allowlist's attack surface — outpost
    /// enforces the listed patterns literally and does not filter
    /// resolved addresses after the fact.
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

    /// Evaluate the policy against a hostname. Rules are scanned in
    /// order; the first terminal match wins. `PolicyAction::Log` is
    /// non-terminal: a matching Log rule is skipped over and scanning
    /// continues. This method is a pure verdict — emitting telemetry
    /// for matched Log rules is the caller's responsibility.
    /// Returns [`Self::default_action`] if no rule produces a verdict.
    pub fn matches_host(&self, host: &str) -> PolicyAction {
        for rule in &self.rules {
            if rule.criteria.matches_host(host) {
                if matches!(rule.action, PolicyAction::Log) {
                    continue;
                }
                return rule.action;
            }
        }
        self.default_action
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
    fn matches_host_uses_default_action_when_no_rule_matches() {
        assert_eq!(
            NetworkPolicy::deny_all().matches_host("example.com"),
            PolicyAction::Deny
        );
        assert_eq!(
            NetworkPolicy::allow_all().matches_host("example.com"),
            PolicyAction::Allow
        );
    }

    #[test]
    fn matches_host_applies_allow_rules_over_deny_default() {
        let policy = NetworkPolicy::from_allowed_hosts(
            ["api.example.com", "*.cdn.example.com"].iter().copied(),
        )
        .unwrap();

        assert_eq!(policy.matches_host("api.example.com"), PolicyAction::Allow);
        assert_eq!(
            policy.matches_host("img.cdn.example.com"),
            PolicyAction::Allow
        );
        assert_eq!(policy.matches_host("evil.com"), PolicyAction::Deny);
        assert_eq!(policy.matches_host("cdn.example.com"), PolicyAction::Deny);
    }

    #[test]
    fn matches_host_log_is_non_terminal() {
        let allow_pattern = DomainPattern::parse("*.example.com").unwrap();
        let policy = NetworkPolicy {
            default_action: PolicyAction::Deny,
            rules: vec![
                PolicyRule {
                    action: PolicyAction::Log,
                    criteria: MatchCriteria::Any,
                },
                PolicyRule {
                    action: PolicyAction::Allow,
                    criteria: MatchCriteria::Domain(allow_pattern),
                },
            ],
        };

        assert_eq!(policy.matches_host("api.example.com"), PolicyAction::Allow);
        assert_eq!(policy.matches_host("evil.com"), PolicyAction::Deny);
    }

    #[test]
    fn match_criteria_all_requires_every_child_to_match() {
        let inner_allow = DomainPattern::parse("*.example.com").unwrap();
        let criteria =
            MatchCriteria::All(vec![MatchCriteria::Any, MatchCriteria::Domain(inner_allow)]);

        assert!(criteria.matches_host("api.example.com"));
        assert!(!criteria.matches_host("api.other.com"));
    }
}
