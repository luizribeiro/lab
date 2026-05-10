# m3-tui-sessions — commits

> **Status:** round-2 draft. Round 1 returned 6 blockers + 4
> high + 5 medium (`commits-pi-review-1.md`). Round 2
> restructures: (a) moves the **rfl-bus-fixture extensions
> (formerly c25)** to before the frontend supervisor group
> so c19+ frontend tests can use the new modes without
> forward references; (b) moves the
> **`shutdown_with_outcome` extraction** from c06 into the
> frontend group alongside `FrontendHandle::shutdown` (it
> was nonsensical in c06 — those types don't exist yet);
> (c) fixes the c01/c03 workspace-member registration
> contradiction; (d) renumbers — m3 now has **34 commits
> in 12 groups** (round-1 said 11 groups but had 12; pi
> medium #5). All other pi findings are addressed inline.
> Awaiting pi convergence.

Drafted from `scope.md` (round 22 — converged after 22 pi
rounds, 6 zero-blocker rounds; commit `da9473c`). Each
commit is one logical idea **and leaves the workspace
green** — pre-commit hooks (rustfmt + clippy + cargo test)
gate every commit; intermediate non-green states are not
allowed. Commits land sequentially on per-commit branches
`agents/m3/c<NN>` rebased onto `rafaello-v0.1`, no merge
commits, no force pushes. Tests land with the code that
exercises them per `~/.claude/CLAUDE.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes:
  `rafaello-core`, `rafaello-tui`, `rafaello`,
  `rfl-bus-fixture`, `rafaello` (workspace),
  `rafaello-m3` (docs).
- "Acceptance" lists new tests + the pre-commit invariants
  the commit must keep green.
- "Depends on" cites the *lowest* commit numbers whose code
  or types this commit references. A commit only lands after
  every declared dependency has landed on `rafaello-v0.1`.
- Test files live per scope §I's placement rules:
  - `rafaello-core/tests/` — broker, session store, renderer
    pipeline, supervisor (incl. fault-injection), manifest.
  - `rafaello-tui/tests/` — anything spawning `rfl-tui`
    (uses `env!("CARGO_BIN_EXE_rfl-tui")`).
  - `rafaello/tests/` — the headline `rfl chat` end-to-end
    tests (uses `env!("CARGO_BIN_EXE_rfl")`; resolves
    `rfl-tui` and `rfl-bus-fixture` via the
    `workspace_bin_path` helper).
- Per-commit agents pre-flight `nix develop --impure
  --command cargo test --manifest-path rafaello/Cargo.toml
  --workspace --features rafaello-core/test-fixture` until
  green before invoking pre-commit hooks.
- **Every per-commit agent prompt MUST include `--features
  rafaello-core/test-fixture`** in cited cargo invocations
  for any test that depends on the fixture binary.
- Per the m1 lesson §4.2, the milestone driver inlines the
  full row text + every acceptance bullet verbatim into each
  per-commit prompt; agents do NOT re-read `commits.md`.
- **Driver-owned actions, NOT per-commit agent actions:**
  pushing branches to origin, capturing CI URLs, writing
  `retrospective.md` (Phase 4, not a per-commit task).

## m3a / m3b checkpoint

No internal split is planned. The driver re-evaluates after
**c14** (renderer pipeline complete) and after **c25**
(session store + controller landed); if a split becomes
obviously beneficial, the driver opens an m3a / m3b
owner-ratification request.

## Canonical test names

Wherever `scope.md` and `commits.md` both name a test, this
`commits.md` is canonical. The headline test is
**`rfl_chat_demo_bar.rs`** (lands at c33).

---

## Group 0 — Foundation: workspace deps + crate scaffolds + lib targets

### c01 — chore(rafaello): add m3 deps to `[workspace.dependencies]` (deps-only)

- **What.** scope §W1 — two concrete edits to
  `rafaello/Cargo.toml`. NO workspace-member edit (pi-1
  Blocker #1: members edit folded into c03):
  - **Edit existing entries**: `tokio.features` adds
    `"process"` to the existing array.
  - **Add new entries**: `ratatui = "0.29"`,
    `crossterm = "0.28"`,
    `rusqlite = { version = "0.32", features = ["bundled"] }`,
    `tui-input = "0.10"`, `unicode-width = "0.2"`,
    `ulid = { version = "1", features = ["serde"] }`.
  Existing `chrono`, `tempfile`, `serial_test`,
  `tracing-test`, `tracing-subscriber`, `members` list
  unchanged.
- **Why.** scope §W1.
- **Depends on.** baseline.
- **Acceptance.** `cargo metadata --manifest-path
  rafaello/Cargo.toml --format-version 1` succeeds.
  `cargo build -p rafaello-core` green. `cargo doc -p
  rafaello-core --no-deps` warning-free.

### c02 — chore(rafaello-core): wire m3 deps to `rafaello-core/Cargo.toml`

- **What.** scope §W2: `[dependencies]` adds `rusqlite`,
  `ulid`, `chrono` with `workspace = true`. No new
  features (the existing `test-fixture` feature is
  unchanged per scope §W5).
- **Why.** scope §W2 — renderer types and SessionStore
  live in rafaello-core; ratatui/crossterm do NOT.
- **Depends on.** c01.
- **Acceptance.** `cargo build -p rafaello-core` green.
  `cargo build -p rafaello-core --features test-fixture`
  green. m2's 357-test suite still passes.

### c03 — feat(rafaello-tui): scaffold the new crate + register workspace member

- **What.** scope §W3 + pi-1 Blocker #1:
  - Create `rafaello/crates/rafaello-tui/`:
    `Cargo.toml` with `[package] name = "rafaello-tui"`,
    `version = "0.0.0"`, `edition = "2021"`. `[lib]`
    target. `[[bin]] name = "rfl-tui", path =
    "src/bin/rfl_tui.rs"`. `[dependencies]`:
    `rafaello-core` (path-dep), `ratatui`, `crossterm`,
    `tui-input`, `tokio`, `tracing`, `fittings-core`,
    `fittings-server`, `fittings-client`,
    `fittings-transport`, `serde`, `serde_json`,
    `async-trait`, `anyhow`, `unicode-width` — all with
    `workspace = true`. `[dev-dependencies]`:
    `tempfile`, `serial_test`, `tracing-test`.
  - `src/lib.rs`: `//! rafaello-tui scaffolding.`
  - `src/bin/rfl_tui.rs`: minimal `fn main() { eprintln!
    ("rfl-tui: scaffolding only."); std::process::exit(0); }`.
  - **Edit `rafaello/Cargo.toml`** to add the new member:
    `members = ["crates/rafaello", "crates/rafaello-core",
    "crates/rafaello-tui"]`.
- **Why.** scope §W3.
- **Depends on.** c01, c02.
- **Acceptance.** `cargo build --workspace` green. `cargo
  build -p rafaello-tui --bin rfl-tui` green. `cargo doc
  --workspace --no-deps` warning-free.

### c04 — feat(rafaello): add `[lib]` target + clap CLI scaffold

- **What.** scope §W4 + pi-8 B3:
  - Add `[lib]` target + `crates/rafaello/src/lib.rs`
    exporting `RflChatCli`, `run_cli`, `RflChatError`,
    `resolve_tui_path` stubs.
  - `[[bin]] name = "rfl"` keeps `path =
    "src/main.rs"`; main becomes `fn main() ->
    std::process::ExitCode { rafaello::run_cli() }`.
  - `[dependencies]`: `rafaello-core` (path-dep — NOT
    `rafaello-tui`; scope §W4),
    `tokio`, `tracing`, `tracing-subscriber`,
    `clap = { version = "4", features = ["derive"] }`,
    `anyhow`. `[dev-dependencies]`: `tempfile`,
    `serial_test`, `tracing-test`.
- **Why.** scope §W4 + pi-8 B3 (lib target so
  `resolve_tui_path` is reachable from
  `rafaello/tests/`).
- **Depends on.** c02, c03.
- **Acceptance.** `cargo build -p rafaello` green.
  `cargo run -p rafaello -- --help` prints help.
  `cargo doc -p rafaello --no-deps` warning-free.

---

## Group 1 — Back-reach to m1: namespace tightening

### c05 — feat(rafaello-core): reject unknown publish namespaces in manifest validation

- **What.** scope §M1.1 + §M1.2: extend
  `rafaello_core::validate::check_publish_topic` to
  reject truly unknown top-level segments. Add
  `ValidationError::PublishUnknownNamespace { topic,
  namespace }`. Existing variants unchanged.
- **Why.** scope §M1, m2 retro §2.8.
- **Depends on.** c02.
- **Acceptance.** New test
  `rafaello-core/tests/manifest_publishes_unknown_namespace_rejected.rs`
  covering `evil.foo` → `PublishUnknownNamespace`,
  `core.foo` → `PublishOnReservedNamespace` (existing,
  verify still passes), `frontend.foo` →
  `PublishOnFrontendNamespace` (existing),
  `provider.<own-id>.foo` accepted, `provider.<other-id>
  .foo` → `ProviderNamespaceMismatch`. All 357 m2
  tests + 1 new = 358 pass.

---

## Group 2 — PluginSupervisor fault injection + restored m2 unwind tests

### c06 — feat(rafaello-core::supervisor): TestHooks 3 inject points (pre-spawn / post-spawn-pre-register / post-register)

- **What.** scope §H6.1 + §H6.2 (pi-1 Blocker #2: this
  commit no longer includes the frontend
  `shutdown_with_outcome` seam — that's nonsensical
  here because frontend types don't exist yet; the seam
  moves to c22 alongside `FrontendHandle::shutdown`):
  - Extend `TestHooks` with `inject_pre_spawn_fault`,
    `inject_post_spawn_pre_register_fault`,
    `inject_post_register_fault` + matching
    `*_consumed` accessors. Three one-shot `AtomicBool`s.
    Production builds compile out via cfg.
  - Wire the three hooks into `PluginSupervisor::spawn`
    body (scope §H6.2): pre-spawn fires AFTER
    socketpair / proxy / sandbox-builder allocation,
    AFTER private-state-dir creation, BEFORE
    `tokio_command.spawn()`; post-spawn-pre-register
    fires AFTER `spawn` and BEFORE
    `broker.register_plugin`; post-register fires
    AFTER reaper/watcher spawn and BEFORE
    `tokio::spawn(server.serve())` (scope §H6.2 pi-17
    #2 placement).
  - Faults return `SpawnError::SandboxBuild { canonical,
    source: anyhow::anyhow!("test-injected
    <hook-name> fault") }` where `<hook-name>` is
    `"pre-spawn"` / `"post-spawn-pre-register"` /
    `"post-register"`.
- **Why.** scope §H6.1 + §H6.2; m2 retro §5.1.
- **Depends on.** c02.
- **Acceptance.** New test
  `rafaello-core/tests/supervisor_test_hooks_three_inject_points.rs`
  — table-driven over the three accessor pairs: arm
  each fault, call spawn, assert `_consumed`
  returned true exactly once and `SpawnError::SandboxBuild`
  was returned. The actual unwind verification lands
  in c07.

### c07 — test(rafaello-core): re-add m2 unwind tests + new third inject point coverage

- **What.** scope §H6.3 + §I positive matrix
  (pi-1 medium #5 wording: lists six unwind test
  files):
  - `supervisor_spawn_unwinds_after_register.rs` —
    arms post-register fault.
  - `supervisor_spawn_post_register_reaps_child.rs`
    (Linux-only `#[cfg(target_os = "linux")]`) — arms
    post-register fault; assert `last_reaped_pid` is
    the spawned child pid.
  - `supervisor_spawn_unwinds_after_socketpair.rs` —
    arms pre-spawn fault.
  - `supervisor_spawn_unwinds_after_socketpair_fd_baseline.rs`
    (Linux-only) — `/proc/self/fd` returns to baseline.
  - `supervisor_spawn_unwinds_post_spawn_pre_register.rs`
    (cross-platform) — arms post-spawn-pre-register
    fault; assert hook consumed,
    `try_reserve_registration` succeeds, `in_flight`
    cleared.
  - `supervisor_spawn_unwinds_post_spawn_pre_register_fd_baseline.rs`
    (Linux-only) — fd-count baseline.
  Note: `frontend_shutdown_dead_watch_paths.rs` was
  bundled here in round 1; pi-1 Blocker #2 surfaces
  that those tests need `shutdown_with_outcome` which
  doesn't exist until c22. They move to c22.
- **Why.** scope §H6.3, §I, m2 retro §5.1.
- **Depends on.** c06.
- **Acceptance.** All six unwind tests pass on Linux;
  the cross-platform tests pass on macOS too.

---

## Group 3 — Entry + RenderTree types

### c08 — feat(rafaello-core::entry): Entry + EntryMetadata + helper types + payload structs

- **What.** scope §E1 + §E3:
  - New module `rafaello_core::entry` with `Entry`,
    `EntryMetadata`, `EntryAuthor`, `EntryFallback`,
    `StreamState` (v1: `Final` only).
  - Sub-module `rafaello_core::entry::payloads::*`
    with the eight built-in payload structs.
  - All derive `Serialize`, `Deserialize`, `Clone`,
    `Debug`, with `#[serde(deny_unknown_fields)]` on
    payload structs.
- **Why.** scope §E1 + §E3.
- **Depends on.** c02.
- **Acceptance.** `entry_serde_round_trip.rs` (eight
  cases) + `entry_stream_state_rejects_open.rs`.

### c09 — feat(rafaello-core::entry): RenderNode + RawFormat + node serde

- **What.** scope §E2 + §E4:
  - `RenderNode` enum (15 variants per Stream E §4.1),
    `RawFormat` enum (Ansi / Html / Plain).
  - Internally tagged on `node` per Stream E §4.2.
  - `Unknown { kind, payload, fallback: EntryFallback }`.
- **Why.** scope §E2 + §E4.
- **Depends on.** c08.
- **Acceptance.** `render_node_serde_round_trip.rs`
  (15 variants) + `render_node_unknown_carries_entry_fallback.rs`.

---

## Group 4 — Renderer pipeline + built-in renderers

### c10 — feat(rafaello-core::renderer): registry + Renderer trait + Capabilities

- **What.** scope §R1 + §R2 + §R4 + §R5:
  - `Renderer` trait, `RendererRegistry` (with
    `new`, `with_builtins`, `register`, `get`),
    `Capabilities` (with `raw_formats`),
    `RendererError`.
  - `with_builtins()` is empty in c10 (registers
    nothing yet); built-in renderers land in c12 + c13.
- **Why.** scope §R1 + §R2 + §R4 + §R5.
- **Depends on.** c09.
- **Acceptance.** Two tests (pi-1 medium #5: was
  contradicting itself with a "with_builtins is empty"
  assertion that c12 would invalidate; round-2
  asserts on `RendererRegistry::new()` — never
  populated):
  - `renderer_registry_register_and_get.rs` — register
    a synthetic renderer, retrieve via `get`, assert
    Arc identity.
  - `renderer_registry_new_is_empty.rs` —
    `RendererRegistry::new().get("text")` returns
    `None`. (Stable across c12 + c13.)

### c11 — feat(rafaello-core::renderer): RenderPipeline with Path A / B / C

- **What.** scope §R3 — three-path pipeline (Path A
  unknown-kind / Path B panic-or-Err / Path C
  capability-driven downgrade). Implementation uses
  `std::panic::catch_unwind(AssertUnwindSafe(...))`.
- **Why.** scope §R3 + Stream E §6.
- **Depends on.** c10.
- **Acceptance.** Six new tests (pi-1 medium #5: was
  "five" but listed six):
  - `renderer_pipeline_unknown_kind_falls_back_with_author_fallback.rs`.
  - `renderer_pipeline_unknown_kind_no_fallback_uses_default_callout.rs`.
  - `renderer_pipeline_panic_falls_through_to_path_a.rs`
    (`tracing-test` asserts error log).
  - `renderer_pipeline_renderer_err_falls_through_to_path_a.rs`.
  - `renderer_capabilities_downgrade_unsupported_node.rs`.
  - `renderer_capabilities_downgrade_unsupported_raw_format.rs`.

### c12 — feat(rafaello-core::renderer): built-in renderers — text + heading + code_block + thinking

- **What.** scope §R2 + §E3: implement four built-in
  renderers under `rafaello_core::renderer::builtins::*`
  and wire them into `RendererRegistry::with_builtins()`.
- **Why.** scope §R2.
- **Depends on.** c11.
- **Acceptance.** Four tests:
  `renderer_builtin_text.rs`, `renderer_builtin_heading.rs`,
  `renderer_builtin_code_block.rs`,
  `renderer_builtin_thinking.rs`.

### c13 — feat(rafaello-core::renderer): built-in renderers — tool_call + tool_result + image + error

- **What.** Remaining four kinds wired into
  `with_builtins()`.
- **Why.** scope §R2.
- **Depends on.** c12.
- **Acceptance.** Four tests:
  `renderer_builtin_tool_call.rs`,
  `renderer_builtin_tool_result.rs`,
  `renderer_builtin_image.rs`,
  `renderer_builtin_error.rs`.

---

## Group 5 — Broker frontend extension

### c14 — feat(rafaello-core::broker_acl): BrokerAcl frontends + AttachId + FrontendAcl + Publisher::Frontend

- **What.** scope §B1 + Publisher enum extension only:
  - `AttachId(String)` newtype with validating
    `new`. `AttachIdParseError`.
  - `FrontendAcl { subscribe_patterns, auto_subscribes,
    publish_topics }`.
  - `BrokerAcl.frontends: BTreeMap<AttachId, FrontendAcl>`
    field.
  - `Publisher` extended with `Frontend(AttachId)`.
  - Compile only — no broker logic changes yet.
- **Why.** scope §B1.
- **Depends on.** c02.
- **Acceptance.** `attach_id_validates.rs` (covers
  valid + invalid inputs). m2 tests still pass with
  `BrokerAcl::default()` empty `frontends`.

### c15 — feat(rafaello-core::bus): BrokerError frontend variants + register_frontend + RegisteredFrontend RAII

- **What.** scope §B2:
  - `BrokerError::FrontendNotInAcl`,
    `FrontendAlreadyRegistered`,
    `FrontendNotRegistered`.
  - **Reshape** existing `PublishOutsideGrant` and
    `InvalidInReplyTo` to take `publisher: Publisher`
    (source-breaking m2-test rewrite, contained to
    rafaello-core).
  - `Broker::register_frontend`,
    `try_reserve_frontend_registration`,
    `frontend_acl`.
  - RAII guard `RegisteredFrontend`.
  - `Broker::new` extended to revalidate frontend ACL
    `publish_topics` (validate_topic) AND
    subscribe_patterns / auto_subscribes
    (validate_pattern).
- **Why.** scope §B2 + §B6.
- **Depends on.** c14.
- **Acceptance.** Four new tests +
  m2 publisher-typed-test rewrites pass:
  - `broker_register_frontend_unknown_attach_id_rejected.rs`.
  - `broker_register_frontend_duplicate_rejected.rs`.
  - `broker_construct_with_invalid_frontend_pattern_rejected.rs`.
  - `broker_construct_with_invalid_frontend_publish_topic_rejected.rs`.

### c16 — feat(rafaello-core::bus): handle_frontend_publish + frontend publish authority + fan-out

- **What.** scope §B3 + §B4 + §B5:
  - `Broker::handle_frontend_publish`.
  - `PublisherIdentity::Frontend { attach_id }`
    promoted to live (`kind: "frontend"`).
  - Authority enforcement (scope §B4): grammar →
    namespace lookup → segment-count → grant
    membership → fan-out.
  - Fan-out for frontend subscribers.
- **Why.** scope §B3 + §B4 + §B5.
- **Depends on.** c15.
- **Acceptance.** Five new tests:
  - `frontend_publish_on_reserved_namespace_rejected.rs`.
  - `frontend_publish_unknown_namespace_rejected.rs`.
  - `frontend_publish_two_segment_topic_rejected.rs`.
  - `frontend_publish_outside_grant_rejected.rs`.
  - `frontend_subscribes_to_core_session_events.rs`.

---

## Group 6 — rfl-bus-fixture extensions (fixture self-timeout + 5 m3 modes)

This group moves AHEAD of the frontend supervisor work so
c19+ frontend tests can use `signal_ready` /
`exit_immediately` etc. without forward references
(pi-1 Blocker #4).

### c17 — feat(rfl-bus-fixture): `RFL_FIXTURE_MAX_LIFETIME` + 5 m3 fixture modes

- **What.** scope §L1 + §L1a: extend m2's
  `rafaello-core/src/bin/rfl_bus_fixture.rs`:
  - All long-running modes read
    `RFL_FIXTURE_MAX_LIFETIME` (default 60 s) and
    `process::exit(0)` after that.
  - **Five new modes** dispatched on
    `RFL_FIXTURE_MODE`:
    - `signal_ready`: adopt RFL_BUS_FD, send
      `peer.call("frontend.ready", json!({}))`,
      sleep until SIGTERM/MAX_LIFETIME.
    - `exit_immediately`: exit 0 without sending
      ready.
    - `hold_silent`: adopt RFL_BUS_FD, run client
      serve, hold connection without sending ready.
    - `signal_ready_then_exit_n`: send ready, sleep
      200 ms, exit with `RFL_FIXTURE_EXIT_CODE`
      (default 7).
    - `probe_fd_closed`: takes `--probe-fd <N>`,
      `nix::fcntl::fcntl(N, F_GETFD)`, exit 0 on
      **`Errno::EBADF`** (pi-1 Blocker #5: round-1
      mistakenly said ESRCH; the correct errno for
      a non-inherited fd checked via F_GETFD is
      EBADF) else non-zero.
- **Why.** scope §L1 + §L1a; m2 retro §4.4.
- **Depends on.** c02.
- **Acceptance.** Five new tests
  `rafaello-core/tests/fixture_mode_<NAME>.rs`
  exercising each mode through `Command::new` +
  `RFL_FIXTURE_MODE` env. Existing m2 fixture tests
  still pass.

---

## Group 7 — Frontend supervisor + handle + shutdown algorithm

### c18 — feat(rafaello-core::frontend): module + types + FrontendConfig + FrontendSpawnError

- **What.** scope §F1 + §F2 (types only, no spawn body):
  - New module `rafaello_core::frontend` with public
    surface: `FrontendSupervisor`, `CompiledFrontend`
    (with `attach_id: String` per pi-8 M1),
    `FrontendPaths`, `FrontendHandle` (struct shape
    from scope §F1 with all 9 fields including
    `child_stderr` and `config`),
    `FrontendConfig` (with `Default` impl),
    `ShutdownReport`, `FrontendReadyError`,
    `PaintError`, `FrontendBusPublishService`,
    `FrontendReadyService`,
    `FrontendExtraServiceFactory`.
  - `FrontendSpawnError` typed enum +
    `InvalidFrontendPlanReason` companion enum
    re-exported from `lib.rs`.
- **Why.** scope §F1 + §F2.
- **Depends on.** c14, c15 (FrontendHandle holds
  `RegisteredFrontend`).
- **Acceptance.** Build-only test
  `m3_frontend_error_surface_compiles.rs` (m1 c02 +
  m2 c05 pattern).

### c19 — feat(rafaello-core::frontend): Phase A validation + try_reserve check

- **What.** scope §F3 Phase A:
  - `FrontendSupervisor::spawn` Phase A: AttachId
    validation, control-char check, relative-path
    check, exec-bit stat, reserved-env check,
    `try_reserve_frontend_registration`. Returns
    `FrontendSpawnError::InvalidPlan { reason: ... }`
    on every failure mode.
- **Why.** scope §F3 Phase A.
- **Depends on.** c15 (try_reserve), c18.
- **Acceptance.** Seven negative tests under
  `rafaello-core/tests/`:
  - `frontend_spawn_invalid_attach_id_rejected.rs`.
  - `frontend_spawn_relative_entry_path_refused.rs`.
  - `frontend_spawn_control_chars_in_path_refused.rs`.
  - `frontend_spawn_entry_not_executable_refused.rs`.
  - `frontend_spawn_reserved_env_in_pass_refused.rs`.
  - `frontend_spawn_reserved_env_in_set_refused.rs`.
  - `frontend_register_unknown_attach_id_rejected.rs`.

### c20 — feat(rafaello-core::frontend): Phase B steps 1-8 (env, socketpair, private state dir, spawn, take stderr)

- **What.** scope §F3 Phase B steps 1-8: socketpair,
  command construction, env apply, FD_CLOEXEC strip,
  private state dir create_dir_all (pi-17 #3),
  stderr piped (pi-10 Blocker 1),
  command.spawn(), child.stderr.take().
- **Why.** scope §F3 Phase B 1-8.
- **Depends on.** c17 (uses `signal_ready` mode in the
  smoke test below — pi-1 Blocker #4 fixed by Group
  6 reorder), c19.
- **Acceptance.** New test
  `frontend_spawn_phase_b_smoke.rs` — spawns
  `rfl-bus-fixture` in `signal_ready` mode (now
  available from c17); assert child PID captured,
  child stderr taken, env applied. Plus
  `frontend_spawn_creates_private_state_dir.rs`
  asserting the per-frontend dir lands at
  `${PROJECT_ROOT}/.rafaello-frontend-data/<attach-id>/`.

### c21 — feat(rafaello-core::frontend): Phase B steps 9-12 (watches, reaper, watcher, server build)

- **What.** scope §F3 Phase B steps 9-12: readiness
  watch channel, reaper-outcome watch channel, reaper
  task spawn, reaper-watcher task spawn (consumes
  reaper's JoinHandle, on JoinError pushes
  `ReaperPanicked`), fittings_server::Server build
  with `FrontendBusPublishService` +
  `FrontendReadyService` composed via
  `FrontendExtraServiceFactory`.
- **Why.** scope §F3 + scope reaper-watcher spec.
- **Depends on.** c17 (signal_ready), c20.
- **Acceptance.** Two tests:
  - `frontend_reaper_publishes_exited_on_clean_exit.rs`
    — spawn fixture in `signal_ready` mode with
    `RFL_FIXTURE_MAX_LIFETIME=1`; await outcome; assert
    `Exited(status)` with `status.success()`.
  - `frontend_reaper_publishes_reaper_panicked_on_panic.rs`
    — induce reaper-task panic via cfg-gated
    `inject_reaper_panic` hook; assert `ReaperPanicked`
    is published.

### c22 — feat(rafaello-core::frontend): shutdown_with_outcome seam + dead-watch tests

- **What.** scope §F2 / §F4 / §"shutdown_with_outcome"
  (pi-1 Blocker #2: this seam moved out of c06 — it
  belongs alongside `FrontendHandle::shutdown`):
  - Extract the shutdown algorithm into a pure
    async helper `rafaello_core::frontend::shutdown::
    shutdown_with_outcome(cached, child_pid, config,
    signal_fn, probe_fn, serve_handle,
    register_guard) -> ShutdownReport` per scope §F4.
- **Why.** scope §F4 + scope §"shutdown_with_outcome";
  the seam is a unit-testable extraction of the
  shutdown algorithm.
- **Depends on.** c18 (FrontendConfig + ShutdownReport
  types).
- **Acceptance.** Two new unit tests in
  `rafaello-core/tests/frontend_shutdown_dead_watch_paths.rs`:
  - `dead_watch_waitfailed_child_already_gone`:
    `cached = Some(WaitFailed(...))`,
    mock `signal_fn` records SIGTERM, mock
    `probe_fn` returns `Err(ESRCH)` on first call →
    SIGKILL is NOT sent, `used_sigkill = false`.
  - `dead_watch_reaper_panicked_child_alive`:
    `cached = Some(ReaperPanicked)`, `signal_fn`
    records SIGTERM and SIGKILL, `probe_fn` returns
    `Ok(())` (alive) then `Err(ESRCH)` (gone) →
    `used_sigkill = true`. Verifies kill_fn call
    sequence and `ShutdownReport` flags.

### c23 — feat(rafaello-core::frontend): Phase B steps 13-15 — register, serve, return; FrontendHandle Drop + shutdown

- **What.** scope §F3 step 13-15 + §F4:
  - `Broker::register_frontend(...)` →
    `FrontendHandle.register_guard`.
  - `tokio::spawn(server.serve())` →
    `FrontendHandle.serve_handle`.
  - `FrontendHandle::wait`,
    `FrontendHandle::wait_ready`,
    `FrontendHandle::take_child_stderr`,
    `FrontendHandle::has_signalled_ready`.
  - `FrontendHandle::shutdown(mut self)` calls
    `shutdown_with_outcome` from c22.
  - `FrontendHandle::Drop` — Exited-skip /
    abnormal-best-kill / None-best-kill per scope §F4.
- **Why.** scope §F3 step 13-15 + §F4.
- **Depends on.** c17, c21, c22.
- **Acceptance.** Six tests:
  - `frontend_handle_wait_ready_resolves_on_signal.rs`
    (uses `signal_ready`).
  - `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs`
    (uses `exit_immediately`).
  - `frontend_handle_wait_resolves_on_child_exit.rs`
    (uses `signal_ready` + `MAX_LIFETIME=1`).
  - `frontend_handle_drop_does_not_leak_zombie.rs`
    (Linux-only; uses
    `respond_peer_call` mode from m2).
  - `frontend_handle_shutdown_skips_kill_on_exited.rs`.
  - `frontend_handle_shutdown_kills_on_waitfailed.rs`
    (uses `shutdown_with_outcome` directly with
    mock signal_fn / probe_fn).

---

## Group 8 — Session store + controller

### c24 — feat(rafaello-core::session): SessionStore::open with Flock<File> + lock-first ordering

- **What.** scope §S1 + §S5 + §S3 (partial):
  - New module `rafaello_core::session`.
  - `SessionStore { conn: Mutex<Connection>,
    lock_guard: Flock<File>, ... }`.
  - `SessionStore::open(state_dir)` lock-first ordering:
    mkdir, open lockfile O_CLOEXEC,
    `Flock::lock(file, LockExclusiveNonblock)`,
    write own pid, open SQLite + PRAGMAs + tables.
  - `session_id()`, `lock_fd_for_test()` (cfg-gated).
  - `SessionError` enum with `Io`, `Sqlite`, `Serde`,
    `Locked { holder_pid: Option<u32> }`,
    `SchemaMismatch` (the `Publish` variant lands at
    c26 with SessionController).
- **Why.** scope §S1 + §S5 + §S3.
- **Depends on.** c02, c17 (uses `probe_fd_closed`
  fixture mode for the fd-not-inherited test).
- **Acceptance.** Five tests:
  - `session_store_open_creates_state_dir.rs`.
  - `session_store_concurrent_open_errors.rs`
    (cross-platform).
  - `session_store_locked_unknown_holder_errors.rs`
    (empty / corrupt lockfile yields `holder_pid:
    None`).
  - `session_store_schema_mismatch_errors.rs`.
  - `session_store_lock_fd_not_inherited_by_child.rs`
    (uses `probe_fd_closed` mode from c17).

### c25 — feat(rafaello-core::session): append_entry + load_entries + StoredEntry

- **What.** scope §S1 + §S2:
  - SQL schema population (tables created at c24;
    rows + queries fill in here).
  - `StoredEntry { seq: u64, entry: Entry }`.
  - `append_entry`, `load_entries` returning
    `Vec<StoredEntry>` ordered by seq.
- **Why.** scope §S1 + §S2.
- **Depends on.** c08 (Entry), c24.
- **Acceptance.** Two tests:
  - `session_store_round_trip.rs`.
  - `session_store_seq_monotonic.rs`.

### c26 — feat(rafaello-core::session): SessionController + finalize_entry + replay_history + SessionError::Publish

- **What.** scope §S1 controller bullets:
  - `SessionController { store, pipeline, broker }`.
  - `finalize_entry(entry, caps).await` — append →
    render → publish on `core.session.entry.finalized`
    with `replay: false`.
  - `replay_history(caps).await` — iterates
    `load_entries`, renders each, publishes with
    `replay: true`.
  - `SessionError::Publish { source: BrokerError }`
    variant (pi-12 #2).
- **Why.** scope §S1 + pi-12 #2.
- **Depends on.** c11 (RenderPipeline), c16 (broker
  publish_core), c25.
- **Acceptance.** Two tests using
  `in_memory_broker_with_tui_and_observer_acl()`
  helper (defined inline in this commit; `m3_harness`
  formal harness lands later in c33):
  - `session_controller_finalize_entry.rs` — assert
    SQLite row + one `entry.finalized` event with
    `replay: false`.
  - `session_controller_replay_history.rs` — three
    pre-seeded entries → three events with `replay:
    true` in seq order.

---

## Group 9 — TUI binary

### c27 — feat(rafaello-tui::bin): rfl-tui core — env parsing, fd adoption, fittings client, frontend.ready RPC

- **What.** scope §T1 + §T2 step 1-3:
  - Parse `RFL_BUS_FD`, `RFL_PROJECT_ROOT`,
    `RFL_TUI_TEST_MODE`, `RFL_TUI_READY_DELAY_MS`,
    `RFL_TUI_MAX_LIFETIME`.
  - Adopt inherited fd as `tokio::net::UnixStream`.
  - Build `fittings_client::Client` with
    `BusEventHandler` (notification handler).
  - After handler is wired, call
    `peer.call("frontend.ready", json!({}))`.
  - `RFL_TUI_READY_DELAY_MS` insertion before the
    `peer.call`.
- **Why.** scope §T1 + §T2 steps 1-3.
- **Depends on.** c03, c16 (frontend publish).
- **Acceptance.**
  `rafaello-tui/tests/tui_handler_calls_frontend_ready.rs`
  — spawn rfl-tui in headless mode, assert
  `frontend.ready` arrives parent-side via test-side
  `FrontendReadyService` mock.

### c28 — feat(rafaello-tui::bin): headless test mode + stderr sentinels + RFL_TUI_MAX_LIFETIME self-timeout

- **What.** scope §T2 step 4 + sentinels:
  - `RFL_TUI_TEST_MODE=1`: skip terminal init,
    in-memory log, exit on
    `core.lifecycle.test_done` event.
  - Stderr sentinels (raw, NO `rfl-tui:` prefix —
    forwarder adds it):
    `"bus.event topic=<topic> seq=<n>"`,
    `"test-done"`,
    `"project-root=<abs-path>"`.
  - `RFL_TUI_MAX_LIFETIME` self-timeout in test mode.
- **Why.** scope §T2 step 4 + sentinels.
- **Depends on.** c27.
- **Acceptance.** Four tests:
  - `tui_test_mode_logs_bus_events_to_stderr.rs`.
  - `tui_test_mode_exits_on_test_done.rs`.
  - `tui_test_mode_self_timeout_exits_zero.rs`.
  - `tui_sends_frontend_ready_after_handler_registration.rs`
    (deterministic-callback ordering, scope §I).

### c29 — feat(rafaello-tui::paint): Painter::draw_with_panic_isolation + paint_node + lib unit test

- **What.** scope §I `tui_paint_panic_isolation` +
  scope §T4 + §T5:
  - `pub fn rafaello_tui::paint::draw_with_panic_isolation(
    term, &RenderNode) -> Result<(), PaintError>`.
  - Internal `paint_node` per Stream E §4 variants.
  - `#[cfg(test)] PaintAction` enum +
    `draw_with_panic_isolation_for_test`.
  - Library unit test `#[cfg(test)] mod tests` in
    `rafaello-tui/src/paint.rs` exercising the
    panic-isolation seam against
    `ratatui::backend::TestBackend`.
- **Why.** scope §T4 + §T5.
- **Depends on.** c09 (RenderNode), c18 (PaintError
  type — pi-1 high #2).
- **Acceptance.** Unit test passes; assert that a
  panicking `PaintAction::RunPanicking` produces
  `[render error: ...]` on the test backend AND the
  next render call proceeds normally.

### c30 — feat(rafaello-tui::bin): production-mode UI loop with crossterm + raw mode + ratatui

- **What.** scope §T2 step 5-7 + §T6: terminal init,
  alternate-screen, raw-mode, redraw on event arrival,
  q-to-quit, arrow-key scroll, restore-on-exit. Uses
  `Painter::draw_with_panic_isolation` from c29
  (pi-1 Blocker #6: this commit now properly depends
  on c29 instead of forward-referencing it).
- **Why.** scope §T2 step 5-7 + §T6.
- **Depends on.** c28, c29.
- **Acceptance.** No automated production-mode tests
  (production mode is exercised by humans only;
  scope §I `rfl_chat_demo_bar` uses headless mode).
  A manual smoke recording lands in c34
  `manual-validation.md`.

---

## Group 10 — `rfl chat` subcommand

### c31 — feat(rafaello::lib): RflChatCli + RflChatError + resolve_tui_path pure function

- **What.** scope §C1 + §C3:
  - clap `Cli` with `Chat { project_root:
    Option<PathBuf> }` subcommand.
  - `RflChatError` with all variants used in §C2.
  - `resolve_tui_path(env, current_exe)` pure
    function per scope §C3.
- **Why.** scope §C1 + §C3 + pi-15 #5.
- **Depends on.** c04.
- **Acceptance.** Three resolve_tui_path unit tests
  (per scope §C3): `_env_override.rs`,
  `_sibling_lookup.rs`, `_unresolved.rs`.

### c32 — feat(rafaello::lib): rfl chat orchestration steps 1-7 — open/spawn/wait_ready

- **What.** scope §C2 steps 1-7. Includes the EnvPlan
  `pass` allowlist construction (scope §C2 step 5),
  child stderr forwarder task with serialised writer,
  `wait_ready` three-outcome mapping with parent-side
  `"rfl-chat: frontend-ready-observed"` sentinel.
- **Why.** scope §C2 steps 1-7.
- **Depends on.** c17 (fixture modes used by tests),
  c23 (FrontendHandle), c26 (controller),
  c27 (rfl-tui in test mode), c31.
- **Acceptance.** Seven CLI tests under
  `rafaello/tests/`:
  - `rfl_chat_locked_session_errors_with_holder_pid.rs`.
  - `rfl_chat_locked_session_unknown_holder_errors.rs`.
  - `rfl_chat_relative_project_root_canonicalises.rs`.
  - `rfl_chat_nonexistent_project_root_errors.rs`.
  - `rfl_chat_resolves_tui_via_env_override.rs`
    (uses real rfl-tui in test mode).
  - `rfl_chat_frontend_exits_before_ready_errors.rs`
    (uses `exit_immediately`).
  - `rfl_chat_frontend_ready_timeout_errors.rs`
    (uses `hold_silent`).

### c33 — feat(rafaello::lib): rfl chat orchestration steps 8-10 — replay/harness/wait + cleanup guard

- **What.** scope §C2 steps 8-10 + cleanup guard:
  Option/take ownership pattern, single shutdown +
  forwarder.await regardless of result, replay,
  in-test fixture-entry harness (when
  `RFL_HARNESS_FIXTURES=1`), step-10 outcome
  reading from `frontend_handle.wait().await`,
  RflChatError mapping.
- **Why.** scope §C2 steps 8-10 + cleanup guard.
- **Depends on.** c32, c17 (uses
  `signal_ready_then_exit_n` for the post-ready test).
- **Acceptance.** Two CLI tests:
  - `rfl_chat_replay_withheld_until_frontend_ready.rs`
    (single combined stderr stream, line-order
    assertion).
  - `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
    (uses `signal_ready_then_exit_n`).

---

## Group 11 — Demo-bar headline + manual validation

### c34 — test(rafaello): rfl_chat_demo_bar.rs + workspace_bin_path helper + manual-validation.md

- **What.** scope §I `rfl_chat_demo_bar.rs` + scope
  §H `workspace_bin_path` helper + scope §"Manual
  validation" record (round-2 bundles the headline
  test and the manual-validation doc; either could
  split if pi prefers, but they land together as the
  milestone-close commit):
  - `rafaello/tests/common/workspace_bin_path.rs`
    helper.
  - `rfl_chat_demo_bar.rs` end-to-end: spawn
    `rfl chat` with `RFL_HARNESS_FIXTURES=1` +
    `RFL_TUI_TEST_MODE=1` against tempdir; assert
    nine SQLite rows + nine `"rfl-tui: bus.event"`
    lines.
  - `rafaello/plans/milestones/m3-tui-sessions/manual-validation.md`
    documenting cargo-test-all output (Linux + macOS
    CI URL), real interactive `rfl chat` recording,
    macOS CI URL.
- **Why.** scope §I + §"Manual validation" + §"Acceptance
  summary".
- **Depends on.** c33.
- **Acceptance.** `rfl_chat_demo_bar.rs` passes on
  Linux. macOS CI green. `manual-validation.md`
  exists with all required items.

---

## Phase 4 — `retrospective.md` is driver-owned, NOT a per-commit task

The milestone driver writes `retrospective.md` in Phase 4
after all 34 commits land. Pi review (m1: 4 rounds; m2:
2 rounds). Drift fixes land before retrospective ratifies.
Then merge to `rafaello-v0.1` linear-history.

Anticipated drift items (from scope §"Acceptance summary"):
Stream E renderer-RFC banner; `PublisherIdentity::Frontend`
schema additions to Stream A; Capabilities staging note in
overview §10.1; Replay-via-`entry.finalized` decisions row;
BrokerError variant additions decisions row; m1 publishes-
grant tightening (lands in c05); m1 lock-side
`check_lock_publish_topic` deferral; `FrontendSupervisor`
lock-correspondence claim; fixture self-timeout
documentation; m3 frontend ACL `publish_topics = []`
handover to m4.

## What changed from prior drafts

### Round-1 changes addressing pi commits round-1

- **Blocker #1**: c01 is now deps-only; the
  `crates/rafaello-tui` workspace member registration
  moved into c03 alongside the directory creation.
- **Blocker #2**: `shutdown_with_outcome` extraction
  moved out of c06 (where the frontend types it
  references don't exist yet) into a new c22 inside
  the frontend group, depending on c18's type
  introduction. The dead-watch tests
  (`frontend_shutdown_dead_watch_paths.rs`) move with
  it.
- **Blocker #3**: c10's "with_builtins is empty" test
  replaced with a "`RendererRegistry::new()` is empty"
  test that stays valid through c12 + c13.
- **Blocker #4**: rfl-bus-fixture extensions (formerly
  c25) moved to **c17**, ahead of the frontend
  supervisor group (c18-c23). Frontend tests can now
  use `signal_ready` / `exit_immediately` /
  `hold_silent` / `signal_ready_then_exit_n` /
  `probe_fd_closed` modes without forward references.
- **Blocker #5**: c17's `probe_fd_closed` mode exits
  on `EBADF` (correct), not `ESRCH` (round-1 mistake).
- **Blocker #6**: c30 (production-mode UI loop) now
  depends on c29 (Painter); c30 was forward-
  referencing c29's `draw_with_panic_isolation`.
- **High #1**: c19 + c20 now declare c15 dependency
  for `try_reserve_frontend_registration`.
- **High #2**: c29 now declares c18 dependency for
  `PaintError`.
- **High #3**: c32 / c33 now declare c17 / c27
  dependencies for fixture modes + rfl-tui test
  mode.
- **High #4**: forward-reference notes ("re-points",
  "harness preview") removed; concrete
  inline-helper-or-deferred-test ownership assigned
  to specific commits.
- **Medium #5 counts**: c07 lists six unwind tests
  (was "five"); c11 lists six pipeline tests (was
  "five"); c32 lists seven CLI tests (was "three"
  — split across c32 + c33 now).
- **Medium #5 group count**: 12 groups (was
  miscounted as "11" in the round-1 preamble).
- **Medium #5 H6 wording**: c06 H6 hook placement
  matches scope §H6.2 verbatim — pre-spawn fires
  AFTER private-state-dir creation, AFTER
  socketpair / proxy / sandbox-builder allocation,
  BEFORE `tokio_command.spawn()`.
