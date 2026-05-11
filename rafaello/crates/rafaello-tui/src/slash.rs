//! c17 / scope §SL1+§SL2: TUI-side slash-command parser.
//!
//! Lines beginning with `/` are turned into typed [`SlashCommand`]s
//! which the TUI publishes verbatim on `frontend.tui.slash_command`.
//! Parse failures collapse to [`SlashKind::Unknown`] (carrying the raw
//! input) so core's audit log captures the attempt.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlashKind {
    Grant,
    ListGrants,
    Revoke,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlashCommand {
    pub command: SlashKind,
    pub args: Value,
}

impl SlashCommand {
    fn unknown(raw: &str) -> Self {
        Self {
            command: SlashKind::Unknown,
            args: json!({ "raw": raw }),
        }
    }
}

pub fn parse(input: &str) -> SlashCommand {
    let Some(rest) = input.strip_prefix('/') else {
        return SlashCommand::unknown(input);
    };
    let mut parts = rest.split_whitespace();
    let Some(head) = parts.next() else {
        return SlashCommand::unknown(input);
    };
    let tail: Vec<&str> = parts.collect();
    match head {
        "grant" => parse_grant(input, &tail),
        "grants" if tail.as_slice() == ["list"] => SlashCommand {
            command: SlashKind::ListGrants,
            args: json!({}),
        },
        "revoke" if tail.len() == 1 => SlashCommand {
            command: SlashKind::Revoke,
            args: json!({ "grant_id": tail[0] }),
        },
        _ => SlashCommand::unknown(input),
    }
}

fn parse_grant(raw: &str, tail: &[&str]) -> SlashCommand {
    let Some((tool, kvs)) = tail.split_first() else {
        return SlashCommand::unknown(raw);
    };
    let mut template: BTreeMap<String, Value> = BTreeMap::new();
    for kv in kvs {
        let Some((k, v)) = kv.split_once('=') else {
            return SlashCommand::unknown(raw);
        };
        if k.is_empty() {
            return SlashCommand::unknown(raw);
        }
        template.insert(k.to_string(), Value::String(v.to_string()));
    }
    SlashCommand {
        command: SlashKind::Grant,
        args: json!({ "tool": tool, "template": template }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_grant_basic() {
        let c = parse("/grant tool_a foo=bar");
        assert_eq!(c.command, SlashKind::Grant);
        assert_eq!(c.args["tool"], "tool_a");
        assert_eq!(c.args["template"], json!({ "foo": "bar" }));
    }

    #[test]
    fn parse_grants_list() {
        assert_eq!(parse("/grants list").command, SlashKind::ListGrants);
    }

    #[test]
    fn parse_revoke() {
        let c = parse("/revoke abc123");
        assert_eq!(c.command, SlashKind::Revoke);
        assert_eq!(c.args["grant_id"], "abc123");
    }

    #[test]
    fn parse_unknown_preserves_raw() {
        let c = parse("/foo bar baz");
        assert_eq!(c.command, SlashKind::Unknown);
        assert_eq!(c.args["raw"], "/foo bar baz");
    }

    #[test]
    fn parse_grant_malformed_kv_is_unknown() {
        let c = parse("/grant tool_a notakv");
        assert_eq!(c.command, SlashKind::Unknown);
        assert_eq!(c.args["raw"], "/grant tool_a notakv");
    }
}
