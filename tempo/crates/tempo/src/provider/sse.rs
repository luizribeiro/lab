use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
}

impl Delta {
    pub fn any_token_text(&self) -> Option<&str> {
        [
            self.content.as_deref(),
            self.reasoning_content.as_deref(),
            self.reasoning.as_deref(),
        ]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamChunk {
    pub delta: Delta,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedChunk {
    Done,
    Chunk(StreamChunk),
}

#[derive(Debug, Error)]
pub enum SseParseError {
    #[error("malformed SSE chunk JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parse the payload of a single SSE `data:` line (the prefix must already be stripped).
pub fn parse_data_line(data: &str) -> Result<ParsedChunk, SseParseError> {
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return Ok(ParsedChunk::Done);
    }
    let raw: RawChunk = serde_json::from_str(trimmed)?;
    let (delta, finish_reason) = raw
        .choices
        .into_iter()
        .next()
        .map(|c| (c.delta, c.finish_reason))
        .unwrap_or_default();
    Ok(ParsedChunk::Chunk(StreamChunk {
        delta,
        finish_reason,
        usage: raw.usage,
    }))
}

#[derive(Deserialize)]
struct RawChunk {
    #[serde(default)]
    choices: Vec<RawChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct RawChoice {
    #[serde(default)]
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unwrap_chunk(parsed: ParsedChunk) -> StreamChunk {
        match parsed {
            ParsedChunk::Chunk(c) => c,
            ParsedChunk::Done => panic!("expected chunk, got [DONE]"),
        }
    }

    #[test]
    fn done_sentinel_is_recognized() {
        assert_eq!(parse_data_line("[DONE]").unwrap(), ParsedChunk::Done);
        assert_eq!(parse_data_line("  [DONE]  ").unwrap(), ParsedChunk::Done);
    }

    #[test]
    fn role_only_chunk_has_no_content() {
        let data = r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        let chunk = unwrap_chunk(parse_data_line(data).unwrap());
        assert_eq!(chunk.delta.role.as_deref(), Some("assistant"));
        assert_eq!(chunk.delta.content, None);
        assert_eq!(chunk.finish_reason, None);
        assert_eq!(chunk.usage, None);
    }

    #[test]
    fn content_chunk_extracts_delta() {
        let data = r#"{"choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk = unwrap_chunk(parse_data_line(data).unwrap());
        assert_eq!(chunk.delta.content.as_deref(), Some("Hello"));
        assert_eq!(chunk.delta.role, None);
    }

    #[test]
    fn finish_reason_chunk_is_captured() {
        let data = r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let chunk = unwrap_chunk(parse_data_line(data).unwrap());
        assert_eq!(chunk.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn final_usage_chunk_is_extracted() {
        let data = r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":34,"total_tokens":46}}"#;
        let chunk = unwrap_chunk(parse_data_line(data).unwrap());
        assert_eq!(chunk.delta, Delta::default());
        assert_eq!(
            chunk.usage,
            Some(Usage {
                prompt_tokens: 12,
                completion_tokens: 34,
            })
        );
    }

    #[test]
    fn any_token_text_prefers_content() {
        let d = Delta {
            content: Some("hi".into()),
            reasoning_content: Some("rc".into()),
            reasoning: Some("r".into()),
            ..Default::default()
        };
        assert_eq!(d.any_token_text(), Some("hi"));
    }

    #[test]
    fn any_token_text_falls_back_to_reasoning_content() {
        let d = Delta {
            reasoning_content: Some("thinking".into()),
            ..Default::default()
        };
        assert_eq!(d.any_token_text(), Some("thinking"));
    }

    #[test]
    fn any_token_text_falls_back_to_reasoning() {
        let d = Delta {
            reasoning: Some("ponder".into()),
            ..Default::default()
        };
        assert_eq!(d.any_token_text(), Some("ponder"));
    }

    #[test]
    fn any_token_text_none_when_all_absent() {
        assert_eq!(Delta::default().any_token_text(), None);
    }

    #[test]
    fn any_token_text_skips_empty_content_and_uses_reasoning() {
        let d = Delta {
            content: Some(String::new()),
            reasoning_content: Some("rc".into()),
            ..Default::default()
        };
        assert_eq!(d.any_token_text(), Some("rc"));
    }

    #[test]
    fn any_token_text_none_when_all_empty() {
        let d = Delta {
            content: Some(String::new()),
            reasoning_content: Some(String::new()),
            reasoning: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(d.any_token_text(), None);
    }

    #[test]
    fn malformed_json_returns_err() {
        let err = parse_data_line("{not json").unwrap_err();
        assert!(matches!(err, SseParseError::Json(_)), "got {err:?}");
    }

    #[test]
    fn fixture_stream_with_usage_parses_in_order() {
        let frames = [
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":2,"total_tokens":9}}"#,
            "[DONE]",
        ];

        let parsed: Vec<ParsedChunk> = frames
            .iter()
            .map(|f| parse_data_line(f).expect("parse"))
            .collect();

        let content: String = parsed
            .iter()
            .filter_map(|p| match p {
                ParsedChunk::Chunk(c) => c.delta.content.clone(),
                ParsedChunk::Done => None,
            })
            .collect();
        assert_eq!(content, "Hello");

        let usage = parsed.iter().find_map(|p| match p {
            ParsedChunk::Chunk(c) => c.usage,
            _ => None,
        });
        assert_eq!(
            usage,
            Some(Usage {
                prompt_tokens: 7,
                completion_tokens: 2,
            })
        );

        assert_eq!(parsed.last(), Some(&ParsedChunk::Done));
    }

    #[test]
    fn missing_usage_stream_is_parseable() {
        let frames = [
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ];

        let usage_seen = frames.iter().any(|f| {
            matches!(
                parse_data_line(f),
                Ok(ParsedChunk::Chunk(StreamChunk { usage: Some(_), .. }))
            )
        });
        assert!(!usage_seen, "no usage event expected");
    }
}
