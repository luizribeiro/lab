//! `rfl-openai` — OpenAI Chat Completions provider plugin
//! (scope §OP3 + §OP1 conversation history).
//!
//! Subscribes to `core.session.user_message` and
//! `core.session.tool_result`, maintains a per-session in-memory
//! `Vec<Msg>`, sends a Chat Completion request on each user_message,
//! and publishes `provider.openai.tool_request` /
//! `provider.openai.assistant_message` per the §OP1 finish_reason
//! table. Conversation history forwarded to the model includes
//! observed user messages (`role: "user"`), prior assistant messages
//! (`role: "assistant"`), and tool_results (`role: "tool"` with
//! `tool_call_id` taken from `in_reply_to[0]`).
//!
//! Tool-schema discovery (`core.tools_list`, §OP2) lands in c34;
//! until then the adapter trusts whatever tool name the model
//! proposes.

use std::collections::HashSet;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use fittings_client::{Client, InboundNotification};
use fittings_core::context::PeerHandle;
use fittings_core::error::FittingsError;
use fittings_core::message::JsonRpcId;
use fittings_core::transport::Connector;
use fittings_transport::stdio::StdioTransport;
use rafaello_core::supervisor::ToolSchema;
use rafaello_openai::{
    map_to_assistant, read_required_api_key, read_required_endpoint_url, read_required_model,
    ChatCompletionRequest, Msg, OpenaiError, ToolCall, ToolCallFn, WireClient,
};
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, Mutex};
use ulid::Ulid;

const MAX_FRAME_BYTES: usize = 1 << 20;

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct State {
    history: Vec<Msg>,
    observed_user_messages: Vec<JsonRpcId>,
    observed_tool_results: Vec<JsonRpcId>,
    seen_tool_results: HashSet<JsonRpcId>,
}

impl State {
    fn new() -> Self {
        Self {
            history: Vec::new(),
            observed_user_messages: Vec::new(),
            observed_tool_results: Vec::new(),
            seen_tool_results: HashSet::new(),
        }
    }

    fn assistant_in_reply_to(&self) -> Vec<JsonRpcId> {
        let mut v = self.observed_user_messages.clone();
        v.extend(self.observed_tool_results.iter().cloned());
        v
    }
}

struct Config {
    provider_id: String,
    model: String,
    api_key: String,
    endpoint: String,
    tools: Vec<Value>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let fd = parse_bus_fd()?;
    let mut cfg = read_config()?;

    let transport = adopt_bus_fd(fd).context("rfl-openai: adopt bus fd")?;
    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rfl-openai: client connect")?;
    let notifications = client.subscribe_notifications();

    cfg.tools = match client.call("core.tools_list", json!({})).await {
        Ok(v) => openai_tools_from_response(&v),
        Err(e) => {
            eprintln!("openai: core.tools_list failed: {e}");
            std::process::exit(1);
        }
    };

    let state = Arc::new(Mutex::new(State::new()));
    let wire = Arc::new(WireClient::new(cfg.endpoint.clone(), cfg.api_key.clone()));
    let peer = client.peer();

    run_loop(notifications, state, wire, peer, Arc::new(cfg)).await
}

fn openai_tools_from_response(resp: &Value) -> Vec<Value> {
    let tools_json = resp.get("tools").cloned().unwrap_or(Value::Null);
    let parsed: Vec<ToolSchema> = serde_json::from_value(tools_json).unwrap_or_default();
    parsed
        .into_iter()
        .map(|t| {
            let mut func = serde_json::Map::new();
            func.insert("name".into(), Value::String(t.name));
            if let Some(d) = t.description {
                func.insert("description".into(), Value::String(d));
            }
            func.insert("parameters".into(), t.parameters_schema);
            json!({"type": "function", "function": Value::Object(func)})
        })
        .collect()
}

async fn run_loop(
    mut notifications: broadcast::Receiver<InboundNotification>,
    state: Arc<Mutex<State>>,
    wire: Arc<WireClient>,
    peer: PeerHandle,
    cfg: Arc<Config>,
) -> Result<()> {
    loop {
        let note = match notifications.recv().await {
            Ok(n) => n,
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        };
        if note.method != "bus.event" {
            continue;
        }
        let Some(params) = note.params else { continue };
        let topic = params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let payload = params.get("payload").cloned().unwrap_or(Value::Null);
        let bus_request_id: Option<JsonRpcId> = params
            .get("request_id")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let in_reply_to: Option<Vec<JsonRpcId>> = params
            .get("in_reply_to")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        match topic.as_str() {
            "core.session.user_message" => {
                let state = state.clone();
                let wire = wire.clone();
                let peer = peer.clone();
                let cfg = cfg.clone();
                tokio::spawn(async move {
                    handle_user_message(&state, &wire, &peer, &cfg, payload, bus_request_id).await;
                });
            }
            "core.session.tool_result" => {
                handle_tool_result(&state, payload, bus_request_id, in_reply_to).await;
            }
            _ => {}
        }
    }
}

async fn handle_user_message(
    state: &Arc<Mutex<State>>,
    wire: &WireClient,
    peer: &PeerHandle,
    cfg: &Config,
    payload: Value,
    bus_request_id: Option<JsonRpcId>,
) {
    let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let messages = {
        let mut st = state.lock().await;
        if let Some(rid) = bus_request_id.clone() {
            st.observed_user_messages.push(rid);
        }
        st.history.push(Msg {
            role: "user".to_string(),
            content: Some(text),
            tool_calls: None,
            tool_call_id: None,
        });
        st.history.clone()
    };

    let req = ChatCompletionRequest {
        model: cfg.model.clone(),
        messages,
        tools: if cfg.tools.is_empty() {
            None
        } else {
            Some(cfg.tools.clone())
        },
        tool_choice: None,
        stream: false,
    };

    match wire.chat(&req).await {
        Ok(resp) => publish_response(state, peer, cfg, resp).await,
        Err(err) => publish_error(state, peer, cfg, &err).await,
    }
}

async fn publish_response(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    cfg: &Config,
    resp: rafaello_openai::ChatCompletionResponse,
) {
    if resp.choices.is_empty() {
        publish_error(state, peer, cfg, &OpenaiError::EmptyChoices).await;
        return;
    }
    if resp.choices.len() > 1 {
        tracing::warn!(
            choice_count = resp.choices.len(),
            "openai: response has multiple choices; using choices[0]"
        );
    }
    let choice = resp.choices.into_iter().next().expect("non-empty");
    let content_text = choice
        .message
        .content
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned();
    let tool_calls = choice.message.tool_calls.unwrap_or_default();

    if content_text.is_none() && tool_calls.is_empty() {
        publish_error(state, peer, cfg, &OpenaiError::EmptyChoices).await;
        return;
    }

    if let Some(text) = content_text {
        publish_assistant_message(state, peer, cfg, text).await;
    }

    let mut rewritten: Vec<ToolCall> = Vec::new();
    for tc in tool_calls.iter() {
        let args_value = match serde_json::from_str::<Value>(&tc.function.arguments) {
            Ok(v) => v,
            Err(e) => {
                publish_assistant_message(
                    state,
                    peer,
                    cfg,
                    map_to_assistant(&OpenaiError::InvalidToolArgs(e.to_string())),
                )
                .await;
                continue;
            }
        };
        let fresh = fresh_ulid();
        rewritten.push(ToolCall {
            id: fresh.clone(),
            kind: "function".to_string(),
            function: ToolCallFn {
                name: tc.function.name.clone(),
                arguments: tc.function.arguments.clone(),
            },
        });
        publish_tool_request(
            state,
            peer,
            cfg,
            JsonRpcId::String(fresh),
            tc.function.name.clone(),
            args_value,
        )
        .await;
    }

    if !rewritten.is_empty() {
        let mut st = state.lock().await;
        st.history.push(Msg {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(rewritten),
            tool_call_id: None,
        });
    }
}

async fn publish_assistant_message(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    cfg: &Config,
    text: String,
) {
    let topic = format!("provider.{}.assistant_message", cfg.provider_id);
    let fresh = JsonRpcId::String(fresh_ulid());
    let in_reply_to = {
        let mut st = state.lock().await;
        st.history.push(Msg {
            role: "assistant".to_string(),
            content: Some(text.clone()),
            tool_calls: None,
            tool_call_id: None,
        });
        st.assistant_in_reply_to()
    };
    let _ = peer.notify(
        "bus.publish",
        json!({
            "topic": topic,
            "payload": {"text": text},
            "request_id": fresh,
            "in_reply_to": in_reply_to,
        }),
    );
}

async fn publish_tool_request(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    cfg: &Config,
    request_id: JsonRpcId,
    tool: String,
    args: Value,
) {
    let topic = format!("provider.{}.tool_request", cfg.provider_id);
    let in_reply_to = {
        let st = state.lock().await;
        st.observed_tool_results.clone()
    };
    let _ = peer.notify(
        "bus.publish",
        json!({
            "topic": topic,
            "payload": {"tool": tool, "args": args},
            "request_id": request_id,
            "in_reply_to": in_reply_to,
        }),
    );
}

async fn publish_error(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    cfg: &Config,
    err: &OpenaiError,
) {
    publish_assistant_message(state, peer, cfg, map_to_assistant(err)).await;
}

async fn handle_tool_result(
    state: &Arc<Mutex<State>>,
    payload: Value,
    bus_request_id: Option<JsonRpcId>,
    in_reply_to: Option<Vec<JsonRpcId>>,
) {
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let tool_call_id = in_reply_to
        .as_ref()
        .and_then(|v| v.first())
        .and_then(|id| match id {
            JsonRpcId::String(s) => Some(s.clone()),
            JsonRpcId::Number(n) => Some(n.to_string()),
            JsonRpcId::Null => None,
        });
    let mut st = state.lock().await;
    if let Some(rid) = bus_request_id {
        if st.seen_tool_results.insert(rid.clone()) {
            st.observed_tool_results.push(rid);
        }
    }
    st.history.push(Msg {
        role: "tool".to_string(),
        content: Some(content),
        tool_calls: None,
        tool_call_id,
    });
}

fn fresh_ulid() -> String {
    Ulid::new().to_string()
}

fn read_config() -> Result<Config> {
    let provider_id = std::env::var("RFL_PROVIDER_ID").unwrap_or_else(|_| "openai".to_string());
    let model = read_required_model().map_err(|e| anyhow!(e))?;
    let endpoint = read_required_endpoint_url().map_err(|e| anyhow!(e))?;
    let api_key = read_required_api_key().map_err(|e| anyhow!(e))?;
    Ok(Config {
        provider_id,
        model,
        api_key,
        endpoint,
        tools: Vec::new(),
    })
}

fn parse_bus_fd() -> Result<RawFd> {
    let raw = std::env::var("RFL_BUS_FD").context("RFL_BUS_FD not set")?;
    let fd: RawFd = raw
        .parse()
        .with_context(|| format!("RFL_BUS_FD must be a non-negative integer (got {raw:?})"))?;
    if fd < 0 {
        return Err(anyhow!(
            "RFL_BUS_FD must be a non-negative integer (got {fd})"
        ));
    }
    Ok(fd)
}

fn adopt_bus_fd(fd: RawFd) -> Result<BusTransport> {
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let std_stream = std::os::unix::net::UnixStream::from(owned);
    std_stream
        .set_nonblocking(true)
        .context("set inherited bus fd to non-blocking")?;
    let stream = tokio::net::UnixStream::from_std(std_stream)
        .context("convert std UnixStream to tokio UnixStream")?;
    let (reader, writer) = stream.into_split();
    Ok(StdioTransport::new(reader, writer, MAX_FRAME_BYTES))
}

struct OneShotConnector {
    transport: Mutex<Option<BusTransport>>,
}

impl OneShotConnector {
    fn new(transport: BusTransport) -> Self {
        Self {
            transport: Mutex::new(Some(transport)),
        }
    }
}

#[async_trait]
impl Connector for OneShotConnector {
    type Connection = BusTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        self.transport
            .lock()
            .await
            .take()
            .ok_or_else(|| FittingsError::transport("OneShotConnector::connect called twice"))
    }
}
