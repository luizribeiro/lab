//! rafaello-tui: front-end TUI binary support library.

pub mod command_result;
pub mod confirm;
pub mod env;
pub mod paint;
pub mod slash;

use crossterm::event::KeyCode;
use fittings_core::message::JsonRpcId;
use serde_json::{json, Value};
use ulid::Ulid;

pub const CONFIRM_REQUEST_TOPIC: &str = "core.session.confirm_request";
pub const CONFIRM_REPLY_TOPIC: &str = "core.session.confirm_reply";
pub const CONFIRM_ANSWER_TOPIC: &str = "frontend.tui.confirm_answer";

#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmDetails {
    pub tool_call_id: String,
    pub tool: String,
    pub args: Value,
    pub sinks: Vec<String>,
    pub always_confirm: bool,
    pub taint: Value,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum InputMode {
    #[default]
    Normal,
    ConfirmOverlay {
        confirm_id: JsonRpcId,
        summary: String,
        details: ConfirmDetails,
        ttl_remaining: u32,
        queued_count: u32,
    },
}

impl InputMode {
    pub fn is_overlay(&self) -> bool {
        matches!(self, InputMode::ConfirmOverlay { .. })
    }

    pub fn input_blocked(&self) -> bool {
        self.is_overlay()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Allow,
    Deny,
    AlwaysAllowSession,
}

impl Answer {
    pub fn as_str(self) -> &'static str {
        match self {
            Answer::Allow => "allow",
            Answer::Deny => "deny",
            Answer::AlwaysAllowSession => "always_allow_session",
        }
    }

    pub fn from_key(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::Char('y') | KeyCode::Char('a') | KeyCode::Enter => Some(Answer::Allow),
            KeyCode::Char('n') | KeyCode::Char('d') | KeyCode::Esc => Some(Answer::Deny),
            KeyCode::Char('s') => Some(Answer::AlwaysAllowSession),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmAnswerEnvelope {
    pub topic: &'static str,
    pub request_id: JsonRpcId,
    pub in_reply_to: Vec<JsonRpcId>,
    pub payload: Value,
}

pub fn build_confirm_answer(confirm_id: &JsonRpcId, answer: Answer) -> ConfirmAnswerEnvelope {
    ConfirmAnswerEnvelope {
        topic: CONFIRM_ANSWER_TOPIC,
        request_id: JsonRpcId::String(Ulid::new().to_string()),
        in_reply_to: vec![confirm_id.clone()],
        payload: json!({
            "request_id": confirm_id.to_string(),
            "answer": answer.as_str(),
        }),
    }
}

pub fn overlay_from_confirm_request(payload: &Value, queued_count: u32) -> Option<InputMode> {
    let request_id = payload.get("request_id").and_then(|v| v.as_str())?;
    let confirm_id = JsonRpcId::String(request_id.to_string());
    let summary = payload
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let ttl_remaining = payload
        .get("ttl_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let d = payload.get("details").cloned().unwrap_or(Value::Null);
    let details = ConfirmDetails {
        tool_call_id: d
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool: d
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        args: d.get("args").cloned().unwrap_or(Value::Null),
        sinks: d
            .get("sinks")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        always_confirm: d
            .get("always_confirm")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        taint: d.get("taint").cloned().unwrap_or(Value::Array(vec![])),
    };
    Some(InputMode::ConfirmOverlay {
        confirm_id,
        summary,
        details,
        ttl_remaining,
        queued_count,
    })
}

pub fn handle_overlay_key(
    mode: &InputMode,
    code: KeyCode,
) -> (InputMode, Option<ConfirmAnswerEnvelope>) {
    let InputMode::ConfirmOverlay { confirm_id, .. } = mode else {
        return (mode.clone(), None);
    };
    let Some(answer) = Answer::from_key(code) else {
        return (mode.clone(), None);
    };
    let env = build_confirm_answer(confirm_id, answer);
    (InputMode::Normal, Some(env))
}

pub fn handle_confirm_reply(mode: &InputMode, reply_payload: &Value) -> InputMode {
    if let InputMode::ConfirmOverlay { confirm_id, .. } = mode {
        if let Some(reply_id) = reply_payload.get("request_id").and_then(|v| v.as_str()) {
            if confirm_id.as_str() == Some(reply_id) {
                return InputMode::Normal;
            }
        }
    }
    mode.clone()
}
