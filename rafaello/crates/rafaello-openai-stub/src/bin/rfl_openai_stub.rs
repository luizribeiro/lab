//! Deterministic stub for the OpenAI Chat Completions API
//! (scope §W2 + §A8).
//!
//! Binds to `127.0.0.1:0`, prints the assigned port to stdout, then
//! serves POST `/v1/chat/completions` with a JSON response read from
//! `--response <path>` (or `RFL_OPENAI_STUB_RESPONSE`). The file
//! contains a `Vec<ChatCompletionResponse>`; successive POSTs return
//! successive elements (the last element is replayed after the list
//! is exhausted, so single-turn tests can ship a 1-element file).
//! Malformed POST bodies get a `400 Bad Request` plus an error line
//! on stderr (the "useful test signal" from scope §W2). A 5s
//! self-timeout (`RFL_FIXTURE_MAX_LIFETIME` pattern from m2 retro
//! §5.4) and SIGTERM both exit the process cleanly.

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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let response_path = resolve_response_path()?;
    let raw = std::fs::read_to_string(&response_path)
        .with_context(|| format!("read response file {response_path}"))?;
    let responses: Vec<serde_json::Value> = serde_json::from_str(&raw).with_context(|| {
        format!("parse {response_path}: expected a JSON array of ChatCompletionResponse")
    })?;
    if responses.is_empty() {
        return Err(anyhow!("response file must contain at least one element"));
    }

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    println!("{port}");
    std::io::stdout().flush().ok();

    let responses = Arc::new(responses);
    let counter = Arc::new(AtomicUsize::new(0));
    let mut sigterm = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = tokio::time::sleep(SELF_TIMEOUT) => {
            eprintln!("rfl-openai-stub: self-timeout {SELF_TIMEOUT:?} reached, exiting");
        }
        _ = sigterm.recv() => {
            eprintln!("rfl-openai-stub: SIGTERM received, exiting");
        }
        res = serve(listener, responses, counter) => {
            res?;
        }
    }
    Ok(())
}

fn resolve_response_path() -> Result<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--response" {
            return args
                .next()
                .ok_or_else(|| anyhow!("--response requires a path argument"));
        }
    }
    env::var("RFL_OPENAI_STUB_RESPONSE")
        .map_err(|_| anyhow!("missing --response <path> or RFL_OPENAI_STUB_RESPONSE"))
}

async fn serve(
    listener: TcpListener,
    responses: Arc<Vec<serde_json::Value>>,
    counter: Arc<AtomicUsize>,
) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let responses = responses.clone();
        let counter = counter.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, responses, counter).await {
                eprintln!("rfl-openai-stub: connection error: {e:#}");
            }
        });
    }
}

async fn handle(
    mut stream: TcpStream,
    responses: Arc<Vec<serde_json::Value>>,
    counter: Arc<AtomicUsize>,
) -> Result<()> {
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
    match serde_json::from_slice::<ChatCompletionRequestShape>(body) {
        Ok(_) => {
            let idx = counter.fetch_add(1, Ordering::SeqCst);
            let pick = &responses[idx.min(responses.len() - 1)];
            let body = serde_json::to_vec(pick)?;
            write_response(&mut stream, 200, "application/json", &body).await
        }
        Err(e) => {
            eprintln!("rfl-openai-stub: malformed request body: {e}");
            write_response(&mut stream, 400, "text/plain", b"malformed request body").await
        }
    }
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
