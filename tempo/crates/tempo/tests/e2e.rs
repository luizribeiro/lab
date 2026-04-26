use std::collections::BTreeSet;
use std::env;

use serde_json::Value;
use tempo::config::Config;
use tempo::output::write_runs_to_path;
use tempo::runner::run_all;
use tempo::test_support::happy_sse_body;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_KEY_ENV: &str = "TEMPO_E2E_API_KEY";

#[tokio::test]
async fn end_to_end_writes_envelope_with_expected_rows() {
    // unsafe required: env::set_var is not thread-safe (Rust 2024 edition).
    unsafe {
        env::set_var(TEST_KEY_ENV, "test-key");
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(happy_sse_body()),
        )
        .mount(&server)
        .await;

    let toml = format!(
        r#"
[suite]
name = "e2e"

[providers.p]
kind = "openai_compatible"
base_url = "{base}"
api_key_env = "{key_env}"

[prompts.short]
kind = "inline"
text = "hi"

[prompts.long]
kind = "inline"
text = "hello there"

[scenarios.decode]
kind = "throughput"
provider = "p"
warmup = 0
runs = 2
timeout_secs = 5
generation = {{ max_tokens = 4, temperature = 0.0 }}
[scenarios.decode.matrix]
model = ["m1", "m2"]
prompt = ["short", "long"]
"#,
        base = server.uri(),
        key_env = TEST_KEY_ENV,
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let out_path = dir.path().join("results.json");

    let cfg = Config::from_toml_str(&toml).expect("config parses");
    let result = run_all(&cfg, &tempo::progress::NoopReporter)
        .await
        .expect("runner ok");
    write_runs_to_path(&out_path, &result.runs).expect("write results");

    assert_eq!(result.total_cells, 4);
    assert_eq!(result.zero_success_cells, 0);

    let bytes = std::fs::read(&out_path).expect("read results");
    let parsed: Value = serde_json::from_slice(&bytes).expect("valid json");
    assert_eq!(parsed["schema_version"], Value::from(2));

    let rows = parsed["rows"].as_array().expect("rows is array");
    assert_eq!(rows.len(), 8);

    let mut cells: BTreeSet<(String, String)> = BTreeSet::new();
    for row in rows {
        assert!(row["error"].is_null(), "row had error: {row}");
        assert!(row["ttft_ms"].is_number(), "row missing ttft_ms: {row}");
        assert!(
            row["decode_tok_s"].is_number(),
            "row missing decode_tok_s: {row}"
        );
        assert!(
            row.get("model").is_none(),
            "v2 must not expose top-level model: {row}"
        );
        assert!(
            row.get("prompt").is_none(),
            "v2 must not expose top-level prompt: {row}"
        );
        let vars = row["vars"].as_object().expect("row.vars is object");
        assert!(vars.contains_key("model"), "vars missing model: {row}");
        assert!(vars.contains_key("prompt"), "vars missing prompt: {row}");
        cells.insert((
            vars["model"].as_str().unwrap().to_string(),
            vars["prompt"].as_str().unwrap().to_string(),
        ));
    }
    assert_eq!(
        cells,
        BTreeSet::from([
            ("m1".to_string(), "short".to_string()),
            ("m1".to_string(), "long".to_string()),
            ("m2".to_string(), "short".to_string()),
            ("m2".to_string(), "long".to_string()),
        ])
    );
}
