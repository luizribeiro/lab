//! Normalized event stream emitted by drivers.

/// A normalized event produced by a driver from an underlying agent's
/// stream-JSON output.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// Incremental assistant message text. `delta` is the NEW text added in
    /// this event, not the cumulative message so far. Callers wishing to
    /// reconstruct the full message must concatenate deltas within a turn.
    AssistantText { delta: String },

    /// A tool invocation started by the agent. `args` is the raw, unvalidated
    /// JSON payload from the underlying agent — schemas vary per tool and per
    /// CLI.
    ToolCall {
        call_id: String,
        name: String,
        args: serde_json::Value,
    },

    /// Result of a previously-emitted ToolCall. `ok=false` indicates the tool
    /// reported an error; `output` carries the textual result either way.
    ToolResult {
        call_id: String,
        ok: bool,
        output: String,
    },

    /// Incremental thinking text (when the underlying model exposes it). Same
    /// delta semantics as AssistantText.
    Thinking { delta: String },

    /// Cumulative token usage observed so far in the current turn. Drivers
    /// SHOULD emit Usage as cumulative-for-the-turn, not per-event deltas.
    Usage {
        input_tokens: u64,
        output_tokens: u64,
    },

    /// Sentinel emitted exactly once per turn when the agent has finished.
    /// `ok` is false when the underlying CLI reported an error for this turn
    /// (e.g. claude's `is_error: true`). For the canonical agent response
    /// text, use [`crate::Turn::final_text`], which concatenates all
    /// `AssistantText` deltas observed during the turn.
    TurnComplete { ok: bool },

    /// Catch-all for agent JSON events that the driver did not normalize to
    /// one of the variants above. Preserves provenance (driver name) plus the
    /// raw value so callers can pattern-match on agent-specific shapes.
    Raw {
        driver: &'static str,
        value: serde_json::Value,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_text_carries_delta() {
        let ev = Event::AssistantText { delta: "hi".into() };
        let Event::AssistantText { delta } = ev else {
            panic!("wrong variant");
        };
        assert_eq!(delta, "hi");
    }

    #[test]
    fn tool_call_preserves_args_json() {
        let ev = Event::ToolCall {
            call_id: "c1".into(),
            name: "read".into(),
            args: serde_json::json!({"path": "x"}),
        };
        let Event::ToolCall { args, .. } = ev else {
            panic!("wrong variant");
        };
        assert_eq!(args["path"], "x");
    }

    #[test]
    fn turn_complete_ok_field_distinguishes_success_and_failure() {
        let a = Event::TurnComplete { ok: true };
        let b = Event::TurnComplete { ok: false };
        assert_ne!(a, b);
    }

    #[test]
    fn raw_records_driver_and_value() {
        let ev = Event::Raw {
            driver: "claude",
            value: serde_json::json!({"type": "unknown"}),
        };
        let Event::Raw { driver, .. } = ev else {
            panic!("wrong variant");
        };
        assert_eq!(driver, "claude");
    }

    #[test]
    fn clone_event_yields_equal_event() {
        let ev = Event::ToolCall {
            call_id: "c1".into(),
            name: "edit".into(),
            args: serde_json::json!({"path": "a", "nested": {"k": 1}}),
        };
        assert_eq!(ev.clone(), ev);
    }
}
