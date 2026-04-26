use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::var::VarValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub suite: String,
    pub scenario: String,
    pub provider: String,
    pub vars: IndexMap<String, VarValue>,
    pub run_idx: u32,
    pub started_at: DateTime<Utc>,
    pub ttft_ms: Option<f64>,
    pub decode_tok_s: Option<f64>,
    pub e2e_ms: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub error: Option<String>,
}
