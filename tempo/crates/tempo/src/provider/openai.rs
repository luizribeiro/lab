use std::time::{Duration, Instant};

use chrono::Utc;
use futures_util::StreamExt;
use serde_json::json;
use tokio::time::timeout;

use crate::matrix::Cell;
use crate::provider::metrics::Run;
use crate::provider::sse::{parse_data_line, ParsedChunk};

pub async fn run_request(cell: &Cell, base_url: &str, api_key: &str, timeout_secs: u64) -> Run {
    let started_at = Utc::now();
    let started = Instant::now();

    let mk_run = |error: Option<String>,
                  ttft_ms: Option<f64>,
                  decode_tok_s: Option<f64>,
                  e2e_ms: Option<f64>,
                  input_tokens: Option<u64>,
                  output_tokens: Option<u64>|
     -> Run {
        Run {
            suite: String::new(),
            scenario: cell.scenario.clone(),
            provider: cell.provider.clone(),
            model: cell.model.clone(),
            prompt: cell.prompt.clone(),
            run_idx: 0,
            started_at,
            ttft_ms,
            decode_tok_s,
            e2e_ms,
            input_tokens,
            output_tokens,
            error,
        }
    };

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = json!({
        "model": cell.model,
        "messages": [{"role": "user", "content": cell.prompt_text}],
        "max_tokens": cell.generation.max_tokens,
        "temperature": cell.generation.temperature,
        "stream": true,
        "stream_options": {"include_usage": true},
    });

    let client = reqwest::Client::new();
    let send_fut = client.post(&url).bearer_auth(api_key).json(&body).send();

    let result = timeout(Duration::from_secs(timeout_secs), async {
        let resp = send_fut.await.map_err(|e| format!("stream_send:{e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("http_{}", status.as_u16()));
        }
        let send_complete = Instant::now();
        consume_stream(resp, send_complete).await
    })
    .await;

    let outcome = match result {
        Err(_) => return mk_run(Some("timeout".to_string()), None, None, None, None, None),
        Ok(Err(e)) => return mk_run(Some(e), None, None, None, None, None),
        Ok(Ok(o)) => o,
    };

    if outcome.input_tokens.is_none() || outcome.output_tokens.is_none() {
        tracing::warn!(
            model = %cell.model,
            "stream completed without usage chunk; token counts will be null",
        );
    }

    let e2e_ms = started.elapsed().as_secs_f64() * 1000.0;
    let ttft_ms = outcome
        .first_content
        .map(|t| t.duration_since(outcome.send_complete).as_secs_f64() * 1000.0);
    let decode_tok_s = match (
        outcome.first_content,
        outcome.last_content,
        outcome.output_tokens,
    ) {
        (Some(first), Some(last), Some(out)) if out > 0 => {
            let secs = last.duration_since(first).as_secs_f64();
            (secs > 0.0).then(|| out as f64 / secs)
        }
        _ => None,
    };

    mk_run(
        None,
        ttft_ms,
        decode_tok_s,
        Some(e2e_ms),
        outcome.input_tokens,
        outcome.output_tokens,
    )
}

struct StreamOutcome {
    send_complete: Instant,
    first_content: Option<Instant>,
    last_content: Option<Instant>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

async fn consume_stream(
    resp: reqwest::Response,
    send_complete: Instant,
) -> Result<StreamOutcome, String> {
    let mut stream = resp.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut first_content: Option<Instant> = None;
    let mut last_content: Option<Instant> = None;
    let mut input_tokens: Option<u64> = None;
    let mut output_tokens: Option<u64> = None;

    while let Some(item) = stream.next().await {
        let bytes = item.map_err(|e| format!("stream_read:{e}"))?;
        buffer.extend_from_slice(&bytes);
        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
            let line = std::str::from_utf8(&line_bytes[..line_bytes.len() - 1])
                .map_err(|e| format!("stream_utf8:{e}"))?
                .trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let parsed = parse_data_line(data).map_err(|e| format!("stream_parse:{e}"))?;
            match parsed {
                ParsedChunk::Done => {}
                ParsedChunk::Chunk(c) => {
                    if let Some(content) = c.delta.content.as_deref() {
                        if !content.is_empty() {
                            let now = Instant::now();
                            if first_content.is_none() {
                                first_content = Some(now);
                            }
                            last_content = Some(now);
                        }
                    }
                    if let Some(u) = c.usage {
                        input_tokens = Some(u.prompt_tokens);
                        output_tokens = Some(u.completion_tokens);
                    }
                }
            }
        }
    }

    Ok(StreamOutcome {
        send_complete,
        first_content,
        last_content,
        input_tokens,
        output_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Generation;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cell() -> Cell {
        Cell {
            scenario: "decode".into(),
            provider: "vllm".into(),
            model: "test-model".into(),
            prompt: "short".into(),
            prompt_text: "Hello".into(),
            generation: Generation {
                max_tokens: 16,
                temperature: 0.0,
            },
        }
    }

    fn happy_sse_body() -> String {
        let frames = [
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":2,"total_tokens":9}}"#,
            "[DONE]",
        ];
        frames
            .iter()
            .map(|f| format!("data: {f}\n\n"))
            .collect::<String>()
    }

    #[tokio::test]
    async fn happy_path_produces_metrics() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(happy_sse_body()),
            )
            .mount(&server)
            .await;

        let run = run_request(&cell(), &server.uri(), "test-key", 5).await;

        assert!(run.error.is_none(), "error: {:?}", run.error);
        assert!(run.ttft_ms.unwrap() > 0.0);
        assert!(run.decode_tok_s.unwrap().is_finite());
        assert!(run.decode_tok_s.unwrap() > 0.0);
        assert_eq!(run.input_tokens, Some(7));
        assert_eq!(run.output_tokens, Some(2));
        assert!(run.e2e_ms.unwrap() > 0.0);
    }

    #[tokio::test]
    async fn http_500_produces_error_row() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let run = run_request(&cell(), &server.uri(), "k", 5).await;
        assert_eq!(run.error.as_deref(), Some("http_500"));
        assert!(run.ttft_ms.is_none());
        assert!(run.decode_tok_s.is_none());
        assert!(run.input_tokens.is_none());
    }

    #[tokio::test]
    async fn timeout_produces_timeout_row() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(happy_sse_body())
                    .set_delay(Duration::from_secs(5)),
            )
            .mount(&server)
            .await;

        let run = run_request(&cell(), &server.uri(), "k", 1).await;
        assert_eq!(run.error.as_deref(), Some("timeout"));
        assert!(run.ttft_ms.is_none());
        assert!(run.decode_tok_s.is_none());
    }

    #[tokio::test]
    async fn missing_usage_emits_null_token_counts() {
        let frames = [
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"!"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ];
        let body: String = frames.iter().map(|f| format!("data: {f}\n\n")).collect();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let run = run_request(&cell(), &server.uri(), "k", 5).await;
        assert!(run.error.is_none(), "error: {:?}", run.error);
        assert!(run.input_tokens.is_none());
        assert!(run.output_tokens.is_none());
        assert!(run.decode_tok_s.is_none(), "no usage means no decode rate");
        assert!(run.ttft_ms.unwrap() > 0.0);
    }
}
