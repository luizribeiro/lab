use std::env;

use thiserror::Error;

use crate::config::{Config, Provider, Scenario};
use crate::matrix::{self, MatrixError};
use crate::provider::metrics::Run;
use crate::provider::openai::run_request;

#[derive(Debug)]
pub struct RunnerOutput {
    pub runs: Vec<Run>,
    pub zero_success_cells: usize,
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("environment variable {0:?} is not set")]
    MissingApiKey(String),
    #[error(transparent)]
    Matrix(#[from] MatrixError),
}

pub async fn run_all(config: &Config) -> Result<RunnerOutput, RunnerError> {
    let suite = config.suite.name.clone();
    let mut runs: Vec<Run> = Vec::new();
    let mut zero_success_cells: usize = 0;

    for (scenario_name, scenario) in &config.scenarios {
        let Scenario::Throughput {
            provider: provider_name,
            warmup,
            runs: runs_count,
            timeout_secs,
            ..
        } = scenario;

        let provider = config
            .providers
            .get(provider_name)
            .expect("Config::validate guarantees scenario.provider exists");
        let Provider::OpenaiCompatible {
            base_url,
            api_key_env,
        } = provider;
        let api_key =
            env::var(api_key_env).map_err(|_| RunnerError::MissingApiKey(api_key_env.clone()))?;

        let cells = matrix::expand(scenario_name, scenario, config)?;
        for cell in &cells {
            for _ in 0..*warmup {
                let _ = run_request(cell, base_url, &api_key, *timeout_secs).await;
            }

            let mut successes = 0u32;
            for run_idx in 0..*runs_count {
                let mut run = run_request(cell, base_url, &api_key, *timeout_secs).await;
                run.suite = suite.clone();
                run.run_idx = run_idx;
                if run.error.is_none() {
                    successes += 1;
                }
                runs.push(run);
            }
            if successes == 0 {
                zero_success_cells += 1;
            }
        }
    }

    Ok(RunnerOutput {
        runs,
        zero_success_cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn happy_sse_body() -> String {
        let frames = [
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{"content":"!"},"finish_reason":null}]}"#,
            r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#,
            "[DONE]",
        ];
        frames
            .iter()
            .map(|f| format!("data: {f}\n\n"))
            .collect::<String>()
    }

    const TEST_KEY_ENV: &str = "TEMPO_RUNNER_TEST_KEY";

    fn config_for(base_url: &str, warmup: u32, runs: u32) -> Config {
        config_with_key(base_url, warmup, runs, TEST_KEY_ENV)
    }

    fn config_with_key(base_url: &str, warmup: u32, runs: u32, api_key_env: &str) -> Config {
        let toml = format!(
            r#"
[suite]
name = "test-suite"

[providers.p]
kind = "openai_compatible"
base_url = "{base_url}"
api_key_env = "{api_key_env}"

[prompts.s]
kind = "inline"
text = "hi"

[scenarios.decode]
kind = "throughput"
provider = "p"
warmup = {warmup}
runs = {runs}
timeout_secs = 5
generation = {{ max_tokens = 4, temperature = 0.0 }}
[scenarios.decode.matrix]
model = ["m1"]
prompt = ["s"]
"#
        );
        Config::from_toml_str(&toml).expect("config parses")
    }

    fn set_test_key() {
        // unsafe required: env::set_var is not thread-safe (Rust 2024 edition).
        unsafe {
            env::set_var(TEST_KEY_ENV, "test-key");
        }
    }

    #[tokio::test]
    async fn happy_path_records_all_runs() {
        set_test_key();
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

        let cfg = config_for(&server.uri(), 0, 3);
        let out = run_all(&cfg).await.expect("runner ok");

        assert_eq!(out.runs.len(), 3);
        assert_eq!(out.zero_success_cells, 0);
        for (i, run) in out.runs.iter().enumerate() {
            assert!(run.error.is_none(), "run {i} error: {:?}", run.error);
            assert_eq!(run.run_idx, i as u32);
            assert_eq!(run.suite, "test-suite");
        }
    }

    #[tokio::test]
    async fn all_500s_records_errors_and_zero_success_cell() {
        set_test_key();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let cfg = config_for(&server.uri(), 0, 2);
        let out = run_all(&cfg).await.expect("runner ok");

        assert_eq!(out.runs.len(), 2);
        assert_eq!(out.zero_success_cells, 1);
        for run in &out.runs {
            assert_eq!(run.error.as_deref(), Some("http_500"));
        }
    }

    #[tokio::test]
    async fn first_run_failure_still_counts_as_success_cell() {
        set_test_key();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(happy_sse_body()),
            )
            .mount(&server)
            .await;

        let cfg = config_for(&server.uri(), 0, 3);
        let out = run_all(&cfg).await.expect("runner ok");

        assert_eq!(out.runs.len(), 3);
        assert_eq!(out.zero_success_cells, 0);
        let errors: Vec<_> = out.runs.iter().map(|r| r.error.clone()).collect();
        assert_eq!(errors[0].as_deref(), Some("http_500"));
        assert!(errors[1].is_none());
        assert!(errors[2].is_none());
    }

    #[tokio::test]
    async fn warmup_runs_are_discarded() {
        set_test_key();
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

        let cfg = config_for(&server.uri(), 2, 3);
        let out = run_all(&cfg).await.expect("runner ok");

        assert_eq!(out.runs.len(), 3, "warmups must not appear in output");
        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 5, "2 warmup + 3 measured = 5 total HTTP");
    }

    #[tokio::test]
    async fn missing_api_key_env_returns_setup_error() {
        const MISSING_KEY: &str = "TEMPO_RUNNER_MISSING_KEY";
        // unsafe required: env::remove_var is not thread-safe (Rust 2024 edition).
        unsafe {
            env::remove_var(MISSING_KEY);
        }
        let cfg = config_with_key("http://unused.example", 0, 1, MISSING_KEY);
        let err = run_all(&cfg).await.unwrap_err();
        assert!(
            matches!(err, RunnerError::MissingApiKey(ref k) if k == MISSING_KEY),
            "got {err:?}"
        );
    }
}
