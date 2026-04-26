use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::output::SCHEMA_VERSION;
use crate::provider::metrics::Run;
use crate::stats;
use crate::summary;

#[derive(Debug, Deserialize)]
struct Envelope {
    schema_version: u32,
    rows: Vec<Run>,
}

pub fn render_report_from_path(path: &Path, color: bool) -> Result<String> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading report file {}", path.display()))?;
    render_report_from_str(&text, color)
}

pub(crate) fn render_report_from_str(text: &str, color: bool) -> Result<String> {
    let envelope: Envelope = serde_json::from_str(text).context("parsing results JSON")?;
    if envelope.schema_version != SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported schema_version {} (expected {})",
            envelope.schema_version,
            SCHEMA_VERSION
        ));
    }
    let cell_stats = stats::aggregate(&envelope.rows);
    Ok(summary::render(&cell_stats, color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::write_runs_to_path;
    use crate::provider::metrics::Run;
    use crate::var::VarValue;
    use chrono::TimeZone;
    use indexmap::IndexMap;

    fn run(model: &str, ttft: f64, decode: f64, idx: u32) -> Run {
        let mut vars: IndexMap<String, VarValue> = IndexMap::new();
        vars.insert("model".into(), VarValue::from(model));
        vars.insert("prompt".into(), VarValue::from("short"));
        Run {
            suite: "s".into(),
            scenario: "decode".into(),
            provider: "p".into(),
            vars,
            run_idx: idx,
            started_at: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            ttft_ms: Some(ttft),
            decode_tok_s: Some(decode),
            e2e_ms: Some(ttft),
            input_tokens: Some(10),
            output_tokens: Some(decode as u64),
            error: None,
        }
    }

    #[test]
    fn round_trip_via_written_file() {
        let runs = vec![
            run("vllm/qwen3.6-27b", 10.0, 80.0, 0),
            run("vllm/qwen3.6-27b", 10.0, 80.0, 1),
            run("mlx/qwen2.5-coder-7b", 30.0, 20.0, 0),
        ];

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("results.json");
        write_runs_to_path(&path, &runs).expect("write ok");

        let table = render_report_from_path(&path, false).expect("render ok");
        assert!(
            table.contains("vllm/qwen3.6-27b"),
            "expected model name in table, got:\n{table}"
        );
        assert!(
            table.contains("mlx/qwen2.5-coder-7b"),
            "expected second model name in table, got:\n{table}"
        );
        assert!(
            table.contains("2/2"),
            "expected aggregated success count 2/2 in table, got:\n{table}"
        );
        assert!(
            table.contains("1/1"),
            "expected aggregated success count 1/1 in table, got:\n{table}"
        );
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let json = r#"{"schema_version": 1, "rows": []}"#;
        let err = render_report_from_str(json, false).expect_err("should reject v1");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("schema_version"),
            "error should mention schema_version, got: {msg}"
        );
    }

    #[test]
    fn rejects_invalid_json() {
        let err = render_report_from_str("{not json", false).expect_err("should reject");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("parsing results JSON"),
            "error should mention parsing, got: {msg}"
        );
    }

    #[test]
    fn missing_file_errors_clearly() {
        let err = render_report_from_path(Path::new("/nonexistent/path.json"), false)
            .expect_err("should fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("reading report file"),
            "error should mention reading, got: {msg}"
        );
    }
}
