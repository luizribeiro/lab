//! `CapabilityPathTemplate` — placeholder-or-absolute paths used
//! by `read_paths` / `read_dirs` / `write_paths` / `write_dirs` /
//! `exec_paths` / `exec_dirs` (scope §M11).
//!
//! Accepts either an absolute host path (e.g. `/usr/bin/rustc`)
//! or a path whose first segment is one of the closed §M8
//! placeholders (`${project}`, `${home}`, `${plugin}`,
//! `${cache}`, `${state}`). Rejects bare relative paths
//! (no implicit cwd anchor), control chars, non-UTF-8, and `\`.
//! `..` segments are *parser-allowed* — the post-expansion
//! containment check lives in [`crate::paths::resolve_under_root`].

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ManifestError;

const KNOWN_PLACEHOLDERS: &[&str] = &["project", "home", "plugin", "cache", "state"];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityPathTemplate(String);

impl CapabilityPathTemplate {
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        if s.is_empty() {
            return Err(ManifestError::CapabilityPathBareRelative);
        }
        for ch in s.chars() {
            if ch == '\\' {
                return Err(ManifestError::CapabilityPathBackslash);
            }
            if (ch as u32) < 0x20 || ch == '\u{7F}' {
                return Err(ManifestError::CapabilityPathControlChar);
            }
        }

        if s.starts_with('/') {
            return Ok(CapabilityPathTemplate(s.to_owned()));
        }

        if let Some(rest) = s.strip_prefix("${") {
            let close = rest
                .find('}')
                .ok_or(ManifestError::CapabilityPathMalformedPlaceholder)?;
            let name = &rest[..close];
            if !KNOWN_PLACEHOLDERS.contains(&name) {
                return Err(ManifestError::UnknownPlaceholder);
            }
            let after = &rest[close + 1..];
            if !after.is_empty() && !after.starts_with('/') {
                return Err(ManifestError::CapabilityPathMalformedPlaceholder);
            }
            return Ok(CapabilityPathTemplate(s.to_owned()));
        }

        Err(ManifestError::CapabilityPathBareRelative)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CapabilityPathTemplate {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for CapabilityPathTemplate {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityPathTemplate {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        CapabilityPathTemplate::parse(&s).map_err(de::Error::custom)
    }
}
