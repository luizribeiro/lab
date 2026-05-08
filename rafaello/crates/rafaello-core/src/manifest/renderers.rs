//! `[[renderers]]` array raw decode (scope §M7, parse half).
//!
//! Per the m1-manifest phase boundary, this commit decodes the
//! array-of-tables structure only. The kind grammar — built-in
//! reservation (`text`, `code_block`, `tool_call`, `tool_result`,
//! `error`, `heading`, `thinking`, `image`) and the Stream E §8
//! `<vendor>:<kind>` prefix rule (pi review-4 finding 7) — is
//! deferred to V1 (`validate::manifest_standalone`, c10).
//!
//! Per `decisions.md` row 29, plugin renderer registrations parse
//! and round-trip into the lock for forward compatibility, but
//! subprocess `renderer.render` dispatch is deferred to v2; m3's
//! renderer router is built-in-only and ignores plugin renderer
//! registrations entirely (pi review-2 finding 10).

use serde::{Deserialize, Serialize};

fn default_priority() -> u32 {
    100
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Renderer {
    pub kind: String,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}
