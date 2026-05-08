//! `SafePath` — relative-package paths used by `entry` and
//! `[provides.tool.<n>].grant_match` (scope §M11).
//!
//! Rejects: leading `/`, `..` segments, empty path segments
//! (consecutive or trailing `/`), control characters (U+0000
//! through U+001F plus U+007F), and `\` separators. Non-UTF-8
//! input is structurally impossible — `&str` is already UTF-8.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ManifestError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafePath(String);

impl SafePath {
    pub fn parse(s: &str) -> Result<Self, ManifestError> {
        if s.is_empty() {
            return Err(ManifestError::SafePathEmpty);
        }
        for ch in s.chars() {
            if ch == '\\' {
                return Err(ManifestError::SafePathBackslash);
            }
            if (ch as u32) < 0x20 || ch == '\u{7F}' {
                return Err(ManifestError::SafePathControlChar);
            }
        }
        if s.starts_with('/') {
            return Err(ManifestError::SafePathLeadingSlash);
        }
        for seg in s.split('/') {
            if seg.is_empty() {
                return Err(ManifestError::SafePathEmptySegment);
            }
            if seg == ".." {
                return Err(ManifestError::SafePathParentDir);
            }
        }
        Ok(SafePath(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SafePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for SafePath {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SafePath {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        SafePath::parse(&s).map_err(de::Error::custom)
    }
}
