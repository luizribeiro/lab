use std::path::PathBuf;

use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::driver::{Auth, CommandSpec, Driver, ReasoningLevel, TurnOptions};
use crate::{Event, ParseError};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    #[default]
    Default,
    AutoEdit,
    Yolo,
    Plan,
}

#[derive(Default, Debug, Clone)]
pub struct GeminiConfig {
    pub binary: Option<PathBuf>,
    pub auth: Auth,
    pub default_model: Option<String>,
    pub approval_mode: ApprovalMode,
    pub skip_trust: bool,
    pub extra_env: Vec<(String, String)>,
}

#[derive(Default, Debug, Clone)]
pub struct Gemini {
    pub config: GeminiConfig,
}

impl Gemini {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_config(config: GeminiConfig) -> Self {
        Self { config }
    }
}

impl Driver for Gemini {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn command(&self, session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec {
        let program = self
            .config
            .binary
            .clone()
            .unwrap_or_else(|| PathBuf::from("gemini"));

        let mut args: Vec<String> = vec![
            "-p".into(),
            prompt.to_string(),
            "--output-format".into(),
            "stream-json".into(),
            "--session-id".into(),
            session_id.to_string(),
        ];

        if let Some(model) = opts.model.as_ref().or(self.config.default_model.as_ref()) {
            args.push("--model".into());
            args.push(model.clone());
        }

        let approval = match self.config.approval_mode {
            ApprovalMode::Default => None,
            ApprovalMode::AutoEdit => Some("auto_edit"),
            ApprovalMode::Yolo => Some("yolo"),
            ApprovalMode::Plan => Some("plan"),
        };
        if let Some(a) = approval {
            args.push("--approval-mode".into());
            args.push(a.into());
        }

        if self.config.skip_trust {
            args.push("--skip-trust".into());
        }

        let mut env = self.config.extra_env.clone();
        env.extend(opts.env.iter().cloned());
        if let Some(level) = opts.reasoning {
            let s = match level {
                ReasoningLevel::Low => "low",
                ReasoningLevel::Medium => "medium",
                ReasoningLevel::High => "high",
            };
            env.push(("GEMINI_REASONING".into(), s.into()));
        }
        if let Auth::ApiKey(secret) = &self.config.auth {
            env.push(("GEMINI_API_KEY".into(), secret.expose_secret().to_string()));
        }

        args.extend(opts.raw_args.iter().cloned());

        CommandSpec { program, args, env }
    }

    fn parse(&self, value: serde_json::Value) -> Result<Vec<Event>, ParseError> {
        let event_type = value.get("type").and_then(|v| v.as_str());
        match event_type {
            Some("message") => {
                let role = value
                    .get("role")
                    .and_then(|v| v.as_str())
                    .ok_or(ParseError::MissingField("role"))?;
                if role == "assistant" {
                    let content = value
                        .get("content")
                        .and_then(|v| v.as_str())
                        .ok_or(ParseError::MissingField("content"))?;
                    Ok(vec![Event::AssistantText {
                        delta: content.to_string(),
                    }])
                } else {
                    Ok(vec![Event::Raw {
                        driver: "gemini",
                        value,
                    }])
                }
            }
            Some("result") => {
                let status = value.get("status").and_then(|v| v.as_str()).unwrap_or("");
                let ok = status == "success";

                let mut events = Vec::new();
                if let Some(stats) = value.get("stats") {
                    if let (Some(it), Some(ot)) = (
                        stats.get("input_tokens").and_then(|v| v.as_u64()),
                        stats.get("output_tokens").and_then(|v| v.as_u64()),
                    ) {
                        events.push(Event::Usage {
                            input_tokens: it,
                            output_tokens: ot,
                        });
                    }
                }
                events.push(Event::TurnComplete {
                    ok,
                    final_text: None,
                });
                Ok(events)
            }
            _ => Ok(vec![Event::Raw {
                driver: "gemini",
                value,
            }]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    fn nil() -> Uuid {
        Uuid::nil()
    }

    #[test]
    fn default_command_argv_snapshot() {
        let spec = Gemini::new().command(nil(), "hello", &TurnOptions::default());
        let rendered = format!("{} {}", spec.program.display(), spec.args.join(" "));
        expect![[r#"
            gemini -p hello --output-format stream-json --session-id 00000000-0000-0000-0000-000000000000
        "#]]
        .assert_eq(&format!("{}\n", rendered));
        assert!(spec.env.is_empty());
    }

    #[test]
    fn approval_mode_yolo_emits_flag() {
        let driver = Gemini::with_config(GeminiConfig {
            approval_mode: ApprovalMode::Yolo,
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let i = spec
            .args
            .iter()
            .position(|a| a == "--approval-mode")
            .unwrap();
        assert_eq!(spec.args[i + 1], "yolo");
    }

    #[test]
    fn skip_trust_flag_appears_when_configured() {
        let driver = Gemini::with_config(GeminiConfig {
            skip_trust: true,
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        assert!(spec.args.iter().any(|a| a == "--skip-trust"));
    }

    #[test]
    fn apikey_auth_injects_env_var_without_leaking_to_debug() {
        let driver = Gemini::with_config(GeminiConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("ai-test-XYZ")),
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let (_, v) = spec
            .env
            .iter()
            .find(|(k, _)| k == "GEMINI_API_KEY")
            .expect("env set");
        assert_eq!(v, "ai-test-XYZ");
        assert!(!format!("{driver:?}").contains("ai-test-XYZ"));
    }

    #[test]
    fn greeting_fixture_parses_to_expected_events() {
        let raw = include_str!("../../tests/fixtures/gemini/greeting.jsonl");
        let gemini = Gemini::new();
        let mut events: Vec<Event> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            events.extend(gemini.parse(value).expect("parse ok"));
        }
        expect_test::expect_file!["../../tests/fixtures/gemini/greeting.events.snap"]
            .assert_eq(&format!("{events:#?}\n"));
    }

    #[test]
    fn assistant_message_missing_content_errors() {
        let v = serde_json::json!({"type":"message","role":"assistant"});
        let err = Gemini::new().parse(v).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("content")));
    }
}
