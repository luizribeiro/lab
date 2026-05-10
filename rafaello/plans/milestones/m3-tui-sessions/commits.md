# m3-tui-sessions — commits

> **Status:** round-1 draft. Awaiting pi adversarial review.
> Drafted from `scope.md` (round 22 — converged after 22 pi
> rounds, 6 zero-blocker rounds; commit `da9473c`). Each
> commit is one logical idea **and leaves the workspace
> green** — pre-commit hooks (rustfmt + clippy + cargo test)
> gate every commit; intermediate non-green states are not
> allowed. Commits land sequentially on per-commit branches
> `agents/m3/c<NN>` rebased onto `rafaello-v0.1`, no merge
> commits, no force pushes. Tests land with the code that
> exercises them per `~/.claude/CLAUDE.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes:
  `rafaello-core` (the core crate m3 grows), `rafaello-tui`
  (the new TUI crate), `rafaello` (the rfl bin + lib),
  `rfl-bus-fixture` (m2's test-only fixture binary, extended
  in m3), `rafaello` (workspace `Cargo.toml`),
  `rafaello-m3` (docs).
- "Acceptance" lists new tests + the pre-commit invariants
  the commit must keep green.
- "Depends on" cites the *lowest* commit numbers whose code
  or types this commit references. A commit only lands after
  every declared dependency has landed on `rafaello-v0.1`.
- Test files live under the appropriate crate's `tests/`
  directory per scope §I's placement rules:
  - `rafaello-core/tests/` — broker, session store, renderer
    pipeline, supervisor (incl. fault-injection), manifest.
  - `rafaello-tui/tests/` — anything spawning `rfl-tui` (uses
    `env!("CARGO_BIN_EXE_rfl-tui")`).
  - `rafaello/tests/` — the headline `rfl chat` end-to-end
    tests (uses `env!("CARGO_BIN_EXE_rfl")`; resolves
    `rfl-tui` via `workspace_bin_path` helper).
- Per-commit agents pre-flight `nix develop --impure
  --command cargo test --manifest-path rafaello/Cargo.toml
  --workspace --features rafaello-core/test-fixture` until
  green before invoking pre-commit hooks.
- **Every per-commit agent prompt MUST include `--features
  rafaello-core/test-fixture`** in cited cargo invocations
  for any test that depends on the fixture binary. m2's
  scope §"Risks" #1 set the precedent.
- Per the m1 lesson §4.2, the milestone driver inlines the
  full row text + every acceptance bullet verbatim into each
  per-commit prompt; agents do NOT re-read `commits.md`.
- **Driver-owned actions, NOT per-commit agent actions:**
  pushing branches to origin, capturing CI URLs, writing
  `retrospective.md` (Phase 4, not a per-commit task).

## m3a / m3b checkpoint

No internal split is planned. The driver re-evaluates after
**c14** (renderer pipeline complete) and after **c24**
(session store + controller landed); if a split becomes
obviously beneficial (e.g. ratatui surfaces a macOS-only
blocker mid-c26), the driver opens an m3a / m3b
owner-ratification request.

## Canonical test names

Wherever `scope.md` and `commits.md` both name a test, this
`commits.md` is canonical. The headline test is
**`rfl_chat_demo_bar.rs`** (lands at c33 — the end-to-end
gate that ties broker + supervisor + controller + TUI
together).

---

## Group 0 — Foundation: workspace deps + crate scaffold + lib targets

### c01 — chore(rafaello): add m3 deps to `[workspace.dependencies]` + register `rafaello-tui` member

- **What.** Three concrete edits to `rafaello/Cargo.toml`
  (scope §W1):
  - **Edit existing entries**: `tokio.features` adds
    `"process"` to the existing array.
  - **Add new entries**: `ratatui = "0.29"`,
    `crossterm = "0.28"`,
    `rusqlite = { version = "0.32", features = ["bundled"] }`,
    `tui-input = "0.10"`, `unicode-width = "0.2"`,
    `ulid = { version = "1", features = ["serde"] }`.
  - **Add the new workspace member** `crates/rafaello-tui`
    to `[workspace] members`.
  Existing `chrono`, `tempfile`, `serial_test`,
  `tracing-test`, `tracing-subscriber` entries are
  unchanged.
- **Why.** scope §W1 verbatim.
- **Depends on.** baseline.
- **Acceptance.** `nix develop --impure --command cargo
  metadata --manifest-path rafaello/Cargo.toml
  --format-version 1` succeeds and shows the new entries
  resolved; `nix develop --impure --command cargo build
  --manifest-path rafaello/Cargo.toml -p rafaello-core`
  green (no rafaello-core consumer of the new deps yet —
  table-only addition); `nix develop --impure --command
  cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` warning-free. Note:
  `crates/rafaello-tui` is now in `members` but does not
  yet exist on disk; cargo treats unsourced members as a
  build error, so c01's commit lands AFTER c03 below
  creates the directory. Reorder: c01 deps-only, c02 wires
  rafaello-core, c03 creates rafaello-tui (and at c03 the
  workspace member is real). Round-1 splits c01 between
  "deps" (this commit, members unchanged) and a later
  "register member" step folded into c03.

### c02 — chore(rafaello-core): wire m3 deps to `rafaello-core/Cargo.toml`

- **What.** Update `rafaello/crates/rafaello-core/Cargo.toml`
  per scope §W2:
  - `[dependencies]` adds `rusqlite`, `ulid`, `chrono`
    with `workspace = true`.
  No new features. The existing `test-fixture` feature
  is unchanged (scope §W5).
- **Why.** scope §W2 — renderer types and SessionStore live
  in rafaello-core; ratatui/crossterm do NOT.
- **Depends on.** c01.
- **Acceptance.** `cargo build -p rafaello-core` green.
  `cargo build -p rafaello-core --features test-fixture`
  green. Existing 357-test m2 suite still passes.

### c03 — feat(rafaello-tui): scaffold the new crate with `[lib]` + `rfl-tui` `[[bin]]`

- **What.** Create `rafaello/crates/rafaello-tui/`:
  - `Cargo.toml`: `[package] name = "rafaello-tui"`,
    `version = "0.0.0"`, `edition = "2021"`. `[lib]`
    target. `[[bin]] name = "rfl-tui", path =
    "src/bin/rfl_tui.rs"`. `[dependencies]`:
    `rafaello-core` (path-dep), `ratatui`, `crossterm`,
    `tui-input`, `tokio`, `tracing`, `fittings-core`,
    `fittings-server`, `fittings-client`,
    `fittings-transport`, `serde`, `serde_json`,
    `async-trait`, `anyhow`, `unicode-width`. All with
    `workspace = true`. `[dev-dependencies]`:
    `tempfile`, `serial_test`, `tracing-test`.
  - `src/lib.rs`: empty stub
    (`//! rafaello-tui scaffolding.`).
  - `src/bin/rfl_tui.rs`: minimal `fn main() {
    eprintln!("rfl-tui: scaffolding only.");
    std::process::exit(0); }`.
  Now that the directory exists, the c01 `members`
  registration is satisfied.
- **Why.** scope §W3 — separate crate keeps
  ratatui/crossterm out of rafaello-core's compile graph.
- **Depends on.** c01, c02.
- **Acceptance.** `cargo build --workspace` green.
  `cargo build -p rafaello-tui --bin rfl-tui` green.
  `cargo doc --workspace --no-deps` warning-free.

### c04 — feat(rafaello): add `[lib]` target + `clap`-derived CLI scaffold

- **What.** Update `rafaello/crates/rafaello/Cargo.toml`
  per scope §W4:
  - Add `[lib]` target + `crates/rafaello/src/lib.rs`
    exporting `RflChatCli`, `run_cli`, `RflChatError`,
    `resolve_tui_path` stubs.
  - `[[bin]] name = "rfl"` keeps existing `path =
    "src/main.rs"`; main becomes `fn main() ->
    std::process::ExitCode { rafaello::run_cli() }`.
  - `[dependencies]`: `rafaello-core`,
    `tokio`, `tracing`, `tracing-subscriber`,
    `clap = { version = "4", features = ["derive"] }`,
    `anyhow`. NO `rafaello-tui` (scope §W4 — TUI is
    spawned, not linked).
  - `[dev-dependencies]`: `tempfile`, `serial_test`,
    `tracing-test`.
- **Why.** scope §W4 + pi-8 B3 (lib target so
  `resolve_tui_path` is reachable from
  `rafaello/tests/`).
- **Depends on.** c02, c03.
- **Acceptance.** `cargo build -p rafaello` green.
  `cargo run -p rafaello -- --help` prints the clap-
  derived help. `cargo doc -p rafaello --no-deps`
  warning-free. m2's 357 tests still pass.

---

## Group 1 — Back-reach to m1: namespace tightening

### c05 — feat(rafaello-core): reject unknown publish namespaces in manifest validation

- **What.** scope §M1.1 + §M1.2: extend
  `rafaello_core::validate::check_publish_topic` to
  reject truly unknown top-level segments. Add
  `ValidationError::PublishUnknownNamespace { topic,
  namespace }`. Existing variants
  (`PublishOnReservedNamespace`,
  `PublishOnFrontendNamespace`,
  `ProviderNamespaceMismatch`) are unchanged.
- **Why.** scope §M1, m2 retro §2.8. m3-or-m4 follow-up
  pinned to m3 here.
- **Depends on.** c02.
- **Acceptance.** New test
  `rafaello-core/tests/manifest_publishes_unknown_namespace_rejected.rs`
  covering: `evil.foo` →
  `ValidationError::PublishUnknownNamespace`,
  `core.foo` → `PublishOnReservedNamespace` (existing,
  verify still passes), `frontend.foo` →
  `PublishOnFrontendNamespace` (existing),
  `provider.<own-id>.foo` from a provider manifest →
  accepted (existing), `provider.<other-id>.foo` →
  `ProviderNamespaceMismatch` (existing). All 357
  existing tests + 1 new = 358 pass.

---

## Group 2 — Supervisor fault injection + restored m2 unwind tests

### c06 — feat(rafaello-core::supervisor): TestHooks 3 inject points + shutdown_with_outcome extraction

- **What.** scope §H6.1 + §F4 +
  `frontend_shutdown_dead_watch_paths` seam:
  - Extend `TestHooks` with `inject_pre_spawn_fault`,
    `inject_post_spawn_pre_register_fault`,
    `inject_post_register_fault` + their matching
    `*_consumed` accessors. Three one-shot
    `AtomicBool`s. Production builds compile out via
    cfg.
  - Wire the three hooks into `PluginSupervisor::spawn`
    body at the documented points (scope §H6.2 — pre-
    spawn = post-socketpair-pre-spawn; post-spawn-pre-
    register = post-spawn-pre-register_plugin;
    post-register = post-register-pre-serve).
    Faults return
    `SpawnError::SandboxBuild { canonical, source:
    anyhow::anyhow!("test-injected <hook-name> fault") }`.
  - **Extract the shutdown algorithm** into a pure
    async helper `rafaello_core::frontend::shutdown::shutdown_with_outcome(
    cached, child_pid, config, signal_fn, probe_fn,
    serve_handle, register_guard) -> ShutdownReport`
    per scope §F2 / §F4 / §"shutdown seam". m3's
    production `FrontendHandle::shutdown` will call
    this from c21; landing it now keeps the seam-
    testable for the dead-watch tests in c07.
- **Why.** scope §H6.1, §F4, §"shutdown_with_outcome"; m2
  retro §5.1 — closes the largest known coverage gap in
  m2.
- **Depends on.** c02.
- **Acceptance.** New test
  `rafaello-core/tests/supervisor_test_hooks_three_inject_points.rs`
  — table-driven over the three accessor pairs:
  arm each fault, call spawn, assert
  `_consumed` returned true exactly once and
  `SpawnError::SandboxBuild` was returned. The actual
  unwind verification lands in c07 (this commit only
  proves the hook plumbing).

### c07 — test(rafaello-core): re-add the three deleted m2 unwind tests + new third inject point

- **What.** scope §H6.3 + scope §I positive matrix:
  five new test files in `rafaello-core/tests/`:
  - `supervisor_spawn_unwinds_after_register.rs` —
    arms post-register fault; spawn returns
    `SpawnError::SandboxBuild`; broker has canonical
    in ACL but no live registration; supervisor's
    `in_flight` is cleared.
  - `supervisor_spawn_post_register_reaps_child.rs`
    (Linux-only `#[cfg(target_os = "linux")]`) — arms
    post-register fault; assert `last_reaped_pid` is
    the spawned child pid via the reaper.
  - `supervisor_spawn_unwinds_after_socketpair.rs` —
    arms pre-spawn fault; spawn returns
    `SpawnError::SandboxBuild`; assert (no child
    spawned, fd-count baseline check is in the next
    file).
  - `supervisor_spawn_unwinds_post_spawn_pre_register.rs`
    (cross-platform) — arms post-spawn-pre-register
    fault; assert hook consumed,
    `try_reserve_registration` succeeds afterward,
    `in_flight` cleared.
  - `supervisor_spawn_unwinds_post_spawn_pre_register_fd_baseline.rs`
    (Linux-only) + `unwinds_after_socketpair_fd_baseline.rs`
    — `/proc/self/fd` returns to baseline after
    failed spawns.
  Plus the dead-watch unit tests:
  - `frontend_shutdown_dead_watch_paths.rs` covering
    `dead_watch_waitfailed_child_already_gone` and
    `dead_watch_reaper_panicked_child_alive` against
    the c06 `shutdown_with_outcome` seam with mock
    `signal_fn` + `probe_fn` per scope §F4 + scope
    §"shutdown_with_outcome".
- **Why.** scope §H6.3, §I, m2 retro §5.1.
- **Depends on.** c06.
- **Acceptance.** All five unwind tests pass on Linux; the
  cross-platform tests pass on macOS too;
  `frontend_shutdown_dead_watch_paths.rs` exercises both
  branches with deterministic assertions on signal_fn
  call sequence (SIGTERM only for `child_already_gone`;
  SIGTERM + SIGKILL for `child_alive`) and
  ShutdownReport flags.

---

## Group 3 — Entry + RenderTree types

### c08 — feat(rafaello-core::entry): Entry + EntryMetadata + helper types

- **What.** scope §E1 + §E3:
  - New module `rafaello_core::entry` with `pub struct
    Entry`, `pub struct EntryMetadata`, `pub enum
    EntryAuthor`, `pub struct EntryFallback`, `pub
    enum StreamState` (v1: `Final` only).
  - Sub-module `rafaello_core::entry::payloads::*`
    with the eight built-in payload structs
    (`text`, `heading`, `code_block`, `tool_call`,
    `tool_result`, `thinking`, `image`, `error`).
  - All derive `Serialize`, `Deserialize`, `Clone`,
    `Debug`, with `#[serde(deny_unknown_fields)]` on
    the payload structs (m2 c06 pattern).
- **Why.** scope §E1 + §E3.
- **Depends on.** c02 (ulid serde feature).
- **Acceptance.** New test
  `rafaello-core/tests/entry_serde_round_trip.rs` —
  for each built-in payload kind, build a sample
  `Entry`, serialize to JSON, deserialize, assert
  field equality. Plus `entry_stream_state_rejects_open.rs`
  — decoding `{"stream_state":"open"}` errors per
  scope §E1 v1-only constraint.

### c09 — feat(rafaello-core::entry): RenderNode + RawFormat + node serde

- **What.** scope §E2 + §E4:
  - `pub enum RenderNode` (15 variants per Stream E
    §4.1), `pub enum RawFormat` (Ansi / Html / Plain).
  - Internally tagged on `node` per Stream E §4.2
    (`#[serde(tag = "node")]`).
  - `Unknown { kind: String, payload: Value, fallback:
    EntryFallback }` shape per scope §E2.
- **Why.** scope §E2 + §E4 + Stream E §4.
- **Depends on.** c08.
- **Acceptance.** New test
  `rafaello-core/tests/render_node_serde_round_trip.rs`
  — for each of the 15 variants, build a sample, JSON
  round-trip, assert equality. Plus
  `render_node_unknown_carries_entry_fallback.rs`.

---

## Group 4 — Renderer pipeline + built-in renderers

### c10 — feat(rafaello-core::renderer): registry + Renderer trait + Capabilities

- **What.** scope §R1 + §R2 + §R4 + §R5:
  - `pub trait Renderer: Send + Sync + 'static { fn
    render(&self, entry: &Entry, caps: &Capabilities)
    -> Result<RenderNode, RendererError>; }`.
  - `pub struct RendererRegistry` with
    `new() / with_builtins() / register / get`
    public surface (pi-8 B4: `register` is public so
    rafaello-core/tests/ can inject test renderers).
  - `pub struct Capabilities { unicode, color, width,
    height, image, interactive, scrollback, nodes:
    BTreeSet<String>, raw_formats: BTreeSet<String> }`.
  - `pub enum RendererError { MissingPayloadField,
    InvalidPayload, Internal }`.
  - `with_builtins()` is empty in c10 (registers
    nothing yet); built-in renderers land in c12+c13.
- **Why.** scope §R1 + §R2 + §R4 + §R5.
- **Depends on.** c09.
- **Acceptance.** New test
  `rafaello-core/tests/renderer_registry_register_and_get.rs`
  — register a synthetic renderer for kind `"test:x"`,
  retrieve via `get`, assert it's the same Arc.
  Plus `renderer_registry_with_builtins_is_empty_for_now.rs`
  asserting `RendererRegistry::with_builtins()` returns
  zero entries (sanity check against later commits
  that grow the set).

### c11 — feat(rafaello-core::renderer): RenderPipeline with Path A / B / C

- **What.** scope §R3 — three-path pipeline:
  - Path A: unknown entry kind → Block { Text } from
    fallback.text, OR default Callout if no fallback.
  - Path B: renderer panic / Err → fall through to
    Path A; log at `tracing::error!` (panic) /
    `tracing::warn!` (Err).
  - Path C: capability-driven node downgrade (walks
    the tree, replacing unsupported nodes with
    `Unknown { kind: "<node-name>", payload, fallback:
    entry.fallback }`). For `Raw { format }`, also
    inspect `caps.raw_formats`.
  Implementation uses `std::panic::catch_unwind(
  AssertUnwindSafe(|| renderer.render(...)))`.
- **Why.** scope §R3 + Stream E §6.
- **Depends on.** c10.
- **Acceptance.** Five new tests:
  - `renderer_pipeline_unknown_kind_falls_back_with_author_fallback.rs`
  - `renderer_pipeline_unknown_kind_no_fallback_uses_default_callout.rs`
  - `renderer_pipeline_panic_falls_through_to_path_a.rs`
    (uses `tracing-test` to assert error log)
  - `renderer_pipeline_renderer_err_falls_through_to_path_a.rs`
  - `renderer_capabilities_downgrade_unsupported_node.rs`
    (Image excluded from caps.nodes)
  - `renderer_capabilities_downgrade_unsupported_raw_format.rs`
    (Raw{Html} excluded from caps.raw_formats)

### c12 — feat(rafaello-core::renderer): built-in renderers — text + heading + code_block + thinking

- **What.** scope §R2 + §E3: implement four built-in
  renderers under `rafaello_core::renderer::builtins::*`:
  - `text` → `Text { text, emphasis }` (or
    Block-of-Text when markdown=true; m3 keeps it
    simple — emit Text raw).
  - `heading` → `Heading { level, text }`.
  - `code_block` → `Code { code, lang }`.
  - `thinking` → `Collapsed { summary: Text("thinking"),
    detail: Text(text), default_open: false }`.
  Wire them into `RendererRegistry::with_builtins()`.
- **Why.** scope §R2.
- **Depends on.** c11.
- **Acceptance.** New tests
  `renderer_builtin_text.rs`, `renderer_builtin_heading.rs`,
  `renderer_builtin_code_block.rs`,
  `renderer_builtin_thinking.rs` — for each kind,
  build a sample Entry, render via a fresh
  `RendererRegistry::with_builtins()` + `RenderPipeline`,
  assert tree shape against hand-written expected
  JSON.

### c13 — feat(rafaello-core::renderer): built-in renderers — tool_call + tool_result + image + error

- **What.** Same shape as c12 for the remaining four
  kinds:
  - `tool_call` → `KeyValue { pairs: [name, args, status] }`.
  - `tool_result` → renders the embedded `content`
    `RenderNode` (which is already a tree) wrapped in
    a Block with a status header.
  - `image` → `Image { uri, mime, alt, bytes_b64 }`.
  - `error` → `Callout { kind: "error", child:
    KeyValue { pairs: [code, message, data?] } }`.
  Add to `with_builtins()`.
- **Why.** scope §R2.
- **Depends on.** c12.
- **Acceptance.** Four new tests, same pattern as c12.

---

## Group 5 — Broker frontend extension

### c14 — feat(rafaello-core::broker_acl): BrokerAcl frontends + AttachId + FrontendAcl + Publisher::Frontend

- **What.** scope §B1 + §B2 partial:
  - `pub struct AttachId(String)` newtype with
    `pub fn AttachId::new(s: &str) -> Result<AttachId,
    AttachIdParseError>` validating against
    `^[a-z][a-z0-9-]{0,31}$`. Plus
    `AttachIdParseError` typed error.
  - `pub struct FrontendAcl { subscribe_patterns,
    auto_subscribes, publish_topics }`.
  - `pub struct BrokerAcl` extended with
    `frontends: BTreeMap<AttachId, FrontendAcl>`.
  - `pub enum Publisher` extended with
    `Frontend(AttachId)`.
  - **Compile only** — no broker logic changes yet.
- **Why.** scope §B1 + §B2.
- **Depends on.** c02.
- **Acceptance.** New test
  `attach_id_validates.rs` covers valid + invalid
  inputs (empty, capital letters, leading digit,
  too long, control chars). Existing m2 tests still
  pass — `BrokerAcl::default()` returns an empty
  `frontends` map.

### c15 — feat(rafaello-core::bus): BrokerError frontend variants + register_frontend + RegisteredFrontend RAII

- **What.** scope §B2:
  - `BrokerError::FrontendNotInAcl(AttachId)`,
    `FrontendAlreadyRegistered(AttachId)`,
    `FrontendNotRegistered(AttachId)`.
  - **Reshape** existing `PublishOutsideGrant` and
    `InvalidInReplyTo` to take `publisher: Publisher`
    instead of `canonical: CanonicalId` (source-
    breaking for m2 tests; this commit also updates
    every m2 test that pattern-matches on these
    variants — the change is contained to
    rafaello-core).
  - `pub fn Broker::register_frontend(&self, attach_id:
    AttachId, peer: PeerHandle) -> Result<RegisteredFrontend,
    BrokerError>` symmetric to `register_plugin`.
  - `pub fn Broker::try_reserve_frontend_registration`
    cheap precheck.
  - `pub fn Broker::frontend_acl(&self, attach_id:
    &AttachId) -> Option<FrontendAcl>`.
  - RAII guard `RegisteredFrontend` mirrors
    `RegisteredPlugin`.
  - **Construction-time validation**: `Broker::new`
    extends m2 §B10 to revalidate
    `frontend_acl.publish_topics` (validate_topic)
    AND `subscribe_patterns` / `auto_subscribes`
    (validate_pattern).
- **Why.** scope §B2 + §B6, pi-7 #10.
- **Depends on.** c14.
- **Acceptance.** New tests:
  - `broker_register_frontend_unknown_attach_id_rejected.rs`.
  - `broker_register_frontend_duplicate_rejected.rs`.
  - `broker_construct_with_invalid_frontend_pattern_rejected.rs`.
  - `broker_construct_with_invalid_frontend_publish_topic_rejected.rs`.
  m2 tests rewritten for `Publisher`-shaped variants
  pass.

### c16 — feat(rafaello-core::bus): handle_frontend_publish + frontend publish authority + fan-out

- **What.** scope §B3 + §B4 + §B5:
  - `pub fn Broker::handle_frontend_publish(&self,
    attach_id: &AttachId, raw_params: &Value) ->
    Result<(), BrokerError>` symmetric to
    `handle_plugin_publish`.
  - Promote `PublisherIdentity::Frontend { attach_id:
    String }` from commented-future to live in the
    serde-tagged enum (`kind: "frontend"`).
  - Authority enforcement (scope §B4): grammar →
    namespace lookup → segment-count → grant
    membership → fan-out. Cases:
    `core/provider/plugin` → `PublishOnReservedNamespace`;
    `evil.foo` → `UnknownNamespace`;
    `frontend` with <3 segments → `PublishOnReservedNamespace`;
    `frontend.<other>.foo` → `PublishOnReservedNamespace`;
    `frontend.<own>.foo` not in `publish_topics` →
    `PublishOutsideGrant`; in grant → fan-out.
  - Fan-out (scope §B5): frontend subscribers receive
    `core.session.**` etc. the same way plugins do.
- **Why.** scope §B3 + §B4 + §B5.
- **Depends on.** c15.
- **Acceptance.** New tests:
  - `frontend_publish_on_reserved_namespace_rejected.rs`
    (core/provider/plugin from a frontend).
  - `frontend_publish_unknown_namespace_rejected.rs`
    (`evil.foo`).
  - `frontend_publish_two_segment_topic_rejected.rs`
    (`frontend.tui` only).
  - `frontend_publish_outside_grant_rejected.rs`
    (`frontend.tui.confirm_answer` not in m3 grant).
  - `frontend_subscribes_to_core_session_events.rs`
    (in-process, no subprocess — uses fittings
    in-memory transport).

---

## Group 6 — Frontend supervisor

### c17 — feat(rafaello-core::frontend): module + types + FrontendConfig + FrontendSpawnError

- **What.** scope §F1 + §F2:
  - New module `rafaello_core::frontend` with public
    surface: `FrontendSupervisor`, `CompiledFrontend`
    (with `attach_id: String`), `FrontendPaths`,
    `FrontendHandle` (struct shape from scope §F1
    with all 9 fields), `FrontendConfig` (with
    `Default` impl), `ShutdownReport`,
    `FrontendReadyError`, `PaintError`,
    `FrontendBusPublishService`,
    `FrontendReadyService`,
    `FrontendExtraServiceFactory`.
  - `FrontendSpawnError` typed enum +
    `InvalidFrontendPlanReason` companion enum
    re-exported from `lib.rs`.
  - **No spawn body yet** — types only.
- **Why.** scope §F1 + §F2.
- **Depends on.** c02, c14.
- **Acceptance.** New build-only test
  `m3_frontend_error_surface_compiles.rs` (m1 c02 +
  m2 c05 pattern) asserting each variant is reachable
  via `rafaello_core::Error`.

### c18 — feat(rafaello-core::frontend): Phase A validation + try_reserve check

- **What.** scope §F3 Phase A:
  - `FrontendSupervisor::spawn` Phase A:
    `AttachId::new(&plan.attach_id)` validation,
    control-char check, relative-path check, exec-bit
    stat, reserved-env check, broker
    try_reserve_frontend_registration. Returns
    `FrontendSpawnError::InvalidPlan { reason: ... }`
    on every failure mode.
- **Why.** scope §F3 Phase A.
- **Depends on.** c17.
- **Acceptance.** New negative tests under
  `rafaello-core/tests/`:
  - `frontend_spawn_invalid_attach_id_rejected.rs`
  - `frontend_spawn_relative_entry_path_refused.rs`
  - `frontend_spawn_control_chars_in_path_refused.rs`
  - `frontend_spawn_entry_not_executable_refused.rs`
  - `frontend_spawn_reserved_env_in_pass_refused.rs`
  - `frontend_spawn_reserved_env_in_set_refused.rs`
  - `frontend_register_unknown_attach_id_rejected.rs`
    (via `try_reserve_frontend_registration` failure).

### c19 — feat(rafaello-core::frontend): Phase B steps 1-7 (env, socketpair, spawn, take stderr, private state dir)

- **What.** scope §F3 Phase B steps 1-8 (note step 5
  is `create_dir_all` for private state, scope round-
  21 wording):
  - socketpair (CLOEXEC + macOS fcntl fallback per m2
    §5.7),
  - tokio::process::Command construction,
  - env apply (env_clear + RFL_BUS_FD +
    RFL_PROJECT_ROOT + RFL_PRIVATE_STATE_DIR +
    env.pass + env.set),
  - pre_exec FD_CLOEXEC strip on the inherited fd,
  - private state dir create_dir_all,
  - command.stderr(Stdio::piped()),
  - command.spawn(),
  - child.stderr.take().
- **Why.** scope §F3 Phase B 1-8.
- **Depends on.** c18.
- **Acceptance.** New test
  `frontend_spawn_phase_b_smoke.rs` — spawns
  `rfl-bus-fixture` in `signal_ready` mode (will be
  added in c25 — c19 uses `respond_peer_call`-mode
  for now, then c25 re-points the test post-merge to
  `signal_ready`); assert child PID is captured,
  child stderr is taken, env is applied. (This is a
  pre-flight test for c20-c21's full lifecycle work;
  the test will continue to be useful as a smoke
  coverage even after the full handle returns.)

### c20 — feat(rafaello-core::frontend): Phase B steps 8-11 (watches, reaper task, watcher task, server build)

- **What.** scope §F3 Phase B steps 9-12: readiness
  watch channel construction, reaper-outcome watch
  channel, reaper task spawn, reaper-watcher task
  spawn (consumes reaper's JoinHandle, on JoinError
  pushes `ReaperPanicked`), fittings_server::Server
  build with `FrontendBusPublishService` +
  `FrontendReadyService` composed via
  `FrontendExtraServiceFactory`.
- **Why.** scope §F3 + scope "Spawn the reaper +
  reaper-watcher tasks".
- **Depends on.** c19.
- **Acceptance.** New tests:
  - `frontend_reaper_publishes_exited_on_clean_exit.rs`
    — spawn fixture, await outcome, assert
    `Exited(status)` with `status.success()`.
  - `frontend_reaper_publishes_reaper_panicked_on_panic.rs`
    — induce a reaper-task panic via a test-only
    `inject_reaper_panic` cfg-gated hook;
    assert `ReaperPanicked` is published.

### c21 — feat(rafaello-core::frontend): Phase B steps 12-15 — register, serve, return; FrontendHandle Drop + shutdown

- **What.** scope §F3 step 12-15 + §F4:
  - `Broker::register_frontend(...)` →
    `FrontendHandle.register_guard`.
  - `tokio::spawn(server.serve())` →
    `FrontendHandle.serve_handle`.
  - `FrontendHandle` constructed with all 9 fields
    populated.
  - `FrontendHandle::wait` (resolves on
    reaper-outcome watch).
  - `FrontendHandle::wait_ready` (resolves on
    readiness watch + handles SenderDropped).
  - `FrontendHandle::take_child_stderr`.
  - `FrontendHandle::has_signalled_ready`.
  - `FrontendHandle::shutdown(mut self)` —
    take()s disarmable fields, calls
    `shutdown_with_outcome` from c06, returns
    `ShutdownReport` per scope §F4 spec.
  - `FrontendHandle::Drop` — Exited-skip /
    abnormal-best-kill / None-best-kill per scope
    §F4.
- **Why.** scope §F3 step 12-15 + §F4.
- **Depends on.** c20.
- **Acceptance.** New tests:
  - `frontend_handle_wait_ready_resolves_on_signal.rs`
    (rfl-bus-fixture `signal_ready` mode — note c25
    adds the mode; c21 uses a pre-c25 stub mode
    that's identical for this purpose).
  - `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs`.
  - `frontend_handle_wait_resolves_on_child_exit.rs`.
  - `frontend_handle_drop_does_not_leak_zombie.rs`
    (Linux-only).
  - `frontend_handle_shutdown_skips_kill_on_exited.rs`.
  - `frontend_handle_shutdown_kills_on_waitfailed.rs`
    (uses `shutdown_with_outcome` directly with
    mock signal_fn / probe_fn).

---

## Group 7 — Session store + controller

### c22 — feat(rafaello-core::session): SessionStore::open with Flock<File> + lock-first ordering

- **What.** scope §S1 + §S5 + §S3 (partial):
  - New module `rafaello_core::session`.
  - `pub struct SessionStore { conn: Mutex<Connection>,
    lock_guard: Flock<File>, ... }`.
  - `pub fn SessionStore::open(state_dir: &Path) ->
    Result<Self, SessionError>` with the lock-first
    ordering from scope §S1: mkdir state_dir, open
    lockfile O_CLOEXEC, `Flock::lock(file,
    LockExclusiveNonblock)`, write own pid, open
    SQLite + run PRAGMAs + create tables + verify
    schema_version.
  - `pub fn SessionStore::session_id(&self) -> &str`.
  - `pub fn SessionStore::lock_fd_for_test(&self) ->
    RawFd` (cfg-gated).
  - `SessionError` enum with `Io`, `Sqlite`, `Serde`,
    `Locked { holder_pid: Option<u32> }`,
    `SchemaMismatch`. (The `Publish` variant adds at
    c24 when SessionController lands.)
- **Why.** scope §S1 + §S5 + §S3.
- **Depends on.** c02.
- **Acceptance.** New tests:
  - `session_store_open_creates_state_dir.rs`.
  - `session_store_concurrent_open_errors.rs`
    (cross-platform — uses a probe child).
  - `session_store_locked_unknown_holder_errors.rs`
    (empty / corrupt lockfile yields `holder_pid:
    None`).
  - `session_store_schema_mismatch_errors.rs`.
  - `session_store_lock_fd_not_inherited_by_child.rs`
    (uses `probe_fd_closed` mode — c25 adds it; c22
    uses a temporary stub probe child via
    `/bin/sh -c 'exec 3<&-; ...'` shell-level
    fcntl until c25 lands).

### c23 — feat(rafaello-core::session): append_entry + load_entries + StoredEntry + schema

- **What.** scope §S1 + §S2:
  - SQL schema: `entries` table + `session_meta`
    table (table creation in c22; this commit fills
    the rows + queries).
  - `pub struct StoredEntry { seq: u64, entry: Entry }`.
  - `pub fn SessionStore::append_entry(&self, entry:
    &Entry) -> Result<u64, SessionError>`.
  - `pub fn SessionStore::load_entries(&self) ->
    Result<Vec<StoredEntry>, SessionError>` ordered
    by `seq`.
- **Why.** scope §S1 + §S2.
- **Depends on.** c22.
- **Acceptance.** New tests:
  - `session_store_round_trip.rs` — append three
    entries (text, code_block, tool_call), close,
    reopen, load — see all three back in seq order
    starting from 0.
  - `session_store_seq_monotonic.rs` — append four
    entries, observe seq 0/1/2/3.

### c24 — feat(rafaello-core::session): SessionController::new + finalize_entry + replay_history

- **What.** scope §S1 (controller bullets):
  - `pub struct SessionController { store, pipeline,
    broker }`.
  - `pub async fn finalize_entry(&self, entry: Entry,
    caps: &Capabilities) -> Result<(), SessionError>`
    — append → render → publish on
    `core.session.entry.finalized` with `replay:
    false`.
  - `pub async fn replay_history(&self, caps:
    &Capabilities) -> Result<(), SessionError>` —
    iterates `load_entries`, renders each, publishes
    with `replay: true`.
  - **`SessionError::Publish { source: BrokerError }`**
    variant added (pi-12 #2).
- **Why.** scope §S1 SessionController + pi-12 #2.
- **Depends on.** c11, c16, c23.
- **Acceptance.** New tests:
  - `session_controller_finalize_entry.rs` — uses
    `in_memory_broker_with_tui_and_observer_acl()`
    helper (lands in c25's harness preview); for
    c24 the test uses an inline observer-plugin
    helper. After
    `controller.finalize_entry(entry).await`, assert
    (a) row in SQLite, (b) one
    `core.session.entry.finalized` event fired with
    rendered tree, (c) replay flag false.
  - `session_controller_replay_history.rs` — pre-seed
    three entries; `replay_history` publishes three
    events with `replay: true` in seq order.

---

## Group 8 — Fixture self-timeout + L1a modes (m2 fixture extension)

### c25 — feat(rfl-bus-fixture): `RFL_FIXTURE_MAX_LIFETIME` + 5 m3 fixture modes

- **What.** scope §L1 + §L1a: extend
  `rafaello-core/src/bin/rfl_bus_fixture.rs` (m2's
  test fixture):
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
      ESRCH else non-zero.
- **Why.** scope §L1 + §L1a; m2 retro §4.4.
- **Depends on.** c02.
- **Acceptance.** Five new tests
  `rafaello-core/tests/fixture_mode_<NAME>.rs`
  exercising each mode through `Command::new` +
  `RFL_FIXTURE_MODE` env. Existing m2 fixture tests
  still pass.

---

## Group 9 — TUI binary

### c26 — feat(rafaello-tui::bin): `rfl-tui` core — env parsing, fd adoption, fittings client, BusEventHandler, frontend.ready RPC

- **What.** scope §T1 + §T2 step 1-3:
  - `rfl-tui` binary parses `RFL_BUS_FD`,
    `RFL_PROJECT_ROOT`, `RFL_TUI_TEST_MODE`,
    `RFL_TUI_READY_DELAY_MS`, `RFL_TUI_MAX_LIFETIME`.
  - Adopts inherited socketpair fd as
    `tokio::net::UnixStream`.
  - Builds `fittings_client::Client` with
    `BusEventHandler` (notification handler).
  - After handler is wired, calls
    `peer.call("frontend.ready", json!({}))` and
    prints stderr sentinel.
  - `RFL_TUI_READY_DELAY_MS` insertion before the
    `peer.call`.
- **Why.** scope §T1 + §T2 steps 1-3.
- **Depends on.** c03, c16.
- **Acceptance.** New test
  `rafaello-tui/tests/tui_handler_calls_frontend_ready.rs`
  — spawns rfl-tui in headless mode, asserts the
  `frontend.ready` RPC arrives parent-side via a
  test-side `FrontendReadyService` mock.

### c27 — feat(rafaello-tui::bin): headless test mode + stderr sentinels + RFL_TUI_MAX_LIFETIME self-timeout

- **What.** scope §T2 step 4 + sentinels:
  - When `RFL_TUI_TEST_MODE=1`: skip terminal init,
    in-memory log, exit on first
    `core.lifecycle.test_done` event.
  - Stderr sentinels (raw, no `rfl-tui:` prefix —
    forwarder adds it):
    `"bus.event topic=<topic> seq=<n>"`,
    `"test-done"`,
    `"project-root=<abs-path>"`.
  - `RFL_TUI_MAX_LIFETIME` self-timeout in test mode.
- **Why.** scope §T2 step 4 + scope §T2 sentinels.
- **Depends on.** c26.
- **Acceptance.** New tests:
  - `tui_test_mode_logs_bus_events_to_stderr.rs`.
  - `tui_test_mode_exits_on_test_done.rs`.
  - `tui_test_mode_self_timeout_exits_zero.rs`.
  - `tui_sends_frontend_ready_after_handler_registration.rs`
    (deterministic-callback ordering, scope §I).

### c28 — feat(rafaello-tui::bin): production-mode UI loop with crossterm + raw mode + ratatui

- **What.** scope §T2 step 5-7: terminal init,
  alternate-screen, raw-mode, redraw on event arrival,
  q-to-quit, arrow-key scroll, restore-on-exit.
  No mouse, no theming.
- **Why.** scope §T2 step 5-7 + §T6.
- **Depends on.** c27, c29 (Painter — actually painter
  belongs alongside; merge ordering: c28 lands UI loop
  scaffold using a stub `paint_frame(&[entries])`,
  c29 fills the painter; reorder if pi prefers).
- **Acceptance.** A manual smoke recording (lands in
  c34 `manual-validation.md`) demonstrates a real
  interactive `rfl chat` against the harness fixture.
  No automated production-mode tests in c28
  (production mode is exercised by humans only;
  scope §I rfl_chat_demo_bar uses headless mode).

### c29 — feat(rafaello-tui::paint): Painter::draw_with_panic_isolation + paint_node + lib unit test

- **What.** scope §I `tui_paint_panic_isolation` +
  scope §T4 + §T5:
  - `pub fn rafaello_tui::paint::draw_with_panic_isolation(
    term, &RenderNode) -> Result<(), PaintError>`.
  - Internal `paint_node` per Stream E §4 variant.
  - `#[cfg(test)] PaintAction` enum +
    `draw_with_panic_isolation_for_test`.
  - Library unit test in `rafaello-tui/src/paint.rs`
    `#[cfg(test)] mod tests` exercising the panic-
    isolation seam against `ratatui::backend::TestBackend`.
- **Why.** scope §T4 + §T5 + scope §I `tui_paint_panic_isolation`.
- **Depends on.** c09 (RenderNode), c03 (rafaello-tui crate).
- **Acceptance.** Unit test passes; assertion that a
  panicking variant produces `[render error: ...]` row
  on the test backend AND the next render call
  proceeds normally.

---

## Group 10 — `rfl chat` subcommand

### c30 — feat(rafaello::lib): RflChatCli + RflChatError + resolve_tui_path pure function + 3 unit tests

- **What.** scope §C1 + §C3 + scope `RflChatError`
  variants:
  - clap `Cli` with `Chat { project_root:
    Option<PathBuf> }` subcommand.
  - `pub enum RflChatError` with all variants used
    in §C2 (`ProjectRootInvalid`, `TuiPathUnresolved`,
    `FrontendReadyTimeout`,
    `FrontendExitedBeforeReady`,
    `FrontendExitedAbnormally`).
  - `pub fn resolve_tui_path(env, current_exe) ->
    Result<PathBuf, RflChatError>` pure function
    per scope §C3.
  - Three unit tests in
    `rafaello/tests/resolve_tui_path_*.rs` using
    synthetic env + exe paths.
- **Why.** scope §C1 + §C3 + pi-15 #5.
- **Depends on.** c04.
- **Acceptance.** Three resolve_tui_path tests pass.

### c31 — feat(rafaello::lib): rfl chat orchestration steps 1-7 — open/spawn/wait_ready

- **What.** scope §C2 steps 1-7:
  - resolve project root (canonicalize per pi-17
    #4),
  - resolve `rfl-tui` path,
  - open SessionStore (acquires flock),
  - build BrokerAcl with single `tui` frontend
    entry,
  - construct Broker / RendererRegistry /
    RenderPipeline / SessionController,
  - build CompiledFrontend with EnvPlan.pass
    allowlist (scope §C2 step 5),
  - `frontend_supervisor.spawn(...)` (note:
    FrontendSupervisor still uses
    `tokio::process::Command` — scope §F3),
  - take child stderr + spawn forwarder task
    serialised on tokio::sync::Mutex<()>,
  - `wait_ready` with bounded timeout, three
    outcomes mapped to RflChatError variants per
    scope §C2 step 7 (Ok prints
    `"rfl-chat: frontend-ready-observed"` on
    parent stderr).
- **Why.** scope §C2 steps 1-7.
- **Depends on.** c21 (FrontendHandle), c24 (controller),
  c30 (RflChatError).
- **Acceptance.** Three CLI tests under
  `rafaello/tests/`:
  - `rfl_chat_locked_session_errors_with_holder_pid.rs`.
  - `rfl_chat_locked_session_unknown_holder_errors.rs`.
  - `rfl_chat_relative_project_root_canonicalises.rs`.
  - `rfl_chat_nonexistent_project_root_errors.rs`.
  - `rfl_chat_resolves_tui_via_env_override.rs`
    (uses real rfl-tui in test mode).
  - `rfl_chat_frontend_exits_before_ready_errors.rs`.
  - `rfl_chat_frontend_ready_timeout_errors.rs`.

### c32 — feat(rafaello::lib): rfl chat orchestration steps 8-10 — replay/harness/wait + cleanup guard

- **What.** scope §C2 steps 8-10 + cleanup guard:
  - cleanup-guard ownership pattern per scope §C2
    pseudocode (Option/take, single shutdown +
    forwarder.await regardless of inner result),
  - replay_history,
  - in-test fixture-entry harness (only when
    `RFL_HARNESS_FIXTURES=1`),
  - wait on FrontendHandle.wait, map outcome to
    RflChatStep10Outcome, propagate via
    `RflChatError::FrontendExitedAbnormally`,
  - shutdown via guard, then drop store/controller.
- **Why.** scope §C2 steps 8-10 + cleanup guard.
- **Depends on.** c31.
- **Acceptance.**
  - `rfl_chat_replay_withheld_until_frontend_ready.rs`
    (single combined stderr stream, line-order
    assertion).
  - `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
    (uses `signal_ready_then_exit_n` fixture).

---

## Group 11 — Demo-bar headline + manual validation

### c33 — test(rafaello): rfl_chat_demo_bar.rs + workspace_bin_path helper

- **What.** scope §I rafaello/tests/ `rfl_chat_demo_bar.rs`
  + `rafaello/tests/common/workspace_bin_path.rs`:
  - `workspace_bin_path(name: &str) -> PathBuf`
    helper resolving target-dir + binary name.
  - End-to-end test that spawns `rfl chat` with
    `RFL_HARNESS_FIXTURES=1` + `RFL_TUI_TEST_MODE=1`
    against a tempdir project root; asserts (a)
    nine SQLite rows after shutdown matching the
    eight built-in kinds + one unknown kind, (b)
    nine `"rfl-tui: bus.event"` lines on combined
    stderr.
- **Why.** scope §I + §"Acceptance summary".
- **Depends on.** c32.
- **Acceptance.** `rfl_chat_demo_bar.rs` passes on
  Linux. macOS CI green by milestone close.

### c34 — docs(rafaello-m3): write `manual-validation.md`

- **What.** scope §"Manual validation": new
  `rafaello/plans/milestones/m3-tui-sessions/manual-validation.md`
  capturing:
  - cargo-test-all output (Linux + macOS CI URL).
  - real interactive `rfl chat` recording (qquit
    works, alternate-screen restores, fallbacks
    render, locked-session errors with holder pid).
  - macOS CI run URL.
- **Why.** scope §"Manual validation" + §"Acceptance summary".
- **Depends on.** c33.
- **Acceptance.** `manual-validation.md` exists with
  the items listed; macOS CI URL captured (after
  pushing the milestone branch to origin).

---

## Phase 4 — `retrospective.md` is driver-owned, NOT a per-commit task

The milestone driver writes `retrospective.md` in Phase 4
after all 34 commits land. The retrospective lands as a
separate commit on a `agents/m3/retro` worktree, reviewed
by pi (m1 needed 4 rounds; m2 needed 2). Drift fixes
identified by pi land before retrospective ratification.
The milestone branch then merges to `rafaello-v0.1` with
linear history per `decisions.md` row 33.

Anticipated drift items (from scope §"Acceptance summary"):

- Stream E renderer-RFC banner.
- `PublisherIdentity::Frontend` schema additions to
  Stream A.
- Capabilities staging note in overview §10.1.
- Replay over `core.session.entry.finalized` decisions row.
- BrokerError variant additions decisions row.
- m1 publishes-grant unknown-namespace tightening
  (lands in c05 here; retrospective records the
  back-reach).
- `m1 lock-side check_lock_publish_topic` deferral.
- `FrontendSupervisor` lock-correspondence claim.
- Fixture self-timeout (`RFL_FIXTURE_MAX_LIFETIME`).
- m3 frontend ACL `publish_topics = []` handover to m4.

## What changed from prior drafts

This is round-1; awaiting pi adversarial review.
