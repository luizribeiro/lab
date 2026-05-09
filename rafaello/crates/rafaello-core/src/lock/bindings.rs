//! `.bindings` — manifest-derived authority snapshot (scope §L4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::lock::load_policy::LoadPolicy;
use crate::manifest::safepath::SafePath;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    #[serde(default)]
    pub provider: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renderer_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tool_meta: BTreeMap<String, ToolMeta>,
    #[serde(default)]
    pub load: LoadPolicy,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolMeta {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sinks: Vec<String>,
    #[serde(default)]
    pub sinks_inferred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_match: Option<SafePath>,
    #[serde(default)]
    pub always_confirm: bool,
}
