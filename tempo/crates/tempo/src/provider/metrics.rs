use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub suite: String,
    pub scenario: String,
    pub provider: String,
    pub model: String,
    pub prompt: String,
    pub run_idx: u32,
    pub started_at: DateTime<Utc>,
    pub ttft_ms: Option<f64>,
    pub decode_tok_s: Option<f64>,
    pub e2e_ms: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub error: Option<String>,
}
