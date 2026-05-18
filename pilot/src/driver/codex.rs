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

use crate::driver::{
    AgentPaths, Auth, CommandSpec, Driver, ReasoningLevel, TurnInput, TurnOptions,
};
use crate::{Event, ParseError};

#[non_exhaustive]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[non_exhaustive]
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
    pub paths: AgentPaths,
    pub additional_dirs: Vec<PathBuf>,
    pub state: CodexPilotState,
}

#[non_exhaustive]
#[derive(Default, Debug, Clone)]
pub struct CodexPilotState {
    /// Optional path to a JSON file mapping pilot session UUIDs to
    /// captured codex thread ids. When set, [`Driver::observe`] persists
    /// each `thread.started` to this file and [`Driver::resume_command`]
    /// looks it up — enabling `Session::resume(...)` to actually continue
    /// a previous codex thread across program restarts.
    ///
    /// When `None` (default), only the in-memory map is used; resuming
    /// across processes silently degrades to a fresh first-turn command
    /// (also logged via `tracing::warn!`).
    ///
    /// # Cross-process safety
    /// All persistence operations across ALL `Codex` instances in this
    /// process are serialized through a single internal mutex, so the
    /// "concurrent observe()s drop a mapping" race is impossible within
    /// a single process. Two PROCESSES writing to the same path remain
    /// unsafe (no OS file lock); use distinct paths per process if you
    /// need that. A future commit may add `File::try_lock` (stable in
    /// Rust 1.89+; pilot's MSRV is 1.85).
    pub thread_store_path: Option<PathBuf>,
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
            paths: AgentPaths::default(),
            additional_dirs: Vec::new(),
            state: CodexPilotState::default(),
        }
    }
}

#[non_exhaustive]
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

    fn command(
        &self,
        _session_id: Uuid,
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

        for d in &self.config.additional_dirs {
            args.push("--add-dir".into());
            args.push(d.to_string_lossy().into_owned());
        }

        args.extend(opts.extra_args.iter().cloned());

        args.push(prompt.to_string());

        let mut env = self.config.extra_env.clone();
        env.extend(opts.env.iter().cloned());
        if let Auth::ApiKey(secret) = &self.config.auth {
            env.push(("OPENAI_API_KEY".into(), secret.expose_secret().to_string()));
        }
        if let Some(home) = &self.config.paths.config_home {
            env.push(("CODEX_HOME".into(), home.to_string_lossy().into_owned()));
        }

        Ok(CommandSpec { program, args, env })
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
        let thread_id = self
            .thread_ids
            .lock()
            .ok()
            .and_then(|m| m.get(&session_id).cloned());

        let thread_id = thread_id.or_else(|| {
            self.config
                .state
                .thread_store_path
                .as_ref()
                .and_then(|p| load_thread_map(p).get(&session_id).cloned())
                .inspect(|tid| {
                    if let Ok(mut m) = self.thread_ids.lock() {
                        m.insert(session_id, tid.clone());
                    }
                })
        });

        let Some(thread_id) = thread_id else {
            tracing::warn!(
                session_id = %session_id,
                "codex resume_command: no captured thread_id for this session (in-memory or persisted); falling back to a fresh `codex exec`. \
                 Set CodexConfig.state.thread_store_path to enable cross-process resume."
            );
            return self.command(session_id, input, opts);
        };

        let mut spec = self.command(session_id, &TurnInput::Text(String::new()), opts)?;
        spec.args.pop();
        spec.args.push("resume".into());
        spec.args.push(thread_id);
        spec.args.push(prompt.to_string());
        Ok(spec)
    }

    fn observe(&self, session_id: Uuid, raw: &serde_json::Value) {
        if raw.get("type").and_then(|v| v.as_str()) != Some("thread.started") {
            return;
        }
        let Some(tid) = raw.get("thread_id").and_then(|v| v.as_str()) else {
            return;
        };

        if let Ok(mut map) = self.thread_ids.lock() {
            map.insert(session_id, tid.to_string());
        }

        if let Some(path) = &self.config.state.thread_store_path {
            // Process-wide serialization for all codex persistence operations.
            // Strictly less throughput than per-path locking, but the writes are
            // infrequent and tiny, and this is provably race-free across all
            // `Codex` instances in a single process.
            static PERSIST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = PERSIST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

            let mut on_disk = load_thread_map(path);
            on_disk.insert(session_id, tid.to_string());
            if let Err(e) = save_thread_map(path, &on_disk) {
                tracing::warn!(error = %e, path = %path.display(), "failed to persist codex thread map");
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
                    "command_execution" => {
                        let id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .ok_or(ParseError::MissingField("item.id"))?
                            .to_string();
                        let command = item
                            .get("command")
                            .and_then(|v| v.as_str())
                            .ok_or(ParseError::MissingField("item.command"))?
                            .to_string();
                        let exit_code =
                            item.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
                        let output = item
                            .get("aggregated_output")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Ok(vec![
                            Event::ToolCall {
                                call_id: id.clone(),
                                name: "command_execution".to_string(),
                                args: serde_json::json!({ "command": command }),
                            },
                            Event::ToolResult {
                                call_id: id,
                                ok: exit_code == 0,
                                output,
                            },
                        ])
                    }
                    "file_change" => {
                        let id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .ok_or(ParseError::MissingField("item.id"))?
                            .to_string();
                        let changes = item
                            .get("changes")
                            .cloned()
                            .ok_or(ParseError::MissingField("item.changes"))?;
                        Ok(vec![
                            Event::ToolCall {
                                call_id: id.clone(),
                                name: "file_change".to_string(),
                                args: serde_json::json!({ "changes": changes }),
                            },
                            Event::ToolResult {
                                call_id: id,
                                ok: true,
                                output: String::new(),
                            },
                        ])
                    }
                    _ => Ok(vec![Event::Raw {
                        driver: "codex",
                        value,
                    }]),
                }
            }
            Some("error") => {
                let msg = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(vec![Event::AssistantText { delta: msg }])
            }
            Some("turn.failed") => Ok(vec![Event::TurnComplete { ok: false }]),
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
                events.push(Event::TurnComplete { ok: true });
                Ok(events)
            }
            _ => Ok(vec![Event::Raw {
                driver: "codex",
                value,
            }]),
        }
    }
}

fn load_thread_map(path: &std::path::Path) -> std::collections::HashMap<Uuid, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return std::collections::HashMap::new();
    };
    let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, String>>(&text)
    else {
        return std::collections::HashMap::new();
    };
    parsed
        .into_iter()
        .filter_map(|(k, v)| Uuid::parse_str(&k).ok().map(|u| (u, v)))
        .collect()
}

fn save_thread_map(
    path: &std::path::Path,
    map: &std::collections::HashMap<Uuid, String>,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let serializable: std::collections::HashMap<String, &str> = map
        .iter()
        .map(|(u, t)| (u.to_string(), t.as_str()))
        .collect();
    let text = serde_json::to_string_pretty(&serializable).map_err(std::io::Error::other)?;
    let unique = uuid::Uuid::new_v4();
    let mut tmp_name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("threads.json"));
    tmp_name.push(format!(".tmp.{unique}"));
    let tmp = path.with_file_name(&tmp_name);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
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
        let spec = Codex::new()
            .command(
                nil(),
                &TurnInput::Text("hello".into()),
                &TurnOptions::default(),
            )
            .unwrap();
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
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let i = spec.args.iter().position(|a| a == "--sandbox").unwrap();
        assert_eq!(spec.args[i + 1], "workspace-write");
    }

    #[test]
    fn skip_git_repo_check_can_be_disabled() {
        let driver = Codex::with_config(CodexConfig {
            skip_git_repo_check: false,
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(!spec.args.iter().any(|a| a == "--skip-git-repo-check"));
    }

    #[test]
    fn apikey_auth_injects_openai_api_key_without_leaking_to_debug() {
        let driver = Codex::with_config(CodexConfig {
            auth: Auth::ApiKey(secrecy::SecretString::from("sk-codex-test")),
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
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
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

        let spec = codex
            .resume_command(
                sid,
                &TurnInput::Text("follow-up".into()),
                &TurnOptions::default(),
            )
            .unwrap();
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
        let spec = codex
            .resume_command(
                sid,
                &TurnInput::Text("no thread id yet".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(!spec.args.iter().any(|a| a == "resume"));
    }

    #[test]
    fn resume_command_fallback_is_identical_to_command() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let resumed = codex
            .resume_command(sid, &TurnInput::Text("x".into()), &TurnOptions::default())
            .unwrap();
        let fresh = codex
            .command(sid, &TurnInput::Text("x".into()), &TurnOptions::default())
            .unwrap();
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
    fn persistence_enables_cross_instance_resume() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let sid = Uuid::new_v4();

        let cfg = CodexConfig {
            state: CodexPilotState {
                thread_store_path: Some(path.clone()),
            },
            ..Default::default()
        };
        let first = Codex::with_config(cfg);
        first.observe(
            sid,
            &serde_json::json!({
                "type": "thread.started",
                "thread_id": "019e0000-0000-0000-0000-000000000abc"
            }),
        );

        let cfg = CodexConfig {
            state: CodexPilotState {
                thread_store_path: Some(path.clone()),
            },
            ..Default::default()
        };
        let second = Codex::with_config(cfg);
        let spec = second
            .resume_command(
                sid,
                &TurnInput::Text("follow-up".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let i = spec.args.iter().position(|a| a == "resume").unwrap();
        assert_eq!(spec.args[i + 1], "019e0000-0000-0000-0000-000000000abc");
        assert_eq!(spec.args[i + 2], "follow-up");
    }

    #[test]
    fn persistence_disabled_falls_back_when_state_missing() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let resumed = codex
            .resume_command(sid, &TurnInput::Text("x".into()), &TurnOptions::default())
            .unwrap();
        let fresh = codex
            .command(sid, &TurnInput::Text("x".into()), &TurnOptions::default())
            .unwrap();
        assert_eq!(resumed.args, fresh.args);
    }

    struct CwdGuard {
        original: std::path::PathBuf,
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn save_with_bare_filename_does_not_error() {
        let _serial = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp_dir = tempfile::tempdir().unwrap();
        let _guard = CwdGuard {
            original: std::env::current_dir().unwrap(),
        };
        std::env::set_current_dir(&tmp_dir).unwrap();

        save_thread_map(
            std::path::Path::new("threads.json"),
            &[(Uuid::nil(), "abc".to_string())].into_iter().collect(),
        )
        .expect("save with bare filename must succeed");
    }

    #[test]
    fn config_home_sets_codex_home_env() {
        let driver = Codex::with_config(CodexConfig {
            paths: AgentPaths {
                config_home: Some(PathBuf::from("/tmp/my-codex")),
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
            .find(|(k, _)| k == "CODEX_HOME")
            .expect("env set");
        assert_eq!(v, "/tmp/my-codex");
    }

    #[test]
    fn additional_dirs_emits_repeated_add_dir_flags() {
        let driver = Codex::with_config(CodexConfig {
            additional_dirs: vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
            ..Default::default()
        });
        let spec = driver
            .command(
                nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        let positions: Vec<usize> = spec
            .args
            .iter()
            .enumerate()
            .filter(|(_, a)| a == &"--add-dir")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(positions.len(), 2, "two --add-dir flags expected");
        assert_eq!(spec.args[positions[0] + 1], "/tmp/a");
        assert_eq!(spec.args[positions[1] + 1], "/tmp/b");
    }

    #[test]
    fn tool_use_fixture_parses_to_expected_events() {
        let raw = include_str!("../../tests/fixtures/codex/tool_use.jsonl");
        let codex = Codex::new();
        let mut events: Vec<Event> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            events.extend(codex.parse(value).expect("parse ok"));
        }
        expect_test::expect_file!["../../tests/fixtures/codex/tool_use.events.snap"]
            .assert_eq(&format!("{events:#?}\n"));
    }

    #[test]
    fn error_fixture_parses_to_expected_events() {
        let raw = include_str!("../../tests/fixtures/codex/error.jsonl");
        let codex = Codex::new();
        let mut events: Vec<Event> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            events.extend(codex.parse(value).expect("parse ok"));
        }
        expect_test::expect_file!["../../tests/fixtures/codex/error.events.snap"]
            .assert_eq(&format!("{events:#?}\n"));

        let final_text: String = events
            .iter()
            .filter_map(|e| match e {
                Event::AssistantText { delta } => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        assert!(final_text.contains("nonexistent-model-12345"));
    }

    #[test]
    fn command_execution_item_yields_toolcall_and_toolresult() {
        let v = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "item_x",
                "type": "command_execution",
                "command": "ls /tmp",
                "aggregated_output": "a\nb\n",
                "exit_code": 0,
                "status": "completed",
            },
        });
        let evs = Codex::new().parse(v).unwrap();
        assert_eq!(evs.len(), 2);
        assert!(
            matches!(&evs[0], Event::ToolCall { call_id, name, .. } if call_id == "item_x" && name == "command_execution")
        );
        assert!(
            matches!(&evs[1], Event::ToolResult { call_id, ok: true, output } if call_id == "item_x" && output == "a\nb\n")
        );
    }

    #[test]
    fn command_execution_nonzero_exit_is_toolresult_ok_false() {
        let v = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "item_x",
                "type": "command_execution",
                "command": "false",
                "aggregated_output": "",
                "exit_code": 42,
                "status": "completed",
            },
        });
        let evs = Codex::new().parse(v).unwrap();
        assert_eq!(evs.len(), 2);
        assert!(matches!(&evs[1], Event::ToolResult { ok: false, .. }));
    }

    #[test]
    fn file_change_item_yields_toolcall_and_empty_toolresult() {
        let v = serde_json::json!({
            "type": "item.completed",
            "item": {
                "id": "item_y",
                "type": "file_change",
                "changes": [{"path":"/tmp/x","kind":"add"}],
                "status": "completed",
            },
        });
        let evs = Codex::new().parse(v).unwrap();
        assert_eq!(evs.len(), 2);
        assert!(matches!(&evs[0], Event::ToolCall { .. }));
        assert!(matches!(&evs[1], Event::ToolResult { ok: true, output, .. } if output.is_empty()));
    }

    #[test]
    fn error_event_emits_synthetic_assistant_text() {
        let v = serde_json::json!({"type":"error","message":"bad model"});
        let evs = Codex::new().parse(v).unwrap();
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], Event::AssistantText { delta } if delta == "bad model"));
    }

    #[test]
    fn turn_failed_yields_turncomplete_ok_false() {
        let v = serde_json::json!({"type":"turn.failed","error":{"message":"x"}});
        let evs = Codex::new().parse(v).unwrap();
        assert_eq!(evs.len(), 1);
        assert!(matches!(&evs[0], Event::TurnComplete { ok: false }));
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
