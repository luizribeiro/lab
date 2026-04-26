use std::time::{Duration, Instant};

use chrono::Utc;
use futures_util::StreamExt;
use serde_json::json;
use tokio::time::timeout;

use crate::matrix::Cell;
use crate::progress::ProgressReporter;
use crate::provider::metrics::Run;
use crate::provider::sse::{parse_data_line, ParsedChunk};

pub async fn run_request(
    cell: &Cell,
    prompt_text: &str,
    base_url: &str,
    api_key: &str,
    timeout_secs: u64,
    reporter: &dyn ProgressReporter,
    cell_id: &str,
) -> Run {
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
        "messages": [{"role": "user", "content": prompt_text}],
        "max_tokens": cell.generation.max_tokens,
        "temperature": cell.generation.temperature,
        "stream": true,
        "stream_options": {"include_usage": true},
    });

    let client = reqwest::Client::new();
    let request = client.post(&url).bearer_auth(api_key).json(&body);

    let result = timeout(Duration::from_secs(timeout_secs), async {
        let request_initiated = Instant::now();
        let resp = request
            .send()
            .await
            .map_err(|e| format!("stream_send:{e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("http_{}", status.as_u16()));
        }
        consume_stream(resp, request_initiated, reporter, cell_id).await
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
        .first_token
        .map(|t| t.duration_since(outcome.request_initiated).as_secs_f64() * 1000.0);
    let decode_tok_s = match (
        outcome.first_token,
        outcome.last_token,
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
    request_initiated: Instant,
    first_token: Option<Instant>,
    last_token: Option<Instant>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

async fn consume_stream(
    resp: reqwest::Response,
    request_initiated: Instant,
    reporter: &dyn ProgressReporter,
    cell_id: &str,
) -> Result<StreamOutcome, String> {
    let mut stream = resp.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut first_token: Option<Instant> = None;
    let mut last_token: Option<Instant> = None;
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
                    if c.delta.any_token_text().is_some() {
                        let now = Instant::now();
                        if first_token.is_none() {
                            first_token = Some(now);
                        }
                        last_token = Some(now);
                        reporter.token_received(cell_id);
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
        request_initiated,
        first_token,
        last_token,
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
            prompt_template: false,
            generation: Generation {
                max_tokens: 16,
                temperature: 0.0,
            },
        }
    }

    fn sse_body(frames: &[&str]) -> String {
        frames.iter().map(|f| format!("data: {f}\n\n")).collect()
    }

    fn happy_sse_body() -> String {
        sse_body(&[
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":2,"total_tokens":9}}"#,
            "[DONE]",
        ])
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

        let run = run_request(
            &cell(),
            "Hello",
            &server.uri(),
            "test-key",
            5,
            &crate::progress::NoopReporter,
            "test-cell",
        )
        .await;

        assert!(run.error.is_none(), "error: {:?}", run.error);
        assert!(run.ttft_ms.unwrap() > 0.0);
        assert!(run.decode_tok_s.unwrap().is_finite());
        assert!(run.decode_tok_s.unwrap() > 0.0);
        assert_eq!(run.input_tokens, Some(7));
        assert_eq!(run.output_tokens, Some(2));
        assert!(run.e2e_ms.unwrap() > 0.0);
    }

    #[tokio::test]
    async fn ttft_includes_header_delay() {
        let server = MockServer::start().await;
        let delay = Duration::from_millis(150);
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(happy_sse_body())
                    .set_delay(delay),
            )
            .mount(&server)
            .await;

        let run = run_request(
            &cell(),
            "Hello",
            &server.uri(),
            "test-key",
            5,
            &crate::progress::NoopReporter,
            "test-cell",
        )
        .await;

        assert!(run.error.is_none(), "error: {:?}", run.error);
        let ttft_ms = run.ttft_ms.expect("ttft_ms should be present");
        assert!(
            ttft_ms >= delay.as_secs_f64() * 1000.0,
            "ttft_ms {ttft_ms} should be >= delay {}ms",
            delay.as_millis(),
        );
    }

    #[tokio::test]
    async fn http_500_produces_error_row() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let run = run_request(
            &cell(),
            "Hello",
            &server.uri(),
            "k",
            5,
            &crate::progress::NoopReporter,
            "test-cell",
        )
        .await;
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

        let run = run_request(
            &cell(),
            "Hello",
            &server.uri(),
            "k",
            1,
            &crate::progress::NoopReporter,
            "test-cell",
        )
        .await;
        assert_eq!(run.error.as_deref(), Some("timeout"));
        assert!(run.ttft_ms.is_none());
        assert!(run.decode_tok_s.is_none());
    }

    #[tokio::test]
    async fn reasoning_only_stream_sets_ttft_and_decode_rate() {
        let body = sse_body(&[
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"","reasoning_content":"Hmm"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"","reasoning_content":" let"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"","reasoning_content":" me"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}"#,
            "[DONE]",
        ]);

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

        let run = run_request(
            &cell(),
            "Hello",
            &server.uri(),
            "k",
            5,
            &crate::progress::NoopReporter,
            "test-cell",
        )
        .await;
        assert!(run.error.is_none(), "error: {:?}", run.error);
        assert!(run.ttft_ms.unwrap() > 0.0);
        let rate = run.decode_tok_s.expect("decode_tok_s should be present");
        assert!(rate.is_finite() && rate > 0.0, "decode_tok_s={rate}");
        assert_eq!(run.output_tokens, Some(3));
    }

    #[tokio::test]
    async fn missing_usage_emits_null_token_counts() {
        let body = sse_body(&[
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"!"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ]);

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

        let run = run_request(
            &cell(),
            "Hello",
            &server.uri(),
            "k",
            5,
            &crate::progress::NoopReporter,
            "test-cell",
        )
        .await;
        assert!(run.error.is_none(), "error: {:?}", run.error);
        assert!(run.input_tokens.is_none());
        assert!(run.output_tokens.is_none());
        assert!(run.decode_tok_s.is_none(), "no usage means no decode rate");
        assert!(run.ttft_ms.unwrap() > 0.0);
    }
}
