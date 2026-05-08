//! Top-level `Lock` (scope §L1, §L7, §L9).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::LockError;
use crate::lock::bindings::Bindings;
use crate::lock::canonical_id::CanonicalId;
use crate::lock::flags::LockFlags;
use crate::lock::grant::Grant;
use crate::lock::session::SessionTable;
use crate::manifest::safepath::SafePath;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    #[serde(default, rename = "plugin", skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<CanonicalId, PluginEntry>,
    #[serde(default, skip_serializing_if = "session_is_empty")]
    pub session: SessionTable,
}

fn session_is_empty(s: &SessionTable) -> bool {
    s.provider_active.is_none() && s.tool_owner.is_empty()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginEntry {
    pub entry: SafePath,
    pub digest: String,
    pub manifest_digest: String,
    pub granted_at: DateTime<Utc>,
    #[serde(default)]
    pub grant: Grant,
    #[serde(default)]
    pub bindings: Bindings,
    #[serde(default)]
    pub flags: LockFlags,
}

impl Lock {
    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("Lock serialization is infallible")
    }

    pub fn from_toml(s: &str) -> Result<Self, LockError> {
        toml::from_str(s).map_err(LockError::from)
    }
}
