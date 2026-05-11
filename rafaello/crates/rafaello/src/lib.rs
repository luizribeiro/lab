//! `rafaello` library: CLI surface and shared types for the `rfl` binary.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};

use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::EnvPlan;
use rafaello_core::entry::{Entry, EntryFallback, RenderNode, ToolCallStatus};
use rafaello_core::error::{BrokerError, FrontendSpawnError, ReaperOutcome};
use rafaello_core::frontend::{
    CompiledFrontend, FrontendConfig, FrontendHandle, FrontendPaths, FrontendSupervisor,
};
use rafaello_core::renderer::{Capabilities, RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionError, SessionStore};

#[derive(Debug, Parser)]
#[command(name = "rfl", version, about = "rafaello — minimal coding agent")]
pub struct RflChatCli {
    #[command(subcommand)]
    pub command: RflChatCommand,
}

#[derive(Debug, Subcommand)]
pub enum RflChatCommand {
    /// Start an interactive chat session.
    Chat {
        /// Project root directory. Defaults to the current working directory.
        #[arg(long)]
        project_root: Option<PathBuf>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RflChatError {
    #[error("project root invalid at {path:?}: {source}")]
    ProjectRootInvalid {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("rfl-tui path unresolved (tried RFL_TUI_PATH env, then sibling of current_exe)")]
    TuiPathUnresolved,
    #[error("FrontendExitedBeforeReady: {reason}")]
    FrontendExitedBeforeReady { reason: String },
    #[error("FrontendReadyTimeout")]
    FrontendReadyTimeout,
    #[error("FrontendExitedAbnormally: {reason}")]
    FrontendExitedAbnormally { reason: String },
    #[error("session: {0}")]
    Session(#[from] Box<SessionError>),
    #[error("frontend spawn: {0}")]
    Spawn(#[from] Box<FrontendSpawnError>),
    #[error("broker: {0}")]
    Broker(#[from] Box<BrokerError>),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve the path to the `rfl-tui` binary.
///
/// Two-level lookup:
/// 1. `RFL_TUI_PATH` env-var override (used by tests and non-installed targets).
/// 2. Sibling of `current_exe()` — the canonical installed-binary location.
pub fn resolve_tui_path(
    env: &dyn Fn(&str) -> Option<OsString>,
    current_exe: &Path,
) -> Result<PathBuf, RflChatError> {
    if let Some(value) = env("RFL_TUI_PATH") {
        return Ok(PathBuf::from(value));
    }
    if let Some(parent) = current_exe.parent() {
        let sibling = parent.join("rfl-tui");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    Err(RflChatError::TuiPathUnresolved)
}

const ENV_PASS_ALLOWLIST: &[&str] = &[
    "TERM",
    "COLORTERM",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "RFL_TUI_TEST_MODE",
    "RFL_TUI_READY_DELAY_MS",
    "RFL_TUI_MAX_LIFETIME",
    "RFL_TUI_TEST_MESSAGE",
    "RFL_FIXTURE_MODE",
    "RFL_FIXTURE_MAX_LIFETIME",
    "RFL_FIXTURE_EXIT_CODE",
];

/// Orchestrate `rfl chat` per scope §C2 steps 1–7 (c29 temporary tail
/// returns `Ok(())` on `Ok(Ok(()))` from `wait_ready`).
pub async fn run_chat(project_root: Option<PathBuf>) -> Result<(), RflChatError> {
    // Step 1: project root.
    let raw = match project_root {
        Some(p) => p,
        None => std::env::current_dir().map_err(|source| RflChatError::ProjectRootInvalid {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let project_root = raw
        .canonicalize()
        .map_err(|source| RflChatError::ProjectRootInvalid {
            path: raw.clone(),
            source,
        })?;

    // Step 2: rfl-tui path.
    let current_exe = std::env::current_exe()?;
    let tui_path = resolve_tui_path(&|k| std::env::var_os(k), &current_exe)?;

    // Step 3: SessionStore (flock first).
    let state_dir = project_root.join(".rafaello").join("state");
    let store = match SessionStore::open(&state_dir) {
        Ok(s) => s,
        Err(SessionError::Locked { holder_pid }) => {
            print_lock_held(holder_pid);
            return Err(RflChatError::Session(Box::new(SessionError::Locked {
                holder_pid,
            })));
        }
        Err(e) => return Err(RflChatError::Session(Box::new(e))),
    };

    // Step 4: BrokerAcl.
    let attach = AttachId::new("tui").expect("attach id 'tui' is well-formed");
    let mut subscribe_patterns = BTreeSet::new();
    subscribe_patterns.insert("core.session.**".to_string());
    subscribe_patterns.insert("core.lifecycle.**".to_string());
    let mut publish_topics = BTreeSet::new();
    publish_topics.insert("frontend.tui.user_message".to_string());
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach,
        FrontendAcl {
            subscribe_patterns,
            auto_subscribes: BTreeSet::new(),
            publish_topics,
        },
    );
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends,
    };

    // Step 5: broker → supervisor → registry → pipeline → controller → CompiledFrontend.
    let broker = Broker::new(acl).map_err(|e| RflChatError::Broker(Box::new(e)))?;
    let supervisor = FrontendSupervisor::new(broker.clone(), FrontendConfig::default());
    let registry = Arc::new(RendererRegistry::with_builtins());
    let pipeline = RenderPipeline::new(registry);
    let controller = SessionController::new(store, pipeline, broker.clone());
    let caps = Capabilities::tui_default();

    let pass: Vec<String> = ENV_PASS_ALLOWLIST
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let compiled = CompiledFrontend {
        attach_id: "tui".to_string(),
        entry_absolute: tui_path,
        argv: Vec::new(),
        env: EnvPlan {
            pass,
            set: BTreeMap::new(),
        },
    };

    // Step 6: spawn TUI + stderr forwarder.
    let paths = FrontendPaths {
        project_root: project_root.clone(),
    };
    let mut handle = supervisor
        .spawn(&compiled, &paths)
        .await
        .map_err(|e| RflChatError::Spawn(Box::new(e)))?;

    let stderr_writer_lock = Arc::new(tokio::sync::Mutex::new(()));
    let forwarder = handle.take_child_stderr().map(|stderr| {
        let lock = stderr_writer_lock.clone();
        tokio::spawn(forward_child_stderr(stderr, lock))
    });

    // Step 7: wait_ready with 5s timeout.
    let outcome = tokio::time::timeout(Duration::from_secs(5), handle.wait_ready()).await;

    match outcome {
        Ok(Ok(())) => {
            write_parent_sentinel(&stderr_writer_lock, "rfl-chat: frontend-ready-observed").await;
            run_post_ready(handle, forwarder, &controller, &broker, &caps).await
        }
        Ok(Err(_ready_err)) => {
            let _ = handle.shutdown().await;
            if let Some(j) = forwarder {
                let _ = j.await;
            }
            Err(RflChatError::FrontendExitedBeforeReady {
                reason: "ready-watch sender dropped before ready was signalled".to_string(),
            })
        }
        Err(_elapsed) => {
            let _ = handle.shutdown().await;
            if let Some(j) = forwarder {
                let _ = j.await;
            }
            Err(RflChatError::FrontendReadyTimeout)
        }
    }
}

fn print_lock_held(holder_pid: Option<u32>) {
    let mut stderr = std::io::stderr().lock();
    let _ = match holder_pid {
        Some(pid) => writeln!(
            stderr,
            "session lock held by pid {pid}; another rfl chat is running for this project."
        ),
        None => writeln!(
            stderr,
            "session lock held by an unknown process; remove `.rafaello/state/session.lock` if no other rfl chat is running for this project."
        ),
    };
    let _ = stderr.flush();
}

async fn forward_child_stderr(
    stderr: tokio::process::ChildStderr,
    writer_lock: Arc<tokio::sync::Mutex<()>>,
) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _g = writer_lock.lock().await;
        let mut out = std::io::stderr().lock();
        let _ = writeln!(out, "rfl-tui: {line}");
        let _ = out.flush();
    }
}

async fn write_parent_sentinel(writer_lock: &Arc<tokio::sync::Mutex<()>>, line: &str) {
    let _g = writer_lock.lock().await;
    let mut out = std::io::stderr().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

async fn run_post_ready(
    mut handle: FrontendHandle,
    forwarder: Option<tokio::task::JoinHandle<()>>,
    controller: &SessionController,
    broker: &Broker,
    caps: &Capabilities,
) -> Result<(), RflChatError> {
    let mut step10_outcome: Option<Result<(), String>> = None;
    let result: Result<(), RflChatError> = async {
        controller
            .replay_history(caps)
            .await
            .map_err(|e| RflChatError::Session(Box::new(e)))?;
        if std::env::var_os("RFL_HARNESS_FIXTURES").as_deref() == Some(std::ffi::OsStr::new("1")) {
            run_fixture_harness(controller, broker, caps).await?;
        }
        let outcome = handle.wait().await;
        step10_outcome = Some(map_outcome(&outcome));
        Ok(())
    }
    .await;

    let _report = handle.shutdown().await;
    if let Some(j) = forwarder {
        let _ = j.await;
    }

    match (result, step10_outcome) {
        (Ok(()), Some(Ok(()))) => Ok(()),
        (Ok(()), Some(Err(reason))) => Err(RflChatError::FrontendExitedAbnormally { reason }),
        (Err(e), _) => Err(e),
        _ => unreachable!("step10_outcome must be set whenever result is Ok"),
    }
}

fn map_outcome(outcome: &ReaperOutcome) -> Result<(), String> {
    match outcome {
        ReaperOutcome::Exited(status) if status.success() => Ok(()),
        ReaperOutcome::Exited(status) => Err(format!(
            "frontend exited abnormally: code={:?} signal={:?}",
            status.code(),
            status.signal()
        )),
        ReaperOutcome::WaitFailed(e) => Err(format!(
            "frontend wait failed: errno={:?} ({e})",
            e.raw_os_error()
        )),
        ReaperOutcome::ReaperPanicked => {
            Err("frontend reaper panicked before producing an outcome".to_string())
        }
        _ => Err("frontend produced an unrecognised reaper outcome".to_string()),
    }
}

async fn run_fixture_harness(
    controller: &SessionController,
    broker: &Broker,
    caps: &Capabilities,
) -> Result<(), RflChatError> {
    let entries = vec![
        Entry::new_text("Hello m3."),
        Entry::new_heading(1, "m3 demo"),
        Entry::new_code_block("fn main() {}", Some("rust")),
        Entry::new_thinking("planning the next step"),
        Entry::new_tool_call(
            "call-1",
            "read_file",
            json!({"path": "Cargo.toml"}),
            ToolCallStatus::Ok,
        ),
        Entry::new_tool_result(
            "call-1",
            true,
            RenderNode::Text {
                text: "ok".into(),
                emphasis: None,
            },
        ),
        Entry::new_image("file:///fake.png", "image/png", "alt text"),
        Entry::new_error("E001", "synthetic error"),
        Entry::new_unknown(
            "myorg:custom",
            json!({"foo": "bar"}),
            EntryFallback {
                text: "unknown demo".into(),
                markdown: None,
                summary: None,
            },
        ),
    ];
    for entry in entries {
        controller
            .finalize_entry(entry, caps)
            .await
            .map_err(|e| RflChatError::Session(Box::new(e)))?;
    }
    broker
        .publish_core("core.lifecycle.test_done", json!({}))
        .map_err(|e| RflChatError::Broker(Box::new(e)))?;
    Ok(())
}

pub fn run_cli() -> ExitCode {
    let cli = RflChatCli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("rfl-chat: failed to build tokio runtime: {err}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async move {
        match cli.command {
            RflChatCommand::Chat { project_root } => match run_chat(project_root).await {
                Ok(()) => ExitCode::SUCCESS,
                Err(RflChatError::Session(boxed))
                    if matches!(*boxed, SessionError::Locked { .. }) =>
                {
                    ExitCode::FAILURE
                }
                Err(err) => {
                    eprintln!("rfl-chat: {err:?}");
                    ExitCode::FAILURE
                }
            },
        }
    })
}
