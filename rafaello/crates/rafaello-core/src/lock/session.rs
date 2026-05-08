//! `[session]` table per scope §L1.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionTable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_active: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_owner: BTreeMap<String, String>,
}
