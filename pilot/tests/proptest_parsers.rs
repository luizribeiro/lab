//! Property-based testing for driver parse() implementations.
//!
//! Invariant: for any well-formed `serde_json::Value`, `parse()` must
//! return either `Ok(Vec<Event>)` or `Err(ParseError)`. It must NEVER
//! panic, overflow the stack, or otherwise diverge.

use pilot::{Claude, Codex, Driver, Gemini, Pi};
use proptest::prelude::*;

/// Known field names that real driver parse() implementations look at.
/// Mixing these into proptest-generated objects exercises the nested
/// branches that random short strings would never reach.
const KNOWN_KEYS: &[&str] = &[
    "type",
    "message",
    "content",
    "text",
    "role",
    "delta",
    "usage",
    "input_tokens",
    "output_tokens",
    "input",
    "output",
    "tool_use",
    "tool_result",
    "tool_use_id",
    "id",
    "name",
    "args",
    "thinking",
    "result",
    "is_error",
    "subtype",
    "stop_reason",
    "status",
    "stats",
    "session_id",
    "model",
    "assistantMessageEvent",
    "contentIndex",
    "toolResults",
    "version",
    "cwd",
    "timestamp",
    "thread_id",
    "item",
    "cached_input_tokens",
    "reasoning_output_tokens",
];

/// String values that act as discriminators inside parse() match arms.
/// Mixing these into proptest-generated strings exercises the
/// type-dispatching branches across all drivers.
const KNOWN_DISCRIMINATORS: &[&str] = &[
    // claude content block types
    "text",
    "tool_use",
    "tool_result",
    "thinking",
    // claude / gemini event types
    "system",
    "assistant",
    "user",
    "result",
    "rate_limit_event",
    "init",
    "message",
    "tool_use",
    // pi event types
    "session",
    "agent_start",
    "agent_end",
    "turn_start",
    "turn_end",
    "message_start",
    "message_end",
    "message_update",
    // pi message_update inner types
    "text_start",
    "text_delta",
    "text_end",
    // statuses / subtypes
    "success",
    "error",
    // roles (already in event types but explicit here for clarity)
    "assistant",
    "user",
    // codex event types
    "thread.started",
    "turn.started",
    "turn.completed",
    "item.completed",
    "agent_message",
    "function_call",
    "function_call_output",
];

/// Recursive strategy for arbitrary JSON values, bounded so tests run fast.
fn arb_value() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(|n| serde_json::Value::Number(n.into())),
        prop_oneof![
            prop::sample::select(KNOWN_DISCRIMINATORS)
                .prop_map(|s| serde_json::Value::String(s.to_string())),
            ".{0,16}".prop_map(serde_json::Value::String),
        ],
    ];
    leaf.prop_recursive(
        4,  // recursion depth
        16, // max total nodes
        4,  // collection size
        |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::Array),
                prop::collection::vec(
                    (
                        prop_oneof![
                            prop::sample::select(KNOWN_KEYS).prop_map(String::from),
                            ".{0,8}".prop_map(String::from),
                        ],
                        inner,
                    ),
                    0..4,
                )
                .prop_map(|kvs| serde_json::Value::Object(kvs.into_iter().collect())),
            ]
        },
    )
}

/// Strategy that more often hits realistic event-type shapes (objects with a
/// "type" field of a known driver event name) without locking out random data.
fn arb_value_with_typed_keys() -> impl Strategy<Value = serde_json::Value> {
    let known_types = prop_oneof![
        // claude events
        Just("system"),
        Just("assistant"),
        Just("user"),
        Just("result"),
        // gemini events
        Just("init"),
        Just("message"),
        Just("result"),
        // pi events
        Just("session"),
        Just("agent_start"),
        Just("agent_end"),
        Just("turn_start"),
        Just("turn_end"),
        Just("message_start"),
        Just("message_end"),
        Just("message_update"),
        // unknown
        Just("nope_definitely_unknown"),
    ];
    (known_types, arb_value()).prop_map(|(ty, rest)| {
        let mut map = serde_json::Map::new();
        map.insert("type".into(), serde_json::Value::String(ty.into()));
        if let serde_json::Value::Object(more) = rest {
            for (k, v) in more {
                map.insert(k, v);
            }
        }
        serde_json::Value::Object(map)
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn claude_parse_never_panics(v in arb_value()) {
        let _ = Claude::new().parse(v);
    }

    #[test]
    fn codex_parse_never_panics(v in arb_value()) {
        let _ = Codex::new().parse(v);
    }

    #[test]
    fn gemini_parse_never_panics(v in arb_value()) {
        let _ = Gemini::new().parse(v);
    }

    #[test]
    fn pi_parse_never_panics(v in arb_value()) {
        let _ = Pi::new().parse(v);
    }

    #[test]
    fn claude_parse_never_panics_with_typed_keys(v in arb_value_with_typed_keys()) {
        let _ = Claude::new().parse(v);
    }

    #[test]
    fn codex_parse_never_panics_with_typed_keys(v in arb_value_with_typed_keys()) {
        let _ = Codex::new().parse(v);
    }

    #[test]
    fn gemini_parse_never_panics_with_typed_keys(v in arb_value_with_typed_keys()) {
        let _ = Gemini::new().parse(v);
    }

    #[test]
    fn pi_parse_never_panics_with_typed_keys(v in arb_value_with_typed_keys()) {
        let _ = Pi::new().parse(v);
    }
}
