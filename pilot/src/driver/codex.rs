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
    pub state: CodexPilotState,
}

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
    /// Each in-process `Codex` instance serializes its own writes via an
    /// internal mutex. Multiple PROCESSES writing to the same path are NOT
    /// fully synchronized — a last-writer-wins race is possible, in which
    /// the losing writer's update is silently dropped. File integrity is
    /// preserved (atomic rename + unique temp names mean no torn writes or
    /// corrupted JSON), but if you need stronger guarantees, scope the
    /// path per-process. A future commit may add OS-level file locking
    /// (Rust 1.89+ exposes portable `File::try_lock`; pilot's MSRV is 1.85).
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
            state: CodexPilotState::default(),
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

    fn command(
        &self,
        _session_id: Uuid,
        prompt: &str,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
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
        prompt: &str,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
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
            return self.command(session_id, prompt, opts);
        };

        let mut spec = self.command(session_id, "", opts)?;
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
            let path_lock = lock_for_path(path);
            let _guard = path_lock.lock().unwrap_or_else(|e| e.into_inner());
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

/// Best-effort path normalization that doesn't require the target file
/// (or even its parent) to exist. We use this to key our process-wide
/// lock registry so different spellings of the same logical file
/// (`threads.json` / `./threads.json` / absolute path) share the same
/// mutex.
fn normalize_for_key(path: &std::path::Path) -> std::path::PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }

    let absolute: std::path::PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::new())
            .join(path)
    };
    let stripped: std::path::PathBuf = absolute
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect();

    let mut existing: &std::path::Path = &stripped;
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(p), Some(name)) => {
                tail.push(name.to_os_string());
                existing = p;
            }
            _ => break,
        }
    }

    let mut result = std::fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for name in tail.iter().rev() {
        result.push(name);
    }
    result
}

/// Returns a process-wide mutex shared by all `Codex` instances writing
/// to the same canonical path. Without this, two `Codex::with_config(...)`
/// instances pointed at the same store would race their
/// load-modify-save sequences in `observe()`.
fn lock_for_path(path: &std::path::Path) -> std::sync::Arc<std::sync::Mutex<()>> {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));

    let key = normalize_for_key(path);
    let mut map = registry.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
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
            .command(nil(), "hello", &TurnOptions::default())
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
            .command(nil(), "hi", &TurnOptions::default())
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
            .command(nil(), "hi", &TurnOptions::default())
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
            .command(nil(), "hi", &TurnOptions::default())
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
            .command(nil(), "hi", &TurnOptions::default())
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
            .resume_command(sid, "follow-up", &TurnOptions::default())
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
            .resume_command(sid, "no thread id yet", &TurnOptions::default())
            .unwrap();
        assert!(!spec.args.iter().any(|a| a == "resume"));
    }

    #[test]
    fn resume_command_fallback_is_identical_to_command() {
        let codex = Codex::new();
        let sid = Uuid::new_v4();
        let resumed = codex
            .resume_command(sid, "x", &TurnOptions::default())
            .unwrap();
        let fresh = codex.command(sid, "x", &TurnOptions::default()).unwrap();
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
            .resume_command(sid, "follow-up", &TurnOptions::default())
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
            .resume_command(sid, "x", &TurnOptions::default())
            .unwrap();
        let fresh = codex.command(sid, "x", &TurnOptions::default()).unwrap();
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
    fn lock_for_path_dedups_equivalent_spellings_for_missing_files() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let base = tmp_dir.path().to_path_buf();
        let a = base.join("missing").join("threads.json");
        let b = base.join(".").join("missing").join("threads.json");
        let c = {
            let mut p = base.clone();
            p.push("./missing/./threads.json");
            p
        };

        let la = lock_for_path(&a);
        let lb = lock_for_path(&b);
        let lc = lock_for_path(&c);

        assert!(
            std::sync::Arc::ptr_eq(&la, &lb),
            "a and b must share a lock"
        );
        assert!(
            std::sync::Arc::ptr_eq(&la, &lc),
            "a and c must share a lock"
        );
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
