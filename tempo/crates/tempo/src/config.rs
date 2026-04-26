use std::collections::BTreeMap;

use serde::Deserialize;
use thiserror::Error;

use crate::template::{self, TemplateError};

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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Matrix {
    pub model: Vec<String>,
    pub prompt: Vec<String>,
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
            for prompt in &matrix.prompt {
                if !self.prompts.contains_key(prompt) {
                    return Err(ConfigError::UnknownPrompt {
                        scenario: name.clone(),
                        prompt: prompt.clone(),
                    });
                }
            }
        }
        for (name, prompt) in &self.prompts {
            let Prompt::Inline { text, template } = prompt;
            if *template {
                template::validate(text).map_err(|source| ConfigError::InvalidTemplate {
                    prompt: name.clone(),
                    source,
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
        assert_eq!(matrix.model.len(), 2);
        assert_eq!(matrix.prompt, vec!["short", "long"]);

        let Provider::OpenaiCompatible {
            base_url,
            api_key_env,
        } = cfg.providers.get("vllm").unwrap();
        assert_eq!(base_url, "http://litellm.internal/v1");
        assert_eq!(api_key_env, "LITELLM_KEY");
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
}
