# m3 — sessions, local-spawned TUI, built-in rendering — scope

> **Status:** round-20 draft. Trajectory of finding
> counts (b/h/m/l): r1 10/-/-/3, r2 5/-/2/-, r3 3/-/3/-,
> r4 3/3/0/1, r5 2/-/11/3, r6 6/-/5/3, r7 5/-/3/1,
> r8 3/-/5/0, r9 3/2/3/2, r10 1/2/2/1, r11 1/3/1/1,
> r12 0/2/2/1, r13 1/1/2/1, r14 2/3/2/0, r15 1/1/3/0,
> r16 0/3/2/2, r17 0/2/2/1, r18 1/2/2/0,
> r19 1/0/2/0. Round 20 highlights:
> - Step-10 explicit shutdown+drain block fully
>   replaced with a forward reference to the
>   cleanup guard (pi-19 #1).
> - Lock-side `check_lock_publish_topic` deferral
>   added to anticipated-drift list in Acceptance
>   summary (pi-19 #2).
> - Post-spawn-pre-register unwind test split into
>   cross-platform (`unwinds_post_spawn_pre_register.rs`,
>   asserts hook consumed + no broker registration
>   + in_flight cleared) and Linux-only
>   (`unwinds_post_spawn_pre_register_fd_baseline.rs`,
>   asserts `/proc/self/fd` returns to baseline)
>   (pi-19 #3).
>
> Round-19 highlights (kept for trajectory context):
> - Cleanup guard is now the SOLE teardown path;
>   step 10 only reads outcome / returns errors,
>   never calls shutdown directly (pi-18 #1).
> - H6 gains a third inject point
>   `inject_post_spawn_pre_register_fault` for the
>   distinct ownership state where Child exists but
>   no broker registration / reaper task; new
>   matching test (pi-18 #2).
> - §B4 frontend publish: 2-segment `frontend.<id>`
>   bare topic rejected as `PublishOnReservedNamespace`
>   (symmetric to m2 §B3 plugin two-segment rule);
>   new negative test (pi-18 #3).
> - §M1 lock-side validation explicitly NOT
>   tightened in m3; recorded as anticipated drift
>   (pi-18 #4).
> - Manual validation command aligned with
>   acceptance gate (pi-18 #5).
>
> Round-18 highlights (kept for trajectory context):
> - §C2 cleanup-guard contract spec'd: every
>   fallible call after TUI spawn runs canonical
>   shutdown + stderr drain via a guard pattern
>   (pi-17 #1).
> - H6 post-register inject point pinned in
>   code-order terms ("after watcher/reaper spawn,
>   before serve install"); round-17's inverted
>   reasoning about pre-register window corrected
>   (pi-17 #2).
> - §F3 step 5 added: create per-frontend private
>   state dir before spawning the child (pi-17 #3).
> - §C2 step 1 canonicalises project root via
>   `Path::canonicalize`; new `ProjectRootInvalid`
>   error variant; two new CLI tests
>   (relative + nonexistent) (pi-17 #4).
> - §B4 frontend-alone wording corrected — handled
>   by `validate_topic` first, not B4 (pi-17 #5).
>
> Round-17 highlights (kept for trajectory context):
> - H6 inject points renamed/clarified:
>   `inject_pre_spawn_fault` (post-socketpair, pre-
>   `tokio_command.spawn`) and
>   `inject_post_register_fault` (post-register, pre-
>   serve-install). Maps to m2 retro §3.3's two
>   distinct unwind windows (pi-16 #1).
> - §M1 simplified to a minimal additive patch:
>   keep existing m1 `PublishOnReservedNamespace` /
>   `PublishOnFrontendNamespace` /
>   `ProviderNamespaceMismatch`; add only one new
>   variant `PublishUnknownNamespace` for truly
>   unknown top-level segments (`evil.foo`).
>   Fixes the round-9-through-16 invented
>   variant/file-name conflicts (pi-16 #2 + #3).
> - Headless TUI log format pinned: human-readable
>   sentinel lines, NOT JSON (pi-16 #4).
> - `tui_sends_frontend_ready_after_handler_registration.rs`
>   uses deterministic callback signals, not
>   wall-clock 1-ms timing (pi-16 #5).
> - Manual validation drops Ctrl+C (raw mode)
>   (pi-16 #6).
> - Banner shutdown-seam signature aligned with
>   detailed split form (pi-16 #7).
>
> Round-16 highlights (kept for trajectory context):
> - shutdown_with_outcome seam signature uses
>   `FnMut(Pid, Signal)` for signals + a separate
>   `FnMut(Pid)` probe (split form, not the
>   Option<Signal> form pi-15 first proposed) so
>   `Fn(Pid)` probe so `kill(pid, 0)` is representable;
>   tests assert exact signal/probe call sequences
>   per branch (pi-15 #1).
> - Stale "intentionally untested" para deleted from
>   §C2 step 10 — only the new strict-coverage para
>   remains (pi-15 #2).
> - Drop summary line replaced with a forward
>   reference to the cached-outcome rules (pi-15 #3).
> - `rfl chat` Locked-error message format spec'd
>   for both `Some(pid)` and `None` cases; new test
>   for unknown-holder branch (pi-15 #4).
> - `workspace_bin_path` helper added to
>   rafaello/tests/common/ for cross-crate binary
>   resolution (pi-15 #5).
>
> Round-15 highlights (kept for trajectory context):
> - Drop's skip-kill aligned with shutdown: only
>   Exited(_) skips; WaitFailed/ReaperPanicked still
>   best-effort SIGKILL (pi-14 #1).
> - §C2 step 10 reordered: shutdown FIRST, then
>   stderr drain. WaitFailed/ReaperPanicked no
>   longer cause infinite stderr-drain hangs
>   (pi-14 #2).
> - Shutdown algorithm split explicitly into
>   live-watch and dead-watch branches; SIGCONT
>   probe replaced with `kill(pid, 0)` (no-op
>   liveness probe); shutdown_kill_grace correctly
>   used for the post-SIGKILL wait (pi-14 #3 + #4
>   + #5).
> - Reaper-outcome injection seam added: shutdown
>   algorithm extracted as `shutdown_with_outcome`
>   pure async function; tests
>   `frontend_shutdown_dead_watch_paths.rs` cover
>   WaitFailed and ReaperPanicked with mock kill_fn
>   (pi-14 #6).
> - `SessionError::Locked.holder_pid` is now
>   `Option<u32>` (None on unknown holder); new
>   negative test for empty/corrupt lockfile
>   (pi-14 #7).
>
> Round-14 highlights (kept for trajectory context):
> - shutdown skip-kill is now Exited(_)-only;
>   WaitFailed/ReaperPanicked proceed with bounded
>   SIGTERM+SIGKILL using `kill(SIGCONT)` ESRCH
>   probes since the watch is dead (pi-13 #1).
> - SIGKILL-after-SIGTERM-grace re-checks the cached
>   outcome to close the recycled-PID race (pi-13 #2).
> - §C2 step 7 wording on shutdown grace fixed —
>   refers to "the handle's FrontendConfig" instead
>   of inappropriate `self.config` (pi-13 #3).
> - `RFL_TUI_MAX_LIFETIME=2` added to the env-override
>   end-to-end test (pi-13 #4).
> - Fixture-mode count corrected to "Five new modes"
>   / "All five modes" (pi-13 #5).
>
> Round-13 highlights (kept for trajectory context):
> - Reaper-watcher task added to §F3 step 10 to
>   produce `ReaperPanicked` outcomes via JoinHandle
>   bridging (pi-12 #1).
> - `ShutdownReport.exit_status` population spec'd
>   from `reaper_outcome.borrow()` after the
>   SIGTERM/KILL flow (pi-12 #2).
> - `RFL_FIXTURE_EXIT_CODE` added to the env.pass
>   allowlist in §C2 step 5 (pi-12 #3).
> - `peer: PeerHandle` added to FrontendHandle
>   struct sketch (pi-12 #4).
> - `self.config.shutdown_*` qualified throughout;
>   stale `flock(LOCK_EX | LOCK_NB)` shorthand
>   replaced with `Flock::lock` invocation (pi-12 #5).
>
> Round-12 highlights (kept for trajectory context):
> - `FrontendHandle.config: FrontendConfig` field
>   added so `shutdown` can read the grace durations
>   (pi-11 #1).
> - `SessionStore.lock_guard: Flock<File>` field
>   added so the project flock survives `open()`
>   (pi-11 #2).
> - Stderr forwarder drained in BOTH the SenderDropped
>   and timeout error paths in §C2 step 7, not only
>   in step 10 (pi-11 #3).
> - New `signal_ready_then_exit_n` fixture mode +
>   `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
>   test for the post-ready abnormal-exit branch;
>   round-11 had a false coverage claim (pi-11 #4).
> - Replay-withheld test wording fixed: completes
>   the truncated assertion + drops stale "single
>   forwarding task" wording (pi-11 #5).
> - Stale "Flock is a different helper" aside in §S5
>   deleted (pi-11 #6).
>
> Round-11 highlights (kept for trajectory context):
> - §F3 Phase B re-ordered as numbered 1-14 with
>   stderr-piped BEFORE spawn, reaper task explicitly
>   in the main sequence (pi-10 Blocker 1).
> - Stderr forwarder JoinHandle stored in `rfl chat`
>   state; awaited before §C2 step 10 teardown
>   (pi-10 High 2).
> - `nix::fcntl::Flock` (RAII helper) replaces the
>   deprecated `nix::fcntl::flock` function form
>   (pi-10 High 3).
> - New CLI test
>   `rfl_chat_frontend_exits_before_ready_errors.rs`
>   covering SenderDropped → FrontendExitedBeforeReady
>   path (pi-10 Medium 4).
> - Abnormal `WaitFailed` / `ReaperPanicked` branches
>   marked intentionally untested with rationale
>   (pi-10 Medium 5).
> - W5 reworded: feature-gate setup unchanged; mode
>   dispatch grows with §L1a (pi-10 Low 6).
>
> Round-10 highlights (kept for trajectory context):
> - Stale "Cooperative shutdown" block deleted from §F4
>   (pi-9 Blocker 1 — round 9 left two conflicting
>   shutdown contracts).
> - §C2 step 10 calls `frontend_handle.shutdown()`
>   exactly once across all four exit cases (pi-9
>   Blocker 2).
> - `FrontendHandle.child_stderr: Option<ChildStderr>`
>   + `take_child_stderr()` API (pi-9 Blocker 3 —
>   round 9 spec'd the forwarder without an API to
>   access the stderr handle).
> - Phase B unwind matrix split by reaper-ownership
>   transition: pre-reaper unwind uses `Child` directly,
>   post-reaper unwind uses `nix::sys::signal::kill`
>   + reaper-watch (pi-9 High 4).
> - SenderDropped path uses bounded `handle.shutdown()`
>   instead of unbounded `wait()` (pi-9 High 5).
> - `SessionStore::lock_fd_for_test()` cfg-gated
>   accessor (pi-9 Medium 6).
> - Stderr forwarding uses `tokio::sync::Mutex<Stderr>`
>   for serialised writes; TUI emits raw lines without
>   `rfl-tui:` prefix (the forwarder is the sole prefix
>   source) (pi-9 Medium 7).
> - `ReaperOutcome::Exited(status)` /
>   `WaitFailed(io::Error)` shapes corrected throughout
>   (pi-9 Medium 8).
> - L1a fixture-mode wording / count fixed (Low 9 +
>   Low 10).
>
> Round-9 highlights (kept for trajectory context):
> - §M1 namespace patch made role-aware: provider
>   plugin manifests still publish on
>   `provider.<own-id>.*`; `core.*` and `frontend.*`
>   stay forbidden; unknown namespaces stay
>   `PublishNamespaceUnknown`. `ValidationError`
>   variants instead of `ManifestError` (pi-8 B1).
> - `FrontendHandle::shutdown` checks
>   `reaper_outcome.borrow()` first; if exit already
>   observed, skip SIGTERM/SIGKILL (PID-recycle
>   safety). Drop applies the same check (pi-8 B2).
> - `crates/rafaello/src/lib.rs` added so
>   `resolve_tui_path` is reachable from
>   `rafaello/tests/` (pi-8 B3).
> - `CompiledFrontend.attach_id: String` (round-9 —
>   pi-8 M1: validation moves to spawn-time so the
>   invalid-attach-id test can construct a plan).
> - Phase B unwind matrix spec'd for spawn failures
>   after child spawn (pi-8 M3).
> - `ReaperOutcome::WaitFailed` and
>   `ReaperPanicked` mapped to
>   `RflChatError::FrontendExitedAbnormally`
>   (pi-8 M4).
> - Replay-readiness test uses single combined
>   stderr stream via `rfl chat`'s child-stderr
>   forwarding task (pi-8 M2).
> - `probe_fd_closed` fixture mode uses
>   `nix::fcntl::F_GETFD` instead of `lsof` for
>   cross-platform fd-not-inherited verification
>   (pi-8 M5).
>
> Round-8 highlights (kept for trajectory context):
> - `CompiledFrontend.attach_id: AttachId` (pi-7 #1).
> - `EnvPlan` matches m1's actual shape:
>   `pass: Vec<String>` + `set: BTreeMap<String,String>`,
>   no `clear` field (pi-7 #2).
> - Disarmable `FrontendHandle` — Option<>-wrapped
>   pid/serve/register; `shutdown(mut self)` `take()`s
>   them; Drop is a no-op after shutdown (pi-7 #3).
> - `ReaperOutcome::Exited(ExitStatus)` is the
>   canonical shape; assertion sites use
>   `status.success()` and `status.signal()` rather
>   than a fictional `Signaled` variant (pi-7 #4).
> - `FrontendConfig` fields pinned (pi-7 #5).
> - `RFL_TUI_MAX_LIFETIME=2` in replay-withheld test so
>   the headless TUI self-terminates without test_done
>   (pi-7 #6).
> - `record_subscriber` helper now registers as a
>   plugin via a new `observer` plugin ACL entry
>   (pi-7 #7).
> - `session_store_lock_fd_not_inherited` moved to
>   rafaello-core/tests/ (pi-7 #8).
> - `CompiledFrontend` ACL fields removed; `BrokerAcl`
>   is the single source of truth (pi-7 #9).
>
> Round-7 highlights (kept for trajectory context):
> - §C2 step 5 spec'd `EnvPlan.pass` allowlist for
>   the bundled TUI so test-only knobs survive
>   `env_clear` (B1).
> - `SessionError::Publish { source: BrokerError }`
>   added (B2).
> - Parent-side `frontend-ready-observed` stderr
>   sentinel — replaces the round-6 child-side
>   sentinel that didn't establish ordering against
>   replay (B3).
> - `RendererRegistry::register/new/get` made public
>   (B4).
> - Paint-panic seam uses cfg(test) `PaintAction`
>   enum instead of contaminating `RenderNode`/`RawFormat`
>   (B5).
> - §F4 wait test fixture mode aligned to
>   `signal_ready` everywhere (B6).
> - F1 ownership wording corrected — supervisor is a
>   stateless factory (M1).
> - Out-of-scope macOS language reframed (M2).
> - Post-ready abnormal TUI exit semantics spec'd
>   (M3).
> - `ShutdownReport`, `FrontendReadyError`,
>   `PaintError` types pinned (M4).
> - `RFL_TUI_MAX_LIFETIME` env spec'd in §T2 (M5).
> - L1, L2, L3 cleanups.
>
> Round-6 highlights (kept for trajectory context):
> Round-6 highlights: handle-owned lifecycle model
> resolved (B1 — `FrontendHandle::shutdown`); paint-
> panic seam re-spec'd as a `Painter::draw_with_panic_isolation`
> wrapper unit-tested on `TestBackend` (B2); stale
> oneshot text purged from C2 + Risks #9 (M1); F4 test
> binary aligned to `rfl-bus-fixture` everywhere (M2);
> `RFL_TUI_PATH` end-to-end uses the real `rfl-tui`
> binary (M3); new `hold_silent` fixture mode for the
> ready-timeout test (M4); broker error classification
> distinguishes `UnknownNamespace` vs
> `PublishOnReservedNamespace` for frontends (M5);
> frontend ACL publish_topics validation added (M6);
> `EntryFallback` / `EntryAuthor` / `RawFormat` shapes
> defined (M7); nix `FlockArg` + `nix::libc::O_CLOEXEC`
> (M8); markdown fallback simplified (M9); deterministic
> stderr sentinels for headless TUI (M10); macOS CI is
> a hard ratification gate (M11); typo fix (L1) +
> formatting (L2).
>
> Round-5 highlights (kept for trajectory context):
> - `FrontendHandle._register_guard: RegisteredFrontend`
>   (round 4 spec'd the guard but didn't say where it
>   lived; without ownership the guard would drop at end
>   of `spawn` and immediately unregister the frontend)
>   (pi-4 #1).
> - `FrontendHandle.ready: tokio::sync::watch::Receiver<bool>`
>   replacing oneshot (oneshot is single-shot, can't
>   back idempotent `wait_ready` + non-blocking peek;
>   watch is replayable + supports `borrow`) (pi-4 #2).
> - Naming: canonical `SenderDropped` (one word) (pi-4
>   #3).
> - C2 step 7 maps three outcomes:
>   `Ok` / `Err(SenderDropped)` / timeout → distinct
>   `RflChatError` exits (pi-4 #4).
> - `RFL_CHAT_TEST_OBSERVER_FD` test seam dropped;
>   replay-withheld test observes via TUI headless
>   stderr + new `RFL_TUI_READY_DELAY_MS` env knob
>   (pi-4 #5).
> - `tui_paint_panic_isolation` reframed as a
>   library-level unit test using `ratatui::backend::
>   TestBackend` (headless integration mode never
>   paints, so the integration form was unimplementable)
>   (pi-4 #6).
> - `RFL_TUI_PATH` resolution refactored as a pure
>   `resolve_tui_path(env, current_exe)` testable
>   function with three unit tests + one end-to-end
>   (pi-4 #7).
> Pi convergence pending.
> Round-4 highlights:
> - `FrontendHandle` exposes `wait_ready()` (round 3 had a
>   `oneshot::Sender` inside `FrontendReadyService` with no
>   public surface);
> - `FrontendSupervisor::spawn` now also explicitly spawns
>   the fittings `Server::serve()` task and stores its
>   `JoinHandle` in `FrontendHandle` for shutdown;
> - readiness gate moved out of `SessionController` into
>   `rfl chat` orchestration (pi-3 #2); the corresponding
>   tests moved to `rafaello/tests/`;
> - `frontend_handle_wait_ready_*` tests added at the
>   rafaello-core layer; rafaello-tui's `tui_sends_frontend_ready_after_handler_registration`
>   added;
> - `ulid` features `["serde"]` (pi-3 #3);
> - `option_env!` dropped from `RFL_TUI_PATH` resolution;
>   only env override + sibling lookup remain (pi-3 #4);
> - `rfl` no longer depends on `rafaello-tui` library
>   (pi-3 #6 — TUI is spawned as a subprocess; no library
>   linkage needed).
> - Risks #9 wording updated for the round-3 RPC rename.
> Pi convergence pending.

## Goal

Land the **first user-facing surface** of rafaello: `rfl chat`
opens a terminal UI against a real bus, persists conversation
state to SQLite, and renders entries through the built-in
in-process Rust renderer pipeline. m3 is the structural moment
where m1's data transformation + m2's broker/spawn primitive
become a session: an `rfl` invocation produces a process tree
the user can interact with. Every later milestone (m4 agent
loop + provider plugin, m5 confirmation + sinks) inherits
m3's TUI + session machinery without modification.

The deliverable is:

1. A new in-tree library crate `rafaello-tui` (publishes the
   ratatui-based TUI) and a `[[bin]]` target `rfl-tui` inside
   it. The TUI binary speaks JSON-RPC on `RFL_BUS_FD` exactly
   like a plugin, but is not lockin-sandboxed (decision row
   15: frontends are trusted UI principals).
2. New modules in `rafaello-core`:
   - `frontend` — the local-spawned-frontend supervision path
     (`FrontendSupervisor`), distinct from the
     lockin-sandboxed `PluginSupervisor` because frontends
     bypass lockin.
   - `session` — SQLite-backed entry store under
     `${PROJECT_ROOT}/.rafaello/state/`, plus the project
     lock-file (`session.lock`) that fences concurrent
     `rfl chat` invocations.
   - `renderer` — built-in in-process renderer registry, the
     entry → render-tree pipeline, panic isolation, and
     server-side downgrade.
   - `entry` — the `Entry` ADT (kinds + payloads + metadata
     + fallback) and the `RenderTree` ADT (~14 variants per
     overview §11).
3. Broker extension: `PublisherIdentity::Frontend { attach_id
   }` becomes a live wire variant (m2 stages it as
   commented-out future). Broker ACL gains a `frontends:
   BTreeMap<AttachId, FrontendAcl>` field; frontend publish
   authority enforced symmetrically to plugins (only
   `frontend.<attach-id>.*`; `core.*` / `provider.*` /
   `plugin.*` rejected).
4. `rfl chat` subcommand on the existing `rfl` bin — wires
   `Broker::new` + `FrontendSupervisor` + session store +
   renderer registry into one process tree, spawns the TUI
   child, runs until the TUI exits, persists the session.
5. `TestHooks::inject_fault` mechanism on `PluginSupervisor`
   (m2 retro §5.1, the single largest known coverage gap)
   with two inject points; three deleted m2 unwind tests
   re-added against the mechanism.
6. m1 publishes-grant unknown-namespace parse-time tightening
   (m2 retro §2.8) — small back-reach to m1 in the m3 branch.
7. Integration tests under `rafaello/crates/rafaello-core/tests/`,
   `rafaello/crates/rafaello-tui/tests/`, and
   `rafaello/crates/rafaello/tests/` (the CLI layer —
   the headline `rfl chat` end-to-end test lives here
   per pi-3 #2 / pi-6 L2) exercising the demo bar.

No agent loop, no provider, no tool dispatch in m3. The TUI
runs against an **in-test fixture-entry harness** that injects
static entries through `SessionController::finalize_entry`
(append → render → publish on
`core.session.entry.finalized`); m4 replaces the harness with
the real provider path. (Pi-2 #3 — round 2 had the Goal
contradicting §S/§C; the controller path is canonical and
the only path that hits SQLite + the renderer pipeline.)

### Lock-correspondence claim, extended (m2 §2.6 carryover)

m2's "lock-correspondence is API-level only" claim (m2 retro
§2.6) extends to m3's `FrontendSupervisor`: the supervisor's
public entry point is `spawn(plan: &CompiledFrontend, paths:
&FrontendPaths)`, and `CompiledFrontend` is a
`pub struct` whose fields a caller could hand-mutate. m3 spot-
checks the cases that would crash the underlying spawn — no
control characters in paths, executable exists at the entry
path, no reserved env var collisions in `[env.set]` /
`[env.pass]` — but does NOT prove forge-resistance against a
malicious caller. The retrospective will record this as a v2
nice-to-have, identical reasoning to m2 §2.6.

Frontends do not have a manifest in v1 (the only frontend is
the bundled `rfl-tui`, baked into the workspace), so the
production caller `rfl chat` constructs a `CompiledFrontend`
from compile-time constants. m4+ does not change this.

## Inputs

- `rafaello/plans/overview.md` §3 (process model — TUI is a
  separate process attached over inherited bus socketpair),
  §4 (bus — esp. §4.3 namespaces and §4.4 reserved env
  vars), §10 (frontends + the TUI-only banner), §11
  (renderer model and render tree + the built-in-only +
  final-only banners), §12 (sessions + the
  interactive-only banner), §15.6 (PeerHandle).
- `rafaello/plans/decisions.md` rows **3, 4, 5, 13, 15, 16,
  17, 19, 20, 27, 28, 29, 32, 33, 34, 37, 39, 40**.
- `rafaello/plans/streams/e-renderer/rfc-renderer-model.md`
  end-to-end. v1 reads it through the deferrals pinned in
  rows 28 (no patch ops; `final` only) and 29 (no
  subprocess renderers). The §3 entry shape, §4 render-tree
  shape, §6 fallback rules, and the server-side downgrade
  paragraph in §6 are the m3 contract; §7 streaming patches,
  §8 `frontend.hello`, and §9 subprocess `renderer.render`
  are out of scope. m3's retrospective patches the resulting
  drift (per the m1 banner precedent — the renderer RFC
  drift was already filed by `milestones/README.md`
  §"Stream RFC drift" against m3).
- `rafaello/plans/streams/a-security/rfc-security-model.md`
  §5.7 with its v1-status banner (TUI-as-bus-principal kept;
  external attach deferred). §5.7.1's first bullet
  (local-spawned, fd-passing, broker-bound) is m3's
  authentication contract; §5.7.1's second bullet (UDS
  attach socket, attach token, `rfl serve`), §5.7.2
  (network attach), and §5.7.3's `frontend.<id>.user_message`
  re-emission path are explicitly out of scope.
- `rafaello/plans/glossary.md`.
- m2's `rafaello-core` surface (verified against
  `crates/rafaello-core/src/{bus.rs,supervisor.rs,error.rs}`):
  `Broker`, `BrokerError`, `BusEvent`, `PublisherIdentity`
  (currently `Core | Plugin { canonical, topic_id }`),
  `BrokerAcl`, `PluginAcl`, `PluginSupervisor`, `SpawnHandle`,
  `SpawnError`, `TestHooks` (m2's per-supervisor counter
  struct — m3 extends with `inject_fault`).
- m1's `compile::compile_plugin` + `validate::lock` surfaces
  (the unknown-namespace fix touches `validate::manifest`).
- ratatui + crossterm public API: `ratatui = "0.29"`,
  `crossterm = "0.28"` (verify exact versions at commits.md
  time — the m3 driver picks the latest 0.29.x / 0.28.x
  at draft time and the round-1 commit pins them).
- `rusqlite = "0.32"` with bundled feature, OR `sqlx =
  "0.8"` async driver. m3 picks one; round-1 default is
  `rusqlite` because m3 has no other async-DB consumer and
  `rusqlite` keeps the dependency surface smaller. Pi may
  push back on either choice.

## In scope

Per-commit granularity is the driver's call when drafting
`commits.md`; this section names public API surface and the
test matrix.

### W — workspace dependencies

- **W1 (workspace `Cargo.toml`).** Three concrete edits to
  `rafaello/Cargo.toml` (verified against the live file —
  `chrono` already exists, `tokio` is missing the `process`
  feature, and the `members` list does not yet include the
  new TUI crate):
  - **Edit existing entries.**
    - `tokio.features` adds `"process"` (currently
      `["rt-multi-thread", "macros", "io-util", "net",
      "sync", "time"]`). Required by m3's
      `FrontendSupervisor` (§F3) for `tokio::process::
      Command`.
    - `chrono` already exists as `{ version = "0.4",
      features = ["serde"] }`. m3 leaves it alone — the
      existing entry is sufficient for `DateTime<Utc>`
      serde encode/decode. (Round 1 incorrectly proposed a
      replacement with `default-features = false` +
      `clock`; the existing `default-features = true`
      gives us `clock` and `std` for free, and there is
      no compile-time cost worth chasing in v1.)
  - **Add new entries.**
    - `ratatui = "0.29"` (TUI rendering).
    - `crossterm = "0.28"` (terminal control; ratatui's
      default backend).
    - `rusqlite = { version = "0.32", features =
      ["bundled"] }` (bundles SQLite — no system-sqlite
      dependency; the lockin / devshell stays free of
      system libsqlite3).
    - `tui-input = "0.10"` (line editor for the prompt
      box) — pi may argue against; the alternative is a
      hand-rolled ~80-LoC editor and no dep. m4/m5 will
      need a richer editor either way, so round 2 keeps
      `tui-input`.
    - `unicode-width = "0.2"` (terminal column counts;
      transitive in ratatui but used directly by m3's
      renderer text wrapping).
    - `ulid = { version = "1", features = ["serde"] }`
      (entry ids — overview §11 / Stream E §3 specify
      ULID; pi-3 #3 — round 3 omitted the `serde`
      feature, but the `Entry` derive is `Serialize` /
      `Deserialize` and `Ulid` does not implement
      these without the feature flag).
  - **Add the new workspace member.** `members` currently
    reads `["crates/rafaello", "crates/rafaello-core"]`;
    m3 extends to `["crates/rafaello",
    "crates/rafaello-core", "crates/rafaello-tui"]`.
- **W1 (dev-deps).** No new entries; `tempfile`, `serial_test`,
  `tracing-test`, `tracing-subscriber` already in m2's W1.
  `insta = "1"` is **not** added in m3 — render-tree snapshot
  tests are landed inline as JSON literals rather than
  `insta` snapshots, to avoid a new tooling dep for a small
  number of snapshots.
- **W2.** Edit `rafaello/crates/rafaello-core/Cargo.toml`:
  - `[dependencies]` adds `rusqlite`, `ulid`, `chrono` with
    `workspace = true`. (Renderer types live in
    rafaello-core; `ratatui`/`crossterm` do NOT — those
    belong to `rafaello-tui` only.)
- **W3.** New crate `rafaello/crates/rafaello-tui/`:
  - `[package] name = "rafaello-tui"`, `version = "0.0.0"`,
    `edition = "2021"`.
  - `[lib]` for unit-testable widgets; `[[bin]] name =
    "rfl-tui", path = "src/bin/rfl_tui.rs"`.
  - `[dependencies]`: `rafaello-core` (path-dep), `ratatui`,
    `crossterm`, `tui-input`, `tokio`, `tracing`,
    `fittings-core`, `fittings-server`, `fittings-client`,
    `fittings-transport`, `serde`, `serde_json`,
    `async-trait`, `anyhow`, `unicode-width`.
  - `[dev-dependencies]`: `tempfile`, `serial_test`,
    `tracing-test` with `workspace = true`.
- **W4.** Edit `rafaello/crates/rafaello/Cargo.toml`:
  - **Add a `[lib]` target** (pi-8 B3 — round 8 spec'd
    `pub fn resolve_tui_path` for use from
    `rafaello/tests/`, but binary-only crates do not
    expose public items to integration tests. Round 9
    adds a library target so `resolve_tui_path` and
    other CLI helpers are reachable from
    `rafaello/tests/*.rs`). Layout:
    - `crates/rafaello/src/lib.rs` — exports
      `resolve_tui_path`, `RflChatError`,
      `RflChatCli` (clap `Cli`), `run_chat(...)` —
      the orchestration entry point.
    - `crates/rafaello/src/main.rs` — minimal:
      `fn main() -> RflChatResult { rafaello::run_cli() }`.
  - `[dependencies]` adds `rafaello-core` (path-dep —
    NOT `rafaello-tui`; pi-3 #6 — `rfl` only spawns
    `rfl-tui` as a subprocess via `RFL_TUI_PATH` /
    `current_exe` sibling lookup, so the TUI library
    crate does not need to link into `rfl`. Pulling in
    `ratatui` / `crossterm` transitively into the
    rafaello bin would also bloat the cold-start
    surface unnecessarily). `tokio`, `tracing`,
    `tracing-subscriber`,
    `clap = { version = "4", features = ["derive"] }`,
    `anyhow` with `workspace = true`. (`clap` is m3's first
    introduction; pi may push back if `rfl chat` is the
    only subcommand and a hand-rolled arg parse is enough —
    round-1 keeps `clap` because m4–m6 will add more
    subcommands.)
- **W5.** No `default = ["test-fixture"]` flip on
  `rafaello-core`; m2's opt-in feature gate stays. The
  `rfl-bus-fixture` binary's **feature-gate +
  required-features setup is unchanged from m2**
  (pi-10 Low 6 — round 9/10 added new fixture modes
  per §L1a: `signal_ready`, `exit_immediately`,
  `hold_silent`, `probe_fd_closed`. Round 11
  reframes W5: only the Cargo.toml feature/required
  setup is unchanged; the binary's mode-dispatch
  logic grows with §L1a).

### F — frontend supervisor (`rafaello_core::frontend`)

m2's `PluginSupervisor` spawns lockin-sandboxed plugins. m3
adds a sibling `FrontendSupervisor` because frontends bypass
lockin (decision row 15) — they are trusted UI principals
that speak for the user. Sharing one supervisor type with a
"sandbox: bool" knob is rejected by construction: lockin's
public API does not expose a no-op policy, and conditionally
skipping lockin builder calls inside `PluginSupervisor`
would muddle the "no debug bypass" property m2's scope
§"Lock-correspondence claim" pinned. A separate type makes
the unsandboxed path explicit.

**Lifecycle ownership: handle-owned** (pi-5 B1 — round
5 was internally inconsistent: §F1 said the supervisor
owns lifecycle, §F4 said `FrontendHandle::Drop`
SIGKILLs, §C2 called `handle.shutdown()` which wasn't
spec'd). Round 6 picks the handle-owned model
unambiguously: `FrontendHandle` owns the
`RegisteredFrontend` guard, the `Server::serve` join
handle, the reaper watch, and the readiness watch
receiver. `FrontendHandle` exposes `shutdown(self) ->
ShutdownReport` (cooperative SIGTERM + grace + SIGKILL
+ reaper wait + serve-handle await — the m2-supervisor
shape moved onto the handle). `FrontendSupervisor` is
a pure factory: it constructs handles via `spawn()`
and holds no per-frontend state. m3's `rfl chat`
spawns exactly one frontend, so a supervisor-managed
map adds no value. Rationale for picking
handle-owned over the m2-style supervisor-owned
model: m2's supervisor manages multiple plugins from
many lock entries, so a managed map is load-bearing;
m3's supervisor would be a managed map with at most
one entry, an architecture imposture for a multi-
frontend future that is explicitly v2 (decisions row
27). When external attach lands in v2, that v2
milestone can refactor `FrontendSupervisor` to
m2-style if needed.

- **F1.** New module `rafaello_core::frontend`. Public
  surface:
  - `pub struct FrontendSupervisor` — stateless factory
    holding only a `Broker` clone + a `FrontendConfig`
    (used to construct `FrontendHandle`s via `spawn`).
    The returned handle owns lifecycle resources.
    `new(broker: Broker, config: FrontendConfig) -> Self`.
  - `pub struct FrontendConfig` (pi-7 #5 — round 7
    referenced this without spec'ing fields):
    ```rust
    pub struct FrontendConfig {
        /// Grace period between SIGTERM and SIGKILL during
        /// shutdown. Default 2s.
        pub shutdown_grace: Duration,
        /// Additional grace after SIGKILL before giving up
        /// on the reaper. Default 1s.
        pub shutdown_kill_grace: Duration,
        /// Bounded notification sink size for the parent's
        /// fittings server. Default 1024 (matches m2).
        pub notification_capacity: usize,
        /// Maximum frame size for the inherited socketpair
        /// transport. Default 1MiB.
        pub max_frame_bytes: usize,
    }

    impl Default for FrontendConfig { /* the values above */ }
    ```
    Mirrors m2's `SupervisorConfig` shape so future
    consolidation is mechanical.
  - `pub struct CompiledFrontend` — the spawn-time plan.
    Fields (pi-7 #9 — drops duplicate ACL fields;
    pi-8 M1 — keeps `attach_id` as `String` so the
    invalid-attach-id spawn test can construct an
    invalid plan. The validated `AttachId` type is
    used everywhere downstream of spawn-time
    validation; `CompiledFrontend.attach_id` is the
    pre-validation wire form):
    - `attach_id: String` (raw — Phase A validates via
      `AttachId::new(&plan.attach_id)`; on failure,
      `FrontendSpawnError::InvalidPlan { reason:
      AttachIdInvalid { attach_id } }`).
    - `entry_absolute: PathBuf`.
    - `argv: Vec<OsString>`.
    - `env: EnvPlan` (m1's type, unchanged: `pass:
      Vec<String>`, `set: BTreeMap<String, String>`;
      pi-7 #2 — round 7 used a `BTreeSet<String>` /
      `clear: vec![]` shape that did not match m1).
    The broker ACL fields (`subscribe_patterns`,
    `auto_subscribes`, `publish_topics`) are NOT
    duplicated on `CompiledFrontend` — they live on
    `BrokerAcl.frontends.<attach-id>` only (pi-7 #9).
    `FrontendSupervisor::spawn` reads them from the
    broker via `Broker::frontend_acl(attach_id)` if
    needed.
  - `pub struct FrontendPaths` — `project_root: PathBuf`
    (passed to the child as `RFL_PROJECT_ROOT`).
  - `pub async fn FrontendSupervisor::spawn(&self, plan:
    &CompiledFrontend, paths: &FrontendPaths) ->
    Result<FrontendHandle, FrontendSpawnError>`. The
    handle exposes BOTH the lifecycle (wait/exit) and
    the readiness future — pi-3 #1: round 3 specified
    `FrontendReadyService { tx: oneshot::Sender<()> }`
    but had no API to surface the receiver to the
    caller; round 4 makes the receiver part of the
    handle.
  - `pub struct FrontendHandle` — analogous to m2's
    `SpawnHandle`. Carries:
    - `attach_id`, child pid, peer handle, drop-time
      SIGKILL (round 3 surface, unchanged);
    - the spawned `Server::serve` `JoinHandle` so the
      supervisor can await it during shutdown (pi-3 #1);
    - **`_register_guard: RegisteredFrontend`** (pi-4
      #1: round 3 spec'd `Broker::register_frontend`
      returning a `RegisteredFrontend` RAII guard but
      didn't say where it lives. Round 5 stores it
      inside `FrontendHandle` so the broker registration
      survives until the handle is dropped. Without
      this, the guard would drop at the end of `spawn`,
      immediately unregistering the frontend, and
      replay fan-out to the TUI would never reach it).
    - **`ready: tokio::sync::watch::Receiver<bool>`**
      (pi-4 #2: round 3 used `oneshot::Receiver<()>`,
      but tokio's oneshot is single-use and cannot be
      peeked or re-polled after the value arrives,
      breaking the `has_signalled_ready` non-blocking
      peek and the documented "second `wait_ready` call
      returns `Ok(())` idempotently". Round 5 uses
      `watch::channel(false)` initialised to `false`;
      `FrontendReadyService` flips the watch to `true`
      on first `frontend.ready` arrival; `wait_ready()`
      `.changed().await`-loops until the borrowed value
      is `true`; `has_signalled_ready()` is
      `*self.ready.borrow()`. Watch is replayable —
      late callers see the cached value immediately).
    - `pub async fn FrontendHandle::wait_ready(&mut
      self) -> Result<(), FrontendReadyError>`.
      Behaviour:
      - If the watch's borrowed value is already
        `true`, return `Ok(())` immediately.
      - Otherwise loop on `self.ready.changed().await`.
        On `Ok(())`, re-check the borrowed value; if
        `true`, return `Ok(())`. (Watch's `changed`
        wakes on every send, including spurious sends;
        the loop is the canonical pattern.)
      - On `Err(_)` from `changed()` (the corresponding
        `Sender` has been dropped — the
        `FrontendReadyService` has been torn down
        without ever flipping to `true`, which only
        happens if the child connection closed before
        the RPC arrived), return `FrontendReadyError::
        SenderDropped`. Pi-4 #3 — naming is
        `SenderDropped` (one word, matching the test
        file), NOT `Sender Dropped` (two words from
        round 4 source).
    - `pub fn FrontendHandle::has_signalled_ready(&self)
      -> bool` — `*self.ready.borrow()`. Non-blocking
      peek for tests; takes `&self` (watch's `borrow`
      is non-mutating).
  - **Connection-service composition** (pi-2 #1 / #6:
    round 2 incorrectly said "reuse m2's `BusPublishService`",
    but that service is plugin-keyed (`CanonicalId`) and
    calls `handle_plugin_publish`. Frontends need their
    own service that knows the `AttachId` and calls
    `handle_frontend_publish`):
    - `pub struct FrontendBusPublishService { broker:
      Broker, attach_id: AttachId }` — implements
      `fittings_core::Service` for the inbound
      `bus.publish` notifications, calling
      `broker.handle_frontend_publish(&attach_id,
      raw_params)`. The shape mirrors
      `BusPublishService` from m2 c19.
    - `pub struct FrontendReadyService { tx:
      tokio::sync::watch::Sender<bool> }` (pi-4 #2 —
      round 4 used `oneshot::Sender<()>` which couldn't
      back the replayable `wait_ready`/`has_signalled_ready`
      surface; round 5 uses `watch`). Handles inbound
      `frontend.ready` RPC (request/response — NOT a
      bus publish). On call: `self.tx.send_replace(
      true)` and return `Ok({})`. Second call is a
      no-op (`send_replace` overwrites with `true`
      again, idempotent); logs at `tracing::warn!`
      because a well-behaved frontend calls
      `frontend.ready` exactly once.
    - `pub trait FrontendExtraServiceFactory` —
      analogous to m2's `ExtraServiceFactory`. Composes
      additional services into the parent's fittings
      `Server` at spawn time. m3 uses it to register
      `FrontendReadyService` (for the readiness
      handshake — §C2 step 7) and any test-side
      observers; m4 uses it for the `confirm_answer`
      service.
    - `pub fn FrontendSupervisor::with_extra_services<F:
      FrontendExtraServiceFactory + 'static>(self,
      factory: F) -> Self` — m2 §"with_extra_service"
      pattern, scoped to frontends. The default
      factory composes `FrontendBusPublishService` and
      `FrontendReadyService` only.
- **F2.** Public types — pi-6 M4 fully spec'd shapes:
  ```rust
  pub struct ShutdownReport {
      pub exit_status: Option<ExitStatus>,
      pub used_sigterm: bool,
      pub used_sigkill: bool,
      pub serve_aborted: bool,
      pub elapsed: Duration,
  }

  #[non_exhaustive]
  #[derive(thiserror::Error, Debug)]
  pub enum FrontendReadyError {
      #[error("ready-watch sender dropped before ready was signalled")]
      SenderDropped,
  }

  #[derive(thiserror::Error, Debug)]
  pub enum PaintError {
      #[error("ratatui draw error: {0}")]
      Draw(std::io::Error),
  }
  ```
  m4 may add a `Cancelled` variant to
  `FrontendReadyError` if `wait_ready` ever surfaces
  outer cancellation; m3 does not need it.
  `FrontendSpawnError` typed enum (lives in
  `rafaello_core::error`). Variants:
  - `InvalidPlan { reason: InvalidFrontendPlanReason }` —
    spot-check failures; reasons enumerate `AttachIdInvalid
    { attach_id }`, `EntryNotAbsolute`, `EntryNotExecutable
    { path }`, `ControlCharsInPath { path }`,
    `ReservedEnvName { var }`, `AttachIdNotInAcl {
    attach_id }`, `AttachIdAlreadyRegistered { attach_id }`.
  - `Io { source: std::io::Error }`.
  - `Spawn { source: std::io::Error }` — `tokio::process::
    Command::spawn` failure.
  - `Transport { source: anyhow::Error }` — fittings
    transport failure.
  - `BrokerRegister { source: BrokerError }`.
  Source-error variants are NOT `Clone` / `PartialEq` (same
  pattern as m2's `SpawnError`).
- **F3.** Spawn body. Phases mirror m2 supervisor's Phase A
  (cheap validation) + Phase B (resource allocation), but
  shorter:
  - **Phase A** (cheap validation, no resources):
    - **Validate `attach_id`** by calling
      `AttachId::new(&plan.attach_id)` (pi-8 M1).
      On error, return
      `FrontendSpawnError::InvalidPlan { reason:
      AttachIdInvalid { attach_id: plan.attach_id.clone() } }`.
      The validated `AttachId` is then used for
      every subsequent broker API call.
    - Reject control chars in `entry_absolute` (m2 §SP4
      pattern).
    - Reject relative paths.
    - Stat `entry_absolute` and check executable bit.
    - Reject reserved env names in `env.set` / `env.pass`
      (m1's `RESERVED_ENV_VARS` set, extended in m2 c04 —
      no further extension in m3).
    - `Broker::try_reserve_frontend_registration(attach_id)`
      to fail fast if the attach id is already registered
      or not in the frontend ACL.
  - **Phase B** (resource allocation, async). Round-11
    explicit ordering (pi-10 Blocker 1 — round 10
    ended up with `.stderr(Stdio::piped())`
    configuration AFTER `child.spawn()` was called,
    which won't apply. The reaper-spawn step was also
    only mentioned in the unwind text, not in the main
    sequence):
    1. Create socketpair (`SOCK_STREAM | SOCK_CLOEXEC`,
       m2's pattern + macOS fcntl fallback per m2 §5.7).
    2. Build `tokio::process::Command` (NOT
       `lockin::SandboxBuilder::tokio_command`; m3
       frontend spawns are unsandboxed).
    3. Apply env: `env_clear` then re-inject
       `RFL_BUS_FD`, `RFL_PROJECT_ROOT`,
       `RFL_PRIVATE_STATE_DIR` (the frontend's per-
       plugin state-equivalent dir under
       `${PROJECT_ROOT}/.rafaello-frontend-data/<attach-id>/`
       — see §S6 below), then `env.pass` then
       `env.set` (set wins, m2 c18 pattern).
    4. Inherit fd `RFL_BUS_FD` via `pre_exec` clearing
       the child socketpair end's `FD_CLOEXEC` (the
       unsandboxed analogue of
       `SandboxBuilder::inherit_fd_as`; nix
       `fcntl(F_SETFD, 0)`).
    5. **Create the per-frontend private state dir**
       (pi-17 #3 — round 17 had §S6 saying it's
       created at spawn, but §F3 omitted the step):
       `let state_dir = paths.project_root.join(
       ".rafaello-frontend-data").join(attach_id
       .as_str());` `fs::create_dir_all(&state_dir)
       .map_err(|source| FrontendSpawnError::Io {
       source })?;`. This step is BEFORE the child
       is spawned, so a failure has no child to
       unwind. Inject the path as
       `RFL_PRIVATE_STATE_DIR` in the env apply
       step above (re-set the env var on the
       `Command` here if the apply-env step ran
       before this state_dir derivation).
    6. **Configure stderr**: `command.stderr(Stdio::piped())`
       BEFORE spawning so the child's stderr fd is a
       pipe (pi-10 Blocker 1).
    7. **Spawn the child**: `let mut child =
       command.spawn()?;` (`Child` now exists).
    8. **Take stderr**: `let child_stderr =
       child.stderr.take();` (Option<ChildStderr>;
       moved out of `Child`).
    9. **Construct the readiness watch**:
       `let (ready_tx, ready_rx) =
       tokio::sync::watch::channel(false);`. Hand
       `ready_tx` to a fresh `FrontendReadyService`;
       hold `ready_rx` for the handle.
    10. **Construct the reaper-outcome watch**:
        `let (reaper_tx, reaper_rx) =
        tokio::sync::watch::channel::<Option<Arc<ReaperOutcome>>>(None);`.
    11. **Spawn the reaper + reaper-watcher tasks**.
        The reaper is a `tokio::spawn`ed task that
        moves `Child` in and awaits `child.wait()`,
        pushing `Exited(status)` (or
        `WaitFailed(io::Error)` if `wait()` itself
        errors) into `reaper_tx`. A second
        `tokio::spawn`ed **watcher task** holds the
        reaper's `JoinHandle` and awaits it; on
        `JoinError` (panic / cancellation), it
        pushes `ReaperOutcome::ReaperPanicked` into
        `reaper_tx`. After this step, `Child` is no
        longer accessible; unwind / shutdown signals
        via `nix::sys::signal::kill(Pid, ...)` on
        `child_pid` and observes the reaper watch.
    12. Build a `fittings_server::Server` over the
        parent socketpair end with the composed
        services from `FrontendExtraServiceFactory`
        (default: `FrontendBusPublishService` +
        `FrontendReadyService`).
    13. **Register the frontend with the broker**
        (`Broker::register_frontend(attach_id, peer)`
        — see §B1) BEFORE spawning the serve loop, so
        the registration is in place when fittings
        starts processing inbound notifications. Move
        the returned `RegisteredFrontend` guard into
        the handle (pi-4 #1).
    14. **Spawn the serve loop**: `let serve_handle =
        tokio::spawn(server.serve());`. Store on the
        handle.
    15. Return `FrontendHandle { attach_id, child_pid:
        Some(child_pid), peer, register_guard:
        Some(guard), serve_handle: Some(serve_handle),
        child_stderr, ready: ready_rx, reaper_outcome:
        reaper_rx, config: self.config.clone() }`
        (pi-11 #1).

  **Phase B unwind rules** (pi-8 M3 — round 8 spawned
  the child before transport / register / serve setup
  but did not specify cleanup if a later step
  failed). On any post-`tokio_command.spawn` error,
  before returning `Err(_)`:
  1. If `serve_handle` was spawned, `serve_handle.abort()`
     and `let _ = serve_handle.await`.
  2. If `register_guard` was acquired (broker register
     succeeded), drop it (broker registration
     releases).
  3. If the child was spawned (pi-9 High 4 — round 9
     said `child.start_kill()` + `child.wait()`, but
     by Phase B step "spawn the reaper" the `Child`
     has already been moved into the reaper task and
     can no longer be accessed directly). The order
     of operations in Phase B is: (a)
     `tokio_command.spawn()` produces `Child` →
     (b) take `child.stderr` for the handle → (c)
     spawn the reaper task with the moved `Child` and
     a `watch::Sender<Option<Arc<ReaperOutcome>>>`.
     Unwind cases:
     - **Failure between (a) and (c)** (e.g. taking
       stderr fails, fittings transport setup
       errors): the spawn site still holds `Child`;
       call `child.start_kill()` + `let _ =
       child.wait().await` directly.
     - **Failure after (c) but before broker
       register** (e.g. building the fittings server
       errors): `Child` is in the reaper. Call
       `nix::sys::signal::kill(Pid::from_raw(pid),
       Signal::SIGKILL)` and await the reaper-watch
       receiver until `Some(_)` is observed.
     - **Failure after broker register but before
       returning the handle**: same SIGKILL + reaper
       wait; also drop the `RegisteredFrontend`
       guard.
  4. Drop the socketpair fds (RAII).
  5. Drop the proxy handle (m3 frontends don't have
     one but the unwind framework should be the same
     as m2).
  Phase A failures (validation, before any resource
  allocation) need no unwind — no resources are held.
  This mirrors m2 §SP4's Phase B unwind matrix
  (m2 retrospective §3.3 — m3's §H6 fault-injection
  exercises the equivalent unwind windows in
  `PluginSupervisor`; the same discipline applies
  here).
- **F4.** Lifecycle (pi-1 #4 + pi-5 B1: handle-owned
  model — all lifecycle state lives on
  `FrontendHandle`):
  - At spawn time, `FrontendSupervisor::spawn` hands the
    `tokio::process::Child` to a per-frontend **reaper
    task** spawned with `tokio::spawn`. The reaper owns
    the `Child` and `await`s `child.wait()`. On exit it
    pushes the `ExitStatus` (or pre-`wait` outcome) into
    a `tokio::sync::watch` channel that the handle's
    `reaper_outcome` field reads.
  - `pub async fn FrontendHandle::wait(&mut self) ->
    Arc<ReaperOutcome>` resolves on that watch
    (mirroring m2 §SP4 step 18). Late `wait` callers
    see the cached `Arc` immediately.
  - **Disarmable Drop** (pi-7 #3 — round 7's
    `shutdown(self)` consumed the handle but Drop
    still fired afterward, sending SIGKILL to a
    possibly-recycled PID. Round 8 makes the
    SIGKILL/abort fields `Option<>` so `shutdown` can
    `take()` them, leaving Drop a no-op):
    ```rust
    pub struct FrontendHandle {
        attach_id: AttachId,
        peer: PeerHandle,                          // pi-12 #4 — round 11 omitted from sketch
        // Disarmed-by-shutdown fields:
        child_pid: Option<u32>,
        serve_handle: Option<tokio::task::JoinHandle<...>>,
        register_guard: Option<RegisteredFrontend>,
        child_stderr: Option<tokio::process::ChildStderr>,
        // Always-live fields:
        ready: tokio::sync::watch::Receiver<bool>,
        reaper_outcome: tokio::sync::watch::Receiver<Option<Arc<ReaperOutcome>>>,
        // Configuration (pi-11 #1: shutdown reads
        // shutdown_grace and shutdown_kill_grace from
        // here; round 11 referenced `config.*` without
        // storing it on the handle).
        config: FrontendConfig,
    }

    impl FrontendHandle {
        // Caller takes ownership of the child's
        // piped stderr; `rfl chat` uses this to spawn
        // the line-forwarding task (pi-9 Blocker 3 —
        // round 9 spec'd the forwarder without an
        // API to access the stderr handle).
        pub fn take_child_stderr(&mut self)
            -> Option<tokio::process::ChildStderr>;
    }
    ```
    `Drop` semantics: see the **cached-outcome rules
    below** (after the shutdown algorithm — pi-15 #3:
    Drop is Exited-skip / WaitFailed-or-ReaperPanicked-
    or-None-bestkill, NOT unconditional SIGKILL).
    Always-applied: `serve_handle.abort()` if still
    `Some`; drop `register_guard` if still `Some`. All
    operations idempotent on dead / already-cleared
    values.
  - `pub async fn FrontendHandle::shutdown(mut self)
    -> ShutdownReport`:
    1. `take()` the three disarmable fields
       (`child_pid`, `serve_handle`, `register_guard`)
       BEFORE doing anything async, so a panic in the
       shutdown body still leaves Drop a no-op.
    2. **Check reaper outcome first** (pi-8 B2 +
       pi-13 #1 — only `Exited(_)` is proof the child
       has actually exited. `WaitFailed(_)` /
       `ReaperPanicked` mean the reaper itself
       failed; the child process may still be alive
       and must be cleaned up). Read
       `*self.reaper_outcome.borrow()`:
       - `Some(Arc<ReaperOutcome::Exited(_)>)`: the
         child has already exited; **skip SIGTERM/
         SIGKILL entirely**. Just abort the taken
         `serve_handle` and drop the taken
         `register_guard`. Set `used_sigterm =
         used_sigkill = false`.
       - `Some(Arc<ReaperOutcome::WaitFailed(_)>)` or
         `Some(Arc<ReaperOutcome::ReaperPanicked>)`:
         the reaper failed; the child may still be
         alive. **Watch is dead** (no further
         notifications). Take the **dead-watch
         branch** below: SIGTERM + sleep + probe;
         SIGKILL + sleep + probe.
       - `None`: take the **live-watch branch**:
         SIGTERM + watch-await + re-check; SIGKILL +
         watch-await + re-check.

    Pi-14 #5 separates the two branches explicitly
    so the algorithm is unambiguous. Pi-14 #4
    replaces the unsafe `SIGCONT` liveness probe
    with `kill(pid, 0)` (the portable
    "is-this-pid-alive" no-op signal — Linux+macOS;
    `Errno::ESRCH` means gone, `Ok(())` /
    `Errno::EPERM` mean alive but inaccessible).

    **Live-watch branch** (`reaper_outcome` was
    `None` at the cache check):
    3a. SIGTERM `child_pid`. `used_sigterm = true`.
    3b. `tokio::time::timeout(self.config.shutdown_grace,
        reaper_outcome.changed()).await`.
    3c. Re-check `*self.reaper_outcome.borrow()`:
        - `Some(Exited(_))`: skip SIGKILL; jump to
          step 6.
        - Otherwise: SIGKILL `child_pid`; `used_sigkill
          = true`; `tokio::time::timeout(self.config
          .shutdown_kill_grace, reaper_outcome
          .changed()).await` (pi-14 #3 — the kill
          grace is `shutdown_kill_grace`, not
          `shutdown_grace` again). Failure to confirm
          exit after this is logged at `warn!` but
          does not block.

    **Dead-watch branch** (`reaper_outcome` was
    `WaitFailed`/`ReaperPanicked`):
    3a'. SIGTERM `child_pid`. `used_sigterm = true`.
    3b'. `tokio::time::sleep(self.config.shutdown_grace)`.
    3c'. Probe with `kill(pid, 0)`:
        - `Err(Errno::ESRCH)`: child gone; jump to
          step 6.
        - Otherwise: SIGKILL `child_pid`;
          `used_sigkill = true`; `tokio::time::sleep
          (self.config.shutdown_kill_grace)`; probe
          again. Failure to confirm `ESRCH` after
          this is logged at `warn!` but does not
          block.
    6. Abort `serve_handle.abort()` and `let _ =
       serve_handle.await`.
    7. Drop `register_guard`.
    8. **Populate `ShutdownReport`** (pi-12 #2 — round
       11 added the field but didn't say how it was
       filled): `exit_status` reads
       `*self.reaper_outcome.borrow()` after the
       SIGTERM/KILL flow completes; on
       `Some(Arc<ReaperOutcome::Exited(status)>)`
       store `Some(status)`; otherwise (no outcome
       observed, or `WaitFailed`/`ReaperPanicked`)
       store `None`. `used_sigterm` /
       `used_sigkill` / `serve_aborted` are local
       flags set during the steps above; `elapsed`
       is `start.elapsed()`.
    9. Return the populated `ShutdownReport`.

    The same hazard applies to `Drop` (pi-8 B2 +
    pi-14 #1 — round 14 had Drop skip on any
    `Some(_)` outcome, contradicting shutdown's
    Exited-only rule):
    `FrontendHandle::Drop` reads
    `*self.reaper_outcome.borrow()`:
    - `Some(Arc<ReaperOutcome::Exited(_)>)`: child
      already exited; skip SIGKILL (only
      `serve_handle.abort()` if still `Some` and
      drop `register_guard`).
    - `Some(Arc<ReaperOutcome::WaitFailed(_)>)` or
      `Some(Arc<ReaperOutcome::ReaperPanicked>)`:
      reaper failed but the child may still be
      alive — best-effort SIGKILL on `child_pid`
      (errors logged at `warn!`), then abort
      serve_handle and drop register_guard. NOT
      blocking; Drop does not wait on a kill
      response.
    - `None`: best-effort SIGKILL (the round-7 base
      behaviour for this branch).
  - (Pi-9 Blocker 1: round 9 left a stale "Cooperative
    shutdown" block here that conflicted with the
    canonical one above — round 10 deletes it.
    `FrontendHandle::shutdown(mut self)` is fully
    spec'd in the disarmable-Drop section above; that
    is the single source of truth.)
  - **New positive test**
    `frontend_handle_wait_resolves_on_child_exit.rs`:
    spawn a `rfl-bus-fixture` child (NOT `rfl-tui` —
    pi-5 M2; fixture is in rafaello-core's bin set so
    `CARGO_BIN_EXE_rfl-bus-fixture` is valid) in
    `signal_ready` mode (§L1a) with a
    `RFL_FIXTURE_MAX_LIFETIME=1` so it exits after
    1 s. `handle.wait().await` returns
    `ReaperOutcome::Exited(status)` with
    `status.success()` true (pi-9 Medium 8 — round 9
    used a non-existent `Exited(0)` shape).
  - **New negative test**
    `frontend_handle_drop_does_not_leak_zombie.rs`:
    spawn `rfl-bus-fixture` in `respond_peer_call`
    mode, drop the handle without cooperatively
    shutting down, allow the reaper task a 500 ms
    grace, then assert (pi-6 L3 — `ENOENT` is
    PID-reuse-sensitive): if `/proc/<pid>/status`
    exists, its `State:` field is NOT `Z` (zombie);
    if the file is `ENOENT`, the process disappeared
    entirely (still acceptable). Linux-only.
- **F5.** Out of scope for §F: lockin builder calls (frontends
  are trusted UI principals; decision row 15), outpost
  proxy startup, `bindings.helper_for`, `provider = true`
  refusal (frontends never carry plugin-shaped bindings).

### B — broker extension: frontend principals

m2's broker only knows `Plugin` + `Core`. m3 promotes the
m2-reserved `Frontend { attach_id }` `PublisherIdentity`
variant to live and grows `BrokerAcl` accordingly.

- **B1.** Extend `BrokerAcl`:
  ```rust
  pub struct BrokerAcl {
      pub plugins: BTreeMap<CanonicalId, PluginAcl>,
      pub frontends: BTreeMap<AttachId, FrontendAcl>,  // NEW
      // ... existing fields
  }

  pub struct AttachId(String);  // newtype, validated

  pub struct FrontendAcl {
      pub subscribe_patterns: BTreeSet<String>,
      pub auto_subscribes: BTreeSet<String>,
      pub publish_topics: BTreeSet<String>,
  }
  ```
  m3's `rfl chat` constructs a `BrokerAcl` with a single
  frontend entry: `attach_id = "tui"`, `subscribe_patterns
  = ["core.session.**", "core.lifecycle.**"]`,
  `auto_subscribes = []`, `publish_topics = []`. (m3's TUI
  publishes nothing on the bus — the m4 `confirm_answer`
  and `user_message` topics are out of scope here. Pi may
  push back: round-1 takes the conservative "TUI publishes
  nothing in m3" position.)
- **B2.** New broker registration surface for frontends. m2's
  `BrokerError::{NotInAcl, AlreadyRegistered, PublishOutsideGrant,
  InvalidInReplyTo}` are `CanonicalId`-shaped; round 1 was wrong
  to claim frontend support drops in "without API breakage"
  (pi-1 #10). Round 2 makes the error surface explicitly
  publisher-typed:
  - **New `Publisher` variant**: `Publisher::Frontend(AttachId)`
    (m2's enum is `Core | Plugin(CanonicalId)`; round 2
    extends).
  - **New `BrokerError` variants** (frontend-shaped, mirroring
    the plugin pair):
    - `FrontendNotInAcl(AttachId)`,
    - `FrontendAlreadyRegistered(AttachId)`,
    - `FrontendNotRegistered(AttachId)`.
  - **Generalise** the existing publisher-bearing variants to
    accept the `Publisher` enum. Concretely:
    - `PublishOutsideGrant { publisher: Publisher, topic:
      String }` (m2 currently `{ canonical: CanonicalId,
      topic: String }`),
    - `InvalidInReplyTo { publisher: Publisher, topic: String,
      reason: InReplyToReason }` (m2 currently `{ canonical:
      CanonicalId, ... }`).
    These are source-breaking changes to `BrokerError` and
    require the m2 callers (broker internal call sites + every
    `matches!`-style test) to update accordingly. The change is
    isolated to `rafaello-core` (`BrokerError` is not yet
    surfaced beyond the crate's tests at m2 close).
  - `UnknownNamespace { publisher: Publisher, topic }` and
    `PublishOnReservedNamespace { publisher: Publisher, topic }`
    (m2 already typed as `Publisher`-bearing per m2 §B2; no
    change beyond the new `Frontend` enum arm).
  - **New register/lookup methods**:
    - `pub fn Broker::register_frontend(&self, attach_id:
      AttachId, peer: PeerHandle) -> Result<RegisteredFrontend,
      BrokerError>` — symmetric to `register_plugin`. Errors
      `FrontendNotInAcl` / `FrontendAlreadyRegistered`. RAII
      guard `RegisteredFrontend` mirrors `RegisteredPlugin`.
    - `pub fn Broker::try_reserve_frontend_registration(&self,
      attach_id: &AttachId) -> Result<(), BrokerError>` —
      cheap precheck (m2 §B1 pattern).
    - `pub fn Broker::frontend_acl(&self, attach_id: &AttachId)
      -> Option<FrontendAcl>` — same shape as
      `plugin_acl(canonical)`.
    - `pub fn Broker::handle_frontend_publish(&self, attach_id:
      &AttachId, raw_params: &Value) -> Result<(), BrokerError>`
      — the symmetric handler to `handle_plugin_publish`. m3
      ships this method but m3's `tui` ACL has
      `publish_topics = []`, so its only m3-observable behaviour
      is "always errors" (`PublishOutsideGrant`). m4 / m5
      will exercise the success path when `user_message` /
      `confirm_answer` enter the grant set.
- **B3.** Promote `PublisherIdentity::Frontend { attach_id:
  String }` from commented-future to live. Bus event
  serialisation gains the new variant — `kind: "frontend"`
  per the existing `#[serde(tag = "kind", rename_all =
  "snake_case")]` convention.
- **B4.** Publish authority for frontends (m3 enforcement,
  symmetric to m2 §B3 plugin path) — pi-5 M5: error
  classification mirrors plugin publishes; truly unknown
  top-level segments stay `UnknownNamespace`, known
  segments outside the publisher's authority are
  `PublishOnReservedNamespace`:
  - **Grammar revalidation** (m2 §B5).
  - **Top-level segment lookup**:
    - Not in `{core, provider, plugin, frontend}` →
      `UnknownNamespace { publisher: Publisher::
      Frontend(attach_id), topic }`. (`evil.foo` from
      a frontend.)
    - In `{core, provider, plugin}` →
      `PublishOnReservedNamespace { publisher: ...,
      topic }`. (frontend publishing on `core.*`,
      `provider.*`, `plugin.*`.)
    - `frontend` segment-count check (pi-18 #3 —
      symmetric to m2 §B3 plugin "two-segment
      `plugin.<id>` is reserved" rule):
      - `segments.len() < 3` (e.g. `frontend.tui`
        only) → `PublishOnReservedNamespace`. The
        bare `frontend.<id>` topic is grammar-valid
        but semantically empty; treat as outside any
        frontend's authority.
      - `segments[1] != attach_id` →
        `PublishOnReservedNamespace` (cross-frontend
        masquerade — m3 only has one frontend so
        this is mostly defence-in-depth, but
        identical shape to the plugin case for
        forward compat with v2 multi-frontend).
    - `frontend.<own-attach-id>.<...>` (≥3 segments,
      first two correct) → exact-string check
      against `frontend_acl.publish_topics`. If not
      member → `PublishOutsideGrant`.
  - `auto_subscribes` is NOT publish authority for
    frontends either (m2 §B3's rule applies).
- **B5.** Fan-out (m2 §B7): a frontend subscriber receives
  `core.session.**` events the same way plugins do. Result-
  routing protection (m2 §B7's `plugin.<id>.tool_result` /
  `rpc_reply` no-fan-out) is unchanged — m4 territory. m3
  does not introduce any frontend-targeted result-routing
  carve-out.
- **B6.** `BrokerAcl` defence-in-depth pattern revalidation
  (m2 §B10) extends to the new `frontends` map —
  symmetric with plugin ACL validation:
  - `validate_topic` against every `publish_topics`
    entry (pi-5 M6 — round 5 only mentioned subscribe
    pattern revalidation; plugin ACL construction
    validates publish topics too).
  - `validate_pattern` against every
    `subscribe_patterns` and `auto_subscribes` entry.
  New tests:
  `broker_construct_with_invalid_frontend_pattern_rejected.rs`,
  `broker_construct_with_invalid_frontend_publish_topic_rejected.rs`
  (pi-5 M6),
  `broker_register_frontend_unknown_attach_id_rejected.rs`,
  `broker_register_frontend_duplicate_rejected.rs`.

### S — session store + controller (`rafaello_core::session`)

Sessions persist conversation entries to SQLite. m3 ships
the storage layer **plus a `SessionController` that owns
the canonical entry-finalisation pipeline** — append to
SQLite, render through the renderer pipeline, publish on
the bus, in that order. The fixture-entry harness in m3
goes through the controller; m4's agent loop will replace
the harness with the real provider path. Pi round-1 #2:
without the controller, the demo bar's "nine SQLite rows
after shutdown" assertion (§I) and the "renders all of
them" assertion (§I) would land via two parallel paths
that could drift — one path for both is the contract.

- **S1.** New module `rafaello_core::session`. Public
  surface:
  - `pub struct SessionStore` — owns a
    `rusqlite::Connection` behind a `Mutex` (single-writer;
    m3 has one core process per project per row 34) and a
    **`lock_guard: Flock<File>`** field that holds the
    project's exclusive flock for the store's lifetime
    (pi-11 #2 — round 11 spec'd the Flock RAII switch
    in §S5 but didn't add the field to the struct;
    without retention, the lock would drop at the end
    of `open()`). The `Flock<File>` releases on drop.
  - `pub fn SessionStore::open(state_dir: &Path) ->
    Result<Self, SessionError>` — **lock-first ordering**
    (pi-2 #5):
    1. `fs::create_dir_all(state_dir)`.
    2. Open `${state_dir}/session.lock` with `O_CLOEXEC`
       (§S5).
    3. `Flock::lock(file, FlockArg::LockExclusiveNonblock)`
       (per §S5 — the RAII helper is the canonical
       form; pi-12 #5 — round 12 left a stale
       `flock(LOCK_EX | LOCK_NB)` shorthand here).
       On `EWOULDBLOCK`, read
       holder pid from file content → `SessionError::
       Locked { holder_pid }`. **Do not touch SQLite.**
    4. Truncate the lockfile and write
       `std::process::id()` as the holder pid.
    5. Open `${state_dir}/session.sqlite`; run
       `PRAGMA journal_mode = WAL`,
       `PRAGMA synchronous = NORMAL`; create tables;
       verify schema version. Schema bumps add a
       migration step in m4+; v1 ships schema_version=1
       only.
    Round 2 had the SQLite open before the flock, which
    let a second process touch the WAL before being
    rejected.
  - `pub fn SessionStore::append_entry(&self, entry:
    &Entry) -> Result<u64, SessionError>` — INSERT into
    `entries`; returns the assigned `seq`.
  - `pub fn SessionStore::load_entries(&self) ->
    Result<Vec<StoredEntry>, SessionError>` — SELECT in
    `seq` order. **Returns `StoredEntry { seq: u64,
    entry: Entry }`** so replay can reconstruct the
    canonical envelope (pi-2 #2: round 2 returned
    `Vec<Entry>` and dropped `seq`, which made the
    `replay_history` published envelope inconsistent
    with the fresh `finalize_entry` envelope). Round 3:
    ```rust
    pub struct StoredEntry {
        pub seq: u64,
        pub entry: Entry,
    }
    ```
    `seq` is the SQLite-assigned monotonic ordering;
    not part of `EntryMetadata` (pi-1 #12 verdict
    stays).
  - `pub fn SessionStore::session_id(&self) -> &str` —
    ULID assigned at first open; persisted in a single-
    row `session_meta` table.
  - `#[cfg(any(test, feature = "test-fixture"))] pub fn
    SessionStore::lock_fd_for_test(&self) ->
    std::os::fd::RawFd` (pi-9 Medium 6) — exposes the
    flock fd to integration tests so
    `session_store_lock_fd_not_inherited_by_child.rs`
    can pass it as the `--probe-fd <N>` arg to the
    `probe_fd_closed` fixture mode. Production code
    never calls this.
  - `pub struct SessionController` — bundles a
    `SessionStore` + a `RenderPipeline` + a `Broker`
    handle. Constructed by `rfl chat` after the broker is
    up. Public methods:
    - `pub async fn finalize_entry(&self, entry: Entry,
      caps: &Capabilities) -> Result<(), SessionError>`
      — single canonical entry-publication path:
      1. `store.append_entry(&entry)` (assigns `seq`);
      2. `pipeline.render(&entry, caps)` (panic-isolated
         per §R3);
      3. `broker.publish_core("core.session.entry.finalized",
         json!({ "entry": entry, "tree": tree, "seq":
         seq, "replay": false }))`.
      Errors at step 1 or 3 are surfaced as
      `SessionError`; renderer panics at step 2 are
      handled by `RenderPipeline` itself (catch_unwind
      converts to a `Callout` tree, the publish still
      proceeds — the entry is persisted regardless).
    - `pub async fn replay_history(&self, caps:
      &Capabilities) -> Result<(), SessionError>` —
      iterates `store.load_entries()`, renders each, and
      publishes on the same `core.session.entry.finalized`
      topic with `replay: true` in the metadata payload
      (so a future m4+ TUI can suppress fresh-entry
      animations on replay if needed). m3's TUI does not
      currently distinguish; the metadata flag is wire-
      reserved without a consumer.
    - `pub fn store(&self) -> &SessionStore` — exposed
      for tests that want to assert on persisted state
      after shutdown.
  - **No separate `core.session.entry.replay` topic.** Pi
    round-1 #1 / #9: round 1 invented a `replay` topic
    that does not match overview §11 / Stream E §3
    (which know only `finalized`). Round 2 collapses
    onto `entry.finalized` with a `replay: bool` payload
    flag. The decision is recorded in §"Acceptance
    summary" as anticipated drift; if pi prefers a
    metadata-on-`EntryMetadata` flag rather than a
    payload-envelope flag, that is a no-op rewire and
    round 2 is open to either form.
- **S2.** Schema (one table for v1):
  ```sql
  CREATE TABLE IF NOT EXISTS entries (
      id          TEXT PRIMARY KEY,         -- ULID
      seq         INTEGER NOT NULL UNIQUE,  -- monotonic
      parent      TEXT,                     -- always NULL in v1
      kind        TEXT NOT NULL,
      schema      TEXT,
      payload     TEXT NOT NULL,            -- JSON
      metadata    TEXT NOT NULL,            -- JSON
      fallback    TEXT,                     -- JSON, nullable
      created_at  TEXT NOT NULL             -- ISO 8601
  );

  CREATE TABLE IF NOT EXISTS session_meta (
      key   TEXT PRIMARY KEY,
      value TEXT NOT NULL
  );
  -- value rows: ("session_id", <ulid>), ("schema_version", "1").
  ```
  `seq` is server-assigned at append time
  (`SELECT COALESCE(MAX(seq), -1) + 1`). `kind` is the entry
  kind string (built-in or `<plugin>:<kind>`); `schema` is
  the payload schema URI. v1 leaves `parent` always NULL —
  branching is post-v1 (overview §12).
- **S3.** `SessionError` typed enum: `Io { source }`,
  `Sqlite { source: rusqlite::Error }`, `Serde { source:
  serde_json::Error }`, **`Locked { holder_pid:
  Option<u32> }`** (pi-14 #7 — the contender may
  observe the lockfile empty/stale/corrupt because
  the holder writes its pid AFTER acquiring the
  lock, leaving a window where contention reads
  before the write; `None` represents "lock held
  by an unknown process"), `SchemaMismatch {
  found, expected }`,
  **`Publish { source: BrokerError }`** (pi-6 B2 —
  round 6 had `SessionController::finalize_entry` call
  `broker.publish_core` whose error is a
  `BrokerError`; without a variant the controller's
  `Result<(), SessionError>` cannot typecheck. The
  `Publish` variant is `#[from] BrokerError` so the
  `?` operator is clean).
  `#[non_exhaustive]`, `thiserror`-derived. Re-exported
  from `lib.rs`.
- **S4.** Project state directory layout — `${PROJECT_ROOT}/
  .rafaello/state/`:
  - `session.sqlite` — the entry store.
  - `session.sqlite-wal`, `session.sqlite-shm` — WAL.
  - `session.lock` — flock'd file (see §S5).
  - The state dir is created if missing
    (`fs::create_dir_all`).
- **S5.** Concurrent-access fence details (sequence is in
  §S1 above — lock acquired BEFORE SQLite touch per
  pi-2 #5):
  - Open the lockfile with **`O_CLOEXEC`** so the fd is
    not inherited by the spawned `rfl-tui` child (pi-1
    #3). Use the nix-re-exported constant
    (`nix::libc::O_CLOEXEC`) instead of pulling a
    direct `libc` dep into `rafaello-core`:
    `OpenOptions::new().read(true).write(true).create(true)
    .custom_flags(nix::libc::O_CLOEXEC).open(...)`.
  - **Use `nix::fcntl::Flock`** (pi-10 High 3 —
    round 10 used the deprecated function form
    `nix::fcntl::flock(fd, FlockArg::...)`, which
    nix 0.29 marks `#[deprecated]` and would trip
    the warning-free `cargo doc`/`cargo build`
    acceptance gate). The `Flock` type is the
    RAII-style replacement:
    `Flock::lock(file, FlockArg::LockExclusiveNonblock)
    -> Result<Flock<File>, (File, Errno)>`. On
    `Errno::EWOULDBLOCK` the `(File, Errno)` tuple
    is destructured to read the holder pid from the
    returned File before mapping to
    `SessionError::Locked`. The `Flock<File>` wrapper
    holds the lock for its lifetime; release happens
    on drop.
  - On `EWOULDBLOCK`, read the holder's pid from the file
    contents and return `SessionError::Locked {
    holder_pid }`. The holder writes its pid in
    §S1 step 4.
  - The lock is released on `Drop` (close fd → kernel
    releases). No explicit release path.
  - **New negative test** `session_store_lock_fd_not_inherited_by_child.rs`:
    pi-8 M5 — round 8 used `lsof` on macOS, an
    avoidable external dep on a CI hard gate. Round 9
    uses a portable probe child instead:
    `rfl-bus-fixture` gains a new `probe_fd_closed`
    mode that takes the lock fd number via a CLI arg
    (e.g. `--probe-fd <N>`), calls
    `nix::fcntl::fcntl(N, FcntlArg::F_GETFD)`, and
    exits 0 if the call returns `EBADF` (fd not in
    table — desired) or non-zero otherwise. The test
    opens the store (acquiring the lock fd), passes
    the fd number to the probe child via the arg, and
    asserts the child exits 0. Cross-platform: `EBADF`
    after `O_CLOEXEC`-on-exec is the same on Linux
    and macOS; no `/proc` or `lsof` involved.
  Cross-platform: Linux + macOS both support
  `LockExclusiveNonblock`. flock is per-fd, not per-pid;
  with `O_CLOEXEC` set, fork+exec preserves the lock in
  the parent and the child does not inherit it.
- **S6.** Per-frontend private state directory:
  `${PROJECT_ROOT}/.rafaello-frontend-data/<attach-id>/` —
  injected as `RFL_PRIVATE_STATE_DIR` to the child. m3's
  TUI does not yet write anything there; m6 may use it for
  TUI prefs (scrollback height, color overrides). The dir
  is created by `FrontendSupervisor::spawn` if missing.
  The path is **not** `rafaello-plugin-data` — frontends
  are not plugins; per-plugin private state (decisions
  row 16, refined by row 37) talks about plugins
  specifically. m3 picks `rafaello-frontend-data` as the
  parallel.
- **S7.** Out of scope for §S: branching (`parent` always
  NULL), session replay UI (the TUI loads history at
  startup but does not expose a `/replay` command),
  multi-session daemon (overview §12 last paragraph),
  audit log table (m5 territory — confirmation answers
  audit), attached-frontend log (overview §12; m3 has one
  frontend, no attach surface).

### E — entry + render-tree types

Defined in `rafaello_core::entry`. Stream E §3 + §4 are the
contract. m3 implements only the v1 subset.

- **E1.** `pub struct Entry` matching Stream E §3 with v1
  constraints:
  ```rust
  pub struct Entry {
      pub id: Ulid,
      pub parent: Option<Ulid>,            // always None in v1
      pub kind: String,                    // EntryKind newtype OK
      pub schema: Option<String>,
      pub payload: serde_json::Value,
      pub metadata: EntryMetadata,
      pub fallback: Option<EntryFallback>,
  }

  pub struct EntryMetadata {
      pub created_at: chrono::DateTime<chrono::Utc>,
      pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
      pub author: EntryAuthor,             // user|assistant|tool|system|plugin
      pub plugin: Option<String>,          // when author==plugin
      pub stream_state: StreamState,       // v1: only Final
      pub tags: Vec<String>,
      // NOTE: `seq` is NOT part of EntryMetadata in v1.
      // The SQLite `entries.seq` column is the canonical
      // monotonic ordering; SessionStore::append_entry
      // assigns it server-side. The fan-out wire payload
      // for core.session.entry.finalized carries `seq` at
      // the envelope level (see §S1 SessionController),
      // not under metadata. Round-1 had a duplicate
      // `seq: Option<u64>` here; pi-1 #12 surfaced the
      // ambiguity. v2 may revisit if a streaming-patch
      // ordering field needs to ride alongside the entry.
  }

  #[non_exhaustive]
  pub enum StreamState { Final }            // v1; v2 adds Open|Patch|Closed
  ```
  v1's `StreamState` enum has only `Final`. Wire encoding
  is a string (`"final"`). Other states (`open`/`patch`/
  `closed`) are deferred per row 28; m3 rejects on decode
  if encountered.

  **Helper types** (pi-5 M7 — round 5 used these without
  defining them):
  ```rust
  #[derive(Serialize, Deserialize, Clone, Debug)]
  #[serde(rename_all = "snake_case")]
  pub enum EntryAuthor {
      User,
      Assistant,
      Tool,
      System,
      Plugin,
  }

  #[derive(Serialize, Deserialize, Clone, Debug, Default)]
  pub struct EntryFallback {
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub text: Option<String>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub markdown: Option<String>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub summary: Option<String>,
  }

  #[derive(Serialize, Deserialize, Clone, Debug)]
  #[serde(rename_all = "snake_case")]
  pub enum RawFormat {
      Ansi,
      Html,
      Plain,
  }
  ```
  All built-in entry payload structs in §E3 derive
  `Serialize`, `Deserialize`, `Clone`, `Debug`, with
  `#[serde(deny_unknown_fields)]` on each (m2 c06's
  pattern), and ship under
  `rafaello_core::entry::payloads::*` modules.
- **E2.** `pub enum RenderNode` — the ~14-variant render
  tree per Stream E §4.1. Variants: `Text`, `Heading`,
  `Code`, `Inline`, `Block`, `List`, `KeyValue`, `Table`,
  `Divider`, `Image`, `Link`, `Callout`, `Collapsed`,
  `Raw`, `Unknown`. Internally tagged on `node` per Stream
  E §4.2 (`#[serde(tag = "node")]`).
  - `Unknown { kind: String, payload: serde_json::Value,
    fallback: EntryFallback }` — the server-side downgrade
    target (§R3 below).
  - `Raw { format: RawFormat, body: String }` where
    `RawFormat = Ansi | Html | Plain`. m3's TUI accepts
    only `Ansi` and `Plain`; `Html` triggers downgrade.
- **E3.** Built-in entry kinds (the eight from the m3
  roadmap row): `text`, `heading`, `code_block`,
  `tool_call`, `tool_result`, `error`, `thinking`, `image`.
  Payloads match Stream E §3.1. Each gets a typed Rust
  payload struct under
  `rafaello_core::entry::payloads::*`.
  - `text { text: String, markdown: bool }`
  - `heading { text: String, level: u8 }` (1..=6)
  - `code_block { code: String, lang: Option<String> }`
  - `tool_call { id: String, name: String, args: Value,
    status: ToolCallStatus }`
  - `tool_result { call_id: String, ok: bool, content:
    RenderNode, details: Option<Value> }`
  - `thinking { text: String }`
  - `image { uri: String, mime: String, alt: String,
    bytes_b64: Option<String> }`
  - `error { code: String, message: String, data:
    Option<Value> }`
- **E4.** `Entry`/`RenderNode` JSON serialisation must
  match Stream E §3 / §4.2 exactly so the on-disk SQLite
  representation is human-readable and matches the
  on-the-wire form (the broker fan-out frame for
  `core.session.entry.finalized`). Round-trip tests on
  every built-in kind + every render-node variant.

### R — renderer pipeline (`rafaello_core::renderer`)

Built-in in-process Rust renderers turn an `Entry` into a
`RenderNode`. m3 wires them; the TUI consumes the result.

- **R1.** `pub trait Renderer: Send + Sync + 'static {
  fn render(&self, entry: &Entry, caps: &Capabilities) ->
  Result<RenderNode, RendererError>;
}`. Each built-in kind has one `impl`.
- **R2.** `pub struct RendererRegistry` — wraps
  `BTreeMap<String, Arc<dyn Renderer>>` keyed by `kind`
  string. Public surface (pi-6 B4 — round 6 only had
  `with_builtins`, but the panic / Err integration
  tests under `rafaello-core/tests/` need to register
  test renderers; without a public `register` method
  those tests can't be implemented as integration
  tests):
  - `pub fn RendererRegistry::new() -> Self` — empty
    registry.
  - `pub fn RendererRegistry::with_builtins() -> Self`
    — registers the eight built-in kinds.
  - `pub fn RendererRegistry::register(&mut self, kind:
    String, renderer: Arc<dyn Renderer>) -> Option<Arc<dyn Renderer>>`
    — inserts; returns the previous entry if present
    (so tests can detect collisions; production
    `with_builtins` does not collide).
  - `pub fn RendererRegistry::get(&self, kind: &str) -> Option<&Arc<dyn Renderer>>`
    — accessor used by `RenderPipeline`.
  Plugin (subprocess) renderers are NOT registered
  (decision row 29, deferred to v2).
- **R3.** `pub struct RenderPipeline` — the entry-to-tree
  driver. Pi-1 #9 surfaced that round 1 conflated three
  distinct fallback paths; round 2 separates them per
  Stream E §6 + §6 last paragraph:
  - **Path A — unknown entry kind / renderer-unavailable.**
    The `entry.kind` is not present in the registry. Stream
    E §6 specifies:
    1. If `entry.fallback.text` is set → emit `Block
       { children: [ Text { text: fallback.text,
       emphasis: None } ] }`. **m3 does NOT ship a
       markdown→render-tree parser** (pi-5 M9 — round
       5 said `fallback.markdown` "re-enters Path A"
       which would have required a markdown built-in
       kind that m3 doesn't have). m3's fallback
       inspects `text` only; `fallback.markdown` is
       reserved for a future v2 markdown renderer
       built-in but is not consulted by m3's pipeline.
       Plugin authors are encouraged to populate
       `fallback.text` (Stream E §6 first bullet's
       "strongly encouraged" rule still applies).
    2. If neither is set → emit `Callout { kind: "warn",
       child: KeyValue { pairs: [(\"kind\", entry.kind),
       (\"schema\", entry.schema.unwrap_or(\"\")),
       (\"payload\", payload-stringified)] } }` per
       Stream E §6 second bullet ("ugly on purpose so
       plugin authors notice").
  - **Path B — renderer panic / renderer `Err`.** The
    `entry.kind` IS in the registry but the call panics
    or returns `Err`. Per Stream E §9 last paragraph
    ("a crashing renderer never crashes the daemon"),
    the panic / `Err` is treated as renderer-unavailable
    — the pipeline falls into **Path A** with the same
    fallback rules (author fallback if set, default
    Callout otherwise). The panic is **separately**
    logged at `tracing::error!` with entry id + kind for
    diagnosability; the `Err` is logged at
    `tracing::warn!`. The wire output does NOT include a
    "panicked" diagnostic in the rendered tree — that
    would conflate trust boundaries (the user sees a
    fallback; the operator sees a log). Pi may push back
    on this and prefer an in-tree diagnostic in dev
    builds; round 2 takes the conservative "log-only" cut
    to keep the user-visible tree predictable across dev
    and release builds. Catch implementation:
    `std::panic::catch_unwind(AssertUnwindSafe(||
    renderer.render(entry, caps)))`.
  - **Path C — capability-driven render-tree node
    downgrade.** The renderer returned `Ok(tree)`, but
    one or more nodes are not in the frontend's
    `Capabilities::nodes` set. The pipeline walks the
    tree and downgrades each unsupported subtree to
    `Unknown { kind: "<node-name>",
    payload: <serialised-node-as-Value>, fallback:
    EntryFallback }` per Stream E §6 last paragraph.
    `EntryFallback` here is **the entry's `fallback`
    field** (not a "default Block" — round-1 wording was
    self-contradictory per pi-1 #9; `Unknown.fallback`'s
    type IS `EntryFallback` per E2). If
    `entry.fallback` is `None`, the downgraded subtree
    carries `EntryFallback { text: <node-summary>,
    markdown: None, summary: None }` where
    `node-summary` is a one-line stringification (e.g.
    `"[unsupported Image]"`); the frontend then has a
    minimal text to paint.
  - The three paths are mutually exclusive on a
    per-entry basis: A and B replace the renderer's
    return; C runs only after a successful renderer
    return.
- **R4.** `pub struct Capabilities` — the v1 subset of
  Stream E §5. Fields: `unicode: UnicodeClass`, `color:
  ColorClass`, `width: u16`, `height: Option<u16>`,
  `image: Vec<String>`, `interactive: bool`, `scrollback:
  ScrollbackClass`, `nodes: BTreeSet<String>`,
  **`raw_formats: BTreeSet<String>`** (pi-2 #7: round 2
  said TUI accepts only Ansi/Plain Raw and "Html
  triggers downgrade", but the §R3 Path C downgrade only
  inspected `Capabilities::nodes` — `Raw` is a supported
  node, so the downgrade never fired. Round 3 adds the
  raw-format capability so the downgrade walker can
  test `match node { Raw { format, .. } => caps
  .raw_formats.contains(format), _ => caps.nodes
  .contains(<node-name>) }`). m3's TUI reports
  `raw_formats = {"ansi", "plain"}` and `nodes = full
  set`, so the downgrade path is exercised only by
  tests that synthesise reduced capabilities. m3 has no
  `frontend.hello` handshake (deferred per row 27 +
  overview §10.1 banner) — the TUI capabilities are
  baked in as compile-time constants on the core side,
  indexed by attach id.
- **R5.** `RendererError` typed enum: `MissingPayloadField
  { kind, field }`, `InvalidPayload { kind, message }`,
  `Internal { detail }`. Renderers do not return I/O
  errors (built-in renderers are pure functions — Stream
  E §9 first paragraph).
- **R6.** Out of scope for §R: subprocess `renderer.render`
  request/response (decision row 29), daemon-side render
  cache keyed by `(plugin, kind, payload_hash, caps_hash)`
  (Stream E §9 — depends on subprocess renderers), patch-
  op streaming (decision row 28), `frontend.hello`
  capability negotiation (rows 27 + 34).

### T — TUI (`rafaello_tui` crate)

The bundled ratatui-based terminal frontend. Spawned by
`rfl chat` as a subprocess, attached via `RFL_BUS_FD`,
identified as `frontend.tui`.

- **T1.** New crate scaffolding (§W3).
- **T2.** `[[bin]] rfl-tui` — the binary's `main`:
  1. Parse env: `RFL_BUS_FD` (required; numeric fd),
     `RFL_PROJECT_ROOT` (required; abs path),
     `RFL_TUI_TEST_MODE` (optional; if `=1`, see step 4),
     the fittings reserved env vars.
  2. Adopt the inherited fd as a tokio `UnixStream`,
     wrap with `fittings_transport::stdio::StdioTransport`-
     equivalent (m2 c19 already split on
     `tokio::net::UnixStream`).
  3. Build a `fittings_client::Client` with a
     `BusEventHandler` (notification handler) that turns
     `bus.event` notifications into channel sends to the
     UI thread. Subscribe semantics are broker-managed at
     `register_frontend` time; the client does not issue
     a `bus.subscribe` request (m2 §"Out of scope" — the
     broker rejects `bus.subscribe` with `MethodNotFound`).
     **Once the handler is wired**, send the readiness
     RPC: `peer.call("frontend.ready", json!({})).
     await?` (§C2 step 7). This is the single point in
     `rfl-tui`'s startup that signals "I am subscribed
     and ready to receive entry events"; it lives in
     both production and `RFL_TUI_TEST_MODE` paths
     (the test mode sends `frontend.ready` after the
     in-memory log subscriber is wired).
     **Test-only delay knob**: if
     `RFL_TUI_READY_DELAY_MS` is set (parsed as u64),
     `tokio::time::sleep(Duration::from_millis(N)).
     await` BEFORE the `frontend.ready` call; used by
     `rfl_chat_replay_withheld_until_frontend_ready.rs`
     to verify the parent withholds replay until
     readiness fires.
  4. **Headless test mode** (when `RFL_TUI_TEST_MODE = 1`):
     skip terminal init entirely; collect every received
     `bus.event` into an in-memory log; exit cleanly on
     the first `core.lifecycle.test_done` event with
     `exit_code = 0`. No keyboard handling, no crossterm
     calls. The log is reachable via stderr (one line
     per received entry, **with the human-readable
     sentinel format spec'd below — NOT JSON; pi-16
     #4 — round-9 said JSON, round-6 said sentinel
     lines, round-17 picks sentinel because the
     replay-withheld test asserts on substring
     matches over the combined parent+child stream).
     Test harnesses can parse-and-assert without
     involving a fake terminal.
     **Stderr sentinels** (pi-5 M10 + pi-6 B3 + pi-9
     Medium 7):
     - **TUI side (rfl-tui stderr — RAW, no
       `rfl-tui:` prefix; the rfl-chat-side
       forwarder adds the prefix exclusively to
       avoid double-prefixing)**:
       - `"bus.event topic=<topic> seq=<n>"` (one
         line per received `bus.event`).
       - `"test-done"` printed before exit on
         receipt of `core.lifecycle.test_done`.
       - `"project-root=<abs-path>"` printed during
         startup, after `RFL_PROJECT_ROOT` is
         parsed and validated as absolute (pi-17 #4
         — used by the rfl chat
         relative-project-root canonicalisation
         test).
     - **Parent side (rfl chat stderr)** — see §C2
       step 7:
       - `"rfl-chat: frontend-ready-observed"`
         printed BEFORE replay starts, AFTER
         `handle.wait_ready()` returns. This is the
         test's ordering anchor.
     **Deterministic exit** (pi-1 #7) — the test
     harness publishes `core.lifecycle.test_done`
     after the last fixture entry; the 60 s
     self-timeout (§L1 — extended to `rfl-tui`) is a
     defensive backstop only.
  5. **Production mode** (default; `RFL_TUI_TEST_MODE`
     unset): initialise terminal (crossterm raw mode +
     alternate-screen — m3 picks alternate-screen for the
     v1 cut to keep scrollback handling simple). On exit,
     restore raw mode (Drop guard) so a panic in the TUI
     does not leave the user's terminal corrupted.
  6. Run the UI loop: redraw on each entry-event arrival,
     handle keyboard input (q to quit; arrow keys to
     scroll; no other commands in m3 — the prompt box is
     wired but submitting is a no-op since m3 has no
     `user_message` publish path).
  7. On exit, close the bus connection, restore the
     terminal (production mode only), exit 0.

  **Self-timeout** (pi-6 M5): in `RFL_TUI_TEST_MODE`,
  the TUI reads `RFL_TUI_MAX_LIFETIME` (env, seconds;
  default `60` if unset) and `std::process::exit(0)`
  on elapsed time even without `core.lifecycle.test_done`.
  This is the test-chain leak mitigation parallel to
  L1's fixture self-timeout. Production mode does NOT
  honour this env (a real interactive TUI runs as
  long as the user wants); the env is silently ignored
  when `RFL_TUI_TEST_MODE` is unset. Exit code on
  self-timeout is `0` (the TUI did its job; the
  upstream test harness is responsible for asserting
  whatever else it cares about).
- **T3.** Entry → render-tree happens **on the core side**
  (the TUI subscribes to `core.session.entry.finalized`
  events whose payload is the `Entry` plus a pre-rendered
  `tree: RenderNode`). The TUI never imports the
  `renderer` module — the contract is "core ships an
  already-downgraded render tree; frontend paints it".
  This is the load-bearing simplification of the renderer
  RFC: row 29 defers subprocess renderers, but the more
  general "rendering happens server-side" rule keeps the
  TUI a pure painter.
- **T4.** The TUI's paint function is a `RenderNode ->
  ratatui::widgets::*` translator. Pure function (modulo
  the terminal). Layout decisions (where to put the
  prompt box, whether to use a single-pane or split-pane
  layout) live on the TUI side per Stream E §1 ("layout
  is the frontend's").
- **T5.** Crash isolation: a panic in the paint function
  catches via `std::panic::catch_unwind` at the top of
  the redraw loop and renders a `[render error: ...]`
  line in place of the panicking entry, then continues.
  The TUI process does NOT exit on a paint panic.
- **T6.** Out of scope for §T: confirmation modal (m5),
  user message publish (m4), command palette (post-v1),
  multi-tab UI, mouse support, theming.

### C — `rfl chat` subcommand

The user-facing entry point. Wires every m3 subsystem
together.

- **C1.** Add `clap`-derived `Cli` with one subcommand:
  `Chat { project_root: Option<PathBuf> }`. Default project
  root is the current directory.
- **C2.** `rfl chat` flow (re-ordered per pi-1 #1 — the
  TUI must subscribe before any history is published, or
  it misses every replay event because broker fan-out is
  to live registrations only):
  1. Resolve project root: start from the
     `--project-root` flag if set, else
     `std::env::current_dir()`. **Canonicalize to an
     absolute path** via `Path::canonicalize`; on
     failure (path doesn't exist, permission denied),
     map to `RflChatError::ProjectRootInvalid {
     path, source }` (pi-17 #4 — relative inputs
     would propagate a relative `RFL_PROJECT_ROOT`
     that the TUI rejects). Note: the canonicalize
     also resolves symlinks; m3 considers this a
     feature (the TUI sees a stable, fully-resolved
     path).
  2. Resolve `rfl-tui` binary path (§C3 below).
  3. Open `SessionStore` → acquire flock → on
     `SessionError::Locked { holder_pid }` print a
     friendly error and exit non-zero (matches the
     demo-bar negative). The error message format
     (pi-15 #4):
     - `Some(pid)`: "session lock held by pid {pid};
       another rfl chat is running for this project."
     - `None`: "session lock held by an unknown
       process; remove `.rafaello/state/session.lock`
       if no other rfl chat is running for this
       project."
     The unknown-holder branch happens when the
     contender races the holder's pid-write (§S5 +
     §S3 — `holder_pid: Option<u32>`).
  4. Build the `BrokerAcl` for m3: zero plugins, one
     frontend `tui` with `subscribe_patterns =
     ["core.session.**", "core.lifecycle.**"]`,
     `auto_subscribes = []`, `publish_topics = []`.
  5. `Broker::new(acl)` → `FrontendSupervisor::new` →
     construct the `RendererRegistry::with_builtins()`
     and the `RenderPipeline`. Build the
     `SessionController` bundling store + pipeline +
     broker (§S1). Build the `CompiledFrontend` for
     the bundled TUI:
     ```rust
     CompiledFrontend {
         attach_id: "tui".to_string(),
         entry_absolute: tui_path,  // §C3 resolved
         argv: vec![],
         env: EnvPlan {
             // m1 EnvPlan: pass: Vec<String>, set:
             // BTreeMap<String,String>. No `clear`
             // field (pi-7 #2 — round 7 invented one).
             pass: vec![
                 "TERM".into(), "COLORTERM".into(),
                 "LANG".into(), "LC_ALL".into(),
                 "LC_CTYPE".into(),
                 // Test-only knobs (pi-6 B1):
                 "RFL_TUI_TEST_MODE".into(),
                 "RFL_TUI_READY_DELAY_MS".into(),
                 "RFL_TUI_MAX_LIFETIME".into(),
                 "RFL_FIXTURE_MODE".into(),
                 "RFL_FIXTURE_MAX_LIFETIME".into(),
                 "RFL_FIXTURE_EXIT_CODE".into(),  // pi-12 #3
             ],
             set: BTreeMap::new(),
         },
     }
     ```
     The `pass` set above is the allowlist of env vars
     the TUI / fixture child can observe. Reserved
     RFL names (`RFL_BUS_FD`, `RFL_PROJECT_ROOT`,
     `RFL_PRIVATE_STATE_DIR`) are still injected by
     the supervisor (§F3) on top of this. m4+ may add
     more pass entries (e.g. provider API keys go via
     a different mechanism); m3's allowlist is closed.
     The reserved-RFL list (§F3) and the pass-list
     here are disjoint by construction — pass entries
     listed here are ALL non-reserved.
  6. **Spawn the TUI first**:
     `let mut handle = frontend_supervisor.spawn(&compiled,
     &paths).await?`. This registers the frontend
     with the broker and the broker fan-out is now
     live for `frontend.tui`. **Stderr forwarding
     contract** (pi-8 M2 + pi-9 Blocker 3 + pi-10
     High 2): the supervisor sets
     `Command::stderr(Stdio::piped())` and stores the
     resulting `ChildStderr` on the handle;
     `rfl chat` takes it (`let child_stderr =
     handle.take_child_stderr()
     .expect("piped");`) and spawns a task that
     reads the stderr line-buffered, writing each line
     back to `rfl chat`'s own stderr (acquired via
     `tokio::io::stderr()`) prefixed with `"rfl-tui:
     "`. The forwarder serialises with parent
     sentinel writes via an `Arc<tokio::sync::Mutex<()>>`
     held in `rfl chat`'s top-level state — both the
     forwarder's per-line write and the parent's
     `eprintln!`-equivalent take the mutex around a
     single `write_all` call to a unified stderr
     locked-writer (pi-9 Medium 7). **The TUI binary
     itself does NOT prefix its own stderr lines**
     with `"rfl-tui: "` (round 10's no-double-prefix
     contract). **`rfl chat` retains the forwarder's
     `JoinHandle` so it can be drained** (pi-10
     High 2 — round 10 spawned the forwarder
     fire-and-forget; on TUI exit, the runtime could
     end before the forwarder flushed its final
     lines, making combined-stream assertions flaky).
     The handle is awaited in §C2 step 10's tail (see
     below). The forwarder task also handles EOF
     (child stderr closed) gracefully.
  7. **Wait for TUI subscription readiness.** The
     fittings server fans out `bus.event` notifications
     best-effort with a bounded drop-on-full sink (m2
     §B7 + Stream B); without an explicit handshake the
     parent could publish before the TUI's
     `BusEventHandler` is registered. m3 uses an **RPC
     method `frontend.ready` on the parent's fittings
     server**, NOT a bus topic (pi-2 #6 — naming a bus
     topic for a frontend-originated signal that the
     m3 ACL grants nothing on confused the namespace
     story). The TUI calls
     `peer.call("frontend.ready", json!({}))` once its
     bus event handler is registered; the parent's
     `FrontendReadyService` (§F1) flips a
     `tokio::sync::watch::Sender<bool>` to `true` to
     satisfy this wait (round 5 — the round-4 oneshot
     was replayability-broken). The method lives
     on the fittings *connection* between the parent
     and the TUI — it is not a broker topic and does
     not interact with `BrokerAcl`. The wait is the
     handle's own API:
     `tokio::time::timeout(Duration::from_secs(5),
     handle.wait_ready()).await`. Three outcomes
     (pi-4 #4):
     - `Ok(Ok(()))` → readiness. **Print
       `"rfl-chat: frontend-ready-observed"` to `rfl
       chat`'s own stderr** (parent-side sentinel per
       §T2 — this is the test's ordering anchor;
       pi-6 B3). Proceed to step 8 (replay).
     - `Ok(Err(FrontendReadyError::SenderDropped))` →
       the bus connection closed before
       `frontend.ready` arrived. The child PROCESS
       may or may not have exited yet (pi-9 High 5).
       Map to `RflChatError::FrontendExitedBeforeReady`
       and run `handle.shutdown().await` (bounded by
       the handle's `FrontendConfig` —
       `shutdown_grace + shutdown_kill_grace` —
       pi-13 #3, "self.config" was wrong context
       since this is rfl chat orchestration code).
       **Then drain
       the stderr forwarder**: `let _ =
       stderr_forwarder.await;` (pi-11 #3 — round 11
       only drained in step 10; SenderDropped/timeout
       arms returned from step 7 without draining,
       leaving a final-line race). Exit non-zero
       with stderr citing the report.
     - `Err(_)` (timeout) → `RflChatError::
       FrontendReadyTimeout`. SIGTERM the child via
       `handle.shutdown()`, then drain the stderr
       forwarder (`let _ = stderr_forwarder.await;`,
       pi-11 #3), before exiting non-zero.

     **The gate is enforced in `rfl chat`
     orchestration, NOT inside `SessionController` or
     `Broker`** (pi-3 #2): `controller.replay_history()`
     does not consult readiness; the gate is the
     `handle.wait_ready().await` between TUI spawn and
     replay. This keeps `SessionController` pure
     (append → render → publish, no orchestration
     state) and pushes the gate into the layer that
     owns the orchestration.
  **Cleanup-guard contract for steps 8 onward**
  (pi-17 #1: every fallible call after the TUI is
  spawned MUST run the canonical teardown — bounded
  shutdown + stderr drain — before propagating the
  error). The orchestration uses an explicit
  cleanup guard pattern:
  ```rust
  let mut cleanup_state = Some((frontend_handle,
      stderr_forwarder));
  let result: Result<(), RflChatError> = async {
      // steps 8, 9, 10's wait
      controller.replay_history(&caps).await?;
      if let Some(harness) = harness {
          harness.run(&controller, &caps).await?;
      }
      let outcome = frontend_handle_ref.wait().await;
      // outcome handling per step 10 below
      Ok(())
  }.await;
  // Always run teardown, regardless of result:
  if let Some((handle, forwarder)) = cleanup_state.take() {
      let _ = handle.shutdown().await;
      let _ = forwarder.await;
  }
  result.map_err(|e| /* exit non-zero with stderr cite */)
  ```
  (Pseudocode — implementation may use
  `scopeguard::defer!` or an async drop guard
  pattern; the contract is "exactly one
  shutdown+drain call regardless of which step
  errored".)
  8. Replay session history through the controller:
     `controller.replay_history(&caps).await?`. Every
     entry is published on `core.session.entry.finalized`
     with `replay: true` in the envelope; the broker
     fan-out reaches the TUI which is now subscribed.
  9. **In-test fixture-entry harness** (only when
     `RFL_HARNESS_FIXTURES` is set — production
     `rfl chat` does NOT publish anything; m4 replaces
     this with the agent loop). The harness calls
     `controller.finalize_entry(entry, &caps)` for one
     entry per built-in kind + one unknown kind, all
     with `stream_state: "final"`. After the last
     entry, the harness publishes
     `core.lifecycle.test_done` (which the headless TUI
     uses for clean exit per §T2 step 4).
  10. Wait on `frontend_handle.wait().await` ->
      `Arc<ReaperOutcome>`. The current `ReaperOutcome`
      shape is `Exited(std::process::ExitStatus)` —
      no `Signaled` variant; signal info is read via
      `ExitStatus::signal()` on Unix (pi-7 #4 — round
      7 invented `Signaled(_)` matching). On TUI exit:
      - `Exited(status)` where `status.success()` →
        clean exit; proceed.
      - `Exited(status)` where `!status.success()` —
        any non-zero exit code OR signal termination
        (pi-6 M3): `rfl chat` exits non-zero with
        stderr citing both `status.code()` and
        `status.signal()` if either is `Some`.
        Persisted entries in SQLite are NOT rolled
        back — they committed during replay/finalize
        and are durable across restarts; the next
        `rfl chat` replays them as usual. Map to
        `RflChatError::FrontendExitedAbnormally {
        outcome }`.
      - **`WaitFailed(io::Error)`** (pi-9 Medium 8 —
        m2's actual shape is `WaitFailed(std::io::
        Error)`, not `{ errno }`; assertions read
        `e.raw_os_error()` for the os errno; pi-8 M4 —
        only handled `Exited`; m2's `ReaperOutcome`
        also has `WaitFailed` and `ReaperPanicked`
        variants the supervisor inherits): `rfl
        chat` exits non-zero, citing the errno.
        Treat as abnormal exit;
        `RflChatError::FrontendExitedAbnormally {
        outcome }` covers it.
      - **`ReaperPanicked`** (pi-8 M4): the reaper
        task itself panicked while awaiting the
        child. Same treatment — abnormal exit, log
        the panic.
      **Teardown order is owned by the cleanup
      guard** at the top of step 8 (pi-19 #1 —
      round 19 retained an in-step-10 explicit
      shutdown+drain block that contradicted "guard
      is the sole teardown"). The guard performs:
      (1) `let report = handle.shutdown().await;`
      (the handle has not yet been moved out by
      step 10's outcome read, because the guard
      takes ownership before the async block
      starts), (2) `let _ = forwarder.await;` —
      EOF-on-close drain. Step 10 itself reads
      `*reaper_outcome.borrow()` for outcome
      mapping; the guard handles the actual
      shutdown call afterward.

      **Test coverage** (pi-10 Medium 5 + pi-11 #4 —
      round 10 incorrectly claimed
      `rfl_chat_frontend_exits_before_ready_errors.rs`
      covered the post-ready abnormal-exit branch,
      but `exit_immediately` exits BEFORE
      `frontend.ready`, so it exercises the step-7
      SenderDropped path, not step-10 `Exited(!success)`):
      - **Step-7 SenderDropped path** is covered by
        `rfl_chat_frontend_exits_before_ready_errors.rs`
        (using `exit_immediately` mode).
      - **Step-10 post-ready abnormal-exit path** is
        covered by a new
        `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
        which uses a new `signal_ready_then_exit_n`
        fixture mode (§L1a, round 12) — the fixture
        sends `frontend.ready`, sleeps briefly so the
        readiness gate releases and replay completes,
        then exits with code 7 (or whatever
        non-success code the test specifies via env).
        `rfl chat` maps to
        `RflChatError::FrontendExitedAbnormally`,
        exits non-zero with stderr citing the child's
        exit code.
      - **`WaitFailed` and `ReaperPanicked` —
        covered by a unit-level shutdown test seam.**
        Pi-14 #6 + pi-15 #1 + pi-15 #2 — round 15
        proposed an `Fn(Pid, Signal)` mock that
        couldn't represent the `kill(pid, 0)` no-op
        liveness probe. Round 16 fixes the signature:
        ```rust
        // Pure-async extraction of the shutdown
        // algorithm; production `FrontendHandle::
        // shutdown` calls this with the real signal
        // / probe functions.
        pub async fn shutdown_with_outcome(
            cached: Option<Arc<ReaperOutcome>>,
            child_pid: Pid,
            config: &FrontendConfig,
            mut signal_fn: impl FnMut(Pid, Signal)
                -> Result<(), Errno>,
            mut probe_fn: impl FnMut(Pid)
                -> Result<(), Errno>,
            // ... reaper_outcome rx, serve handle, register guard ...
        ) -> ShutdownReport;
        ```
        `signal_fn(pid, SIGTERM | SIGKILL)` sends a
        signal; `probe_fn(pid)` is the no-op
        liveness probe (production wires
        `nix::sys::signal::kill(pid, None)` for the
        probe — `None` is the "no-op" form;
        equivalent to `kill(pid, 0)` in C). The
        live-watch branch ignores `probe_fn`; the
        dead-watch branch uses it after each
        sleep.
        Tests live in
        `rafaello-core/tests/frontend_shutdown_dead_watch_paths.rs`:
        - `dead_watch_waitfailed_child_already_gone`:
          `cached = Some(WaitFailed(...))`,
          `signal_fn` records the SIGTERM,
          `probe_fn` returns `Err(ESRCH)` on first
          call → SIGKILL is NOT sent (skipped per
          step 3c'), `used_sigkill = false`.
        - `dead_watch_reaper_panicked_child_alive`:
          `cached = Some(ReaperPanicked)`,
          `signal_fn` records SIGTERM and SIGKILL,
          `probe_fn` returns `Ok(())` (alive) on
          first call, `Err(ESRCH)` on second →
          `used_sigkill = true`. Verifies kill_fn
          call sequence and ShutdownReport flags.
        These tests do NOT spawn a real child; they
        exercise the algorithm purely against
        synthetic outcomes and mock signal/probe
        function calls. (Pi-15 #2: the prior
        "intentionally untested" framing is retracted
        — these branches ARE covered by these unit
        tests; only full reaper-task fault injection
        in production code is deferred to m4 if a
        real-world failure surfaces.)

      The cleanup-guard above (pseudocode at the top
      of step 8) runs `frontend_handle.shutdown().await`
      + `stderr_forwarder.await` exactly once on EVERY
      exit path through this match (clean Exited,
      abnormal Exited, WaitFailed, ReaperPanicked).
      Step 10 itself does NOT call shutdown — it
      reads the `ReaperOutcome` to decide the exit
      code / error mapping; the guard performs the
      actual teardown when the async block
      completes. (Pi-18 #1: round 18 had both the
      guard and an explicit shutdown call here,
      which would double-consume `self`.) With pi-8
      B2 + pi-13 #1: the single guard-time shutdown
      is a no-kill drain when the cached outcome is
      `Exited(_)`; for `WaitFailed` /
      `ReaperPanicked`, shutdown still issues
      SIGTERM + SIGKILL since the child may be
      alive. Then drop the controller and the store
      (the store's drop releases the flock). The
      `FrontendSupervisor` itself has no shutdown API
      — it's a stateless factory (pi-5 B1).
- **C3.** `rfl-tui` binary path resolution (pi-1 #6 —
  round 1 said "compile-time constants" without
  specifying the resolution order). Lookup order
  (round 4: pi-3 #4 — `option_env!` removed because
  Cargo does not reliably set `CARGO_BIN_EXE_*`
  cross-package or for `cargo run`-style workflows):
  1. **`RFL_TUI_PATH` env override** (highest priority).
     Used by tests and by anyone running `rfl chat`
     from a non-installed target. The dev workflow is
     `cargo build --workspace --bins` followed by
     `RFL_TUI_PATH=$PWD/target/debug/rfl-tui cargo run
     -p rafaello -- chat`, OR `cargo install --path
     rafaello/crates/rafaello-tui` to install
     `rfl-tui` so the sibling lookup below works.
  2. **Sibling of `current_exe()`**. `current_exe()
     .parent().join("rfl-tui")` — the canonical
     installed-binary location (homebrew, nix wrapper,
     `cargo install`).
  Errors out with a typed `RflChatError::TuiPathUnresolved`
  message naming both lookups it tried. Pi-4 #7:
  factor the resolution into a testable pure function:
  ```rust
  pub fn resolve_tui_path(
      env: &dyn Fn(&str) -> Option<OsString>,
      current_exe: &Path,
  ) -> Result<PathBuf, RflChatError>
  ```
  The production caller passes `&|name| std::env::
  var_os(name)` and `&std::env::current_exe()?`. Tests
  drive synthetic env + a `current_exe` pointing into
  a tempdir, so the negative test does not get
  fooled by a stale `target/debug/rfl-tui` next to the
  real `rfl`. Test coverage:
  - `resolve_tui_path_env_override.rs` (rafaello bin
    test) — synthetic env returns `Some(<temp-stub>)`;
    function returns the stub path.
  - `resolve_tui_path_sibling_lookup.rs` — synthetic
    env returns `None`; `current_exe` points at a
    tempdir containing `rfl-tui`; function returns
    that sibling.
  - `resolve_tui_path_unresolved.rs` — synthetic env
    `None` + tempdir without `rfl-tui` sibling;
    returns `RflChatError::TuiPathUnresolved`.
  - End-to-end `rfl_chat_resolves_tui_via_env_override.rs`
    sets the real `RFL_TUI_PATH` env to **the real
    `rfl-tui` binary** with `RFL_TUI_TEST_MODE=1`
    AND `RFL_TUI_MAX_LIFETIME=2` (pi-13 #4 — without
    the lifetime override the test would block on
    the headless TUI's default 60 s self-timeout
    because no `core.lifecycle.test_done` event is
    published). The pure-function tests above are
    the synthetic-stub coverage; this end-to-end is
    the integration-level "actually boots" gate.
- **C4.** Error handling: every error path prints a
  human-readable message on stderr and returns a non-
  zero exit. Round 2 keeps the simple "1 for any error"
  exit-code stance; m4 may differentiate when the agent
  loop introduces new error classes worth distinguishing.
### H6 — supervisor fault injection (m2 retro §5.1 carryover)

m2 c21 deleted three unwind tests because their synthetic
stub was removed. m3 ships the fault-injection mechanism
those tests should have been written against.

- **H6.1.** Extend m2's `TestHooks` with two one-shot
  inject points:
  ```rust
  pub struct TestHooks { /* m2 fields */ }

  impl TestHooks {
      // m2 fields unchanged.
      pub fn inject_pre_spawn_fault(&self);
      pub fn inject_post_spawn_pre_register_fault(&self);
      pub fn inject_post_register_fault(&self);
      pub fn pre_spawn_fault_consumed(&self) -> bool;
      pub fn post_spawn_pre_register_fault_consumed(&self) -> bool;
      pub fn post_register_fault_consumed(&self) -> bool;
  }
  ```
  Each `inject_*` arms a one-shot atomic; the next spawn
  through that supervisor consumes it and returns
  `SpawnError::SandboxBuild { canonical, source:
  anyhow::anyhow!("test-injected pre-register fault") }`
  (or post-register equivalent) instead of completing
  Phase B. Identical reuse of `SandboxBuild` is fine
  because the synthetic source is clearly tagged.
- **H6.2.** Inject points (pi-16 #1: m2 retro §3.3
  identified two distinct unwind windows; round-16
  conflated them. Round 17 names them precisely
  against the m2 `PluginSupervisor::spawn` body):
  - **Pre-spawn-post-socketpair**: after socketpair /
    proxy / sandbox-builder allocation, BEFORE
    `tokio_command.spawn()` produces the `Child`. On
    fault, no child exists; unwind verifies fd-count
    returns to baseline + proxy / private-state dirs
    cleaned up. This is the m2 `unwinds_after_socketpair`
    coverage.
  - **Post-register**: pinned in code-order terms
    (pi-17 #2: m2's actual ordering is
    `register_plugin` → drop in-flight guard →
    spawn reaper/watcher → spawn `server.serve`):
    **after the reaper/watcher tasks have been
    spawned, BEFORE `tokio::spawn(server.serve())`**.
    On fault, the child is spawned, the reaper IS
    already running, and the broker has live
    registration; unwind verifies registration was
    rolled back, the child was reaped via the
    reaper, and `in_flight` was cleared. This is
    the m2 `unwinds_after_register` +
    `post_register_reaps_child` coverage.
  - **Post-spawn-pre-register** (pi-18 #2: round 18
    incorrectly conflated this window with
    pre-spawn; this is a distinct ownership state —
    `Child` exists, no broker registration, no
    reaper task). Inject AFTER `tokio_command.spawn()`
    and BEFORE `broker.register_plugin(...)`. On
    fault, the `Child` is held by the spawn body
    (not yet moved into the reaper); unwind calls
    `child.start_kill()` + `child.wait().await`
    directly + drops the socketpair fds + drops the
    proxy handle. No broker rollback needed (no
    registration was acquired). New test:
    `tests/supervisor_spawn_unwinds_post_spawn_pre_register.rs`
    — arms post-spawn-pre-register fault; spawn
    returns `SpawnError::SandboxBuild`; assert
    Linux fd-count returned to baseline AND no
    child remains in `/proc` (the synchronous
    `child.wait()` reaped before unwind returned).
    Linux-only.

  m3's three inject points (`pre_spawn`,
  `post_spawn_pre_register`, `post_register`)
  cover all three distinct ownership states in
  m2's spawn body. m3 retro will record this
  three-window split as a refinement of m2 retro
  §3.3's two-window framing.
- **H6.3.** Re-add three deleted tests:
  - `tests/supervisor_spawn_unwinds_after_register.rs` —
    arms post-register fault; spawn returns
    `SpawnError::SandboxBuild`; broker has canonical in
    ACL but no live registration (the unwind dropped the
    `RegisteredPlugin` guard); supervisor's `in_flight`
    is cleared.
  - `tests/supervisor_spawn_post_register_reaps_child.rs`
    — Linux-only (`#[cfg(target_os = "linux")]`); arms
    post-register fault; assert `last_reaped_pid` is the
    spawned child via the reaper.
  - `tests/supervisor_spawn_unwinds_after_socketpair.rs`
    — arms **pre-spawn-post-socketpair** fault;
    spawn returns `SpawnError::SandboxBuild`; no
    child was created; Linux fd-count returns to the
    pre-spawn baseline (read `/proc/self/fd`); proxy
    and private-state dirs are cleaned up.
- **H6.4.** Production builds compile out the
  inject_fault counters entirely (cfg-gated on `test-
  fixture` like m2's existing `TestHooks` accessors).

### M1 — m1 publishes-grant unknown-namespace patch (m2 retro §2.8)

m2 retro §2.8 filed an m1 parse-time gap: m1's manifest
validation accepts unknown top-level namespaces in
`publishes` grants (e.g. a manifest declaring `publishes =
["evil.foo"]` validates), and m2's broker rejects them at
runtime as `UnknownNamespace`. The mirror at parse time was
never tightened. m3 owns it.

- **M1.1.** Extend `rafaello_core::validate::check_publish_topic`
  (the existing helper called by `manifest_standalone`,
  the topic-grammar+namespace gatekeeper that does
  NOT have access to the canonical id). Pi-16 #2 +
  #3: round-9-through-16 invented variants and
  cross-cut rules that conflicted with m1's existing
  shape. Round 17 lands the **minimal additive
  patch**:
  - The existing variants
    `PublishOnReservedNamespace { topic }` (for
    `core.*`) and `PublishOnFrontendNamespace
    { topic }` (for `frontend.*`) are unchanged.
    `provider.*` is already permitted at this layer
    and cross-checked for own-id by
    `manifest_with_id`'s `ProviderNamespaceMismatch`
    rule (also unchanged).
  - **NEW**: add `ValidationError::PublishUnknownNamespace
    { topic, namespace }` for truly unknown top-level
    segments (`evil.foo`, `random.thing`). Currently
    `check_publish_topic`'s `_ => Ok(())` arm
    accepts anything outside `{core, frontend}`;
    round-17 changes it to reject anything outside
    `{core, frontend, plugin, provider}`. The
    `plugin.*` and `provider.*` arms remain
    accepting at this layer; they're cross-checked
    by `manifest_with_id` against the canonical /
    provider id (m1 logic, unchanged).
  - The `manifest_with_id` layer is **unchanged**
    by m3 — own-topic-id and provider-id mismatch
    rules already exist.
- **M1.2.** New test
  `tests/manifest_publishes_unknown_namespace_rejected.rs`:
  - `evil.foo` →
    `ValidationError::PublishUnknownNamespace`.
  - **Existing positives** (m1 — verify they still
    pass after the new variant lands):
    `core.foo` → `PublishOnReservedNamespace`,
    `frontend.foo` → `PublishOnFrontendNamespace`,
    `provider.<own-id>.foo` from a provider manifest
    → accepted, `provider.<other-id>.foo` →
    `ProviderNamespaceMismatch`,
    `plugin.<own-topic-id>.foo` → accepted,
    `plugin.<other-topic-id>.foo` →
    existing m1 own-topic-id mismatch error.
- **M1.3.** Existing m1 tests must continue to pass;
  the tightening is purely additive — the new
  `PublishUnknownNamespace` variant only fires on
  inputs that the prior validator's `_ => Ok(())`
  arm accepted (i.e. truly unknown namespaces),
  which no m1 test exercises positively.
- **M1.4.** **Lock-side validation NOT touched in m3**
  (pi-18 #4 — `check_lock_publish_topic` also has a
  `_ => {}` arm that accepts unknown namespaces in
  hand-authored locks; m3 leaves this as runtime-
  only enforcement at the broker. Reasoning:
  hand-authored lock entries are an explicit
  `--allow-unsafe` user override path; the runtime
  rejection is sufficient defence. m4 may revisit
  if a user-facing failure mode surfaces. Recorded
  as anticipated drift in §"Acceptance summary".)

### I — integration test suite

The §"Demo bar" matrix below is the contract.
Test placement (pi-1 #5: Cargo only reliably exposes
`CARGO_BIN_EXE_<name>` for binaries of the package whose
integration test is being built):

- **`rafaello-core/tests/`** — broker, session store,
  renderer pipeline, supervisor (incl. fault-injection),
  manifest tightening. None of these need `rfl-tui` or
  the `rfl` bin.
- **`rafaello-tui/tests/`** — anything spawning
  `rfl-tui` (uses `env!("CARGO_BIN_EXE_rfl-tui")`,
  resolved within the rafaello-tui crate's test build).
- **`rafaello/tests/`** — the headline `rfl chat` end-
  to-end test (uses `env!("CARGO_BIN_EXE_rfl")`, resolved
  in the rafaello crate's tests; the test-side path to
  `rfl-tui` is set via the `RFL_TUI_PATH` env override
  per §C3, with the path obtained via a build-time
  helper that reads the rafaello-tui crate's binary
  from the workspace target dir — pi may suggest a more
  robust resolver such as `tempfile::workspace_target` or
  `cargo_metadata`; round 2 picks the env-override path
  because it sidesteps any cross-crate resolution
  fragility).

#### Positive matrix

`rafaello-core/tests/`:

- `frontend_register_with_broker.rs` — open a broker in-
  process and call `register_frontend(AttachId::new("tui"),
  peer)`; the frontend lands in the registry; the guard
  drops cleanly. (No subprocess; uses an in-memory
  fittings transport from m2's `m2_harness`.)
- `session_store_round_trip.rs` — open a `SessionStore`
  in a tempdir, append three entries (`text`,
  `code_block`, `tool_call`), close, reopen, load — see
  the three back in `seq` order.
- `session_store_lock_fd_not_inherited_by_child.rs` —
  see §S5 (pi-7 #8: this is a pure SessionStore test;
  belongs in rafaello-core/tests/, not
  rafaello/tests/).
- `session_controller_finalize_entry.rs` — wire a
  `SessionController` against an in-memory broker; call
  `finalize_entry(entry)`; assert (a) the row is in
  SQLite, (b) a `core.session.entry.finalized` event
  fired with the rendered tree, (c) the `replay` flag
  is `false`.
- `session_controller_replay_history.rs` — pre-seed the
  store with three entries, build a fresh
  `SessionController` with a new in-memory broker,
  register an in-process subscriber, call
  `replay_history()`; the subscriber sees three
  `core.session.entry.finalized` events with `replay:
  true`, in `seq` order.
- `renderer_pipeline_built_in_kinds.rs` — for each of
  the eight built-in kinds, render a sample entry; assert
  tree matches a hand-written expected JSON.
- `renderer_pipeline_unknown_kind_falls_back_with_author_fallback.rs`
  — `kind = "myorg:custom"` + author `fallback` set;
  pipeline returns `Block { children: [ Text {
  text: fallback.text } ] }` per §R3 Path A bullet 1.
- `renderer_pipeline_unknown_kind_no_fallback_uses_default_callout.rs`
  — same but no fallback; returns `Callout { kind:
  "warn", child: KeyValue { ... } }` per §R3 Path A
  bullet 2.
- `renderer_pipeline_panic_falls_through_to_path_a.rs` —
  register a test renderer that panics; pipeline logs
  at `tracing::error!`, then falls into Path A (author
  fallback if set, else default `Callout`); the wire
  output contains NO "panicked" diagnostic.
- `renderer_pipeline_renderer_err_falls_through_to_path_a.rs`
  — same but renderer returns `Err(_)`; pipeline logs at
  `tracing::warn!`, falls into Path A.
- `renderer_capabilities_downgrade_unsupported_node.rs` —
  render an entry whose tree contains an `Image` node;
  render with `Capabilities` reporting `nodes`
  excluding `Image`; pipeline downgrades to `Unknown
  { kind: "Image", payload: ..., fallback: <entry's
  fallback or default> }` per §R3 Path C.
- `renderer_capabilities_downgrade_unsupported_raw_format.rs`
  — pi-2 #7: tree contains `Raw { format: "html",
  body: ... }`; `Capabilities::raw_formats = {"ansi",
  "plain"}`; pipeline downgrades the `Raw` subtree to
  `Unknown` despite `Raw` being in `nodes`.
- `supervisor_spawn_unwinds_after_register.rs` — see
  §H6.3.
- `supervisor_spawn_post_register_reaps_child.rs`
  (Linux-only) — see §H6.3.
- `supervisor_spawn_unwinds_after_socketpair.rs` — see
  §H6.3.
- `supervisor_spawn_unwinds_post_spawn_pre_register.rs`
  — see §H6.2 (pi-18 #2 third inject point + pi-19
  #3 platform split). **Cross-platform** assertions:
  hook consumed (`post_spawn_pre_register_fault_consumed`),
  `SpawnError::SandboxBuild` returned, no broker
  registration acquired (`broker.is_registered(canonical)`
  false), `in_flight` cleared. The child is reaped
  synchronously via `child.wait().await` during
  unwind; on success the test asserts the spawn
  function returned without leaving a zombie
  (covered by reading exit_status from the wait
  call inside the unwind path).
- `supervisor_spawn_unwinds_post_spawn_pre_register_fd_baseline.rs`
  (Linux-only — `#[cfg(target_os = "linux")]`) —
  the Linux-specific complement that reads
  `/proc/self/fd` before and after the spawn-with-
  fault to assert fd count returns to baseline
  (the same check `unwinds_after_socketpair`
  uses).
- `frontend_handle_wait_resolves_on_child_exit.rs` — see
  §F4 (pi-6 B6 — round 6 had §F4 say `signal_ready`
  but §I say `respond_peer_call`; round 7 picks
  `signal_ready` everywhere because it exercises the
  readiness path and exits deterministically via
  `RFL_FIXTURE_MAX_LIFETIME=1`). Spawns
  `rfl-bus-fixture` (NOT `rfl-tui` — pi-2 #4) in
  `signal_ready` mode (§L1a) with
  `RFL_FIXTURE_MAX_LIFETIME=1` so the child exits
  after 1 s and `FrontendHandle::wait().await`
  returns `ReaperOutcome::Exited(status)` with
    `status.success()` true (pi-9 Medium 8 — round 9
    used a non-existent `Exited(0)` shape).
- `frontend_handle_drop_does_not_leak_zombie.rs`
  (Linux-only) — see §F4. Same rfl-bus-fixture-based
  approach.
- `frontend_handle_wait_ready_resolves_on_signal.rs` —
  pi-3 #1: at the rafaello-core layer, the contract is
  "`FrontendHandle::wait_ready()` resolves when the
  spawned child sends the `frontend.ready` RPC".
  Spawn `rfl-bus-fixture` in a new `signal_ready` mode
  (added in §F + the fixture extension below) that
  sends a single `peer.call("frontend.ready", json!({}))`
  on startup and then sleeps; assert
  `handle.wait_ready().await` returns `Ok(())`.
- `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs`
  — pi-3 #1: spawn a fixture child that exits without
  ever calling `frontend.ready`;
  `handle.wait_ready().await` returns
  `FrontendReadyError::SenderDropped`.
- `manifest_publishes_unknown_namespace_rejected.rs` —
  see §M1.2.

`rafaello-tui/tests/`:

- `tui_subscribes_to_core_session_events.rs` —
  spawn `rfl-tui` in `RFL_TUI_TEST_MODE`; from a
  parent-side broker fixture, publish one
  `core.session.entry.finalized`; assert the TUI
  process logs the event on its stderr and exits
  cleanly on a follow-up `core.lifecycle.test_done`.
  Uses `env!("CARGO_BIN_EXE_rfl-tui")` (valid because
  the test is in the same crate as the bin).
- `tui_paint_panic_isolation` — pi-5 B2 + pi-6 B5:
  paint-panic isolation is exercised as a
  **library-level unit test** inside
  `rafaello-tui/src/paint.rs` (`#[cfg(test)] mod
  tests`). The seam is structured to keep the
  production `RenderNode` wire enum unchanged
  (pi-6 B5 — round 6's "RawFormat::Test" trick was
  type-incorrect):
  ```rust
  // In rafaello-tui::paint:
  pub fn draw_with_panic_isolation(
      term: &mut ratatui::Terminal<impl Backend>,
      node: &RenderNode,
  ) -> Result<(), PaintError> { ... }

  #[cfg(test)]
  pub(crate) fn draw_with_panic_isolation_for_test(
      term: &mut ratatui::Terminal<impl Backend>,
      action: PaintAction,
  ) -> Result<(), PaintError> { ... }

  #[cfg(test)]
  pub(crate) enum PaintAction<'a> {
      Render(&'a RenderNode),
      RunPanicking,            // synthetic panic source
      RunReturningError,       // synthetic Err source
  }
  ```
  Production paint always uses
  `draw_with_panic_isolation(term, node)`. The
  `#[cfg(test)] PaintAction` variant `RunPanicking`
  triggers a deliberate panic inside the
  `catch_unwind` block; `RunReturningError` returns
  a `PaintError`. Test:
  ```rust
  let backend = ratatui::backend::TestBackend::new(80, 24);
  let mut term = ratatui::Terminal::new(backend)?;
  draw_with_panic_isolation_for_test(&mut term,
      PaintAction::RunPanicking)?;
  draw_with_panic_isolation_for_test(&mut term,
      PaintAction::Render(&RenderNode::Text {
          text: "ok".into(), emphasis: None }))?;
  // Assertions on TestBackend buffer: "[render error: ...]"
  // followed by "ok".
  ```
  No production-side `RenderNode`/`RawFormat`
  contamination. No cfg(test) variant on production
  enums. No subprocess, no headless mode.
- `tui_sends_frontend_ready_after_handler_registration.rs`
  — pi-2 #6 / pi-3 #2 / pi-16 #5: positive ordering
  test. Spawn `rfl-tui` in `RFL_TUI_TEST_MODE`. The
  TUI's startup sends `frontend.ready` only after
  its `BusEventHandler` is registered. Assertion
  (round 17 — replaces the round-6 `1 ms after`
  timing that pi-16 #5 flagged as flaky):
  the parent-side `FrontendReadyService` invokes a
  test-injected callback the moment the
  `frontend.ready` RPC handler runs (a
  `oneshot::Sender<()>` registered via the
  `FrontendExtraServiceFactory`); the test awaits
  that signal, **then** publishes a
  `core.session.entry.finalized` event, **then**
  awaits the TUI's in-memory log via a second
  callback when the handler observes the event. No
  wall-clock waits; the test fails deterministically
  if the ordering is wrong rather than passing
  flakily on schedule.

`rafaello/tests/`:

- `rfl_chat_demo_bar.rs` — **headline test, lands at
  the end of the milestone.** Spawn `rfl chat` against
  a tempdir project root with
  `RFL_HARNESS_FIXTURES=1` and `RFL_TUI_TEST_MODE=1`;
  let the parent + TUI run; assert (pi-16 #4):
  - The SQLite store contains nine `entries` rows
    after shutdown (eight built-in kinds + one
    unknown kind); each row's `kind`, `seq`, and
    `payload` match the harness inputs.
  - The combined stderr stream contains nine
    `"rfl-tui: bus.event topic=core.session.entry
    .finalized seq=N"` lines for `N = 0..=8` (the
    "renders all of them" assertion is the bus-event
    receipt count; m3 does NOT assert on rendered
    pixels because the headless TUI doesn't paint).
  The `rfl-tui` path is provided via `workspace_bin_path("rfl-tui")`
  (see §H below — single resolver shared with the
  `rfl-bus-fixture`-targeting tests).
- `rfl_chat_resolves_tui_via_env_override.rs` — see
  §C3 positive.
- `rfl_chat_locked_session_errors_with_holder_pid.rs` —
  hold the project flock in pid A (an in-test
  `SessionStore::open`); spawn `rfl chat` in pid B
  against the same project root; B exits non-zero with
  stderr citing pid A.
- `rfl_chat_relative_project_root_canonicalises.rs` —
  pi-17 #4: spawn `rfl chat --project-root ./relative/path`
  with cwd set to a tempdir; assert the spawned TUI
  receives `RFL_PROJECT_ROOT` as an absolute path
  (verify via the headless TUI's stderr — TUI prints
  `"rfl-tui: project-root=<abs-path>"` as part of
  startup sentinels per §T2 step 4).
- `rfl_chat_nonexistent_project_root_errors.rs` —
  pi-17 #4: `--project-root /nonexistent/path` →
  exit non-zero with `RflChatError::ProjectRootInvalid`.
- `rfl_chat_locked_session_unknown_holder_errors.rs` —
  pi-15 #4: pre-create `session.lock` empty and hold
  flock from a separate test thread (without writing
  a pid); spawn `rfl chat`; exits non-zero with
  stderr citing "unknown process" and pointing at
  the lockfile path.
- `rfl_chat_replay_withheld_until_frontend_ready.rs` —
  pi-3 #2 + pi-2 #6 + pi-4 #5: this assertion lives at
  the CLI layer because the readiness gate is in `rfl
  chat` orchestration, not in `SessionController`.
  Pi-4 #5 surfaced that round 4's
  `RFL_CHAT_TEST_OBSERVER_FD` seam was specified
  nowhere; round 5 drops it. The test observes via the
  TUI's headless-mode stderr log (§T2 step 4 already
  emits one stderr line per received `bus.event`):
  pre-seed the SQLite store with three entries;
  spawn `rfl chat` with `RFL_TUI_TEST_MODE=1` and
  `RFL_TUI_READY_DELAY_MS=200` (a new env-var
  override for the `rfl-tui` binary, parsed in §T2
  step 3 — it inserts a `tokio::time::sleep` BEFORE
  the `peer.call("frontend.ready", ...)` so the
  parent's readiness wait is delayed); capture
  set `RFL_TUI_MAX_LIFETIME=2` so the headless TUI
  self-terminates after 2 s without needing a
  `core.lifecycle.test_done` event (pi-7 #6 — round
  7's test pre-seeded entries via SQLite without
  going through `RFL_HARNESS_FIXTURES`, so no
  test_done event would have been published; default
  60 s lifetime would have hung the test). Capture
  the **single combined stderr stream** from `rfl
  chat` (pi-8 M2 — round 8 used two separate streams
  with read-time timestamps which is scheduler-flaky.
  Round 9: `rfl chat` line-buffers the spawned
  `rfl-tui` child's stderr and re-emits each line on
  its own stderr with a `"rfl-tui: "` prefix; see
  §C2 step 6's stderr-pipe note. Single combined
  stream is line-ordered because parent and
  forwarder writes both go through the
  `tokio::sync::Mutex`-guarded stderr writer (pi-11
  #5 — round 11 left a stale "single forwarding
  task" wording that conflicted with §C2 step 6's
  mutex-based scheme). Assert (pi-6 B3):
  - `"rfl-chat: frontend-ready-observed"` appears on
    rfl chat's stderr (§C2 step 7's `Ok(Ok(()))` arm
    prints this after `wait_ready` resolves and
    BEFORE `controller.replay_history()` is called);
  - NO `"rfl-tui: bus.event"` line appears before
    that sentinel (line-order assertion only — no
    wall-clock timestamps);
  - exactly three `"rfl-tui: bus.event"` lines
    follow the sentinel before process exit.
- `rfl_chat_frontend_exits_before_ready_errors.rs` —
  pi-10 Medium 4: SenderDropped CLI path. Spawn
  `rfl chat` with `RFL_TUI_PATH` pointing at
  `rfl-bus-fixture` in `exit_immediately` mode
  (§L1a — adopts bus fd, exits 0 without sending
  `frontend.ready`). `rfl chat` exits non-zero with
  `RflChatError::FrontendExitedBeforeReady` mapped
  to stderr citing the child's exit status. 6-second
  `serial_test` gate caps wall-clock.
- `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
  — pi-11 #4: post-ready abnormal-exit path. Spawn
  `rfl chat` with `RFL_TUI_PATH` pointing at
  `rfl-bus-fixture` in `signal_ready_then_exit_n`
  mode with `RFL_FIXTURE_EXIT_CODE=7`. `rfl chat`
  observes `frontend.ready`, replays history (zero
  rows in this test), waits on the TUI, sees
  `Exited(status)` with `status.code() == Some(7)`,
  exits non-zero with `RflChatError::
  FrontendExitedAbnormally` and stderr citing
  exit code 7.
- `rfl_chat_frontend_ready_timeout_errors.rs` — pi-3 #2 +
  pi-2 #6 + pi-5 M4: spawn `rfl chat` with
  `RFL_TUI_PATH` pointing at `rfl-bus-fixture` in
  `hold_silent` mode (§L1a — adopts the bus fd,
  holds the connection open, never calls
  `frontend.ready`). `rfl chat` exits with
  `RflChatError::FrontendReadyTimeout` (mapped to
  exit code 1 + stderr message). The test sets
  `RFL_FIXTURE_MAX_LIFETIME=10` on the spawned
  fixture so it self-exits if `rfl chat` fails to
  SIGTERM it. 6-second `serial_test` gate caps
  wall-clock.

#### Negative matrix

`rafaello-core/tests/`:

- `frontend_publish_on_reserved_namespace_rejected.rs`
  — synthesise a frontend publish on `core.foo`,
  `plugin.foo`, `provider.foo`; broker rejects with
  `PublishOnReservedNamespace { publisher:
  Publisher::Frontend("tui"), topic }`.
- `frontend_publish_two_segment_topic_rejected.rs` —
  pi-18 #3: a frontend publish on `frontend.tui`
  (own attach-id, 2 segments only) → broker rejects
  with `PublishOnReservedNamespace { publisher:
  Publisher::Frontend("tui"), topic: "frontend.tui" }`.
  Symmetric to m2's two-segment `plugin.<id>`
  rejection rule.
- `frontend_publish_unknown_namespace_rejected.rs` —
  pi-5 M5: a frontend publish on `evil.foo` /
  `random.thing` (top-level segment outside
  `{core, provider, plugin, frontend}`) is
  `UnknownNamespace`, NOT
  `PublishOnReservedNamespace`. Symmetric with the
  m2 plugin behaviour.
- `frontend_publish_outside_grant_rejected.rs` — TUI
  attempts `frontend.tui.confirm_answer` (NOT in m3's
  `publish_topics`); broker rejects with
  `PublishOutsideGrant { publisher:
  Publisher::Frontend("tui"), topic }`.
- `frontend_register_unknown_attach_id_rejected.rs` —
  ACL has only `tui`; `register_frontend(AttachId::new(
  "ide"), ...)` fails with `BrokerError::FrontendNotInAcl`.
- `frontend_register_duplicate_rejected.rs` —
  `BrokerError::FrontendAlreadyRegistered`.
- `frontend_spawn_invalid_attach_id_rejected.rs` —
  `attach_id` not matching the regex →
  `FrontendSpawnError::InvalidPlan { reason:
  AttachIdInvalid }`.
- `frontend_spawn_relative_entry_path_refused.rs`,
  `frontend_spawn_control_chars_in_path_refused.rs`,
  `frontend_spawn_entry_not_executable_refused.rs`,
  `frontend_spawn_reserved_env_in_pass_refused.rs`,
  `frontend_spawn_reserved_env_in_set_refused.rs` —
  m2 §SP4 Phase A pattern, replicated.
- `session_store_concurrent_open_errors.rs` — open store
  in pid A; spawn a probe child that calls `open()` on
  the same path; child gets `SessionError::Locked {
  holder_pid: Some(A) }`. Cross-platform Linux + macOS.
- `session_store_locked_unknown_holder_errors.rs` —
  pi-14 #7: pre-create `session.lock` empty and
  acquire flock from a separate test thread (without
  writing a pid); a second `SessionStore::open` call
  errors with `SessionError::Locked { holder_pid:
  None }`. Same test class for a corrupt lockfile
  (random non-numeric bytes).
- `session_store_schema_mismatch_errors.rs` — open store
  whose `session_meta.schema_version = "0"` (manually
  pre-seeded); errors with `SessionError::SchemaMismatch
  { found: "0", expected: "1" }`.

### H — test harness

Module placement (pi-1 #5): renderer / store / broker
helpers go in `rafaello/crates/rafaello-core/tests/common/m3_harness.rs`
(reuses m2's `m2_harness.rs` precedent). TUI-spawning
helpers go in `rafaello/crates/rafaello-tui/tests/common/tui_harness.rs`,
because `env!("CARGO_BIN_EXE_rfl-tui")` is only sound in
the rafaello-tui crate's tests.

`m3_harness.rs` (in rafaello-core):

- `FixtureEntryBuilder` — fluent builder for a synthetic
  `Entry` with each built-in kind. Reused by every
  positive renderer test.
- `TestSessionStore::open_in_tempdir() -> (SessionStore,
  TempDir)` — common setup for store tests.
- `in_memory_broker_with_tui_and_observer_acl()` —
  constructs a `Broker` with: (a) a `tui` frontend ACL
  with `core.session.**` / `core.lifecycle.**`
  subscribe set; (b) a synthetic `observer` PLUGIN
  ACL (pi-7 #7 — round 7 had only the tui frontend
  ACL but `record_subscriber` needs to register as a
  plugin to receive events; round 8 adds an
  observer-plugin entry). The observer plugin's ACL
  has the same subscribe set as the tui frontend and
  empty publish authority.
- `record_subscriber(broker)` — registers an in-process
  plugin-shaped subscriber on the `observer` ACL
  entry that records every event into a `Mutex<Vec<BusEvent>>`.
  Used by §I controller tests
  (`session_controller_finalize_entry.rs`,
  `session_controller_replay_history.rs`).

`tui_harness.rs` (in rafaello-tui):

- `tui_test_mode_command()` — wraps
  `Command::new(env!("CARGO_BIN_EXE_rfl-tui"))
  .env("RFL_TUI_TEST_MODE", "1")
  .env("RFL_BUS_FD", ...)` for the headless TUI mode.
- `parent_socket_pair()` — creates a socketpair and
  returns the parent end as a `tokio::net::UnixStream`,
  child fd as `OwnedFd` to inherit.

`workspace_bin_path.rs` (in rafaello/tests/common/, the
CLI test layer): pi-15 #5 — the rafaello/tests/ negative
tests need to point `RFL_TUI_PATH` at both `rfl-tui`
(rafaello-tui crate) AND `rfl-bus-fixture` (rafaello-core
crate); `env!("CARGO_BIN_EXE_*")` only works for binaries
in the SAME crate as the test, so neither resolves cleanly
from `rafaello/tests/`. Round 16 introduces a shared
helper:
```rust
// rafaello/tests/common/workspace_bin_path.rs
pub fn workspace_bin(name: &str) -> PathBuf {
    // Prefer CARGO_TARGET_DIR if set; otherwise walk up
    // from CARGO_MANIFEST_DIR to the workspace root and
    // use `target/<profile>/<name>`. Profile is "debug"
    // for cargo test, "release" for cargo test --release.
    // Returns an absolute path; panics if the binary is
    // not present (test harness should run cargo build
    // --workspace --bins --features rafaello-core/test-fixture
    // first; the m3 CI workflow does this).
}
```
The headline `rfl_chat_demo_bar.rs`, the negative tests
that point `RFL_TUI_PATH` at `rfl-bus-fixture` modes, and
the env-override end-to-end test all use this helper.

Manual validation: §I integration tests are the contract;
m3's `manual-validation.md` records:

1. `nix develop --impure --command cargo test --manifest-path
   rafaello/Cargo.toml --workspace --features test-fixture`
   green on Linux + macOS (pi-18 #5 — match the
   acceptance-section command exactly).
2. A real interactive `rfl chat` session against the in-
   test fixture-entry harness (i.e. with
   `RFL_HARNESS_FIXTURES=1`), screen-recorded; verify
   eight built-in kinds render readably; verify
   unknown-kind falls back to the author-supplied
   fallback text; verify `q` quits cleanly
   restoring the terminal; verify second `rfl chat`
   in the same project errors with the holder pid.
3. CI green on Linux + macOS.

### Fixture process leak mitigation (m2 retro §4.4 carryover)

m2 retro §4.4 surfaced fixture process leaks when a test
panics before `SpawnHandle::Drop` runs. Two options were
filed; m3 picks the **fixture self-timeout** option (option
2):

- **L1.** Extend `rfl-bus-fixture`'s every long-running mode
  (`respond_peer_call`, `observer`) to read
  `RFL_FIXTURE_MAX_LIFETIME` env (seconds; default
  `60` if unset) and `std::process::exit(0)` after that
  even without SIGTERM.
- **L1a (m3 fixture additions).** Five new modes for
  m3 (pi-5 M4 added `hold_silent`; pi-8 M5 added
  `probe_fd_closed`; pi-11 #4 added
  `signal_ready_then_exit_n`; pi-13 #5 — count
  updated):
  - `signal_ready` — adopts `RFL_BUS_FD`, runs fittings
    `Client::serve` so it can receive notifications,
    sends one `peer.call("frontend.ready", json!({}))`
    on startup, then sleeps until SIGTERM (or
    `RFL_FIXTURE_MAX_LIFETIME`). Used by
    `frontend_handle_wait_ready_resolves_on_signal.rs`.
  - `exit_immediately` — exits 0 without sending
    `frontend.ready`. Used by
    `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs`.
  - `hold_silent` — adopts `RFL_BUS_FD`, runs fittings
    `Client::serve`, holds the connection open
    indefinitely WITHOUT calling `frontend.ready`.
    Used by `rfl_chat_frontend_ready_timeout_errors.rs`
    (pi-5 M4 — the timeout case requires a child
    that holds the bus connection but never signals
    readiness; `exit_immediately` would trip
    `SenderDropped` instead, and a child that doesn't
    adopt `RFL_BUS_FD` would trip transport failure).
  - `signal_ready_then_exit_n` (pi-11 #4) — adopts
    `RFL_BUS_FD`, sends `frontend.ready`, sleeps 200
    ms (so `rfl chat`'s readiness gate releases and
    replay can complete), then `std::process::exit(N)`
    where `N` is read from `RFL_FIXTURE_EXIT_CODE`
    env (default 7). Used by
    `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
    to exercise the step-10 abnormal-exit path.
  - `probe_fd_closed` (pi-8 M5) — takes a CLI arg
    `--probe-fd <N>`, calls
    `nix::fcntl::fcntl(N, FcntlArg::F_GETFD)`, exits
    0 if the call returns `EBADF`, non-zero otherwise.
    Used by `session_store_lock_fd_not_inherited_by_child.rs`
    to portably verify the lock fd was NOT inherited
    across exec (replaces round-8's `lsof`-on-macOS
    approach).
  All five modes reuse the existing fixture
  transport scaffolding from m2 c20.
- **L2.** Tests use the 60 s default unless they need a
  deterministic shorter bound (pi-9 Low 9 — round 9
  said "tests don't override the default" but several
  m3 tests intentionally set `RFL_FIXTURE_MAX_LIFETIME=1`
  / `=2` for fast wait-resolution; both forms are
  valid). The default keeps panicked / abandoned
  worktrees from leaking fixture processes for hours
  (m2 c20 saw a >1 h orphan).
- **L3.** Driver-side reaper (option 1) is rejected because
  it is operationally fragile (greps `pgrep -f` on
  worktree paths) and only catches the m2/m3 driver's
  own runs — local devs running `cargo test` outside the
  driver lose the property. Option 2 is a 5-line code
  change with permanent benefit.

### TestHooks new accessors summary

New / extended in m3 (added on top of m2's struct):

```rust
impl TestHooks {
    // m2 fields unchanged.
    pub fn inject_pre_spawn_fault(&self);
    pub fn inject_post_spawn_pre_register_fault(&self);
    pub fn inject_post_register_fault(&self);
    pub fn pre_spawn_fault_consumed(&self) -> bool;
    pub fn post_spawn_pre_register_fault_consumed(&self) -> bool;
    pub fn post_register_fault_consumed(&self) -> bool;
}
```

m3 does NOT add `TestHooks` for `FrontendSupervisor`
in this milestone — frontend spawns have a much shorter
critical section (no proxy startup, no lockin builder),
the unwind windows are correspondingly smaller, and the
m3 negative matrix for frontend spawns covers them
through the public `FrontendSpawnError` surface alone.
m4 may add frontend-side hooks if a fault scenario
materialises.

## Out of scope

The following are explicitly NOT in m3 and are not allowed
to sneak in via "while I'm here" implementation drift.

- **Provider plugins / agent loop / tool dispatch.** m4. The
  m4 milestone owns `provider.<provider-id>.*` publish
  authority, `core.session.tool_request` /
  `tool_result` re-emission with the canonical taint
  envelope, and the bundled mock provider plugin. m3's
  TUI receives `core.session.entry.finalized` events
  only.
- **Sink confirmation, `user_grants`, taint synthesis,
  taint superset enforcement.** m5.
- **External UDS-attached frontends, `rfl serve`, attach
  socket / token, `frontend.hello` capability handshake.**
  Deferred per decisions rows 27 + 34. m3's TUI is the
  only frontend; capabilities are baked in core-side as
  compile-time constants per attach-id.
- **Subprocess plugin renderers (`renderer.render`).**
  Deferred per row 29. m3 ships only the eight built-in
  kinds.
- **Streaming entry patch ops** (`stream_state: "open"` /
  `"patch"`, `core.session.entry.appended` /
  `core.session.entry.patched` notifications). Deferred
  per row 28. m3 emits `core.session.entry.finalized`
  only with `stream_state: "final"`.
- **Multi-session daemon, attach-multiplexing, branching
  (`parent` field non-NULL).** Post-v1.
- **TUI confirmation modal, `/grant` slash command,
  command palette.** m5 + post-v1.
- **TUI publishing on the bus.** m3's frontend has
  `publish_topics = []`. The `frontend.tui.confirm_answer`
  topic (m5) and `frontend.tui.user_message` topic (m4)
  are not granted in m3. Pi may push back: the m4
  driver could prefer m3 to grant `user_message` ahead
  of time so m4 doesn't change the m3 ACL. Round-1
  takes the conservative "no publish authority" cut to
  keep the m3 negative matrix unambiguous; m4 can
  open up the grant when it lands.
- **Lazy-load orchestrator, `rfl plugin install / start /
  list`.** m4 + later milestones. m3's `rfl chat` does
  not spawn any plugins (the broker has zero plugin
  ACL entries).
- **Audit log table** (m5 confirmation answers).
- **Helper plugins** (deferred per row 26; m1 + m2 already
  guard the surface).
- **Manual interactive macOS smoke testing** (i.e. a
  human-driven smoke recording on a macOS box). m3
  dev work is on Linux; macOS is verified through CI
  only. The CI cargo-test green is a HARD gate per
  the Acceptance section; the manual recording is
  Linux-only. (Pi-6 M2 — round 6's "verified
  post-hoc" wording for macOS contradicted Acceptance;
  round 7 reframes: cross-platform CI green is the
  gate, but interactive end-to-end recording is
  Linux-only.)
- **`PluginSupervisor` extensions beyond `inject_fault`.**
  m3 does NOT introduce per-plugin shutdown,
  `bindings.helper_for`, or any new spawn-time validation
  beyond what m2 shipped.

## Risks

1. **Crossterm + ratatui macOS gotchas.** ratatui 0.29
   uses crossterm 0.28; both are nominally cross-platform
   but real-terminal smoke tests on macOS sometimes
   surface escape-sequence differences. Mitigation: every
   TUI integration test runs in a `RFL_TUI_TEST_MODE`
   headless mode (no terminal init) and asserts on the
   render-tree the TUI received, not on terminal output.
   The end-to-end "real terminal" test is captured only
   in `manual-validation.md` (manual screen recording).
2. **SQLite WAL files in tempdirs.** macOS tempdirs are
   under `/var/folders/...` which can be unusually deep;
   SQLite's WAL aux files (`-wal`, `-shm`) sit in the
   same dir. `rusqlite` 0.32 with bundled feature
   handles this correctly; mitigation is to assert
   nothing about path lengths and use `tempfile::TempDir`
   throughout.
3. **flock cross-platform.** `nix::fcntl::flock` works on
   both Linux and macOS but the underlying `flock(2)` vs
   `fcntl(F_SETLK)` semantics differ subtly (NFS, fork
   inheritance). m3 only uses `flock` from a single
   process holding the fd for its lifetime; the
   pathological cases don't apply. Verified by the
   negative test on both platforms.
4. **Frontend bypass-of-lockin.** `FrontendSupervisor`
   does NOT call lockin. Risk: someone copy-pastes the
   spawn body into a new plugin path later and forgets
   to add lockin. Mitigation: a comment block at the top
   of `frontend.rs` calls out "frontends are NOT
   sandboxed (decisions row 15) — do NOT use this
   module as a template for plugin spawning". m1's
   `lib.rs` re-exports do not lift `FrontendSupervisor`
   into the same namespace as `PluginSupervisor`; they
   live in clearly distinct modules.
5. **Renderer panic isolation under tokio.**
   `catch_unwind` requires `UnwindSafe`; `Entry` and
   `Capabilities` derive `UnwindSafe` (no interior
   mutability). The renderer trait object is wrapped in
   `AssertUnwindSafe` because the trait does not impose
   `UnwindSafe` (impractical for arbitrary impls). The
   pipeline does not share state with renderers — each
   call is a pure function — so the assertion is sound.
6. **CI workflow coverage.** m3 introduces a new bin
   target (`rfl-tui`), a new crate (`rafaello-tui`),
   and a new feature (none in m3, but the existing
   `test-fixture` gate must continue to apply to
   `rafaello-core`). The CI workflow's
   `cargo test --workspace --features test-fixture` in
   `rafaello/Cargo.toml` is m3's baseline; m3 explicitly
   pushes to CI mid-milestone (not at retrospective)
   per m2 §5.7 lesson.
7. **`tui-input` dep choice.** Pi may push back; the
   alternative is hand-rolling. Round-1 takes
   `tui-input` because m4 and m5 will need a richer
   editor (multi-line, command-history) and the dep
   carries that for free.
8. **Replay event class collapsed (round 2).** Round 1
   used a separate `core.session.entry.replay` topic;
   pi-1 #1 / #9 surfaced that this had no Stream E
   anchor and broke the TUI subscribe set. Round 2
   uses one topic (`core.session.entry.finalized`) with
   a `replay: bool` payload-envelope flag. m4 may
   revisit if a richer event class becomes useful, but
   v1 keeps the topic set minimal.
9. **TUI subscription readiness handshake.** §C2 step 7
   waits on `FrontendHandle::wait_ready()` (round 5 —
   the `frontend.ready` RPC method flips a
   `tokio::sync::watch::Sender<bool>` to `true`; the
   handle holds the matching `Receiver<bool>` and
   re-checks `borrow()` after each `changed().await`)
   before replaying / publishing any entry;
   without it, fan-out to a not-yet-subscribed frontend
   is silently dropped (Stream B notification sink is
   bounded, drop-on-full). The 5 s bounded wait is a
   defensive ceiling — real readiness should fire
   within hundreds of ms. The RPC flows on the
   parent↔TUI fittings *connection*, not the bus, so
   no broker ACL is involved.
10. **m1 publishes-grant patch back-reach.** Touching
    `validate::manifest` in m3 is a small back-reach to
    m1 (m2 c04 set the precedent). The tightening is
    additive — manifests that previously accepted
    unknown namespaces would have failed at runtime
    anyway. Mitigation: a one-line note in the commit
    body marks it as the m3-owned m1 patch and points
    at m2 retro §2.8.
11. **Demo-bar headline test spawning a real subprocess
    chain from inside cargo test.** The pattern is a
    stretch of m2's `supervisor_spawn_fixture_happy_path`
    precedent — the rafaello bin spawned from a test,
    which then spawns rfl-tui as a subprocess. Two-
    level subprocess chains can leak processes if any
    layer panics. Mitigations: (a) the L1 fixture self-
    timeout pattern extends to `rfl-tui`'s
    `RFL_TUI_TEST_MODE` (60 s default); (b) the
    deterministic `core.lifecycle.test_done` exit
    signal (§T2 step 4) keeps the test bounded under
    1 s on the happy path; (c) the in-test rafaello
    parent process registers a SIGCHLD-style cleanup so
    a panic in the test propagates kill to both
    children before unwinding.
12. **Frontend reaper / wait race.** The reaper task
    owns the `tokio::process::Child`. `FrontendHandle::
    wait()` resolves on a `tokio::sync::watch` whose
    value transitions from `None` to `Some(Arc<
    ReaperOutcome>)`. Late `wait` callers see the
    cached `Arc` immediately. Same shape as m2 §SP4
    step 18 (pi-5 §1).
13. **`O_CLOEXEC` cross-platform.** Linux (3.x+) and
    macOS (10.7+) both support `O_CLOEXEC` on `open()`.
    `OpenOptions::custom_flags` is the cross-platform
    portable shim. No `fcntl(F_SETFD, FD_CLOEXEC)`
    follow-up needed (unlike socketpair on macOS,
    m2 §5.7).

## Internal split (driver guidance for `commits.md`)

Suggested grouping; `commits.md` picks final granularity.
Pi review may reshape these. m3 is comparable in depth to
m2 — expect 25-32 commits sequentially, possibly with an
m3a/m3b split point after the broker frontend extension
+ session store land but before the TUI crate work begins.

1. **Workspace deps + crate scaffold + m1 unknown-namespace
   patch** (W1-W4 + M1): ~3-4 commits.
2. **TestHooks fault injection + the three deleted unwind
   tests re-added** (H6): ~2 commits. Lands early so the
   m2 §5.1 coverage gap closes before any new code piles
   on top of `PluginSupervisor`.
3. **Entry + RenderTree types** (E1-E4): ~2-3 commits.
4. **Renderer pipeline + built-in renderers** (R1-R6): ~4-5
   commits. Possibly one commit per kind cluster (text +
   heading; code_block + thinking; tool_call + tool_result;
   image + error) if per-commit greenness budget is
   tight.
5. **Broker frontend extension** (B1-B6): ~3 commits.
   Splits into `BrokerAcl` extension; `register_frontend`
   + RAII guard; publish authority + fan-out wiring.
6. **Frontend supervisor** (F1-F5): ~3 commits.
7. **Session store** (S1-S7): ~2-3 commits. Schema +
   round-trip happy path; flock + concurrent-error;
   replay flow.
8. **Fixture self-timeout** (L1-L2): ~1 commit (small
   m2 fixture extension).
9. **TUI crate scaffold + headless test mode** (T1-T2,
   partial): ~2 commits.
10. **TUI render-tree painter** (T3-T5): ~2-3 commits.
11. **`rfl chat` subcommand wiring** (C1-C3): ~2 commits.
12. **Demo-bar headline + manual validation** (the
    `rfl_chat_demo_bar.rs` test + `manual-validation.md`):
    ~2 commits.

Realistic total: **~28 commits sequential**. Comparable to
m2's 31. The m3a/m3b checkpoint is between groups 8 and 9
— after the headless-renderer pipeline + session store
land, before the TUI crate work begins. If a split
materialises during Phase 3 (e.g. ratatui surfaces a
macOS-only blocker), the split is owner-ratified mid-
milestone; default is "ship m3 as one milestone".

## Acceptance summary

m3 is done when:

- Every named test in the §"Positive" and §"Negative"
  matrices is implemented and passes. Tests may split or
  merge during `commits.md` drafting as long as the named
  behaviours are all covered.
- `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml --workspace --features test-fixture`
  green on Linux inside the devshell.
- **macOS CI green is now a hard ratification gate**
  (pi-5 M11): m3 ships a user-facing TUI/session
  surface with cross-platform consequences (flock
  semantics, terminal handling, fd inheritance), so
  m3 cannot ratify with macOS deferred. The
  `cargo test --workspace --features test-fixture`
  job on `macos-latest` must be green before
  retrospective ratification, with the only
  exception being tests explicitly gated
  `#[cfg(target_os = "linux")]` (e.g.
  `frontend_handle_drop_does_not_leak_zombie.rs`,
  `supervisor_spawn_post_register_reaps_child.rs`).
  Tests that fail on macOS for reasons other than
  inherent platform limits must be fixed in m3, not
  deferred to retrospective. (Round 5 said macOS
  failures could be retrospective-time follow-ups;
  round 6 tightens this to match the user-facing
  scope.)
- `nix develop --impure --command cargo build --manifest-path
  rafaello/Cargo.toml --workspace --bins --features
  rafaello-core/test-fixture` green (pi-1 #13 — the
  fixture bin is `required-features = ["test-fixture"]`
  on the `rafaello-core` crate, so `--workspace --bins`
  alone skips it; the explicit feature flag is required).
  Verifies `rfl`, `rfl-tui`, and `rfl-bus-fixture` all
  build.
- `nix develop --impure --command cargo doc --manifest-path
  rafaello/Cargo.toml --workspace --no-deps` warning-free.
- `manual-validation.md` records the items in the manual-
  validation list above (real interactive `rfl chat` run +
  macOS CI URL).
- `retrospective.md` written, with anticipated drift items
  addressed:
  - **Stream E renderer-RFC drift patch.** §7 (patch ops)
    and §8 (`frontend.hello`) and §9 (subprocess
    `renderer.render`) are unimplemented in v1 per
    rows 28 / 27 / 29. Following the m1 banner
    precedent, m3 lands a v1-status banner at the top
    of `streams/e-renderer/rfc-renderer-model.md`
    pointing at the relevant decisions rows. Already
    pre-named by `milestones/README.md` §"Stream RFC
    drift".
  - **`PublisherIdentity::Frontend` schema additions to
    Stream A.** Symmetric to m2's banner addition for
    `Plugin`. The Stream A wire-schema banner expands
    from "m2 wire schemas" to include
    `PublisherIdentity::Frontend { attach_id }` once
    m3 promotes it.
  - **Capabilities staging note in overview §10.1.**
    m3's TUI has compile-time-baked capabilities, not
    a `frontend.hello` handshake (per the row 27
    deferral). Overview §10.1's banner already says
    this; m3's retrospective records the concrete
    indexing scheme used (per attach-id constants).
  - **Replay over `core.session.entry.finalized` with
    `replay: bool` envelope flag.** Round-2 collapse
    away from a separate `entry.replay` topic. New
    decisions row pins the canonical wire shape and the
    `finalized.replay` flag's semantics (true on history
    replay, false on fresh entries). Round 2 is open to
    a metadata-on-`EntryMetadata` flag instead — the
    rewire is mechanical; choice is whichever pi prefers
    by ratification.
  - **Broker error variant additions.** Round 2 adds
    `BrokerError::FrontendNotInAcl`,
    `FrontendAlreadyRegistered`, `FrontendNotRegistered`,
    and reshapes existing `PublishOutsideGrant` /
    `InvalidInReplyTo` to take `publisher: Publisher`
    rather than `canonical: CanonicalId`. The reshape is
    source-breaking for m2's tests but contained to
    `rafaello-core`; retrospective records the migration.
  - **m1 publishes-grant unknown-namespace tightening
    (m2 retro §2.8).** Lands as the §M1 patch within
    m3; retrospective records the back-reach and points
    at `manifest_publishes_unknown_namespace_rejected.rs`
    as the regression baseline.
  - **m1 lock-side `check_lock_publish_topic`
    unknown-namespace gap** (pi-18 #4 / pi-19 #2 —
    `check_publish_topic` (manifest side) was
    tightened by §M1 to reject unknown top-level
    segments, but `check_lock_publish_topic` (lock
    side) still accepts them via its `_ => {}` arm.
    m3 leaves this as runtime-only enforcement at
    the broker; the rationale: hand-authored locks
    are an `--allow-unsafe` user-override path
    where runtime rejection is sufficient defence.
    Recorded for m4 if a user-facing failure
    surfaces.).
  - **`FrontendSupervisor` lock-correspondence claim
    extension.** Same v2 nice-to-have as m2 §2.6, now
    covering both supervisors.
  - **Fixture self-timeout (`RFL_FIXTURE_MAX_LIFETIME`).**
    Lands as a m2-fixture patch in m3; retrospective
    records the choice and explicitly documents the
    rejection of the driver-side reaper alternative.
  - **m3 frontend ACL grants nothing on
    `publish_topics`.** m4's first action will be to
    extend the grant for `frontend.tui.user_message`;
    m5's for `frontend.tui.confirm_answer`. The m3
    retrospective files this as an anticipated m4 / m5
    handover, not an m3 issue.
- No follow-up Stream RFC drift is owed by m3 BEYOND the
  items above. m3 does NOT modify the Stream E RFC body
  in this branch (banner-only, m1 precedent).

m3 ships the first running session: a user types `rfl
chat`, sees a TUI, sees rendered entries, exits cleanly,
and the next `rfl chat` replays them. Every later
milestone (m4 agent loop, m5 confirmation + sinks) layers
on this primitive.
