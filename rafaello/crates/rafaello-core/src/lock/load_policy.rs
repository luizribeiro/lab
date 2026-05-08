//! `LoadPolicy` — lock-side mirror of the manifest's `[load]` enum
//! (scope §L4 per pi review-6 finding 2). Same surface forms as
//! `manifest::Load`: the four string shorthands plus the `Lazy`
//! table.

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

const LOAD_VARIANTS: &[&str] = &["eager", "boot", "manual", "lazy"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadPolicy {
    Eager,
    Boot,
    Manual,
    Lazy {
        event: Vec<String>,
        command: Vec<String>,
        kind: Vec<String>,
    },
}

impl Default for LoadPolicy {
    fn default() -> Self {
        LoadPolicy::Manual
    }
}

impl<'de> Deserialize<'de> for LoadPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(LoadPolicyVisitor)
    }
}

struct LoadPolicyVisitor;

impl<'de> Visitor<'de> for LoadPolicyVisitor {
    type Value = LoadPolicy;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(
            "a load shorthand string (\"eager\" | \"boot\" | \"manual\" | \"lazy\") or a \
             load table with optional `event` / `command` / `kind` arrays",
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<LoadPolicy, E>
    where
        E: de::Error,
    {
        match v {
            "eager" => Ok(LoadPolicy::Eager),
            "boot" => Ok(LoadPolicy::Boot),
            "manual" => Ok(LoadPolicy::Manual),
            "lazy" => Ok(LoadPolicy::Lazy {
                event: Vec::new(),
                command: Vec::new(),
                kind: Vec::new(),
            }),
            other => Err(E::unknown_variant(other, LOAD_VARIANTS)),
        }
    }

    fn visit_map<M>(self, mut map: M) -> Result<LoadPolicy, M::Error>
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

        Ok(LoadPolicy::Lazy {
            event: event.unwrap_or_default(),
            command: command.unwrap_or_default(),
            kind: kind.unwrap_or_default(),
        })
    }
}

impl Serialize for LoadPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LoadPolicy::Eager => serializer.serialize_str("eager"),
            LoadPolicy::Boot => serializer.serialize_str("boot"),
            LoadPolicy::Manual => serializer.serialize_str("manual"),
            LoadPolicy::Lazy {
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
