use std::env;

use serde::Deserialize;
use tempo::config::{Config, Generation, Provider, Scenario};
use tempo::matrix::Cell;
use tempo::output::write_runs_to_path;
use tempo::provider::openai::run_request;
use tempo::runner::run_all;

const FIXTURE: &str = include_str!("../../../examples/vllm-smoke.toml");

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url_override = env::var("LITELLM_BASE_URL")
        .unwrap_or_else(|_| "https://litellm.thepromisedlan.club/v1".to_string());
    let api_key = env::var("LITELLM_API_KEY")?;

    let client = reqwest::Client::new();
    let models: ModelsResponse = client
        .get(format!(
            "{}/models",
            base_url_override.trim_end_matches('/')
        ))
        .bearer_auth(&api_key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let model = probe_for_working_model(&models, &base_url_override, &api_key)
        .await
        .ok_or("no working chat-capable model found")?;
    eprintln!("using model: {model}");

    let mut cfg = Config::from_toml_str(FIXTURE)?;

    for provider in cfg.providers.values_mut() {
        let Provider::OpenaiCompatible {
            base_url,
            api_key_env,
        } = provider;
        *base_url = base_url_override.clone();
        // unsafe required: env::set_var is not thread-safe (Rust 2024 edition).
        unsafe {
            env::set_var(api_key_env, &api_key);
        }
    }
    for scenario in cfg.scenarios.values_mut() {
        let Scenario::Throughput {
            warmup,
            runs,
            matrix,
            ..
        } = scenario;
        *warmup = 1;
        *runs = 3;
        matrix.model = vec![model.clone()];
    }

    let out = run_all(&cfg, &tempo::progress::NoopReporter).await?;
    eprintln!(
        "runs: {}, zero_success_cells: {}",
        out.runs.len(),
        out.zero_success_cells,
    );
    let out_path = env::var("TEMPO_OUT").unwrap_or_else(|_| "/tmp/tempo-results.json".to_string());
    write_runs_to_path(&out_path, &out.runs)?;
    eprintln!("wrote results to {out_path}");

    let bytes = std::fs::read(&out_path)?;
    let parsed: serde_json::Value = serde_json::from_slice(&bytes)?;
    println!("{}", serde_json::to_string_pretty(&parsed)?);
    Ok(())
}

async fn probe_for_working_model(
    models: &ModelsResponse,
    base_url: &str,
    api_key: &str,
) -> Option<String> {
    let skip = ["embed", "embedding", "whisper", "tts", "rerank", "image"];
    for entry in &models.data {
        let lower = entry.id.to_lowercase();
        if skip.iter().any(|s| lower.contains(s)) {
            continue;
        }
        let mut vars: indexmap::IndexMap<String, tempo::var::VarValue> = indexmap::IndexMap::new();
        vars.insert(
            "model".into(),
            tempo::var::VarValue::from(entry.id.as_str()),
        );
        vars.insert("prompt".into(), tempo::var::VarValue::from("probe"));
        let cell = Cell::new(
            "probe".into(),
            "litellm".into(),
            vars,
            "Hi.".into(),
            false,
            Generation {
                max_tokens: 16,
                temperature: 0.0,
            },
        );
        let run = run_request(
            &cell,
            cell.prompt_text(),
            base_url,
            api_key,
            30,
            &tempo::progress::NoopReporter,
            "probe",
        )
        .await;
        if run.error.is_none() && run.ttft_ms.is_some() {
            return Some(entry.id.clone());
        }
        eprintln!(
            "  probe {}: error={:?} ttft={:?}",
            entry.id, run.error, run.ttft_ms
        );
    }
    None
}
