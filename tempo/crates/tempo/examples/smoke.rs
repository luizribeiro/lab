use std::env;

use serde::Deserialize;
use tempo::config::Generation;
use tempo::matrix::Cell;
use tempo::provider::openai::run_request;

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
    let base_url = env::var("LITELLM_BASE_URL")
        .unwrap_or_else(|_| "https://litellm.thepromisedlan.club/v1".to_string());
    let api_key = env::var("LITELLM_API_KEY")?;

    let client = reqwest::Client::new();
    let models: ModelsResponse = client
        .get(format!("{}/models", base_url.trim_end_matches('/')))
        .bearer_auth(&api_key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let candidates = chat_model_candidates(&models);
    if candidates.is_empty() {
        return Err("no chat-capable model found".into());
    }

    for model in &candidates {
        eprintln!("trying model: {model}");
        let cell = Cell {
            scenario: "smoke".into(),
            provider: "litellm".into(),
            model: model.clone(),
            prompt: "haiku".into(),
            prompt_text: "Write a haiku about caching.".into(),
            prompt_template: false,
            generation: Generation {
                max_tokens: 64,
                temperature: 0.0,
            },
        };
        let run = run_request(&cell, &cell.prompt_text, &base_url, &api_key, 60).await;
        let is_upstream_5xx = run
            .error
            .as_deref()
            .is_some_and(|e| e.starts_with("http_5"));
        if is_upstream_5xx {
            eprintln!("  upstream {} — trying next", run.error.as_deref().unwrap());
            continue;
        }
        if run.error.is_none() && run.ttft_ms.is_none() {
            eprintln!("  no content events (likely reasoning-only model) — trying next");
            continue;
        }
        println!("{}", serde_json::to_string_pretty(&run)?);
        return Ok(());
    }
    Err("all candidate models failed with upstream 5xx".into())
}

fn chat_model_candidates(models: &ModelsResponse) -> Vec<String> {
    let skip = ["embed", "embedding", "whisper", "tts", "rerank", "image"];
    models
        .data
        .iter()
        .filter(|m| {
            let lower = m.id.to_lowercase();
            !skip.iter().any(|s| lower.contains(s))
        })
        .map(|m| m.id.clone())
        .collect()
}
