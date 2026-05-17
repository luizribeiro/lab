use std::path::PathBuf;

use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::driver::{AgentPaths, Auth, CommandSpec, Driver, ReasoningLevel, TurnOptions};
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
    pub paths: AgentPaths,
    pub additional_dirs: Vec<PathBuf>,
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

    fn command(
        &self,
        session_id: Uuid,
        prompt: &str,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
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

        if !self.config.additional_dirs.is_empty() {
            args.push("--add-dir".into());
            for d in &self.config.additional_dirs {
                args.push(d.to_string_lossy().into_owned());
            }
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
        if let Some(home) = &self.config.paths.config_home {
            env.push((
                "CLAUDE_CONFIG_DIR".into(),
                home.to_string_lossy().into_owned(),
            ));
        }

        Ok(CommandSpec { program, args, env })
    }

    fn resume_command(
        &self,
        session_id: Uuid,
        prompt: &str,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        // Claude rejects re-use of --session-id with an existing UUID
        // ("Session ID is already in use"). Subsequent turns use --resume
        // to attach to the previously-created session.
        let mut spec = self.command(session_id, prompt, opts)?;
        if let Some(i) = spec.args.iter().position(|a| a == "--session-id") {
            spec.args[i] = "--resume".to_string();
        }
        Ok(spec)
    }

    fn parse(&self, value: serde_json::Value) -> Result<Vec<Event>, ParseError> {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("assistant") => parse_assistant(&value),
            Some("user") => parse_user(&value),
            Some("result") => Ok(vec![parse_result(&value)]),
            _ => Ok(vec![raw(value)]),
        }
    }
}

fn raw(value: serde_json::Value) -> Event {
    Event::Raw {
        driver: "claude",
        value,
    }
}

fn parse_assistant(value: &serde_json::Value) -> Result<Vec<Event>, ParseError> {
    let message = value
        .get("message")
        .and_then(serde_json::Value::as_object)
        .ok_or(ParseError::MissingField("message"))?;

    let mut events = Vec::new();
    if let Some(content) = message.get("content").and_then(serde_json::Value::as_array) {
        for block in content {
            events.push(parse_assistant_block(block)?);
        }
    }
    if let Some(usage) = message.get("usage") {
        events.push(parse_usage(usage)?);
    }
    Ok(events)
}

fn parse_assistant_block(block: &serde_json::Value) -> Result<Event, ParseError> {
    match block.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => {
            let text = block
                .get("text")
                .and_then(serde_json::Value::as_str)
                .ok_or(ParseError::MissingField("text"))?;
            Ok(Event::AssistantText {
                delta: text.to_string(),
            })
        }
        Some("thinking") => {
            let thinking = block
                .get("thinking")
                .and_then(serde_json::Value::as_str)
                .ok_or(ParseError::MissingField("thinking"))?;
            Ok(Event::Thinking {
                delta: thinking.to_string(),
            })
        }
        Some("tool_use") => {
            let id = block
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or(ParseError::MissingField("id"))?;
            let name = block
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or(ParseError::MissingField("name"))?;
            let args = block
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Ok(Event::ToolCall {
                call_id: id.to_string(),
                name: name.to_string(),
                args,
            })
        }
        _ => Ok(raw(block.clone())),
    }
}

fn parse_usage(usage: &serde_json::Value) -> Result<Event, ParseError> {
    let input_tokens = usage
        .get("input_tokens")
        .ok_or(ParseError::MissingField("input_tokens"))?
        .as_u64()
        .ok_or(ParseError::InvalidFieldType {
            field: "input_tokens",
            expected: "u64",
        })?;
    let output_tokens = usage
        .get("output_tokens")
        .ok_or(ParseError::MissingField("output_tokens"))?
        .as_u64()
        .ok_or(ParseError::InvalidFieldType {
            field: "output_tokens",
            expected: "u64",
        })?;
    Ok(Event::Usage {
        input_tokens,
        output_tokens,
    })
}

fn parse_user(value: &serde_json::Value) -> Result<Vec<Event>, ParseError> {
    let Some(content) = value
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(vec![raw(value.clone())]);
    };

    content.iter().map(parse_user_block).collect()
}

fn parse_user_block(block: &serde_json::Value) -> Result<Event, ParseError> {
    match block.get("type").and_then(serde_json::Value::as_str) {
        Some("tool_result") => {
            let call_id = block
                .get("tool_use_id")
                .and_then(serde_json::Value::as_str)
                .ok_or(ParseError::MissingField("tool_use_id"))?;
            let ok = !block
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let output = stringify_tool_result_content(block.get("content"));
            Ok(Event::ToolResult {
                call_id: call_id.to_string(),
                ok,
                output,
            })
        }
        _ => Ok(raw(block.clone())),
    }
}

fn stringify_tool_result_content(content: Option<&serde_json::Value>) -> String {
    match content {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        Some(other) => other.to_string(),
    }
}

fn parse_result(value: &serde_json::Value) -> Event {
    let final_text = value
        .get("result")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let is_error = value
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    Event::TurnComplete {
        ok: !is_error,
        final_text,
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
        let spec = Claude::new()
            .command(nil(), "hello", &TurnOptions::default())
            .unwrap();
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
        let spec = driver.command(nil(), "hi", &opts).unwrap();
        let i = spec.args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(spec.args[i + 1], "claude-opus-4-7");
    }

    #[test]
    fn permission_mode_emits_flag() {
        let driver = Claude::with_config(ClaudeConfig {
            permission_mode: PermissionMode::BypassPermissions,
            ..Default::default()
        });
        let spec = driver
            .command(nil(), "hi", &TurnOptions::default())
            .unwrap();
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
        let spec = Claude::new().command(nil(), "hi", &opts).unwrap();
        let i = spec.args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(spec.args[i + 1], "medium");
    }

    #[test]
    fn apikey_auth_injects_env_var_without_leaking_to_debug() {
        let driver = Claude::with_config(ClaudeConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("sk-test-XYZ")),
            ..Default::default()
        });
        let spec = driver
            .command(nil(), "hi", &TurnOptions::default())
            .unwrap();
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
        let spec = Claude::new().command(nil(), "p", &opts).unwrap();
        let add = spec.args.iter().position(|a| a == "--add-dir").unwrap();
        let prompt = spec.args.iter().rposition(|a| a == "p").unwrap();
        assert!(add < prompt);
    }

    #[test]
    fn greeting_fixture_parses_to_expected_events() {
        let raw = include_str!("../../tests/fixtures/claude/greeting.jsonl");
        let claude = Claude::new();
        let mut events: Vec<Event> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            events.extend(claude.parse(value).expect("parse ok"));
        }
        expect_test::expect_file!["../../tests/fixtures/claude/greeting.events.snap"]
            .assert_eq(&format!("{events:#?}\n"));
    }

    #[test]
    fn assistant_missing_message_errors() {
        let v = serde_json::json!({"type":"assistant"});
        let err = Claude::new().parse(v).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("message")));
    }

    #[test]
    fn resume_command_uses_resume_flag_not_session_id() {
        let spec = Claude::new()
            .resume_command(nil(), "next", &TurnOptions::default())
            .unwrap();
        assert!(spec.args.iter().any(|a| a == "--resume"));
        assert!(!spec.args.iter().any(|a| a == "--session-id"));
    }

    #[test]
    fn config_home_sets_claude_config_dir_env() {
        let driver = Claude::with_config(ClaudeConfig {
            paths: AgentPaths {
                config_home: Some(PathBuf::from("/tmp/my-claude")),
            },
            ..Default::default()
        });
        let spec = driver
            .command(nil(), "hi", &TurnOptions::default())
            .unwrap();
        let (_, v) = spec
            .env
            .iter()
            .find(|(k, _)| k == "CLAUDE_CONFIG_DIR")
            .expect("env set");
        assert_eq!(v, "/tmp/my-claude");
    }

    #[test]
    fn additional_dirs_emits_add_dir_flag() {
        let driver = Claude::with_config(ClaudeConfig {
            additional_dirs: vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
            ..Default::default()
        });
        let spec = driver
            .command(nil(), "hi", &TurnOptions::default())
            .unwrap();
        let i = spec.args.iter().position(|a| a == "--add-dir").unwrap();
        assert_eq!(spec.args[i + 1], "/tmp/a");
        assert_eq!(spec.args[i + 2], "/tmp/b");
    }

    #[test]
    fn result_with_is_error_true_yields_ok_false() {
        let v = serde_json::json!({
            "type": "result",
            "subtype": "error",
            "is_error": true,
            "result": "context limit exceeded"
        });
        let evs = Claude::new().parse(v).expect("parse ok");
        assert_eq!(evs.len(), 1);
        assert!(matches!(
            &evs[0],
            Event::TurnComplete { ok: false, final_text: Some(s) } if s == "context limit exceeded"
        ));
    }
}
