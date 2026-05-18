//! Pi (Inflection) driver.

use crate::driver::{
    AgentPaths, Auth, CommandSpec, Driver, ReasoningLevel, TurnInput, TurnOptions,
};
use crate::{Event, ParseError};
use secrecy::ExposeSecret;
use std::path::PathBuf;
use uuid::Uuid;

/// Configuration for the pi driver.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct PiConfig {
    pub binary: Option<PathBuf>,
    pub auth: Auth,
    /// Pi provider (github-copilot, openai-codex, anthropic, google, etc.).
    /// Pi's default provider depends on its own config and may require
    /// out-of-band authentication. Set explicitly for headless reliability.
    pub provider: Option<String>,
    pub default_model: Option<String>,
    pub extra_env: Vec<(String, String)>,
    pub paths: AgentPaths,
    pub state: PiPilotState,
}

#[non_exhaustive]
#[derive(Default, Debug, Clone)]
pub struct PiPilotState {
    /// Root directory under which per-session storage dirs are created.
    /// Pilot derives a unique subdirectory per session UUID. Default:
    /// `$HOME/.pilot/pi-sessions`.
    pub session_root: Option<PathBuf>,
}

#[non_exhaustive]
#[derive(Default, Debug, Clone)]
pub struct Pi {
    pub config: PiConfig,
}

impl Pi {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_config(config: PiConfig) -> Self {
        Self { config }
    }

    fn session_dir_for(&self, session_id: Uuid) -> PathBuf {
        let root = self.config.state.session_root.clone().unwrap_or_else(|| {
            let home = std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/tmp"));
            home.join(".pilot").join("pi-sessions")
        });
        root.join(session_id.to_string())
    }

    fn base_args(&self, session_id: Uuid, opts: &TurnOptions) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "-p".into(),
            "--mode".into(),
            "json".into(),
            "--session-dir".into(),
            self.session_dir_for(session_id).to_string_lossy().into(),
        ];
        if let Some(provider) = self.config.provider.as_ref() {
            args.push("--provider".into());
            args.push(provider.clone());
        }
        if let Some(model) = opts.model.as_ref().or(self.config.default_model.as_ref()) {
            args.push("--model".into());
            args.push(model.clone());
        }
        if let Some(level) = opts.reasoning {
            let s = match level {
                ReasoningLevel::Low => "low",
                ReasoningLevel::Medium => "medium",
                ReasoningLevel::High => "high",
            };
            args.push("--thinking".into());
            args.push(s.into());
        }
        args
    }

    fn env_for(&self, opts: &TurnOptions) -> Vec<(String, String)> {
        let mut env = self.config.extra_env.clone();
        env.extend(opts.env.iter().cloned());
        if let Auth::ApiKey(secret) = &self.config.auth {
            // Pi reads provider-specific env vars; PI_API_KEY is the most
            // common one. Drivers/configs needing distinct vars can use
            // PiConfig::extra_env to add their own.
            env.push(("PI_API_KEY".into(), secret.expose_secret().to_string()));
        }
        if let Some(home) = &self.config.paths.config_home {
            env.push((
                "PI_CODING_AGENT_DIR".into(),
                home.to_string_lossy().into_owned(),
            ));
        }
        env
    }
}

impl Driver for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        #[allow(unreachable_patterns)]
        let prompt = match input {
            TurnInput::Text(s) => s.as_str(),
            _ => {
                return Err(crate::Error::UnsupportedOption {
                    driver: self.name(),
                    option: "non-text TurnInput",
                });
            }
        };
        let program = self
            .config
            .binary
            .clone()
            .unwrap_or_else(|| PathBuf::from("pi"));
        let mut args = self.base_args(session_id, opts);
        args.extend(opts.extra_args.iter().cloned());
        args.push(prompt.to_string());
        Ok(CommandSpec {
            program,
            args,
            env: self.env_for(opts),
        })
    }

    fn resume_command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        #[allow(unreachable_patterns)]
        let prompt = match input {
            TurnInput::Text(s) => s.as_str(),
            _ => {
                return Err(crate::Error::UnsupportedOption {
                    driver: self.name(),
                    option: "non-text TurnInput",
                });
            }
        };
        let program = self
            .config
            .binary
            .clone()
            .unwrap_or_else(|| PathBuf::from("pi"));
        let mut args = self.base_args(session_id, opts);
        args.push("--continue".into());
        args.extend(opts.extra_args.iter().cloned());
        args.push(prompt.to_string());
        Ok(CommandSpec {
            program,
            args,
            env: self.env_for(opts),
        })
    }

    fn parse(&self, value: serde_json::Value) -> Result<Vec<Event>, ParseError> {
        let event_type = value.get("type").and_then(|v| v.as_str());
        match event_type {
            Some("message_update") => {
                let inner = value
                    .get("assistantMessageEvent")
                    .ok_or(ParseError::MissingField("assistantMessageEvent"))?;
                let kind = inner.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if kind == "text_delta" {
                    let delta = inner
                        .get("delta")
                        .and_then(|v| v.as_str())
                        .ok_or(ParseError::MissingField("delta"))?;
                    Ok(vec![Event::AssistantText {
                        delta: delta.to_string(),
                    }])
                } else {
                    Ok(vec![Event::Raw {
                        driver: "pi",
                        value,
                    }])
                }
            }
            Some("message_end") => {
                let role = value
                    .get("message")
                    .and_then(|m| m.get("role"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                if role == "assistant" {
                    if let Some(usage) = value.get("message").and_then(|m| m.get("usage")) {
                        let input = usage.get("input").and_then(|v| v.as_u64()).ok_or(
                            ParseError::InvalidFieldType {
                                field: "message.usage.input",
                                expected: "u64",
                            },
                        )?;
                        let output = usage.get("output").and_then(|v| v.as_u64()).ok_or(
                            ParseError::InvalidFieldType {
                                field: "message.usage.output",
                                expected: "u64",
                            },
                        )?;
                        return Ok(vec![Event::Usage {
                            input_tokens: input,
                            output_tokens: output,
                        }]);
                    }
                }
                Ok(vec![Event::Raw {
                    driver: "pi",
                    value,
                }])
            }
            Some("turn_end") => Ok(vec![Event::TurnComplete { ok: true }]),
            _ => Ok(vec![Event::Raw {
                driver: "pi",
                value,
            }]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nil() -> Uuid {
        Uuid::nil()
    }

    #[test]
    fn default_command_includes_required_flags() {
        let spec = Pi::new()
            .command(
                nil(),
                &TurnInput::Text("hello".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert_eq!(spec.args.last().unwrap(), "hello");
        assert!(spec.args.contains(&"-p".to_string()));
        let mi = spec.args.iter().position(|a| a == "--mode").unwrap();
        assert_eq!(spec.args[mi + 1], "json");
        assert!(spec.args.iter().any(|a| a == "--session-dir"));
    }

    #[test]
    fn resume_command_adds_continue_flag() {
        let cspec = Pi::new()
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let rspec = Pi::new()
            .resume_command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(!cspec.args.iter().any(|a| a == "--continue"));
        assert!(rspec.args.iter().any(|a| a == "--continue"));
    }

    #[test]
    fn extra_args_appear_after_continue_in_resume() {
        let opts = TurnOptions {
            extra_args: vec!["--my-flag".into()],
            ..Default::default()
        };
        let spec = Pi::new()
            .resume_command(nil(), &TurnInput::Text("hi".into()), &opts)
            .unwrap();
        let cont = spec.args.iter().position(|a| a == "--continue").unwrap();
        let extra = spec.args.iter().position(|a| a == "--my-flag").unwrap();
        let prompt = spec.args.iter().rposition(|a| a == "hi").unwrap();
        assert!(cont < extra, "--continue must come before extra_args");
        assert!(extra < prompt, "extra_args must come before prompt");
    }

    #[test]
    fn provider_emits_provider_flag() {
        let driver = Pi::with_config(PiConfig {
            provider: Some("github-copilot".into()),
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let i = spec.args.iter().position(|a| a == "--provider").unwrap();
        assert_eq!(spec.args[i + 1], "github-copilot");
    }

    #[test]
    fn thinking_passes_reasoning_level() {
        let opts = TurnOptions {
            reasoning: Some(ReasoningLevel::Medium),
            ..Default::default()
        };
        let spec = Pi::new()
            .command(nil(), &TurnInput::Text("hi".into()), &opts)
            .unwrap();
        let i = spec.args.iter().position(|a| a == "--thinking").unwrap();
        assert_eq!(spec.args[i + 1], "medium");
    }

    #[test]
    fn apikey_auth_injects_pi_api_key_without_leaking_to_debug() {
        let driver = Pi::with_config(PiConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("pi-test-XYZ")),
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let (_, v) = spec
            .env
            .iter()
            .find(|(k, _)| k == "PI_API_KEY")
            .expect("env set");
        assert_eq!(v, "pi-test-XYZ");
        assert!(!format!("{driver:?}").contains("pi-test-XYZ"));
    }

    #[test]
    fn config_home_sets_pi_coding_agent_dir_env() {
        let driver = Pi::with_config(PiConfig {
            paths: AgentPaths {
                config_home: Some(PathBuf::from("/tmp/my-pi")),
            },
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let (_, v) = spec
            .env
            .iter()
            .find(|(k, _)| k == "PI_CODING_AGENT_DIR")
            .expect("env set");
        assert_eq!(v, "/tmp/my-pi");
    }

    #[test]
    fn session_dir_is_unique_per_session_id() {
        let driver = Pi::new();
        let d1 = driver.session_dir_for(Uuid::new_v4());
        let d2 = driver.session_dir_for(Uuid::new_v4());
        assert_ne!(d1, d2);
    }

    #[test]
    fn greeting_fixture_parses_to_expected_events() {
        let raw = include_str!("../../tests/fixtures/pi/greeting.jsonl");
        let pi = Pi::new();
        let mut events: Vec<Event> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            events.extend(pi.parse(value).expect("parse ok"));
        }
        expect_test::expect_file!["../../tests/fixtures/pi/greeting.events.snap"]
            .assert_eq(&format!("{events:#?}\n"));
    }

    #[test]
    fn message_update_text_delta_missing_delta_errors() {
        let v = serde_json::json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta"}});
        let err = Pi::new().parse(v).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("delta")));
    }

    #[test]
    fn message_end_with_malformed_usage_errors() {
        let v = serde_json::json!({
            "type": "message_end",
            "message": {
                "role": "assistant",
                "content": [],
                "usage": {"input": "not-a-number", "output": 5}
            }
        });
        let err = Pi::new().parse(v).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFieldType {
                field: "message.usage.input",
                ..
            }
        ));
    }
}
