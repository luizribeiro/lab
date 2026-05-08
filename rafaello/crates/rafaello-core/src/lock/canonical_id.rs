//! `CanonicalId` — `<source>:<name>@<version>` per scope §L8.
//!
//! `source` is `/`-separated segments matching `[a-z0-9._-]+`;
//! the `..` and `.` segments, leading/trailing/double `/`, and
//! empty segments are all rejected (pi review-2 finding 1, pi-2
//! commits-finding 7). `name` matches the topic-segment grammar
//! `[a-z0-9_][a-z0-9_-]*`. `version` is parsed by `semver::Version`.
//!
//! The compiler never uses the canonical id literally as a path
//! segment — path-safe identifiers come from the topic-id form
//! (§T1).

use std::cmp::Ordering;
use std::fmt;

use semver::Version;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::error::LockError;
use crate::validate::topic::is_tool_name;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalId {
    source: String,
    name: String,
    version: Version,
}

impl CanonicalId {
    pub fn parse(s: &str) -> Result<Self, LockError> {
        let (source, rest) = s
            .split_once(':')
            .ok_or_else(|| LockError::CanonicalIdMissingNameSeparator {
                input: s.to_owned(),
            })?;
        let (name, version) = rest.split_once('@').ok_or_else(|| {
            LockError::CanonicalIdMissingVersionSeparator {
                input: s.to_owned(),
            }
        })?;

        validate_source(source)?;
        if !is_tool_name(name) {
            return Err(LockError::CanonicalIdIllegalName {
                name: name.to_owned(),
            });
        }
        let version = Version::parse(version).map_err(|err| LockError::CanonicalIdInvalidVersion {
            version: version.to_owned(),
            source: err,
        })?;

        Ok(CanonicalId {
            source: source.to_owned(),
            name: name.to_owned(),
            version,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &Version {
        &self.version
    }
}

fn validate_source(source: &str) -> Result<(), LockError> {
    if source.is_empty() {
        return Err(LockError::CanonicalIdEmptySource);
    }
    if source.starts_with('/') {
        return Err(LockError::CanonicalIdSourceLeadingSlash);
    }
    if source.ends_with('/') {
        return Err(LockError::CanonicalIdSourceTrailingSlash);
    }
    for seg in source.split('/') {
        if seg.is_empty() {
            return Err(LockError::CanonicalIdSourceEmptySegment);
        }
        if seg == "." || seg == ".." {
            return Err(LockError::CanonicalIdSourceDotSegment {
                segment: seg.to_owned(),
            });
        }
        if !seg
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-')
        {
            return Err(LockError::CanonicalIdIllegalSourceSegment {
                segment: seg.to_owned(),
            });
        }
    }
    Ok(())
}

impl fmt::Display for CanonicalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}@{}", self.source, self.name, self.version)
    }
}

impl PartialOrd for CanonicalId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CanonicalId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl Serialize for CanonicalId {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for CanonicalId {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        CanonicalId::parse(&s).map_err(de::Error::custom)
    }
}
