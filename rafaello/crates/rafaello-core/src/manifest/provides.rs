//! `[provides]` block raw decode (scope §M3, parse half).
//!
//! Tool-name and sink-class grammar (`[a-z0-9_][a-z0-9_-]*` and
//! `[a-z0-9_]+`) are deferred to V1 (`validate::manifest_standalone`,
//! c10). At parse time, `tools` / `provider` / `sinks` are raw
//! strings and the only structural rejection that fires here is
//! `SafePath` on `grant_match` (M11) plus serde type-mismatches
//! (e.g. `sinks = [42]`).
//!
//! `sinks: None` (key absent) is intentionally distinct from
//! `Some(vec![])` (key present, empty) per pi review-2 finding 2 —
//! sink inference (Si1) keys off the `None` arm only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::safepath::SafePath;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Provides {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tool: BTreeMap<String, ToolMetaManifest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ToolMetaManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sinks: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_match: Option<SafePath>,
    #[serde(default)]
    pub always_confirm: bool,
}
