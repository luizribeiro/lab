//! `[load]` block raw decode (scope §M6, parse half).
//!
//! Per the m1-manifest phase boundary, this commit decodes the
//! `Load` enum's surface forms only: the four string shorthands
//! (`"eager"`, `"boot"`, `"manual"`, `"lazy"`) and the table form
//! `{ event = [...], command = [...], kind = [...] }`. Cross-ref
//! checks (`command` ∈ `provides.tools`, `event` patterns against
//! `bus.subscribes`, `kind` against renderer kinds) and the
//! `"lazy"`-string expansion to "all subscribed events / all
//! provided tools / all registered renderers" are deferred to V1
//! (`validate::manifest_standalone`, c10).
//!
//! At parse time the `"lazy"` string shorthand decodes as
//! `Load::Lazy { event: vec![], command: vec![], kind: vec![] }`;
//! V1 distinguishes "explicitly empty lazy table" from "lazy
//! shorthand" via context not present in m1.

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

const LOAD_VARIANTS: &[&str] = &["eager", "boot", "manual", "lazy"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load {
    Eager,
    Boot,
    Manual,
    Lazy {
        event: Vec<String>,
        command: Vec<String>,
        kind: Vec<String>,
    },
}

impl<'de> Deserialize<'de> for Load {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(LoadVisitor)
    }
}

struct LoadVisitor;

impl<'de> Visitor<'de> for LoadVisitor {
    type Value = Load;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(
            "a load shorthand string (\"eager\" | \"boot\" | \"manual\" | \"lazy\") or a \
             load table with optional `event` / `command` / `kind` arrays",
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<Load, E>
    where
        E: de::Error,
    {
        match v {
            "eager" => Ok(Load::Eager),
            "boot" => Ok(Load::Boot),
            "manual" => Ok(Load::Manual),
            "lazy" => Ok(Load::Lazy {
                event: Vec::new(),
                command: Vec::new(),
                kind: Vec::new(),
            }),
            other => Err(E::unknown_variant(other, LOAD_VARIANTS)),
        }
    }

    fn visit_map<M>(self, mut map: M) -> Result<Load, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut event: Option<Vec<String>> = None;
        let mut command: Option<Vec<String>> = None;
        let mut kind: Option<Vec<String>> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "event" => {
                    if event.is_some() {
                        return Err(de::Error::duplicate_field("event"));
                    }
                    event = Some(map.next_value()?);
                }
                "command" => {
                    if command.is_some() {
                        return Err(de::Error::duplicate_field("command"));
                    }
                    command = Some(map.next_value()?);
                }
                "kind" => {
                    if kind.is_some() {
                        return Err(de::Error::duplicate_field("kind"));
                    }
                    kind = Some(map.next_value()?);
                }
                other => {
                    return Err(de::Error::unknown_field(
                        other,
                        &["event", "command", "kind"],
                    ));
                }
            }
        }

        Ok(Load::Lazy {
            event: event.unwrap_or_default(),
            command: command.unwrap_or_default(),
            kind: kind.unwrap_or_default(),
        })
    }
}

impl Serialize for Load {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Load::Eager => serializer.serialize_str("eager"),
            Load::Boot => serializer.serialize_str("boot"),
            Load::Manual => serializer.serialize_str("manual"),
            Load::Lazy {
                event,
                command,
                kind,
            } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("event", event)?;
                map.serialize_entry("command", command)?;
                map.serialize_entry("kind", kind)?;
                map.end()
            }
        }
    }
}
