use std::path::PathBuf;

use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::driver::{
    AgentPaths, Auth, CommandSpec, Driver, ReasoningLevel, TurnInput, TurnOptions,
};
use crate::{Event, ParseError};

#[non_exhaustive]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    #[default]
    Default,
    AutoEdit,
    Yolo,
    Plan,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    pub binary: Option<PathBuf>,
    pub auth: Auth,
    pub default_model: Option<String>,
    pub approval_mode: ApprovalMode,
    /// Pass `--skip-trust` to bypass gemini's untrusted-folder prompt.
    ///
    /// # Default
    /// `true`. Pilot is a headless driver: without skipping the trust
    /// prompt, every `Session::new(...).send(...)` against `Arc::new(Gemini::new())`
    /// would fail in any workdir that hasn't been trusted in an
    /// interactive gemini session first. We chose ergonomics over a
    /// fail-closed default. Callers who want gemini's trust gate to
    /// remain active MUST set this to `false` explicitly via `GeminiConfig`.
    ///
    /// # Security
    /// `skip_trust: true` means gemini will read/execute project-level
    /// gemini config from the workdir without prompting. Pass only paths
    /// you trust to `Session::new(_, workdir)`.
    pub skip_trust: bool,
    pub extra_env: Vec<(String, String)>,
    pub paths: AgentPaths,
    pub include_directories: Vec<PathBuf>,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            binary: None,
            auth: Auth::default(),
            default_model: None,
            approval_mode: ApprovalMode::default(),
            // Default true: pilot is a headless driver, and gemini refuses to run
            // in untrusted folders without this flag. Callers who want gemini's
            // fail-closed trust prompt to apply should set `skip_trust: false`
            // explicitly. (See ../../docs/security.md once we have one.)
            skip_trust: true,
            extra_env: Vec::new(),
            paths: AgentPaths::default(),
            include_directories: Vec::new(),
        }
    }
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
        if self.config.paths.config_home.is_some() {
            return Err(crate::Error::UnsupportedOption {
                driver: "gemini",
                option: "paths.config_home",
            });
        }

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

        if !self.config.include_directories.is_empty() {
            let joined = self
                .config
                .include_directories
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(",");
            args.push("--include-directories".into());
            args.push(joined);
        }

        args.extend(opts.extra_args.iter().cloned());

        Ok(CommandSpec { program, args, env })
    }

    fn resume_command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        let mut spec = self.command(session_id, input, opts)?;
        if let Some(i) = spec.args.iter().position(|a| a == "--session-id") {
            spec.args[i] = "--resume".to_string();
        }
        Ok(spec)
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
                events.push(Event::TurnComplete { ok });
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
        let spec = Gemini::new()
            .command(
                nil(),
                &TurnInput::Text("hello".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let rendered = format!("{} {}", spec.program.display(), spec.args.join(" "));
        expect![[r#"
            gemini -p hello --output-format stream-json --session-id 00000000-0000-0000-0000-000000000000 --skip-trust
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
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let i = spec
            .args
            .iter()
            .position(|a| a == "--approval-mode")
            .unwrap();
        assert_eq!(spec.args[i + 1], "yolo");
    }

    #[test]
    fn skip_trust_can_be_disabled() {
        let driver = Gemini::with_config(GeminiConfig {
            skip_trust: false,
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(!spec.args.iter().any(|a| a == "--skip-trust"));
    }

    #[test]
    fn apikey_auth_injects_env_var_without_leaking_to_debug() {
        let driver = Gemini::with_config(GeminiConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("ai-test-XYZ")),
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
    fn resume_command_uses_resume_flag_not_session_id() {
        let spec = Gemini::new()
            .resume_command(
                nil(),
                &TurnInput::Text("next".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(spec.args.iter().any(|a| a == "--resume"));
        assert!(!spec.args.iter().any(|a| a == "--session-id"));
    }

    #[test]
    fn config_home_on_gemini_returns_unsupported_option_error() {
        let driver = Gemini::with_config(GeminiConfig {
            paths: AgentPaths {
                config_home: Some(PathBuf::from("/tmp/x")),
            },
            ..Default::default()
        });
        let err = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(
            err,
            crate::Error::UnsupportedOption {
                driver: "gemini",
                option: "paths.config_home",
            }
        ));
    }

    #[test]
    fn include_directories_emits_comma_separated_list() {
        let driver = Gemini::with_config(GeminiConfig {
            include_directories: vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let i = spec
            .args
            .iter()
            .position(|a| a == "--include-directories")
            .unwrap();
        assert_eq!(spec.args[i + 1], "/tmp/a,/tmp/b");
    }

    #[test]
    fn assistant_message_missing_content_errors() {
        let v = serde_json::json!({"type":"message","role":"assistant"});
        let err = Gemini::new().parse(v).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("content")));
    }
}
