use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use thiserror::Error;
use ulid::Ulid;

use crate::lock::CanonicalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GrantId(pub Ulid);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantSource {
    SlashCommand,
    AlwaysAllowSession,
}

#[derive(Debug, Clone)]
pub enum GrantMatcher {
    Any,
    Structural { template: Value },
}

#[derive(Debug, Clone)]
pub struct UserGrant {
    pub tool: String,
    pub plugin: CanonicalId,
    pub matcher: GrantMatcher,
    pub added_at: DateTime<Utc>,
    pub source: GrantSource,
}

#[derive(Debug, Error)]
pub enum RevokeError {
    #[error("no grant with id {0:?}")]
    Unknown(GrantId),
}

#[derive(Debug, Default)]
pub struct UserGrants {
    entries: BTreeMap<GrantId, UserGrant>,
}

impl UserGrants {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, grant: UserGrant) -> GrantId {
        let id = GrantId(Ulid::new());
        self.entries.insert(id, grant);
        id
    }

    pub fn list(&self) -> Vec<(GrantId, &UserGrant)> {
        self.entries.iter().map(|(id, g)| (*id, g)).collect()
    }

    pub fn revoke(&mut self, id: GrantId) -> Result<(), RevokeError> {
        self.entries
            .remove(&id)
            .map(|_| ())
            .ok_or(RevokeError::Unknown(id))
    }

    pub fn matches(&self, plugin: &CanonicalId, tool: &str, args: &Value) -> Option<GrantId> {
        for (id, grant) in &self.entries {
            if &grant.plugin != plugin || grant.tool != tool {
                continue;
            }
            let hit = match &grant.matcher {
                GrantMatcher::Any => true,
                GrantMatcher::Structural { template } => structural_subset(template, args),
            };
            if hit {
                return Some(*id);
            }
        }
        None
    }
}

fn structural_subset(template: &Value, args: &Value) -> bool {
    match (template, args) {
        (Value::Object(t), Value::Object(a)) => t.iter().all(|(k, tv)| {
            a.get(k)
                .map(|av| structural_subset(tv, av))
                .unwrap_or(false)
        }),
        (Value::Array(t), Value::Array(a)) => {
            t.len() == a.len()
                && t.iter()
                    .zip(a.iter())
                    .all(|(tv, av)| structural_subset(tv, av))
        }
        (t, a) => t == a,
    }
}
