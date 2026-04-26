use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Deserialize;
use thiserror::Error;

use crate::template::{self, TemplateError};
use crate::var::VarValue;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub suite: Suite,
    pub providers: BTreeMap<String, Provider>,
    pub prompts: BTreeMap<String, Prompt>,
    pub scenarios: BTreeMap<String, Scenario>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provider {
    OpenaiCompatible {
        base_url: String,
        api_key_env: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Prompt {
    Inline {
        text: String,
        #[serde(default = "default_template")]
        template: bool,
    },
}

fn default_template() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scenario {
    Throughput {
        provider: String,
        warmup: u32,
        runs: u32,
        #[serde(default = "default_timeout_secs")]
        timeout_secs: u64,
        generation: Generation,
        matrix: Matrix,
    },
}

fn default_timeout_secs() -> u64 {
    120
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    pub max_tokens: u32,
    pub temperature: f32,
    #[serde(default)]
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct Matrix {
    pub axes: IndexMap<String, Vec<VarValue>>,
    pub include: Vec<IndexMap<String, VarValue>>,
    pub skip: Vec<IndexMap<String, VarValue>>,
}

pub const RESERVED_AXIS_NAMES: &[&str] = &[
    "provider",
    "scenario",
    "run_id",
    "cell_id",
    "vars",
    "generation",
];

impl<'de> Deserialize<'de> for Matrix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let raw: IndexMap<String, toml::Value> = IndexMap::deserialize(deserializer)?;
        let mut axes: IndexMap<String, Vec<VarValue>> = IndexMap::new();
        let mut include: Vec<IndexMap<String, VarValue>> = Vec::new();
        let mut skip: Vec<IndexMap<String, VarValue>> = Vec::new();
        for (key, value) in raw {
            match key.as_str() {
                "include" => {
                    include = parse_include(value).map_err(D::Error::custom)?;
                }
                "skip" => {
                    skip = parse_skip(value).map_err(D::Error::custom)?;
                }
                _ => {
                    let arr = match value {
                        toml::Value::Array(a) => a,
                        other => {
                            return Err(D::Error::custom(format!(
                                "matrix axis {key:?} must be an array, got {}",
                                toml_type_name(&other)
                            )));
                        }
                    };
                    let values: Vec<VarValue> = arr
                        .into_iter()
                        .map(|v| {
                            VarValue::try_from(v)
                                .map_err(|e| D::Error::custom(format!("matrix axis {key:?}: {e}")))
                        })
                        .collect::<Result<_, _>>()?;
                    axes.insert(key, values);
                }
            }
        }
        Ok(Matrix {
            axes,
            include,
            skip,
        })
    }
}

fn parse_include(v: toml::Value) -> Result<Vec<IndexMap<String, VarValue>>, String> {
    let table = match v {
        toml::Value::Table(t) => t,
        other => {
            return Err(format!(
                "matrix.include must be a table with `combinations`, got {}",
                toml_type_name(&other)
            ));
        }
    };
    let mut combos: Option<toml::Value> = None;
    for (k, v) in table {
        match k.as_str() {
            "combinations" => combos = Some(v),
            other => return Err(format!("matrix.include: unknown key {other:?}")),
        }
    }
    let arr = match combos {
        Some(toml::Value::Array(a)) => a,
        Some(other) => {
            return Err(format!(
                "matrix.include.combinations must be an array, got {}",
                toml_type_name(&other)
            ));
        }
        None => return Err("matrix.include requires `combinations`".to_string()),
    };
    arr.into_iter().map(parse_var_map).collect()
}

fn parse_skip(v: toml::Value) -> Result<Vec<IndexMap<String, VarValue>>, String> {
    let arr = match v {
        toml::Value::Array(a) => a,
        other => {
            return Err(format!(
                "matrix.skip must be an array of tables, got {}",
                toml_type_name(&other)
            ));
        }
    };
    arr.into_iter().map(parse_var_map).collect()
}

fn parse_var_map(v: toml::Value) -> Result<IndexMap<String, VarValue>, String> {
    let table = match v {
        toml::Value::Table(t) => t,
        other => {
            return Err(format!(
                "expected a table of var values, got {}",
                toml_type_name(&other)
            ));
        }
    };
    let mut out = IndexMap::new();
    for (k, val) in table {
        let vv = VarValue::try_from(val).map_err(|e| format!("var {k:?}: {e}"))?;
        out.insert(k, vv);
    }
    Ok(out)
}

fn toml_type_name(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "table",
    }
}

fn var_type_name(v: &VarValue) -> &'static str {
    match v {
        VarValue::Bool(_) => "boolean",
        VarValue::Integer(_) => "integer",
        VarValue::Float(_) => "float",
        VarValue::String(_) => "string",
    }
}

impl Matrix {
    fn validate(&self, scenario: &str) -> Result<(), ConfigError> {
        if self.axes.is_empty() && self.include.is_empty() {
            return Err(ConfigError::EmptyMatrix {
                scenario: scenario.to_string(),
            });
        }
        for axis in self.axes.keys() {
            check_reserved_name(scenario, axis)?;
        }
        for entry in self.include.iter().chain(self.skip.iter()) {
            for k in entry.keys() {
                check_reserved_name(scenario, k)?;
            }
        }
        for (axis, values) in &self.axes {
            if values.is_empty() {
                return Err(ConfigError::EmptyAxis {
                    scenario: scenario.to_string(),
                    axis: axis.clone(),
                });
            }
            for v in values {
                check_conventional_type(scenario, axis, v)?;
            }
        }
        for entry in self.include.iter().chain(self.skip.iter()) {
            for (axis, value) in entry {
                check_conventional_type(scenario, axis, value)?;
            }
        }
        Ok(())
    }

    /// Iterate every prompt name that may produce a cell — axes and
    /// include entries. Skip entries are not validated against the prompt
    /// registry: a skip pattern referencing a non-existent prompt simply
    /// matches no tuples.
    pub fn prompt_refs(&self) -> impl Iterator<Item = &str> {
        let from_axes = self
            .axes
            .get("prompt")
            .into_iter()
            .flat_map(|values| values.iter())
            .filter_map(string_value);
        let from_include = self
            .include
            .iter()
            .filter_map(|m| m.get("prompt"))
            .filter_map(string_value);
        from_axes.chain(from_include)
    }
}

fn string_value(v: &VarValue) -> Option<&str> {
    match v {
        VarValue::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn check_reserved_name(scenario: &str, name: &str) -> Result<(), ConfigError> {
    if RESERVED_AXIS_NAMES.contains(&name) {
        return Err(ConfigError::ReservedAxisName {
            scenario: scenario.to_string(),
            name: name.to_string(),
        });
    }
    Ok(())
}

fn check_conventional_type(scenario: &str, axis: &str, v: &VarValue) -> Result<(), ConfigError> {
    let (expected, ok) = match axis {
        "model" | "prompt" => ("string", matches!(v, VarValue::String(_))),
        "max_tokens" => (
            "integer in [0, u32::MAX]",
            matches!(v, VarValue::Integer(i) if *i >= 0 && *i <= u32::MAX as i64),
        ),
        "temperature" | "top_p" => ("float", matches!(v, VarValue::Float(_))),
        _ => return Ok(()),
    };
    if ok {
        Ok(())
    } else {
        Err(ConfigError::InvalidAxisType {
            scenario: scenario.to_string(),
            axis: axis.to_string(),
            expected: expected.to_string(),
            got: var_type_name(v).to_string(),
        })
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("scenario {scenario:?} references unknown provider {provider:?}")]
    UnknownProvider { scenario: String, provider: String },
    #[error("scenario {scenario:?} references unknown prompt {prompt:?}")]
    UnknownPrompt { scenario: String, prompt: String },
    #[error("prompt {prompt:?} has invalid template: {source}")]
    InvalidTemplate {
        prompt: String,
        #[source]
        source: TemplateError,
    },
    #[error("scenario {scenario:?}: {name:?} is a reserved name and cannot be used as a matrix axis or include/skip key")]
    ReservedAxisName { scenario: String, name: String },
    #[error("scenario {scenario:?}: matrix axis {axis:?} must be {expected}, got {got}")]
    InvalidAxisType {
        scenario: String,
        axis: String,
        expected: String,
        got: String,
    },
    #[error("scenario {scenario:?}: matrix axis {axis:?} is empty")]
    EmptyAxis { scenario: String, axis: String },
    #[error("scenario {scenario:?}: matrix has no axes and no include entries")]
    EmptyMatrix { scenario: String },
}

impl Config {
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        let cfg: Config = toml::from_str(s)?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        for (name, scenario) in &self.scenarios {
            let Scenario::Throughput {
                provider, matrix, ..
            } = scenario;
            if !self.providers.contains_key(provider) {
                return Err(ConfigError::UnknownProvider {
                    scenario: name.clone(),
                    provider: provider.clone(),
                });
            }
            matrix.validate(name)?;
            for prompt in matrix.prompt_refs() {
                if !self.prompts.contains_key(prompt) {
                    return Err(ConfigError::UnknownPrompt {
                        scenario: name.clone(),
                        prompt: prompt.to_string(),
                    });
                }
            }
        }
        for (name, prompt) in &self.prompts {
            let Prompt::Inline { text, template } = prompt;
            if *template {
                template::validate(text, &["run_id", "cell_id"]).map_err(|source| {
                    ConfigError::InvalidTemplate {
                        prompt: name.clone(),
                        source,
                    }
                })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../examples/vllm-smoke.toml");

    #[test]
    fn parses_canonical_fixture() {
        let cfg = Config::from_toml_str(FIXTURE).expect("fixture should parse");
        assert_eq!(cfg.suite.name, "vllm-smoke");
        assert!(cfg.providers.contains_key("vllm"));
        assert!(cfg.prompts.contains_key("short"));
        assert!(cfg.prompts.contains_key("long"));

        let Scenario::Throughput {
            provider,
            warmup,
            runs,
            timeout_secs,
            generation,
            matrix,
        } = cfg.scenarios.get("decode").expect("decode scenario");
        assert_eq!(provider, "vllm");
        assert_eq!(*warmup, 1);
        assert_eq!(*runs, 5);
        assert_eq!(*timeout_secs, 120);
        assert_eq!(generation.max_tokens, 256);
        assert_eq!(generation.top_p, None);
        assert_eq!(matrix.axes.get("model").unwrap().len(), 2);
        let prompt_axis: Vec<&str> = matrix
            .axes
            .get("prompt")
            .unwrap()
            .iter()
            .filter_map(|v| match v {
                VarValue::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(prompt_axis, vec!["short", "long"]);

        let Provider::OpenaiCompatible {
            base_url,
            api_key_env,
        } = cfg.providers.get("vllm").unwrap();
        assert_eq!(base_url, "http://litellm.internal/v1");
        assert_eq!(api_key_env, "LITELLM_KEY");
    }

    #[test]
    fn parses_generation_with_top_p() {
        let toml = r#"
[suite]
name = "x"

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hi"

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0, top_p = 0.9 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["s"]
"#;
        let cfg = Config::from_toml_str(toml).expect("parses");
        let Scenario::Throughput { generation, .. } = cfg.scenarios.get("t").unwrap();
        assert_eq!(generation.top_p, Some(0.9));
    }

    #[test]
    fn rejects_unknown_top_level_key() {
        let toml = r#"
[suite]
name = "x"
mystery = true

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hi"

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["s"]
"#;
        let err = Config::from_toml_str(toml).unwrap_err();
        assert!(matches!(err, ConfigError::Toml(_)), "got {err:?}");
    }

    #[test]
    fn rejects_unknown_provider_kind() {
        let toml = r#"
[suite]
name = "x"

[providers.p]
kind = "magic"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hi"

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["s"]
"#;
        let err = Config::from_toml_str(toml).unwrap_err();
        assert!(matches!(err, ConfigError::Toml(_)), "got {err:?}");
    }

    #[test]
    fn rejects_scenario_with_undefined_prompt() {
        let toml = r#"
[suite]
name = "x"

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hi"

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["ghost"]
"#;
        let err = Config::from_toml_str(toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::UnknownPrompt { ref scenario, ref prompt } if scenario == "t" && prompt == "ghost"),
            "got {err:?}",
        );
    }

    #[test]
    fn rejects_prompt_with_unknown_template_var() {
        let toml = r#"
[suite]
name = "x"

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hello {bogus}"

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["s"]
"#;
        let err = Config::from_toml_str(toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidTemplate { ref prompt, .. } if prompt == "s"),
            "got {err:?}",
        );
    }

    #[test]
    fn template_false_skips_validation() {
        let toml = r#"
[suite]
name = "x"

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hello {bogus}"
template = false

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["s"]
"#;
        let cfg = Config::from_toml_str(toml).expect("parses with template=false");
        let Prompt::Inline { text, template } = cfg.prompts.get("s").unwrap();
        assert_eq!(text, "hello {bogus}");
        assert!(!template);
    }

    #[test]
    fn rejects_scenario_with_undefined_provider() {
        let toml = r#"
[suite]
name = "x"

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hi"

[scenarios.t]
kind = "throughput"
provider = "ghost"
warmup = 0
runs = 1
generation = { max_tokens = 1, temperature = 0.0 }
[scenarios.t.matrix]
model = ["m"]
prompt = ["s"]
"#;
        let err = Config::from_toml_str(toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::UnknownProvider { ref scenario, ref provider } if scenario == "t" && provider == "ghost"),
            "got {err:?}",
        );
    }

    fn matrix_toml(matrix_body: &str) -> String {
        format!(
            r#"
[suite]
name = "x"

[providers.p]
kind = "openai_compatible"
base_url = "http://x"
api_key_env = "K"

[prompts.s]
kind = "inline"
text = "hi"

[prompts.l]
kind = "inline"
text = "long"

[scenarios.t]
kind = "throughput"
provider = "p"
warmup = 0
runs = 1
generation = {{ max_tokens = 1, temperature = 0.0 }}
[scenarios.t.matrix]
{matrix_body}
"#
        )
    }

    #[test]
    fn reserved_axis_name_rejected() {
        let toml = matrix_toml(
            r#"
model = ["m"]
prompt = ["s"]
provider = ["x"]
"#,
        );
        let err = Config::from_toml_str(&toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::ReservedAxisName { ref name, .. } if name == "provider"),
            "got {err:?}"
        );
    }

    #[test]
    fn invalid_axis_type_for_max_tokens_rejected() {
        let toml = matrix_toml(
            r#"
model = ["m"]
prompt = ["s"]
max_tokens = ["foo"]
"#,
        );
        let err = Config::from_toml_str(&toml).unwrap_err();
        assert!(
            matches!(
                err,
                ConfigError::InvalidAxisType { ref axis, ref got, .. }
                    if axis == "max_tokens" && got == "string"
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_axis_rejected() {
        let toml = matrix_toml(
            r#"
model = []
prompt = ["s"]
"#,
        );
        let err = Config::from_toml_str(&toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::EmptyAxis { ref axis, .. } if axis == "model"),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_matrix_rejected() {
        let toml = matrix_toml("");
        let err = Config::from_toml_str(&toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::EmptyMatrix { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn parses_include_and_skip() {
        let toml = matrix_toml(
            r#"
model = ["m1"]
prompt = ["s"]

[scenarios.t.matrix.include]
combinations = [
  { model = "m_extra", prompt = "l", max_tokens = 4096 },
]

[[scenarios.t.matrix.skip]]
model = "m1"
prompt = "s"
"#,
        );
        let cfg = Config::from_toml_str(&toml).expect("parses");
        let Scenario::Throughput { matrix, .. } = cfg.scenarios.get("t").unwrap();
        assert_eq!(matrix.axes.len(), 2);
        assert_eq!(matrix.include.len(), 1);
        assert_eq!(
            matrix.include[0].get("max_tokens"),
            Some(&VarValue::from(4096i64))
        );
        assert_eq!(matrix.skip.len(), 1);
        assert_eq!(matrix.skip[0].get("model"), Some(&VarValue::from("m1")));
    }

    #[test]
    fn reserved_name_in_skip_rejected() {
        let toml = matrix_toml(
            r#"
model = ["m"]
prompt = ["s"]

[[scenarios.t.matrix.skip]]
provider = "x"
"#,
        );
        let err = Config::from_toml_str(&toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::ReservedAxisName { ref name, .. } if name == "provider"),
            "got {err:?}"
        );
    }

    #[test]
    fn skip_may_reference_unknown_prompt() {
        let toml = matrix_toml(
            r#"
model = ["m1"]
prompt = ["s"]

[[scenarios.t.matrix.skip]]
prompt = "ghost"
"#,
        );
        Config::from_toml_str(&toml).expect("skip with unknown prompt is allowed");
    }

    #[test]
    fn unknown_prompt_in_include_rejected() {
        let toml = matrix_toml(
            r#"
model = ["m1"]
prompt = ["s"]

[scenarios.t.matrix.include]
combinations = [
  { model = "m_extra", prompt = "ghost" },
]
"#,
        );
        let err = Config::from_toml_str(&toml).unwrap_err();
        assert!(
            matches!(err, ConfigError::UnknownPrompt { ref prompt, .. } if prompt == "ghost"),
            "got {err:?}"
        );
    }
}
