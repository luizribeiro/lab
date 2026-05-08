//! `[bus]` block raw decode (scope §M4, parse half).
//!
//! Per the m1-manifest phase boundary, this commit only decodes
//! the two string lists. Topic / pattern grammar (§5.1), the
//! `core.*` namespace ACL, and the pattern-vs-topic discipline
//! (`**` allowed in `subscribes`, rejected in `publishes`) are
//! deferred to V1 (`validate::manifest_standalone`, c10).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Bus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subscribes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishes: Vec<String>,
}
