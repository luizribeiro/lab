//! Test support for the OpenAI provider crate.
//!
//! - `wire_stub`: minimal one-shot HTTP stub used by the c32 wire
//!   tests (`start`, `sample_request`).
//! - `openai_provider_handle`: c33 bus-side test fixture that
//!   spawns `rfl-openai` against an in-test broker and a multi-
//!   response HTTP stub.

#![allow(dead_code)]

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub mod lock_kit;
pub mod openai_provider_handle;

pub struct Stub {
    pub endpoint: String,
}

pub async fn start(status: u16, body: &'static str) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 8192];
        let _ = tokio::time::timeout(Duration::from_millis(100), sock.read(&mut buf)).await;
        let reason = match status {
            401 => "Unauthorized",
            500 => "Internal Server Error",
            _ => "OK",
        };
        let resp = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.flush().await;
    });
    Stub {
        endpoint: format!("http://127.0.0.1:{port}"),
    }
}

pub fn sample_request() -> rafaello_openai::ChatCompletionRequest {
    rafaello_openai::ChatCompletionRequest {
        model: "vllm/qwen3.6-27b".to_string(),
        messages: vec![rafaello_openai::Msg {
            role: "user".to_string(),
            content: Some("hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
        }],
        tools: None,
        tool_choice: None,
        stream: false,
    }
}
