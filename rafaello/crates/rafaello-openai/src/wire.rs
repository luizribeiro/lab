//! Chat Completions wire client (scope §OP1).
//!
//! Implements the request/response structs and a [`WireClient`]
//! that posts to `<endpoint>/chat/completions` with `Authorization:
//! Bearer <api-key>`, `stream: false`, a 60s per-request timeout,
//! and no retries (m5a). Response translation honours the
//! deterministic edge cases listed in scope §OP1 (empty choices,
//! multiple choices, invalid tool args, unknown tool).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{map_to_assistant, OpenaiError};

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Msg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Msg,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Msg {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFn {
    pub name: String,
    pub arguments: String,
}

pub struct WireClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: String,
}

impl WireClient {
    pub fn new(endpoint: String, api_key: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("build reqwest client");
        Self {
            http,
            endpoint,
            api_key,
        }
    }

    pub async fn chat(
        &self,
        req: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, OpenaiError> {
        let url = format!("{}/chat/completions", self.endpoint);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(req)
            .send()
            .await
            .map_err(|e| OpenaiError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| OpenaiError::Transport(e.to_string()))?;
        if (200..300).contains(&status) {
            serde_json::from_slice::<ChatCompletionResponse>(&bytes).map_err(|e| {
                tracing::warn!(
                    body = %String::from_utf8_lossy(&bytes),
                    "openai: malformed response body"
                );
                OpenaiError::Malformed(e.to_string())
            })
        } else if status == 401 || status == 403 {
            Err(OpenaiError::AuthFailed { status })
        } else if (500..600).contains(&status) {
            Err(OpenaiError::ServerError { status })
        } else {
            let body = String::from_utf8_lossy(&bytes);
            let body_excerpt: String = body.chars().take(200).collect();
            Err(OpenaiError::ClientError {
                status,
                body_excerpt,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEvent {
    AssistantMessage(String),
    ToolRequest {
        call_id: String,
        name: String,
        arguments: serde_json::Value,
    },
}

/// Translate a [`ChatCompletionResponse`] into a sequence of
/// [`TurnEvent`]s per scope §OP1's `finish_reason` /
/// mixed-content table. `known_tools` is the set of tool names
/// from `core.tools_list`; calls naming a tool outside this set
/// produce an `assistant_message` error rather than a tool
/// request.
pub fn translate(resp: ChatCompletionResponse, known_tools: &[String]) -> Vec<TurnEvent> {
    if resp.choices.is_empty() {
        return vec![TurnEvent::AssistantMessage(map_to_assistant(
            &OpenaiError::EmptyChoices,
        ))];
    }
    if resp.choices.len() > 1 {
        tracing::warn!(
            choice_count = resp.choices.len(),
            "openai: response has multiple choices; using choices[0]"
        );
    }
    let choice = &resp.choices[0];
    let mut out = Vec::new();
    if let Some(text) = choice.message.content.as_ref().filter(|s| !s.is_empty()) {
        out.push(TurnEvent::AssistantMessage(text.clone()));
    }
    if let Some(calls) = choice.message.tool_calls.as_ref() {
        for tc in calls {
            if !known_tools.iter().any(|t| t == &tc.function.name) {
                out.push(TurnEvent::AssistantMessage(map_to_assistant(
                    &OpenaiError::UnknownTool(tc.function.name.clone()),
                )));
                continue;
            }
            match serde_json::from_str::<serde_json::Value>(&tc.function.arguments) {
                Ok(args) => out.push(TurnEvent::ToolRequest {
                    call_id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments: args,
                }),
                Err(e) => out.push(TurnEvent::AssistantMessage(map_to_assistant(
                    &OpenaiError::InvalidToolArgs(e.to_string()),
                ))),
            }
        }
    }
    if out.is_empty() {
        out.push(TurnEvent::AssistantMessage(map_to_assistant(
            &OpenaiError::EmptyChoices,
        )));
    }
    out
}
