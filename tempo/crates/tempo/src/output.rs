use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use serde::Serialize;

use crate::provider::metrics::Run;

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Serialize)]
struct Envelope<'a> {
    schema_version: u32,
    rows: &'a [Run],
}

pub fn write_runs<W: Write>(writer: W, runs: &[Run]) -> io::Result<()> {
    let envelope = Envelope {
        schema_version: SCHEMA_VERSION,
        rows: runs,
    };
    serde_json::to_writer_pretty(writer, &envelope)?;
    Ok(())
}

pub fn write_runs_to_path<P: AsRef<Path>>(path: P, runs: &[Run]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_runs(&mut writer, runs)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::var::VarValue;
    use chrono::TimeZone;
    use indexmap::IndexMap;
    use serde_json::Value;

    fn sample_vars() -> IndexMap<String, VarValue> {
        let mut vars: IndexMap<String, VarValue> = IndexMap::new();
        vars.insert("model".into(), VarValue::from("m1"));
        vars.insert("prompt".into(), VarValue::from("short"));
        vars
    }

    fn sample_runs() -> Vec<Run> {
        vec![
            Run {
                suite: "s".into(),
                scenario: "decode".into(),
                provider: "p".into(),
                vars: sample_vars(),
                run_idx: 0,
                started_at: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
                ttft_ms: Some(12.5),
                decode_tok_s: Some(80.0),
                e2e_ms: Some(250.0),
                input_tokens: Some(5),
                output_tokens: Some(20),
                error: None,
            },
            Run {
                suite: "s".into(),
                scenario: "decode".into(),
                provider: "p".into(),
                vars: sample_vars(),
                run_idx: 1,
                started_at: chrono::Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 6).unwrap(),
                ttft_ms: None,
                decode_tok_s: None,
                e2e_ms: None,
                input_tokens: None,
                output_tokens: None,
                error: Some("timeout".into()),
            },
        ]
    }

    #[test]
    fn round_trip_preserves_envelope_and_rows() {
        let runs = sample_runs();
        let mut buf: Vec<u8> = Vec::new();
        write_runs(&mut buf, &runs).expect("write ok");

        let parsed: Value = serde_json::from_slice(&buf).expect("valid json");
        assert_eq!(parsed["schema_version"], Value::from(2));

        let rows = parsed["rows"].as_array().expect("rows is array");
        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0]["scenario"], Value::from("decode"));
        assert_eq!(rows[0]["vars"]["model"], Value::from("m1"));
        assert_eq!(rows[0]["vars"]["prompt"], Value::from("short"));
        assert_eq!(rows[0]["run_idx"], Value::from(0));
        assert_eq!(rows[0]["ttft_ms"], Value::from(12.5));
        assert_eq!(rows[0]["output_tokens"], Value::from(20));
        assert!(rows[0]["error"].is_null());

        assert!(
            rows[0].get("model").is_none(),
            "v2 must not expose top-level model: {}",
            rows[0],
        );
        assert!(
            rows[0].get("prompt").is_none(),
            "v2 must not expose top-level prompt: {}",
            rows[0],
        );

        assert_eq!(rows[1]["run_idx"], Value::from(1));
        assert!(rows[1]["ttft_ms"].is_null());
        assert!(rows[1]["decode_tok_s"].is_null());
        assert_eq!(rows[1]["error"], Value::from("timeout"));

        for key in [
            "suite",
            "scenario",
            "provider",
            "vars",
            "run_idx",
            "started_at",
            "ttft_ms",
            "decode_tok_s",
            "e2e_ms",
            "input_tokens",
            "output_tokens",
            "error",
        ] {
            assert!(rows[0].get(key).is_some(), "row missing field {key}");
        }
    }

    #[test]
    fn write_to_path_creates_readable_file() {
        let runs = sample_runs();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("results.json");

        write_runs_to_path(&path, &runs).expect("write ok");

        let bytes = std::fs::read(&path).expect("read back");
        let parsed: Value = serde_json::from_slice(&bytes).expect("valid json");
        assert_eq!(parsed["schema_version"], Value::from(2));
        assert_eq!(parsed["rows"].as_array().unwrap().len(), 2);

        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains('\n'), "expected pretty-printed JSON");
    }
}
