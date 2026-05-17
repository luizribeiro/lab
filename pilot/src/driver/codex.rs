//! OpenAI Codex driver.
//!
//! # Multi-turn
//! Codex's CLI auto-generates a thread id on the first turn and emits it as
//! a `thread.started` event. This driver overrides [`Driver::observe`] to
//! capture that id (keyed by pilot's session UUID) and reuses it as the
//! positional `resume <thread_id>` argument on subsequent turns.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::driver::{Auth, CommandSpec, Driver, ReasoningLevel, TurnOptions};
use crate::{Event, ParseError};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone)]
pub struct CodexConfig {
    pub binary: Option<PathBuf>,
    pub auth: Auth,
    pub default_model: Option<String>,
    pub sandbox: SandboxMode,
    /// Pass `--skip-git-repo-check`. Defaults to `true` — pilot is a
    /// headless driver and codex refuses to run outside a git repo
    /// without this flag.
    pub skip_git_repo_check: bool,
    /// `codex -c key=value` config overrides.
    pub config_overrides: Vec<(String, String)>,
    pub extra_env: Vec<(String, String)>,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            binary: None,
            auth: Auth::default(),
            default_model: None,
            sandbox: SandboxMode::default(),
            skip_git_repo_check: true,
            config_overrides: Vec::new(),
            extra_env: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Codex {
    pub config: CodexConfig,
    thread_ids: Arc<Mutex<HashMap<Uuid, String>>>,
}

impl Codex {
    pub fn new() -> Self {
        Self {
            config: CodexConfig::default(),
            thread_ids: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn with_config(config: CodexConfig) -> Self {
        Self {
            config,
            thread_ids: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Driver for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn command(&self, _session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec {
        let program = self
            .config
            .binary
            .clone()
            .unwrap_or_else(|| PathBuf::from("codex"));

        let mut args: Vec<String> = vec!["exec".into(), "--json".into()];

        let sandbox = match self.config.sandbox {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite => "workspace-write",
            SandboxMode::DangerFullAccess => "danger-full-access",
        };
        args.push("--sandbox".into());
        args.push(sandbox.into());

        if self.config.skip_git_repo_check {
            args.push("--skip-git-repo-check".into());
        }

        if let Some(model) = opts.model.as_ref().or(self.config.default_model.as_ref()) {
            args.push("--model".into());
            args.push(model.clone());
        }

        for (k, v) in &self.config.config_overrides {
            args.push("-c".into());
            args.push(format!("{k}={v}"));
        }

        if let Some(level) = opts.reasoning {
            let s = match level {
                ReasoningLevel::Low => "low",
                ReasoningLevel::Medium => "medium",
                ReasoningLevel::High => "high",
            };
            args.push("-c".into());
            args.push(format!("reasoning.effort={s}"));
        }

        args.extend(opts.raw_args.iter().cloned());

        args.push(prompt.to_string());

        let mut env = self.config.extra_env.clone();
        env.extend(opts.env.iter().cloned());
        if let Auth::ApiKey(secret) = &self.config.auth {
            env.push(("OPENAI_API_KEY".into(), secret.expose_secret().to_string()));
        }

        CommandSpec { program, args, env }
    }

    /// Build the codex resume invocation.
    ///
    /// # Fallback semantics
    ///
    /// This driver maintains an in-memory `Uuid -> thread_id` map populated
    /// by [`Driver::observe`] as `thread.started` events stream in. If
    /// `session_id` is missing from that map — because:
    ///   - the first turn's events were never drained,
    ///   - this is the first turn after a `Session::resume(...)` call in a
    ///     fresh process where the map was lost, OR
    ///   - the first turn failed before yielding `thread.started`
    ///
    /// — `resume_command` falls back to a fresh `command()` invocation AND
    /// emits a `tracing::warn!`. Continuity is silently broken in those
    /// cases. Programs that need durable resume across process restarts
    /// will need to persist the thread_id themselves (e.g., logged from
    /// `Driver::observe` output) and reconstruct the codex driver state
    /// at startup.
    fn resume_command(&self, session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec {
        let thread_id = self
            .thread_ids
            .lock()
            .ok()
            .and_then(|m| m.get(&session_id).cloned());

        let Some(thread_id) = thread_id else {
            tracing::warn!(
                session_id = %session_id,
                "codex resume_command: no captured thread_id for this session; falling back to a fresh `codex exec` (NEW thread). \
                 Drain previous turns to capture thread.started, or persist thread_id externally for cross-process resume."
            );
            return self.command(session_id, prompt, opts);
        };

        let mut spec = self.command(session_id, "", opts);
        spec.args.pop();
        spec.args.push("resume".into());
        spec.args.push(thread_id);
        spec.args.push(prompt.to_string());
        spec
    }

    fn observe(&self, session_id: Uuid, raw: &serde_json::Value) {
        if raw.get("type").and_then(|v| v.as_str()) == Some("thread.started") {
            if let Some(tid) = raw.get("thread_id").and_then(|v| v.as_str()) {
                if let Ok(mut map) = self.thread_ids.lock() {
                    map.insert(session_id, tid.to_string());
                }
            }
        }
    }

    fn parse(&self, value: serde_json::Value) -> Result<Vec<Event>, ParseError> {
        let event_type = value.get("type").and_then(|v| v.as_str());
        match event_type {
            Some("item.completed") => {
                let item = value.get("item").ok_or(ParseError::MissingField("item"))?;
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match item_type {
                    "agent_message" => {
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .ok_or(ParseError::MissingField("item.text"))?;
                        Ok(vec![Event::AssistantText {
                            delta: text.to_string(),
                        }])
                    }
                    _ => Ok(vec![Event::Raw {
                        driver: "codex",
                        value,
                    }]),
                }
            }
            Some("turn.completed") => {
                let mut events = Vec::new();
                if let Some(usage) = value.get("usage") {
                    if let (Some(it), Some(ot)) = (
                        usage.get("input_tokens").and_then(|v| v.as_u64()),
                        usage.get("output_tokens").and_then(|v| v.as_u64()),
                    ) {
                        events.push(Event::Usage {
                            input_tokens: it,
                            output_tokens: ot,
                        });
                    }
                }
                events.push(Event::TurnComplete {
                    ok: true,
                    final_text: None,
                });
                Ok(events)
            }
            _ => Ok(vec![Event::Raw {
                driver: "codex",
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
        let spec = Codex::new().command(nil(), "hello", &TurnOptions::default());
        let rendered = format!("{} {}", spec.program.display(), spec.args.join(" "));
        expect![[r#"
            codex exec --json --sandbox read-only --skip-git-repo-check hello
        "#]]
        .assert_eq(&format!("{rendered}\n"));
    }

    #[test]
    fn sandbox_workspace_write_emits_flag() {
        let driver = Codex::with_config(CodexConfig {
            sandbox: SandboxMode::WorkspaceWrite,
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let i = spec.args.iter().position(|a| a == "--sandbox").unwrap();
        assert_eq!(spec.args[i + 1], "workspace-write");
    }

    #[test]
    fn skip_git_repo_check_can_be_disabled() {
        let driver = Codex::with_config(CodexConfig {
            skip_git_repo_check: false,
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        assert!(!spec.args.iter().any(|a| a == "--skip-git-repo-check"));
    }

    #[test]
    fn apikey_auth_injects_openai_api_key_without_leaking_to_debug() {
        let driver = Codex::with_config(CodexConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("sk-codex-test")),
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let (_, v) = spec
            .env
            .iter()
            .find(|(k, _)| k == "OPENAI_API_KEY")
            .expect("env set");
        assert_eq!(v, "sk-codex-test");
        assert!(!format!("{driver:?}").contains("sk-codex-test"));
    }

    #[test]
    fn config_overrides_emit_dash_c_flags() {
        let driver = Codex::with_config(CodexConfig {
            config_overrides: vec![("model".into(), "o3".into())],
            ..Default::default()
        });
        let spec = driver.command(nil(), "hi", &TurnOptions::default());
        let i = spec.args.iter().position(|a| a == "-c").unwrap();
        assert_eq!(spec.args[i + 1], "model=o3");
    }

    #[test]
    fn greeting_fixture_parses_to_expected_events() {
        let raw = include_str!("../../tests/fixtures/codex/greeting.jsonl");
        let codex = Codex::new();
        let mut events: Vec<Event> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            events.extend(codex.parse(value).expect("parse ok"));
        }
        expect_test::expect_file!["../../tests/fixtures/codex/greeting.events.snap"]
            .assert_eq(&format!("{events:#?}\n"));
    }

    #[test]
    fn observe_captures_thread_id_for_resume() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let raw = serde_json::json!({
            "type": "thread.started",
            "thread_id": "019e3733-d3a7-7c12-9a36-759558b89551"
        });
        codex.observe(sid, &raw);

        let spec = codex.resume_command(sid, "follow-up", &TurnOptions::default());
        let resume_idx = spec.args.iter().position(|a| a == "resume").unwrap();
        assert_eq!(
            spec.args[resume_idx + 1],
            "019e3733-d3a7-7c12-9a36-759558b89551"
        );
        assert_eq!(spec.args[resume_idx + 2], "follow-up");
    }

    #[test]
    fn resume_command_without_observation_falls_back_to_command() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let spec = codex.resume_command(sid, "no thread id yet", &TurnOptions::default());
        assert!(!spec.args.iter().any(|a| a == "resume"));
    }

    #[test]
    fn resume_command_fallback_is_identical_to_command() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let resumed = codex.resume_command(sid, "x", &TurnOptions::default());
        let fresh = codex.command(sid, "x", &TurnOptions::default());
        assert_eq!(resumed.args, fresh.args);
        assert_eq!(resumed.program, fresh.program);
        assert_eq!(resumed.env, fresh.env);
    }

    #[test]
    fn observe_ignores_non_thread_started_events() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let raw = serde_json::json!({"type": "turn.started"});
        codex.observe(sid, &raw);
        let map = codex.thread_ids.lock().unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn item_completed_agent_message_missing_text_errors() {
        let v = serde_json::json!({
            "type": "item.completed",
            "item": {"id": "item_0", "type": "agent_message"}
        });
        let err = Codex::new().parse(v).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("item.text")));
    }
}
