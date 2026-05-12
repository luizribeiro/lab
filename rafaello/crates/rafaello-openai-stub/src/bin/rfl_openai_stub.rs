//! Deterministic stub for the OpenAI Chat Completions API
//! (scope §W2 + §A8 + §E1).
//!
//! Binds to `127.0.0.1:0`, prints the assigned port to stdout, then
//! serves POST `/v1/chat/completions` with a JSON response chosen from
//! one of two mutually-exclusive sources:
//!
//! * `--response <path>` / `RFL_OPENAI_STUB_RESPONSE` — a JSON file
//!   containing a `Vec<ChatCompletionResponse>`; successive POSTs
//!   return successive elements (the last element is replayed after
//!   the list is exhausted, so single-turn tests can ship a 1-element
//!   file).
//! * `RFL_OPENAI_STUB_SCRIPTED_TURNS=<path-to-toml>` — a TOML script
//!   of `[[turn]]` entries, each with a predicate
//!   (`match_last_user_message` or `match_last_tool_call_function`)
//!   and a `response` literal (`ChatCompletionResponse` JSON value).
//!   Turns are walked in order via an `AtomicUsize` cursor; the cursor
//!   points at the next turn whose predicate must match. Predicate
//!   miss or exhaustion writes a deterministic stderr line and calls
//!   `std::process::exit(1)` (scope §E2).
//!
//! Setting both selectors at startup is an error. Malformed POST
//! bodies get a `400 Bad Request` plus an error line on stderr (the
//! "useful test signal" from scope §W2). A 5s self-timeout
//! (`RFL_FIXTURE_MAX_LIFETIME` pattern from m2 retro §5.4) and SIGTERM
//! both exit the process cleanly.

use std::env;
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::{signal, SignalKind};

const SELF_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
#[allow(dead_code)]
struct ChatCompletionRequestShape {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(default)]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    tool_choice: Option<serde_json::Value>,
    stream: bool,
}

#[derive(Deserialize)]
struct ScriptedTurnsFile {
    #[serde(default, rename = "turn")]
    turns: Vec<ScriptedTurnRaw>,
}

#[derive(Deserialize)]
struct ScriptedTurnRaw {
    #[serde(default)]
    match_last_user_message: Option<String>,
    #[serde(default)]
    match_last_tool_call_function: Option<String>,
    response: String,
}

struct Turn {
    predicate: Predicate,
    response: serde_json::Value,
}

enum Predicate {
    LastUserMessage(String),
    LastToolCallFunction(String),
}

enum Mode {
    Sequence(Arc<Vec<serde_json::Value>>, Arc<AtomicUsize>),
    Scripted(Arc<Vec<Turn>>, Arc<AtomicUsize>),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mode = resolve_mode()?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    println!("{port}");
    std::io::stdout().flush().ok();

    let mut sigterm = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = tokio::time::sleep(SELF_TIMEOUT) => {
            eprintln!("rfl-openai-stub: self-timeout {SELF_TIMEOUT:?} reached, exiting");
        }
        _ = sigterm.recv() => {
            eprintln!("rfl-openai-stub: SIGTERM received, exiting");
        }
        res = serve(listener, mode) => {
            res?;
        }
    }
    Ok(())
}

fn resolve_mode() -> Result<Mode> {
    let response_path = response_path_from_args_or_env();
    let scripted_path = env::var("RFL_OPENAI_STUB_SCRIPTED_TURNS").ok();
    match (response_path, scripted_path) {
        (Some(_), Some(_)) => Err(anyhow!(
            "RFL_OPENAI_STUB_SCRIPTED_TURNS is mutually exclusive with --response / \
             RFL_OPENAI_STUB_RESPONSE"
        )),
        (Some(path), None) => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("read response file {path}"))?;
            let responses: Vec<serde_json::Value> =
                serde_json::from_str(&raw).with_context(|| {
                    format!("parse {path}: expected a JSON array of ChatCompletionResponse")
                })?;
            if responses.is_empty() {
                return Err(anyhow!("response file must contain at least one element"));
            }
            Ok(Mode::Sequence(
                Arc::new(responses),
                Arc::new(AtomicUsize::new(0)),
            ))
        }
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("read scripted-turns file {path}"))?;
            let parsed: ScriptedTurnsFile = toml::from_str(&raw)
                .with_context(|| format!("parse {path}: expected a TOML [[turn]] script"))?;
            if parsed.turns.is_empty() {
                return Err(anyhow!(
                    "scripted-turns file must contain at least one [[turn]]"
                ));
            }
            let turns: Vec<Turn> = parsed
                .turns
                .into_iter()
                .enumerate()
                .map(|(i, t)| turn_from_raw(i, t))
                .collect::<Result<_>>()?;
            Ok(Mode::Scripted(
                Arc::new(turns),
                Arc::new(AtomicUsize::new(0)),
            ))
        }
        (None, None) => Err(anyhow!(
            "missing --response <path> / RFL_OPENAI_STUB_RESPONSE or RFL_OPENAI_STUB_SCRIPTED_TURNS"
        )),
    }
}

fn turn_from_raw(idx: usize, raw: ScriptedTurnRaw) -> Result<Turn> {
    let predicate = match (
        raw.match_last_user_message,
        raw.match_last_tool_call_function,
    ) {
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "[[turn]] {idx}: set exactly one of match_last_user_message / \
                 match_last_tool_call_function"
            ));
        }
        (Some(s), None) => Predicate::LastUserMessage(s),
        (None, Some(s)) => Predicate::LastToolCallFunction(s),
        (None, None) => {
            return Err(anyhow!(
                "[[turn]] {idx}: missing match_last_user_message or \
                 match_last_tool_call_function predicate"
            ));
        }
    };
    let response: serde_json::Value = serde_json::from_str(&raw.response).with_context(|| {
        format!("[[turn]] {idx}: response field must be a JSON ChatCompletionResponse")
    })?;
    Ok(Turn {
        predicate,
        response,
    })
}

fn response_path_from_args_or_env() -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--response" {
            if let Some(p) = args.next() {
                return Some(p);
            }
        }
    }
    env::var("RFL_OPENAI_STUB_RESPONSE").ok()
}

async fn serve(listener: TcpListener, mode: Mode) -> Result<()> {
    let mode = Arc::new(mode);
    loop {
        let (stream, _) = listener.accept().await?;
        let mode = mode.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, mode).await {
                eprintln!("rfl-openai-stub: connection error: {e:#}");
            }
        });
    }
}

async fn handle(mut stream: TcpStream, mode: Arc<Mode>) -> Result<()> {
    let mut buf = Vec::with_capacity(8192);
    let (head_end, content_length, target_ok) = loop {
        let mut tmp = [0u8; 4096];
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(anyhow!("client closed before sending full headers"));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(idx) = find_subslice(&buf, b"\r\n\r\n") {
            let head = std::str::from_utf8(&buf[..idx])
                .map_err(|_| anyhow!("non-utf8 request headers"))?;
            let mut lines = head.lines();
            let request_line = lines.next().unwrap_or("");
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("");
            let target_ok = method == "POST" && target == "/v1/chat/completions";
            let mut content_length: usize = 0;
            for line in lines {
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:") {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
            }
            break (idx + 4, content_length, target_ok);
        }
    };
    if !target_ok {
        return write_response(&mut stream, 404, "text/plain", b"not found").await;
    }
    while buf.len() < head_end + content_length {
        let mut tmp = [0u8; 4096];
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let body_end = (head_end + content_length).min(buf.len());
    let body = &buf[head_end..body_end];
    let request = match serde_json::from_slice::<ChatCompletionRequestShape>(body) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("rfl-openai-stub: malformed request body: {e}");
            return write_response(&mut stream, 400, "text/plain", b"malformed request body").await;
        }
    };

    let response_value = match mode.as_ref() {
        Mode::Sequence(responses, counter) => {
            let idx = counter.fetch_add(1, Ordering::SeqCst);
            responses[idx.min(responses.len() - 1)].clone()
        }
        Mode::Scripted(turns, cursor) => dispatch_scripted(turns, cursor, &request),
    };
    let body = serde_json::to_vec(&response_value)?;
    write_response(&mut stream, 200, "application/json", &body).await
}

fn dispatch_scripted(
    turns: &Arc<Vec<Turn>>,
    cursor: &Arc<AtomicUsize>,
    request: &ChatCompletionRequestShape,
) -> serde_json::Value {
    let idx = cursor.fetch_add(1, Ordering::SeqCst);
    if idx >= turns.len() {
        eprintln!(
            "rfl-openai-stub: scripted turns exhausted; unmatched request: {}",
            request_summary(request)
        );
        std::process::exit(1);
    }
    let turn = &turns[idx];
    if predicate_matches(&turn.predicate, request) {
        return turn.response.clone();
    }
    eprintln!(
        "rfl-openai-stub: scripted turns exhausted; unmatched request: {}",
        request_summary(request)
    );
    std::process::exit(1);
}

fn predicate_matches(predicate: &Predicate, request: &ChatCompletionRequestShape) -> bool {
    match predicate {
        Predicate::LastUserMessage(needle) => last_user_message_content(&request.messages)
            .is_some_and(|content| {
                content
                    .to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase())
            }),
        Predicate::LastToolCallFunction(needle) => {
            last_tool_call_function_name(&request.messages).is_some_and(|name| name == *needle)
        }
    }
}

fn last_user_message_content(messages: &[serde_json::Value]) -> Option<String> {
    for msg in messages.iter().rev() {
        if msg.get("role").and_then(|v| v.as_str()) == Some("user") {
            return msg
                .get("content")
                .and_then(|v| v.as_str())
                .map(String::from);
        }
    }
    None
}

fn last_tool_call_function_name(messages: &[serde_json::Value]) -> Option<String> {
    let mut tool_msg_idx = None;
    for (i, msg) in messages.iter().enumerate().rev() {
        if msg.get("role").and_then(|v| v.as_str()) == Some("tool") {
            tool_msg_idx = Some(i);
            break;
        }
    }
    let tool_msg_idx = tool_msg_idx?;
    let tool_call_id = messages[tool_msg_idx]
        .get("tool_call_id")
        .and_then(|v| v.as_str())?;
    for msg in messages[..tool_msg_idx].iter().rev() {
        if msg.get("role").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        let calls = msg.get("tool_calls").and_then(|v| v.as_array())?;
        for call in calls {
            if call.get("id").and_then(|v| v.as_str()) == Some(tool_call_id) {
                return call
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from);
            }
        }
    }
    None
}

fn request_summary(request: &ChatCompletionRequestShape) -> String {
    let last_role = request
        .messages
        .last()
        .and_then(|m| m.get("role").and_then(|v| v.as_str()))
        .unwrap_or("?");
    format!(
        "model={} messages={} last_role={}",
        request.model,
        request.messages.len(),
        last_role,
    )
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Status",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    stream.shutdown().await.ok();
    Ok(())
}
