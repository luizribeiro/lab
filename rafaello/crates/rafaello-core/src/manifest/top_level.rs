//! `Manifest` top-level type and `Manifest::parse` (scope §M1, §M2).
//!
//! Per the m1-manifest phase boundary, this commit decodes the
//! top-level required + optional fields only. The `[capabilities]`,
//! `[load]`, and `[[renderers]]` blocks land in c07–c09. Grammar checks on `name`, tool names, topic
//! segments, sink classes, and renderer kinds are deferred to V1
//! (`validate::manifest_standalone` in c10).
//!
//! `Manifest::parse` performs a `toml::Table` pre-scan rejecting
//! the reserved keys `runtime`, `rpc`, and `helper_for` before
//! invoking serde with `deny_unknown_fields`.

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::error::ManifestError;
use crate::manifest::bus::Bus;
use crate::manifest::capabilities::Capabilities;
use crate::manifest::provides::Provides;
use crate::manifest::safepath::SafePath;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub name: String,
    pub version: Version,
    pub entry: SafePath,
    pub rafaello: VersionReq,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provides: Option<Provides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<Bus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,
}

const RESERVED_KEYS: [(&str, &str); 3] = [
    (
        "runtime",
        "post-row-30: runtime selection is owned by the lock, not the manifest",
    ),
    (
        "rpc",
        "post-row-31: declare RPC surface via the openrpc.json sibling",
    ),
    (
        "helper_for",
        "deferred to v2: helper-plugin attachment is not in v1",
    ),
];

impl Manifest {
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        let table: toml::Table = toml::from_str(s)?;
        for (key, hint) in RESERVED_KEYS {
            if table.contains_key(key) {
                return Err(ManifestError::ReservedField {
                    field: key.to_owned(),
                    hint,
                });
            }
        }
        let manifest: Manifest = toml::from_str(s)?;
        Ok(manifest)
    }
}
