//! `rafaello` library: CLI surface and shared types for the `rfl` binary.

pub mod install;
pub mod status;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use parking_lot::RwLock;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};

use rafaello_core::agent::AgentLoop;
use rafaello_core::broker_acl::{self, AttachId, FrontendAcl};
use rafaello_core::bus::Broker;
use rafaello_core::compile::{compile_plugin, CompiledPlugin, EnvPlan};
use rafaello_core::digest::{self, RecomputedDigests};
use rafaello_core::entry::{Entry, EntryFallback, RenderNode, ToolCallStatus};
use rafaello_core::error::{
    BrokerError, CompileError, DigestError, FrontendSpawnError, LockError, ManifestError,
    ReaperOutcome, ValidationError,
};
use rafaello_core::frontend::{
    CompiledFrontend, FrontendConfig, FrontendHandle, FrontendPaths, FrontendSupervisor,
};
use rafaello_core::gate::{ConfirmState, ConfirmationGate};
use rafaello_core::lock::{CanonicalId, Lock};
use rafaello_core::manifest::Manifest;
use rafaello_core::paths::PathContext;
use rafaello_core::reemit::ReemitRouter;
use rafaello_core::renderer::{Capabilities, RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionError, SessionStore};
use rafaello_core::slash::SlashHandler;
use rafaello_core::supervisor::{
    PluginSupervisor, SpawnHandle, SpawnPaths, SupervisorConfig, ToolCatalogError,
    ToolSchemaCatalog,
};
use rafaello_core::topic_id;
use rafaello_core::user_grants::UserGrants;
use rafaello_core::validate::{self, LockValidationContext};

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
    /// Install a local plugin fixture into `${PROJECT_ROOT}/rafaello.lock`.
    Install(install::InstallArgs),
    /// Print a per-plugin status summary from `${PROJECT_ROOT}/rafaello.lock`.
    Status,
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
    #[error("LockNotFound: rafaello.lock missing at {path:?}")]
    LockNotFound { path: PathBuf },
    #[error("LockIo: reading {path:?}: {source}")]
    LockIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("LockParse: {source}")]
    LockParse {
        #[source]
        source: Box<LockError>,
    },
    #[error("LockValidation: {source}")]
    LockValidation {
        #[source]
        source: Box<ValidationError>,
    },
    #[error("NoHomeDir")]
    NoHomeDir,
    #[error("ManifestIo for {canonical}: {source}")]
    ManifestIo {
        canonical: CanonicalId,
        #[source]
        source: std::io::Error,
    },
    #[error("ManifestParse for {canonical}: {source}")]
    ManifestParse {
        canonical: CanonicalId,
        #[source]
        source: Box<ManifestError>,
    },
    #[error("Digest for {canonical}: {source}")]
    Digest {
        canonical: CanonicalId,
        #[source]
        source: DigestError,
    },
    #[error("CompilePlugin for {canonical}: {source}")]
    CompilePlugin {
        canonical: CanonicalId,
        #[source]
        source: Box<CompileError>,
    },
    #[error("ToolCatalog: {0}")]
    ToolCatalog(#[from] Box<ToolCatalogError>),
    #[error("NoActiveProvider")]
    NoActiveProvider,
    #[error("ProviderSpawnFailed for {canonical}")]
    ProviderSpawnFailed { canonical: CanonicalId },
    #[error("ToolSpawnFailed for {canonical}")]
    ToolSpawnFailed { canonical: CanonicalId },
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

fn plugin_spawn_paths(project_root: &Path, topic_id: &str) -> SpawnPaths {
    SpawnPaths {
        project_root: project_root.to_path_buf(),
        private_state_dir: project_root.join(".rafaello-plugin-data").join(topic_id),
    }
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
    // c37/c38 follow-up: forward the c37 confirm-answer + grant-before-message
    // hooks so c39's demo-bar test can drive the TUI end-to-end.
    "RFL_TUI_TEST_CONFIRM_ANSWER",
    "RFL_TUI_TEST_CONFIRM_DELAY_MS",
    "RFL_TUI_TEST_GRANT_BEFORE_MESSAGE",
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

    // Step 1b: lock load (scope §C1).
    let lock_path = project_root.join("rafaello.lock");
    let raw_lock = match std::fs::read_to_string(&lock_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(RflChatError::LockNotFound { path: lock_path });
        }
        Err(source) => {
            return Err(RflChatError::LockIo {
                path: lock_path,
                source,
            });
        }
    };
    let lock = Lock::from_toml(&raw_lock).map_err(|source| RflChatError::LockParse {
        source: Box::new(source),
    })?;

    // Step 1c: V3 validation (scope §C2).
    let install_root = project_root.join(".rafaello").join("plugins");
    let plugin_dirs: BTreeMap<CanonicalId, PathBuf> = lock
        .plugins
        .keys()
        .map(|c| {
            (
                c.clone(),
                install_root.join(topic_id::derive(&c.to_string())),
            )
        })
        .collect();
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(RflChatError::NoHomeDir)?;
    let cache_root = project_root.join(".rafaello").join("cache");
    let state_root = project_root.join(".rafaello").join("state");
    let val_ctx = LockValidationContext {
        project_root: project_root.clone(),
        home: home.clone(),
        plugin_dirs: plugin_dirs.clone(),
        cache_root: cache_root.clone(),
        state_root: state_root.clone(),
    };
    validate::lock(&lock, &val_ctx).map_err(|source| RflChatError::LockValidation {
        source: Box::new(source),
    })?;

    // Step 1d: per-plugin compile (scope §C3).
    let mut compiled_plugins: BTreeMap<CanonicalId, CompiledPlugin> = BTreeMap::new();
    for canonical in lock.plugins.keys() {
        let package_dir = plugin_dirs
            .get(canonical)
            .expect("validate::lock would have errored");
        let manifest_path = package_dir.join("rafaello.toml");
        let manifest_raw =
            std::fs::read_to_string(&manifest_path).map_err(|source| RflChatError::ManifestIo {
                canonical: canonical.clone(),
                source,
            })?;
        let manifest =
            Manifest::parse(&manifest_raw).map_err(|source| RflChatError::ManifestParse {
                canonical: canonical.clone(),
                source: Box::new(source),
            })?;
        let canonical_bytes: Vec<u8> = manifest.canonical_bytes();
        let recomputed_digests = RecomputedDigests {
            content: digest::content_digest(package_dir).map_err(|source| {
                RflChatError::Digest {
                    canonical: canonical.clone(),
                    source,
                }
            })?,
            manifest: digest::manifest_digest(&canonical_bytes),
        };
        let topic = topic_id::derive(&canonical.to_string());
        let path_ctx = PathContext {
            project_root: project_root.clone(),
            home: home.clone(),
            plugin_dir: package_dir.clone(),
            cache_dir: cache_root.join(&topic),
            state_dir: state_root.join(&topic),
        };
        let plan =
            compile_plugin(&lock, canonical, &path_ctx, &recomputed_digests).map_err(|source| {
                RflChatError::CompilePlugin {
                    canonical: canonical.clone(),
                    source: Box::new(source),
                }
            })?;
        compiled_plugins.insert(canonical.clone(), plan);
    }

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

    // Step C4: BrokerAcl from lock + tui frontend extension.
    let mut acl =
        broker_acl::compile(&lock).expect("V3 validated; broker_acl::compile cannot fail");
    let attach = AttachId::new("tui").expect("attach id 'tui' is well-formed");
    let mut subscribe_patterns = BTreeSet::new();
    subscribe_patterns.insert("core.session.**".to_string());
    subscribe_patterns.insert("core.lifecycle.**".to_string());
    let mut publish_topics = BTreeSet::new();
    publish_topics.insert("frontend.tui.user_message".to_string());
    publish_topics.insert("frontend.tui.confirm_answer".to_string());
    publish_topics.insert("frontend.tui.slash_command".to_string());
    acl.frontends.insert(
        attach,
        FrontendAcl {
            subscribe_patterns,
            auto_subscribes: BTreeSet::new(),
            publish_topics,
        },
    );

    // Step C4b: tool-schema catalog (scope §OP2 items 1, 7) — built
    // from each plugin's `openrpc.json` before any spawn so the
    // provider's `core.tools_list` call resolves against a fully
    // populated catalog.
    let tool_catalog = Arc::new(
        ToolSchemaCatalog::build(&acl, &compiled_plugins, &plugin_dirs)
            .map_err(|e| RflChatError::ToolCatalog(Box::new(e)))?,
    );

    // Step C5: broker → plugin supervisor → frontend supervisor → registry → pipeline → controller.
    let acl_for_routing = acl.clone();
    let acl_arc = Arc::new(acl.clone());
    let broker = Broker::new(acl).map_err(|e| RflChatError::Broker(Box::new(e)))?;
    let plugin_supervisor = PluginSupervisor::new(
        broker.clone(),
        SupervisorConfig::default(),
        tool_catalog.clone(),
    );

    // c38 / scope §CHAT1: pre-spawn wiring of confirmation + slash
    // surfaces. The gate must be subscribed BEFORE any plugin spawns
    // so the first `core.session.tool_request` reaches it.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let user_grants = Arc::new(RwLock::new(UserGrants::new()));
    let confirm_state = Arc::new(ConfirmState::new());

    let supervisor = FrontendSupervisor::new(broker.clone(), FrontendConfig::default());
    let registry = Arc::new(RendererRegistry::with_builtins());
    let pipeline = RenderPipeline::new(registry);
    let controller = Arc::new(SessionController::new(store, pipeline, broker.clone()));
    let caps = Capabilities::tui_default();
    let audit = controller.audit_writer();

    let slash_schemas = load_grant_match_schemas(&lock, &plugin_dirs);
    let slash_handler = SlashHandler::new(
        broker.clone(),
        acl_arc,
        user_grants.clone(),
        audit.clone(),
        slash_schemas,
        shutdown_rx.clone(),
    );
    let slash_join = slash_handler.start();

    let provider_str = lock
        .session
        .provider_active
        .clone()
        .ok_or(RflChatError::NoActiveProvider)?;
    let provider_canonical =
        CanonicalId::parse(&provider_str).map_err(|_| RflChatError::NoActiveProvider)?;
    let _ = compiled_plugins
        .get(&provider_canonical)
        .ok_or(RflChatError::NoActiveProvider)?;

    let gate = ConfirmationGate::new(
        Arc::new(broker.clone()),
        user_grants.clone(),
        audit.clone(),
        confirm_state.clone(),
        compiled_plugins.clone(),
    );
    let gate_join = gate.spawn();

    // Steps C6–C8: eager-spawn the active provider first, then every
    // other plugin with `bindings.provider = true` (inactive
    // providers — pi-3 M-2 + pi-4 M-1), then every tool plugin.
    let mut spawn_handles: Vec<SpawnHandle> = Vec::new();
    let provider_plan = compiled_plugins
        .get(&provider_canonical)
        .expect("active provider validated above");
    let provider_paths = plugin_spawn_paths(&project_root, &provider_plan.topic_id);
    let provider_handle = plugin_supervisor
        .spawn(provider_plan, &provider_paths)
        .await
        .map_err(|_| RflChatError::ProviderSpawnFailed {
            canonical: provider_canonical.clone(),
        })?;
    spawn_handles.push(provider_handle);

    // c38 / scope §CHAT2 + pi-3 M-2: spawn every other provider
    // (installed-but-not-active). The active provider's canonical id
    // is compared (not its `provider_id`). Inactive provider events
    // (e.g. `provider.mock.**`) are outside `ReemitRouter`'s
    // subscribe scope and so are silently dropped.
    for (canonical, entry) in &lock.plugins {
        if !entry.bindings.provider || *canonical == provider_canonical {
            continue;
        }
        let plan = compiled_plugins
            .get(canonical)
            .expect("compiled_plugins covers every lock entry");
        let paths = plugin_spawn_paths(&project_root, &plan.topic_id);
        let h = plugin_supervisor.spawn(plan, &paths).await.map_err(|_| {
            RflChatError::ProviderSpawnFailed {
                canonical: canonical.clone(),
            }
        })?;
        spawn_handles.push(h);
    }

    for (canonical, entry) in &lock.plugins {
        if entry.bindings.tools.is_empty() || entry.bindings.provider {
            continue;
        }
        let plan = compiled_plugins
            .get(canonical)
            .expect("compiled_plugins covers every lock entry");
        let paths = plugin_spawn_paths(&project_root, &plan.topic_id);
        let h = plugin_supervisor.spawn(plan, &paths).await.map_err(|_| {
            RflChatError::ToolSpawnFailed {
                canonical: canonical.clone(),
            }
        })?;
        spawn_handles.push(h);
    }

    // Steps C9–C10: ReemitRouter + AgentLoop spawned BEFORE the TUI
    // starts, so the user_message → reemit → tool_request → gate →
    // dispatch → tool_result chain is wired before any input
    // arrives. The router opts into the §CT5 confirm_answer arm via
    // `with_confirm_state_and_audit` (c14 builder).
    let router = ReemitRouter::new(
        broker.clone(),
        acl_for_routing.clone(),
        provider_canonical.clone(),
        shutdown_rx.clone(),
    )
    .with_confirm_state_and_audit(confirm_state.clone(), audit.clone());
    let router_join = router.start();

    let agent = AgentLoop::new(
        broker.clone(),
        acl_for_routing,
        controller.clone(),
        caps.clone(),
        shutdown_rx.clone(),
    );
    let agent_join = agent.start();

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

    // Step 7: wait_ready (5s timeout) + post-ready wait; both race against
    // ctrl_c to trigger the shared shutdown sequence at the bottom.
    let inner = run_chat_after_spawns(
        handle,
        forwarder,
        &stderr_writer_lock,
        &controller,
        &broker,
        &caps,
    )
    .await;

    // Steps C11–C12: signal shutdown → join router + agent + slash
    // handler → drop plugin handles → plugin supervisor shutdown.
    // The gate task exits when the broker subscription channel
    // closes during the supervisor shutdown that drops the broker's
    // last registration; we abort it explicitly to bound shutdown
    // latency.
    let _ = shutdown_tx.send(true);
    let _ = router_join.await;
    let _ = agent_join.await;
    let _ = slash_join.await;
    gate_join.abort();
    let _ = gate_join.await;
    drop(spawn_handles);
    let _shutdown_report = plugin_supervisor.shutdown().await;

    inner
}

/// Build the per-tool `grant_match` JSON Schema map consumed by
/// [`SlashHandler`] at `/grant` time. Walks every lock entry,
/// resolves each `tool_meta.grant_match` path against its plugin
/// dir, and loads + parses the file. Failures are logged at
/// `tracing::warn!` and the tool is omitted from the schema map
/// (treated as "no schema" — `compile_template` then accepts any
/// shape).
fn load_grant_match_schemas(
    lock: &Lock,
    plugin_dirs: &BTreeMap<CanonicalId, PathBuf>,
) -> BTreeMap<String, Value> {
    let mut out: BTreeMap<String, Value> = BTreeMap::new();
    for (canonical, entry) in &lock.plugins {
        let Some(pkg_dir) = plugin_dirs.get(canonical) else {
            continue;
        };
        for (tool, meta) in &entry.bindings.tool_meta {
            let Some(rel) = meta.grant_match.as_ref() else {
                continue;
            };
            let path = pkg_dir.join(rel.as_str());
            match std::fs::read_to_string(&path) {
                Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                    Ok(v) => {
                        out.insert(tool.clone(), v);
                    }
                    Err(err) => {
                        tracing::warn!(
                            tool = %tool,
                            path = ?path,
                            error = %err,
                            "rfl chat: grant_match schema failed to parse; omitting"
                        );
                    }
                },
                Err(err) => {
                    tracing::warn!(
                        tool = %tool,
                        path = ?path,
                        error = %err,
                        "rfl chat: grant_match schema unreadable; omitting"
                    );
                }
            }
        }
    }
    out
}

async fn run_chat_after_spawns(
    mut handle: FrontendHandle,
    forwarder: Option<tokio::task::JoinHandle<()>>,
    stderr_writer_lock: &Arc<tokio::sync::Mutex<()>>,
    controller: &Arc<SessionController>,
    broker: &Broker,
    caps: &Capabilities,
) -> Result<(), RflChatError> {
    let ready = tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => ReadyOutcome::CtrlC,
        res = tokio::time::timeout(Duration::from_secs(5), handle.wait_ready()) => {
            match res {
                Ok(Ok(())) => ReadyOutcome::Ready,
                Ok(Err(_)) => ReadyOutcome::SenderDropped,
                Err(_) => ReadyOutcome::Timeout,
            }
        }
    };

    match ready {
        ReadyOutcome::Ready => {
            write_parent_sentinel(stderr_writer_lock, "rfl-chat: frontend-ready-observed").await;
            run_post_ready(handle, forwarder, controller, broker, caps).await
        }
        ReadyOutcome::CtrlC => {
            let _ = handle.shutdown().await;
            if let Some(j) = forwarder {
                let _ = j.await;
            }
            Ok(())
        }
        ReadyOutcome::SenderDropped => {
            let _ = handle.shutdown().await;
            if let Some(j) = forwarder {
                let _ = j.await;
            }
            Err(RflChatError::FrontendExitedBeforeReady {
                reason: "ready-watch sender dropped before ready was signalled".to_string(),
            })
        }
        ReadyOutcome::Timeout => {
            let _ = handle.shutdown().await;
            if let Some(j) = forwarder {
                let _ = j.await;
            }
            Err(RflChatError::FrontendReadyTimeout)
        }
    }
}

enum ReadyOutcome {
    Ready,
    CtrlC,
    SenderDropped,
    Timeout,
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
    controller: &Arc<SessionController>,
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
        let exited_via_ctrl_c = tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => true,
            outcome = handle.wait() => {
                step10_outcome = Some(map_outcome(&outcome));
                false
            }
        };
        if exited_via_ctrl_c {
            step10_outcome = Some(Ok(()));
        }
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
            RflChatCommand::Install(args) => match install::run(args) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("rfl-install: {err}");
                    ExitCode::FAILURE
                }
            },
            RflChatCommand::Status => match status::run() {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("rfl-status: {err}");
                    ExitCode::FAILURE
                }
            },
        }
    })
}
