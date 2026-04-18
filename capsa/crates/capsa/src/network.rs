use std::sync::Arc;

use capsa_core::{
    DomainPattern, MatchCriteria, NetworkPolicy, NetworkProcesses, PolicyAction, PolicyRule,
};

use crate::error::{BuildError, StartError};

#[derive(Debug, Clone)]
pub struct Network {
    pub(crate) policy: NetworkPolicy,
}

impl Network {
    pub fn builder() -> NetworkBuilder {
        NetworkBuilder::default()
    }

    /// Spawn the network daemon and return a [`NetworkHandle`] that
    /// VMs can attach to. The daemon stays alive as long as at least
    /// one clone of the returned handle exists; the last drop
    /// SIGKILLs it.
    pub fn start(&self) -> Result<NetworkHandle, StartError> {
        let processes = NetworkProcesses::spawn(Some(self.policy.clone()))
            .map_err(|e| StartError::NetworkSpawn(e.into()))?;
        Ok(NetworkHandle {
            inner: Arc::new(processes),
        })
    }
}

/// Handle to a running network daemon. Cheaply cloneable — every
/// clone shares the same daemon. SIGKILLs the daemon when the last
/// clone is dropped.
#[derive(Clone)]
pub struct NetworkHandle {
    #[allow(dead_code)]
    pub(crate) inner: Arc<NetworkProcesses>,
}

impl std::fmt::Debug for NetworkHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkHandle").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetworkBuilder {
    allow_all: bool,
    host_patterns: Vec<String>,
}

impl NetworkBuilder {
    pub fn allow_all_hosts(mut self) -> Self {
        self.allow_all = true;
        self
    }

    pub fn allow_host(mut self, pattern: impl AsRef<str>) -> Self {
        self.host_patterns.push(pattern.as_ref().to_string());
        self
    }

    pub fn allow_hosts<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.host_patterns
            .extend(patterns.into_iter().map(|p| p.as_ref().to_string()));
        self
    }

    pub fn build(self) -> Result<Network, BuildError> {
        let default_action = if self.allow_all {
            PolicyAction::Allow
        } else {
            PolicyAction::Deny
        };

        let mut rules = Vec::with_capacity(self.host_patterns.len());
        for raw in self.host_patterns {
            let pattern =
                DomainPattern::parse(&raw).map_err(|e| BuildError::InvalidHostPattern {
                    pattern: raw.clone(),
                    reason: e.to_string(),
                })?;
            rules.push(PolicyRule {
                action: PolicyAction::Allow,
                criteria: MatchCriteria::Domain(pattern),
            });
        }

        Ok(Network {
            policy: NetworkPolicy {
                default_action,
                rules,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_builder_produces_deny_all_policy() {
        let network = Network::builder().build().expect("build should succeed");
        assert_eq!(network.policy.default_action, PolicyAction::Deny);
        assert!(network.policy.rules.is_empty());
    }

    #[test]
    fn allow_all_hosts_sets_default_to_allow() {
        let network = Network::builder()
            .allow_all_hosts()
            .build()
            .expect("build should succeed");
        assert_eq!(network.policy.default_action, PolicyAction::Allow);
    }

    #[test]
    fn allow_host_adds_exact_rule() {
        let network = Network::builder()
            .allow_host("api.example.com")
            .build()
            .expect("build should succeed");

        assert_eq!(network.policy.rules.len(), 1);
        let rule = &network.policy.rules[0];
        assert_eq!(rule.action, PolicyAction::Allow);
        assert_eq!(
            rule.criteria,
            MatchCriteria::Domain(DomainPattern::Exact("api.example.com".into()))
        );
    }

    #[test]
    fn allow_host_accepts_wildcard_pattern() {
        let network = Network::builder()
            .allow_host("*.cdn.example.com")
            .build()
            .expect("build should succeed");

        let rule = &network.policy.rules[0];
        assert_eq!(
            rule.criteria,
            MatchCriteria::Domain(DomainPattern::Wildcard("cdn.example.com".into()))
        );
    }

    #[test]
    fn allow_hosts_preserves_order() {
        let network = Network::builder()
            .allow_hosts(["a.example.com", "b.example.com", "*.c.example.com"])
            .build()
            .expect("build should succeed");

        assert_eq!(network.policy.rules.len(), 3);
        assert_eq!(
            network.policy.rules[0].criteria,
            MatchCriteria::Domain(DomainPattern::Exact("a.example.com".into()))
        );
        assert_eq!(
            network.policy.rules[1].criteria,
            MatchCriteria::Domain(DomainPattern::Exact("b.example.com".into()))
        );
        assert_eq!(
            network.policy.rules[2].criteria,
            MatchCriteria::Domain(DomainPattern::Wildcard("c.example.com".into()))
        );
    }

    #[test]
    fn build_surfaces_invalid_pattern_with_offending_input() {
        let err = Network::builder()
            .allow_host("api.example.com")
            .allow_host("*example.com")
            .build()
            .expect_err("malformed wildcard should fail build");

        let BuildError::InvalidHostPattern { pattern, reason } = err;
        assert_eq!(pattern, "*example.com");
        assert!(
            reason.contains("wildcard"),
            "reason missing detail: {reason}"
        );
    }
}
