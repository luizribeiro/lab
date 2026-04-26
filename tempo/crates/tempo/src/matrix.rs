use indexmap::IndexMap;
use thiserror::Error;

use crate::config::{Config, Generation, Prompt, Scenario};
use crate::var::VarValue;

#[derive(Debug, Clone)]
pub struct Cell {
    scenario: String,
    provider: String,
    model: String,
    prompt: String,
    prompt_text: String,
    prompt_template: bool,
    generation: Generation,
    vars: IndexMap<String, VarValue>,
}

impl Cell {
    pub fn new(
        scenario: String,
        provider: String,
        vars: IndexMap<String, VarValue>,
        prompt_text: String,
        prompt_template: bool,
        generation: Generation,
    ) -> Self {
        let model = require_string_var(&vars, "model");
        let prompt = require_string_var(&vars, "prompt");
        Self {
            scenario,
            provider,
            model,
            prompt,
            prompt_text,
            prompt_template,
            generation,
            vars,
        }
    }

    pub fn scenario(&self) -> &str {
        &self.scenario
    }
    pub fn provider(&self) -> &str {
        &self.provider
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn prompt_text(&self) -> &str {
        &self.prompt_text
    }
    pub fn prompt_template(&self) -> bool {
        self.prompt_template
    }
    pub fn generation(&self) -> &Generation {
        &self.generation
    }
    pub fn vars(&self) -> &IndexMap<String, VarValue> {
        &self.vars
    }
}

fn require_string_var(vars: &IndexMap<String, VarValue>, key: &str) -> String {
    match vars.get(key) {
        Some(VarValue::String(s)) => s.clone(),
        Some(other) => {
            panic!("Cell::new: vars[{key:?}] must be VarValue::String, got {other:?}")
        }
        None => panic!("Cell::new: vars is missing required key {key:?}"),
    }
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
            let mut vars: IndexMap<String, VarValue> = IndexMap::new();
            vars.insert("model".to_string(), VarValue::from(model.as_str()));
            vars.insert("prompt".to_string(), VarValue::from(prompt_name.as_str()));
            cells.push(Cell::new(
                scenario_name.to_string(),
                provider.clone(),
                vars,
                text.clone(),
                *template,
                generation.clone(),
            ));
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
            .map(|c| (c.model(), c.prompt(), c.prompt_text()))
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
            assert_eq!(cell.scenario(), "decode");
            assert_eq!(cell.provider(), "vllm");
            assert_eq!(cell.generation().max_tokens, 16);
        }
    }

    #[test]
    fn expanded_cells_have_vars_matching_accessors() {
        let cfg = config_with(&["m1", "m2"], &["short", "long"]);
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");

        for cell in &cells {
            let vars = cell.vars();
            assert_eq!(
                vars.get("model"),
                Some(&VarValue::from(cell.model())),
                "vars.model must match cell.model() for cell {:?}",
                (cell.model(), cell.prompt())
            );
            assert_eq!(
                vars.get("prompt"),
                Some(&VarValue::from(cell.prompt())),
                "vars.prompt must match cell.prompt() for cell {:?}",
                (cell.model(), cell.prompt())
            );
        }
    }

    #[test]
    #[should_panic(expected = "vars is missing required key \"model\"")]
    fn cell_new_panics_without_model_var() {
        let mut vars: IndexMap<String, VarValue> = IndexMap::new();
        vars.insert("prompt".into(), VarValue::from("short"));
        let _ = Cell::new(
            "decode".into(),
            "vllm".into(),
            vars,
            "text".into(),
            false,
            Generation {
                max_tokens: 16,
                temperature: 0.0,
            },
        );
    }

    #[test]
    #[should_panic(expected = "vars is missing required key \"prompt\"")]
    fn cell_new_panics_without_prompt_var() {
        let mut vars: IndexMap<String, VarValue> = IndexMap::new();
        vars.insert("model".into(), VarValue::from("m1"));
        let _ = Cell::new(
            "decode".into(),
            "vllm".into(),
            vars,
            "text".into(),
            false,
            Generation {
                max_tokens: 16,
                temperature: 0.0,
            },
        );
    }

    #[test]
    #[should_panic(expected = "must be VarValue::String")]
    fn cell_new_panics_when_model_is_not_string() {
        let mut vars: IndexMap<String, VarValue> = IndexMap::new();
        vars.insert("model".into(), VarValue::from(42i64));
        vars.insert("prompt".into(), VarValue::from("short"));
        let _ = Cell::new(
            "decode".into(),
            "vllm".into(),
            vars,
            "text".into(),
            false,
            Generation {
                max_tokens: 16,
                temperature: 0.0,
            },
        );
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
