use std::path::PathBuf;

use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::driver::{Auth, CommandSpec, Driver, ReasoningLevel, TurnOptions};
use crate::{Event, ParseError};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    BypassPermissions,
}

#[derive(Default, Debug, Clone)]
pub struct ClaudeConfig {
    pub binary: Option<PathBuf>,
    pub auth: Auth,
    pub default_model: Option<String>,
    pub permission_mode: PermissionMode,
    pub extra_env: Vec<(String, String)>,
}

#[derive(Default, Debug, Clone)]
pub struct Claude {
    pub config: ClaudeConfig,
}

impl Claude {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_config(config: ClaudeConfig) -> Self {
        Self { config }
    }
}

impl Driver for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn command(&self, session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec {
        let program = self
            .config
            .binary
            .clone()
            .unwrap_or_else(|| PathBuf::from("claude"));

        let mut args: Vec<String> = vec![
            "-p".into(),
            "--verbose".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--session-id".into(),
            session_id.to_string(),
        ];

        if let Some(model) = opts.model.as_ref().or(self.config.default_model.as_ref()) {
            args.push("--model".into());
            args.push(model.clone());
        }

        let perm = match self.config.permission_mode {
            PermissionMode::Default => None,
            PermissionMode::AcceptEdits => Some("acceptEdits"),
            PermissionMode::BypassPermissions => Some("bypassPermissions"),
        };
        if let Some(p) = perm {
            args.push("--permission-mode".into());
            args.push(p.into());
        }

        if let Some(level) = opts.reasoning {
            let s = match level {
                ReasoningLevel::Low => "low",
                ReasoningLevel::Medium => "medium",
                ReasoningLevel::High => "high",
            };
            args.push("--effort".into());
            args.push(s.into());
        }

        args.extend(opts.raw_args.iter().cloned());

        args.push(prompt.to_string());

        let mut env = self.config.extra_env.clone();
        env.extend(opts.env.iter().cloned());
        match &self.config.auth {
            Auth::Ambient => {}
            Auth::ApiKey(secret) => {
                env.push((
                    "ANTHROPIC_API_KEY".into(),
                    secret.expose_secret().to_string(),
                ));
            }
        }

        CommandSpec { program, args, env }
    }

    fn parse(&self, value: serde_json::Value) -> Result<Event, ParseError> {
        Ok(Event::Raw {
            driver: "claude",
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::ReasoningLevel;
    use expect_test::expect;

    fn nil() -> Uuid {
        Uuid::nil()
    }

    #[test]
    fn default_command_argv_snapshot() {
        let spec = Claude::new().command(nil(), "hello", &TurnOptions::default());
        let rendered = format!("{} {}", spec.program.display(), spec.args.join(" "));
        expect![[r#"
            claude -p --verbose --output-format stream-json --session-id 00000000-0000-0000-0000-000000000000 hello
        "#]]
        .assert_eq(&format!("{}\n", rendered));
        assert!(spec.env.is_empty());
    }

    #[test]
    fn model_override_takes_precedence_over_default() {
        let driver = Claude::with_config(ClaudeConfig {
            default_model: Some("claude-sonnet-4-6".into()),
            ..Default::default()
        });
        let opts = TurnOptions {
            model: Some("claude-opus-4-7".into()),
            ..Default::default()
        };
        let spec = driver.command(nil(), "hi", &opts);
        let i = spec.args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(spec.args[i + 1], "claude-opus-4-7");
    }

    #[test]
    fn permission_mode_emits_flag() {
        let driver = Claude::with_config(ClaudeConfig {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let i = spec
            .args
            .iter()
            .position(|a| a == "--permission-mode")
            .unwrap();
        assert_eq!(spec.args[i + 1], "bypassPermissions");
    }

    #[test]
    fn effort_passes_reasoning_level() {
        let opts = TurnOptions {
            reasoning: Some(ReasoningLevel::Medium),
            ..Default::default()
        };
        let spec = Claude::new().command(nil(), "hi", &opts);
        let i = spec.args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(spec.args[i + 1], "medium");
    }

    #[test]
    fn apikey_auth_injects_env_var_without_leaking_to_debug() {
        let driver = Claude::with_config(ClaudeConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("sk-test-XYZ")),
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let (_, v) = spec
            .env
            .iter()
            .find(|(k, _)| k == "ANTHROPIC_API_KEY")
            .expect("env set");
        assert_eq!(v, "sk-test-XYZ");
        assert!(!format!("{driver:?}").contains("sk-test-XYZ"));
    }

    #[test]
    fn raw_args_appear_before_prompt() {
        let opts = TurnOptions {
            raw_args: vec!["--add-dir".into(), "/tmp".into()],
            ..Default::default()
        };
        let spec = Claude::new().command(nil(), "p", &opts);
        let add = spec.args.iter().position(|a| a == "--add-dir").unwrap();
        let prompt = spec.args.iter().rposition(|a| a == "p").unwrap();
        assert!(add < prompt);
    }
}
