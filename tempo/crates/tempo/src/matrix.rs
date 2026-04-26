use std::collections::HashSet;

use indexmap::IndexMap;
use thiserror::Error;

use crate::config::{Config, Generation, Matrix, Prompt, Scenario};
use crate::dimensions::Dimensions;
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
        generation_defaults: Generation,
    ) -> Self {
        let model = require_string_var(&vars, "model");
        let prompt = require_string_var(&vars, "prompt");
        let generation = resolve_generation(&generation_defaults, &vars);
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

    pub fn dimensions(&self) -> Dimensions {
        Dimensions {
            scenario: self.scenario.clone(),
            provider: self.provider.clone(),
            vars: self.vars.clone(),
        }
    }
}

fn resolve_generation(defaults: &Generation, vars: &IndexMap<String, VarValue>) -> Generation {
    let max_tokens = match vars.get("max_tokens") {
        Some(VarValue::Integer(i)) if *i >= 0 && *i <= u32::MAX as i64 => *i as u32,
        _ => defaults.max_tokens,
    };
    let temperature = match vars.get("temperature") {
        Some(VarValue::Float(f)) => *f as f32,
        _ => defaults.temperature,
    };
    let top_p = match vars.get("top_p") {
        Some(VarValue::Float(f)) => Some(*f as f32),
        _ => defaults.top_p,
    };
    Generation {
        max_tokens,
        temperature,
        top_p,
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
    #[error("scenario {scenario:?} references unknown prompt {prompt:?}")]
    UnknownPrompt { scenario: String, prompt: String },
    #[error("scenario {scenario:?}: matrix tuple is missing required {which:?} variable")]
    MissingConventionalVar {
        scenario: String,
        which: &'static str,
    },
    #[error("scenario {scenario:?}: matrix tuple has non-string {which:?} variable")]
    NonStringConventionalVar {
        scenario: String,
        which: &'static str,
    },
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

    let tuples = build_tuples(matrix);

    let mut cells = Vec::with_capacity(tuples.len());
    for vars in tuples {
        let prompt_name = match vars.get("prompt") {
            Some(VarValue::String(s)) => s.clone(),
            Some(_) => {
                return Err(MatrixError::NonStringConventionalVar {
                    scenario: scenario_name.to_string(),
                    which: "prompt",
                });
            }
            None => {
                return Err(MatrixError::MissingConventionalVar {
                    scenario: scenario_name.to_string(),
                    which: "prompt",
                });
            }
        };
        match vars.get("model") {
            Some(VarValue::String(_)) => {}
            Some(_) => {
                return Err(MatrixError::NonStringConventionalVar {
                    scenario: scenario_name.to_string(),
                    which: "model",
                });
            }
            None => {
                return Err(MatrixError::MissingConventionalVar {
                    scenario: scenario_name.to_string(),
                    which: "model",
                });
            }
        }
        let Prompt::Inline { text, template } =
            config
                .prompts
                .get(&prompt_name)
                .ok_or_else(|| MatrixError::UnknownPrompt {
                    scenario: scenario_name.to_string(),
                    prompt: prompt_name.clone(),
                })?;
        cells.push(Cell::new(
            scenario_name.to_string(),
            provider.clone(),
            vars,
            text.clone(),
            *template,
            generation.clone(),
        ));
    }
    Ok(cells)
}

fn build_tuples(matrix: &Matrix) -> Vec<IndexMap<String, VarValue>> {
    let cross = cross_product(&matrix.axes);
    let mut seen: HashSet<Vec<(String, VarValue)>> = HashSet::new();
    let mut out: Vec<IndexMap<String, VarValue>> = Vec::new();
    for tuple in cross.into_iter().chain(matrix.include.iter().cloned()) {
        if seen.insert(canonical_key(&tuple)) {
            out.push(tuple);
        }
    }
    out.retain(|t| !matrix.skip.iter().any(|s| matches_skip(t, s)));
    out
}

fn cross_product(axes: &IndexMap<String, Vec<VarValue>>) -> Vec<IndexMap<String, VarValue>> {
    let mut out: Vec<IndexMap<String, VarValue>> = vec![IndexMap::new()];
    for (name, values) in axes {
        let mut next = Vec::with_capacity(out.len() * values.len());
        for existing in &out {
            for v in values {
                let mut row = existing.clone();
                row.insert(name.clone(), v.clone());
                next.push(row);
            }
        }
        out = next;
    }
    out
}

fn canonical_key(m: &IndexMap<String, VarValue>) -> Vec<(String, VarValue)> {
    let mut entries: Vec<(String, VarValue)> =
        m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn matches_skip(tuple: &IndexMap<String, VarValue>, skip: &IndexMap<String, VarValue>) -> bool {
    skip.iter().all(|(k, v)| tuple.get(k) == Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(matrix_toml: &str) -> Config {
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
{matrix_toml}
"#
        );
        Config::from_toml_str(&toml).expect("fixture parses")
    }

    #[test]
    fn three_axis_cross_product_count_and_tuples() {
        let cfg = make_config(
            r#"
model = ["m1", "m2"]
prompt = ["short"]
topic = ["a", "b"]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert_eq!(cells.len(), 4);
        let triples: Vec<(&str, &str, VarValue)> = cells
            .iter()
            .map(|c| {
                (
                    c.model(),
                    c.prompt(),
                    c.vars().get("topic").cloned().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            triples,
            vec![
                ("m1", "short", VarValue::from("a")),
                ("m1", "short", VarValue::from("b")),
                ("m2", "short", VarValue::from("a")),
                ("m2", "short", VarValue::from("b")),
            ]
        );
    }

    #[test]
    fn include_adds_tuples_not_in_cross_product() {
        let cfg = make_config(
            r#"
model = ["m1"]
prompt = ["short"]
[scenarios.decode.matrix.include]
combinations = [
  { model = "m_extra", prompt = "long", max_tokens = 4096 },
]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[1].model(), "m_extra");
        assert_eq!(cells[1].prompt(), "long");
        assert_eq!(
            cells[1].vars().get("max_tokens"),
            Some(&VarValue::from(4096i64))
        );
    }

    #[test]
    fn include_duplicates_are_deduped() {
        let cfg = make_config(
            r#"
model = ["m1"]
prompt = ["short"]
[scenarios.decode.matrix.include]
combinations = [
  { model = "m1", prompt = "short" },
  { model = "m_extra", prompt = "long" },
  { model = "m_extra", prompt = "long" },
]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert_eq!(cells.len(), 2);
        let pairs: Vec<(&str, &str)> = cells.iter().map(|c| (c.model(), c.prompt())).collect();
        assert_eq!(pairs, vec![("m1", "short"), ("m_extra", "long")]);
    }

    #[test]
    fn skip_partial_pattern_removes_matching_tuples() {
        let cfg = make_config(
            r#"
model = ["m1", "m2"]
prompt = ["short", "long"]

[[scenarios.decode.matrix.skip]]
model = "m1"
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        let pairs: Vec<(&str, &str)> = cells.iter().map(|c| (c.model(), c.prompt())).collect();
        assert_eq!(pairs, vec![("m2", "short"), ("m2", "long")]);
    }

    #[test]
    fn skip_after_include_works() {
        let cfg = make_config(
            r#"
model = ["m1"]
prompt = ["short"]
[scenarios.decode.matrix.include]
combinations = [
  { model = "m_extra", prompt = "long" },
]

[[scenarios.decode.matrix.skip]]
model = "m_extra"
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        let pairs: Vec<(&str, &str)> = cells.iter().map(|c| (c.model(), c.prompt())).collect();
        assert_eq!(pairs, vec![("m1", "short")]);
    }

    fn make_config_with_generation(generation: &str, matrix_toml: &str) -> Config {
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
generation = {{ {generation} }}
[scenarios.decode.matrix]
{matrix_toml}
"#
        );
        Config::from_toml_str(&toml).expect("fixture parses")
    }

    #[test]
    fn matrix_var_overrides_max_tokens_default() {
        let cfg = make_config_with_generation(
            "max_tokens = 16, temperature = 0.0",
            r#"
model = ["m1"]
prompt = ["short"]
max_tokens = [128, 256]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0].generation().max_tokens, 128);
        assert_eq!(cells[1].generation().max_tokens, 256);
    }

    #[test]
    fn matrix_var_overrides_temperature_default() {
        let cfg = make_config_with_generation(
            "max_tokens = 16, temperature = 0.0",
            r#"
model = ["m1"]
prompt = ["short"]
temperature = [0.7]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert!((cells[0].generation().temperature - 0.7).abs() < 1e-6);
    }

    #[test]
    fn matrix_var_sets_top_p_when_default_absent() {
        let cfg = make_config_with_generation(
            "max_tokens = 16, temperature = 0.0",
            r#"
model = ["m1"]
prompt = ["short"]
top_p = [0.95]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert!((cells[0].generation().top_p.unwrap() - 0.95).abs() < 1e-6);
    }

    #[test]
    fn matrix_var_overrides_top_p_default() {
        let cfg = make_config_with_generation(
            "max_tokens = 16, temperature = 0.0, top_p = 0.5",
            r#"
model = ["m1"]
prompt = ["short"]
top_p = [0.9]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert!((cells[0].generation().top_p.unwrap() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn cell_inherits_top_p_default_when_no_override() {
        let cfg = make_config_with_generation(
            "max_tokens = 16, temperature = 0.0, top_p = 0.8",
            r#"
model = ["m1"]
prompt = ["short"]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");
        assert_eq!(cells[0].generation().top_p, Some(0.8));
    }

    #[test]
    fn expanded_cells_have_vars_matching_accessors() {
        let cfg = make_config(
            r#"
model = ["m1", "m2"]
prompt = ["short", "long"]
"#,
        );
        let scenario = cfg.scenarios.get("decode").unwrap();
        let cells = expand("decode", scenario, &cfg).expect("expand ok");

        for cell in &cells {
            let vars = cell.vars();
            assert_eq!(vars.get("model"), Some(&VarValue::from(cell.model())));
            assert_eq!(vars.get("prompt"), Some(&VarValue::from(cell.prompt())));
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
                top_p: None,
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
                top_p: None,
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
                top_p: None,
            },
        );
    }
}
