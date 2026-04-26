use thiserror::Error;

use crate::config::{Config, Generation, Prompt, Scenario};

#[derive(Debug, Clone)]
pub struct Cell {
    pub scenario: String,
    pub provider: String,
    pub model: String,
    pub prompt: String,
    pub prompt_text: String,
    pub prompt_template: bool,
    pub generation: Generation,
}

#[derive(Debug, Error)]
pub enum MatrixError {
    #[error("scenario {0:?}: matrix.model is empty")]
    EmptyModelAxis(String),
    #[error("scenario {0:?}: matrix.prompt is empty")]
    EmptyPromptAxis(String),
    #[error("scenario {scenario:?} references unknown prompt {prompt:?}")]
    UnknownPrompt { scenario: String, prompt: String },
}

pub fn expand(
    scenario_name: &str,
    scenario: &Scenario,
    config: &Config,
) -> Result<Vec<Cell>, MatrixError> {
    let Scenario::Throughput {
        provider,
        generation,
        matrix,
        ..
    } = scenario;

    if matrix.model.is_empty() {
        return Err(MatrixError::EmptyModelAxis(scenario_name.to_string()));
    }
    if matrix.prompt.is_empty() {
        return Err(MatrixError::EmptyPromptAxis(scenario_name.to_string()));
    }

    let mut cells = Vec::with_capacity(matrix.model.len() * matrix.prompt.len());
    for model in &matrix.model {
        for prompt_name in &matrix.prompt {
            let Prompt::Inline { text, template } =
                config
                    .prompts
                    .get(prompt_name)
                    .ok_or_else(|| MatrixError::UnknownPrompt {
                        scenario: scenario_name.to_string(),
                        prompt: prompt_name.clone(),
                    })?;
            cells.push(Cell {
                scenario: scenario_name.to_string(),
                provider: provider.clone(),
                model: model.clone(),
                prompt: prompt_name.clone(),
                prompt_text: text.clone(),
                prompt_template: *template,
                generation: generation.clone(),
            });
        }
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(model: &[&str], prompt: &[&str]) -> Config {
        let quote = |items: &[&str]| {
            items
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let model_list = quote(model);
        let prompt_list = quote(prompt);
        let toml = format!(
            r#"
[suite]
name = "t"

[providers.vllm]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.short]
kind = "inline"
text = "short text"

[prompts.long]
kind = "inline"
text = "long text"

[scenarios.decode]
kind = "throughput"
provider = "vllm"
warmup = 0
runs = 1
generation = {{ max_tokens = 16, temperature = 0.0 }}
[scenarios.decode.matrix]
model = [{model_list}]
prompt = [{prompt_list}]
"#
        );
        Config::from_toml_str(&toml).expect("fixture parses")
    }

    #[test]
    fn expands_2x2_in_model_outer_prompt_inner_order() {
        let cfg = config_with(&["m1", "m2"], &["short", "long"]);
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");

        assert_eq!(cells.len(), 4);

        let pairs: Vec<(&str, &str, &str)> = cells
            .iter()
            .map(|c| (c.model.as_str(), c.prompt.as_str(), c.prompt_text.as_str()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("m1", "short", "short text"),
                ("m1", "long", "long text"),
                ("m2", "short", "short text"),
                ("m2", "long", "long text"),
            ]
        );

        for cell in &cells {
            assert_eq!(cell.scenario, "decode");
            assert_eq!(cell.provider, "vllm");
            assert_eq!(cell.generation.max_tokens, 16);
        }
    }

    #[test]
    fn empty_model_axis_returns_error() {
        let cfg = config_with(&[], &["short"]);
        let scenario = cfg.scenarios.get("decode").unwrap();
        let err = expand("decode", scenario, &cfg).unwrap_err();
        assert!(
            matches!(err, MatrixError::EmptyModelAxis(ref s) if s == "decode"),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_prompt_axis_returns_error() {
        let cfg = config_with(&["m1"], &[]);
        let scenario = cfg.scenarios.get("decode").unwrap();
        let err = expand("decode", scenario, &cfg).unwrap_err();
        assert!(
            matches!(err, MatrixError::EmptyPromptAxis(ref s) if s == "decode"),
            "got {err:?}"
        );
    }
}
