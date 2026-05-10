# m3-tui-sessions — commits

> **Status:** round-6 draft. Round 1: 6b + 4h + 5m.
> Round 2: 3b + 3h + 3m. Round 3: 1b + 3h + 3m.
> Round 4: 2b + 2h + 4m. Round 5: 1b + 0h + 4m
> (`commits-pi-review-5.md`).
> Round 6 cleans up the c08+c09 collapse drift:
> - c09 self-dependency fixed → depends on c08
>   (pi-5 B1).
> - c08 acceptance count "Five new tests" (was
>   "Three"; pi-5 M1).
> - Group 6 heading "+ 6 m3 modes" (was "+ 5"; pi-5
>   M2).
> - c26 deps c08 for RenderNode (was c09; pi-5 M3).
> - Phase 4 "all 31 commits" (was "32"; pi-5 M4).
>
> Round-5 highlights (kept for trajectory):
> Round 5 collapses c08+c09 into a single commit (the
> RenderNode dep on `tool_result.content` made the
> split unimplementable per pi-4 B1) and accepts a
> direct `serde_json` dep in the rafaello bin (pi-4 B2
> — the no-direct-dep claim was unnecessary friction
> for the harness's `tool_call` args). Net: m3 =
> **31 commits in 12 groups**.
>
> Round-5 highlights:
> - **c08+c09 merge**: scope's `tool_result { content:
>   RenderNode }` couples Entry payloads to RenderNode
>   irreducibly. New consolidated c08 covers Entry +
>   metadata + helpers + payload structs + RenderNode
>   + RawFormat + ALL constructors (pi-4 B1).
> - rafaello bin gains direct `serde_json` dep (pi-4
>   B2). Removed from c04 already-listed deps; c04
>   updated.
> - c08 constructor contract spelled out fully:
>   `ToolCallStatus` enum introduced; `new_tool_call`
>   takes an explicit `id: &str` parameter; author
>   mapping per kind documented (pi-4 H1).
> - c20 wiring test mechanism pinned: uses a new
>   `frontend_bus_publish` fixture mode that calls
>   `peer.notify("bus.publish", ...)`; assertion
>   reads from a registered observer plugin's
>   `core.lifecycle.publish_rejected` event with
>   `code = "publish_outside_grant"` (pi-4 H2). The
>   fixture mode adds to c16.
> - c19 `Depends on` adds c14 (pi-4 M1).
> - c20 acceptance "Nine tests" (was "Eight"; pi-4
>   M2).
> - c27 manual-smoke note points at Phase-4
>   driver-owned manual-validation.md, not c31
>   (pi-4 M3).
> - "What changed from prior drafts" stale bullets
>   updated (pi-4 M4).
>
> Round-4 highlights (kept for trajectory context):
> - c08 `Entry::new_*` constructors fully spec'd
>   (pi-3 B1).
> - c24 build-only; integration test
>   `tui_handler_calls_frontend_ready.rs` moves to
>   c25 (pi-3 H1).
> - c07 unwind contract pinned: NO private-state-dir
>   removal assertion (pi-3 H2).
> - c16 acceptance now lists existing-mode
>   `respond_peer_call_self_timeout.rs` regression
>   (pi-3 H3).
> - c20 adds `frontend_bus_publish_service_routes_to_handle_frontend_publish.rs`
>   wiring assertion (pi-3 M1).
> - Stale "round-2 draft" / "34 commits" banner +
>   "controller landed by c22" checkpoint corrected
>   (pi-3 M2 + M3).

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
**c13** (renderer pipeline complete) and after **c23**
(SessionController landed — c21 SessionStore + c22
StoredEntry + c23 Controller); if a split becomes
obviously beneficial, the driver opens an m3a / m3b
owner-ratification request.

## Canonical test names

Wherever `scope.md` and `commits.md` both name a test, this
`commits.md` is canonical. The headline test is
**`rfl_chat_demo_bar.rs`** (lands at c31).

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
    `anyhow`, **`serde_json`** (pi-4 B2 — the
    in-test fixture-entry harness in c30 needs to
    construct `tool_call.args` and the unknown-kind
    payload as `serde_json::Value`; the round-3
    "no direct serde_json dep" goal was unnecessary
    friction). `[dev-dependencies]`: `tempfile`,
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
  moves to c19 alongside `FrontendHandle::shutdown`):
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

- **What.** scope §H6.3 + §I positive matrix.
  **Unwind assertion contract** (pi-3 H2 — round 17
  scope §H6.2 says private-state dirs are NOT
  cleaned up on unwind, but H6.3 wording implied
  they were; round-3 commits.md pins it
  unambiguously): every unwind test asserts
  fd-count baseline (Linux-only files) + proxy
  cleanup + `in_flight` cleared + (post-register
  tests only) broker registration rolled back. Tests
  do NOT assert private-state-dir removal — m3
  considers per-frontend dirs user-scoped artifacts
  that the next `rfl chat` reuses.
  Six unwind test files:
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
  doesn't exist until c19. They move to c19.
- **Why.** scope §H6.3, §I, m2 retro §5.1.
- **Depends on.** c06.
- **Acceptance.** All six unwind tests pass on Linux;
  the cross-platform tests pass on macOS too.

---

## Group 3 — Entry + RenderTree types

### c08 — feat(rafaello-core::entry): Entry + RenderNode + payloads + constructors (consolidated)

- **What.** scope §E1 + §E2 + §E3 + §E4. Pi-4 B1
  consolidates round-3's c08 + c09 split because
  scope's `tool_result { content: RenderNode }`
  payload makes the split unimplementable (payload
  module needs RenderNode; RenderNode was deferred
  to c09; c08 wouldn't compile).
  - New module `rafaello_core::entry` with `Entry`,
    `EntryMetadata`, `EntryAuthor`, `EntryFallback`,
    `StreamState` (v1: `Final` only),
    `ToolCallStatus { Pending, Running, Ok, Error }`
    (pi-4 H1 — was implicit; now explicit).
  - **`RenderNode` enum** (15 variants per Stream E
    §4.1), **`RawFormat` enum** (Ansi / Html /
    Plain). Internally tagged on `node` per Stream E
    §4.2. `Unknown { kind, payload, fallback:
    EntryFallback }` shape per scope §E2.
  - Sub-module `rafaello_core::entry::payloads::*`
    with the eight built-in payload structs (incl.
    `tool_result { call_id, ok, content: RenderNode,
    details: Option<Value> }`).
  - All derive `Serialize`, `Deserialize`, `Clone`,
    `Debug`, with `#[serde(deny_unknown_fields)]` on
    payload structs.
  - **Public constructor methods on `Entry`** for the
    eight built-in kinds (pi-4 H1: explicit
    signatures + `id` parameter for tool_call /
    tool_result + author mapping):
    - `Entry::new_text(text: &str) -> Self`,
      author = Assistant.
    - `Entry::new_heading(level: u8, text: &str) -> Self`,
      author = Assistant.
    - `Entry::new_code_block(code: &str, lang: Option<&str>) -> Self`,
      author = Assistant.
    - `Entry::new_thinking(text: &str) -> Self`,
      author = Assistant.
    - `Entry::new_tool_call(id: &str, name: &str, args: serde_json::Value, status: ToolCallStatus) -> Self`,
      author = Assistant. The `id` is the tool-call
      correlation id (separate from `Entry.id`); the
      m4 controller will reuse this id on the
      matching `tool_result`.
    - `Entry::new_tool_result(call_id: &str, ok: bool, content: RenderNode) -> Self`,
      author = Tool.
    - `Entry::new_image(uri: &str, mime: &str, alt: &str) -> Self`,
      author = Assistant.
    - `Entry::new_error(code: &str, message: &str) -> Self`,
      author = System.
    - `Entry::new_unknown(kind: &str, payload: serde_json::Value, fallback: EntryFallback) -> Self`,
      author = Plugin.
    Each constructor sets `id: Ulid::new()`,
    `metadata.created_at: Utc::now()`,
    `metadata.stream_state: Final`, `parent: None`,
    `metadata.tags: vec![]`. The `tool_call` /
    `tool_result` / `unknown` constructors require
    callers to provide `serde_json::Value` for the
    free-form fields; pi-4 B2 — the rafaello bin
    accepts a direct `serde_json` dep (added in c04
    update below) since the harness needs it for
    `tool_call.args` and the unknown-kind payload.
- **Why.** scope §E1 + §E2 + §E3 + §E4 + pi-4 B1
  + pi-4 B2 + pi-4 H1.
- **Depends on.** c02.
- **Acceptance.** Five new tests (pi-5 M1 — count
  updated for the c08+c09 collapse):
  - `entry_serde_round_trip.rs` (eight built-in
    payload cases — round-trip JSON encode/decode).
  - `entry_stream_state_rejects_open.rs`
    (`{"stream_state":"open"}` errors on decode).
  - `render_node_serde_round_trip.rs` (15 variants).
  - `render_node_unknown_carries_entry_fallback.rs`.
  - `entry_constructors_smoke.rs` — for each of the
    nine `Entry::new_*` constructors, build a
    sample, assert the resulting metadata fields
    populated correctly (author per kind,
    stream_state = Final, id is a fresh Ulid,
    created_at is a recent timestamp).

---

## Group 4 — Renderer pipeline + built-in renderers

### c09 — feat(rafaello-core::renderer): registry + Renderer trait + Capabilities

- **What.** scope §R1 + §R2 + §R4 + §R5:
  - `Renderer` trait, `RendererRegistry` (with
    `new`, `with_builtins`, `register`, `get`),
    `Capabilities` (with `raw_formats`),
    `RendererError`.
  - `with_builtins()` is empty in c09 (registers
    nothing yet); built-in renderers land in c11 + c12.
- **Why.** scope §R1 + §R2 + §R4 + §R5.
- **Depends on.** c08 (Entry / RenderNode / payloads —
  pi-5 B1 corrects round-5's renumber-induced
  self-dependency).
- **Acceptance.** Two tests (pi-1 medium #5: was
  contradicting itself with a "with_builtins is empty"
  assertion that c11 would invalidate; round-2
  asserts on `RendererRegistry::new()` — never
  populated):
  - `renderer_registry_register_and_get.rs` — register
    a synthetic renderer, retrieve via `get`, assert
    Arc identity.
  - `renderer_registry_new_is_empty.rs` —
    `RendererRegistry::new().get("text")` returns
    `None`. (Stable across c11 + c12.)

### c10 — feat(rafaello-core::renderer): RenderPipeline with Path A / B / C

- **What.** scope §R3 — three-path pipeline (Path A
  unknown-kind / Path B panic-or-Err / Path C
  capability-driven downgrade). Implementation uses
  `std::panic::catch_unwind(AssertUnwindSafe(...))`.
- **Why.** scope §R3 + Stream E §6.
- **Depends on.** c09.
- **Acceptance.** Six new tests (pi-1 medium #5: was
  "five" but listed six):
  - `renderer_pipeline_unknown_kind_falls_back_with_author_fallback.rs`.
  - `renderer_pipeline_unknown_kind_no_fallback_uses_default_callout.rs`.
  - `renderer_pipeline_panic_falls_through_to_path_a.rs`
    (`tracing-test` asserts error log).
  - `renderer_pipeline_renderer_err_falls_through_to_path_a.rs`.
  - `renderer_capabilities_downgrade_unsupported_node.rs`.
  - `renderer_capabilities_downgrade_unsupported_raw_format.rs`.

### c11 — feat(rafaello-core::renderer): built-in renderers — text + heading + code_block + thinking

- **What.** scope §R2 + §E3: implement four built-in
  renderers under `rafaello_core::renderer::builtins::*`
  and wire them into `RendererRegistry::with_builtins()`.
- **Why.** scope §R2.
- **Depends on.** c10.
- **Acceptance.** Four tests:
  `renderer_builtin_text.rs`, `renderer_builtin_heading.rs`,
  `renderer_builtin_code_block.rs`,
  `renderer_builtin_thinking.rs`.

### c12 — feat(rafaello-core::renderer): built-in renderers — tool_call + tool_result + image + error

- **What.** Remaining four kinds wired into
  `with_builtins()`.
- **Why.** scope §R2.
- **Depends on.** c11.
- **Acceptance.** Four tests:
  `renderer_builtin_tool_call.rs`,
  `renderer_builtin_tool_result.rs`,
  `renderer_builtin_image.rs`,
  `renderer_builtin_error.rs`.

---

## Group 5 — Broker frontend extension

### c13 — feat(rafaello-core::broker_acl): BrokerAcl frontends + AttachId + FrontendAcl + Publisher::Frontend

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

### c14 — feat(rafaello-core::bus): BrokerError frontend variants + register_frontend + RegisteredFrontend RAII

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
- **Depends on.** c13.
- **Acceptance.** Four new tests +
  m2 publisher-typed-test rewrites pass:
  - `broker_register_frontend_unknown_attach_id_rejected.rs`.
  - `broker_register_frontend_duplicate_rejected.rs`.
  - `broker_construct_with_invalid_frontend_pattern_rejected.rs`.
  - `broker_construct_with_invalid_frontend_publish_topic_rejected.rs`.

### c15 — feat(rafaello-core::bus): handle_frontend_publish + frontend publish authority + fan-out

- **What.** scope §B3 + §B4 + §B5:
  - `Broker::handle_frontend_publish`.
  - `PublisherIdentity::Frontend { attach_id }`
    promoted to live (`kind: "frontend"`).
  - Authority enforcement (scope §B4): grammar →
    namespace lookup → segment-count → grant
    membership → fan-out.
  - Fan-out for frontend subscribers.
- **Why.** scope §B3 + §B4 + §B5.
- **Depends on.** c14.
- **Acceptance.** Five new tests:
  - `frontend_publish_on_reserved_namespace_rejected.rs`.
  - `frontend_publish_unknown_namespace_rejected.rs`.
  - `frontend_publish_two_segment_topic_rejected.rs`.
  - `frontend_publish_outside_grant_rejected.rs`.
  - `frontend_subscribes_to_core_session_events.rs`.

---

## Group 6 — rfl-bus-fixture extensions (fixture self-timeout + 6 m3 modes)

This group moves AHEAD of the frontend supervisor work so
c18+ frontend tests can use `signal_ready` /
`exit_immediately` etc. without forward references
(pi-1 Blocker #4).

### c16 — feat(rfl-bus-fixture): `RFL_FIXTURE_MAX_LIFETIME` + 6 m3 fixture modes

- **What.** scope §L1 + §L1a + pi-4 H2 (adds the
  `frontend_bus_publish` mode for c20's wiring
  test): extend m2's
  `rafaello-core/src/bin/rfl_bus_fixture.rs`:
  - All long-running modes read
    `RFL_FIXTURE_MAX_LIFETIME` (default 60 s) and
    `process::exit(0)` after that.
  - **Six new modes** dispatched on
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
      **`Errno::EBADF`** (pi-1 Blocker #5).
    - **`frontend_bus_publish`** (pi-4 H2 — for c20's
      wiring test): adopt RFL_BUS_FD, send ready,
      then `peer.notify("bus.publish", json!({
      "topic": <RFL_FIXTURE_PUBLISH_TOPIC>,
      "payload": {} }))`, then sleep until SIGTERM/
      MAX_LIFETIME. The fixture child cannot
      directly observe the broker's reply (notify
      has no response), so the c20 test relies on
      a parent-side observer plugin to detect the
      `core.lifecycle.publish_rejected` event with
      `code = "publish_outside_grant"`.
- **Why.** scope §L1 + §L1a + pi-4 H2; m2 retro §4.4.
- **Depends on.** c02.
- **Acceptance.** Seven new tests under
  `rafaello-core/tests/`:
  - `fixture_mode_signal_ready.rs`,
    `fixture_mode_exit_immediately.rs`,
    `fixture_mode_hold_silent.rs`,
    `fixture_mode_signal_ready_then_exit_n.rs`,
    `fixture_mode_probe_fd_closed.rs`,
    `fixture_mode_frontend_bus_publish_smoke.rs`
    (six new modes through `Command::new` +
    `RFL_FIXTURE_MODE` env).
  - `fixture_mode_respond_peer_call_self_timeout.rs`
    (pi-2 M3 + pi-3 H3 — existing m2
    `respond_peer_call` mode honoring
    `RFL_FIXTURE_MAX_LIFETIME` per scope §L1).
  Existing m2 fixture tests still pass.

---

## Group 7 — Frontend supervisor + handle + shutdown algorithm

### c17 — feat(rafaello-core::frontend): module + types + FrontendConfig + FrontendSpawnError

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
- **Depends on.** c13, c14 (FrontendHandle holds
  `RegisteredFrontend`).
- **Acceptance.** Build-only test
  `m3_frontend_error_surface_compiles.rs` (m1 c02 +
  m2 c05 pattern).

### c18 — feat(rafaello-core::frontend): Phase A validation + try_reserve check

- **What.** scope §F3 Phase A:
  - `FrontendSupervisor::spawn` Phase A: AttachId
    validation, control-char check, relative-path
    check, exec-bit stat, reserved-env check,
    `try_reserve_frontend_registration`. Returns
    `FrontendSpawnError::InvalidPlan { reason: ... }`
    on every failure mode.
- **Why.** scope §F3 Phase A.
- **Depends on.** c14 (try_reserve), c17.
- **Acceptance.** Seven negative tests under
  `rafaello-core/tests/`:
  - `frontend_spawn_invalid_attach_id_rejected.rs`.
  - `frontend_spawn_relative_entry_path_refused.rs`.
  - `frontend_spawn_control_chars_in_path_refused.rs`.
  - `frontend_spawn_entry_not_executable_refused.rs`.
  - `frontend_spawn_reserved_env_in_pass_refused.rs`.
  - `frontend_spawn_reserved_env_in_set_refused.rs`.
  - `frontend_register_unknown_attach_id_rejected.rs`.

### c19 — feat(rafaello-core::frontend): shutdown_with_outcome seam + dead-watch unit tests

- **What.** scope §F4 + §"shutdown_with_outcome" (pi-2
  H2 — full signature spelled out, including the
  reaper outcome receiver):
  - Extract the shutdown algorithm as a pure async
    helper:
    ```rust
    pub async fn rafaello_core::frontend::shutdown::shutdown_with_outcome(
        cached: Option<Arc<ReaperOutcome>>,
        child_pid: nix::unistd::Pid,
        config: &FrontendConfig,
        reaper_outcome_rx: tokio::sync::watch::Receiver<Option<Arc<ReaperOutcome>>>,
        serve_handle: Option<tokio::task::JoinHandle<()>>,
        register_guard: Option<RegisteredFrontend>,
        signal_fn: impl FnMut(Pid, Signal) -> Result<(), Errno>,
        probe_fn: impl FnMut(Pid) -> Result<(), Errno>,
    ) -> ShutdownReport;
    ```
  - This commit lands the seam BEFORE the
    consolidated Phase B + handle in c20 so
    `FrontendHandle::shutdown` can simply call this
    function. ReaperOutcome is m2's existing type in
    `rafaello_core::error`.
- **Why.** scope §F4 + pi-1 Blocker #2 + pi-2 H2.
- **Depends on.** c14 (RegisteredFrontend — pi-4 M1),
  c17 (FrontendConfig + ShutdownReport).
- **Acceptance.** Two new unit tests in
  `rafaello-core/tests/frontend_shutdown_dead_watch_paths.rs`:
  - `dead_watch_waitfailed_child_already_gone`:
    `cached = Some(WaitFailed(...))`, mock
    `signal_fn` records SIGTERM, mock `probe_fn`
    returns `Err(Errno::ESRCH)` on first call →
    SIGKILL NOT sent, `used_sigkill = false`.
  - `dead_watch_reaper_panicked_child_alive`:
    `cached = Some(ReaperPanicked)`, `signal_fn`
    records SIGTERM and SIGKILL, `probe_fn` returns
    `Ok(())` then `Err(Errno::ESRCH)` →
    `used_sigkill = true`. (This test covers the
    `ReaperPanicked` branch — pi-2 B2: round-2's
    `inject_reaper_panic` hook test is dropped
    because no commit owned the hook; the seam-
    level coverage here is sufficient.)

### c20 — feat(rafaello-core::frontend): Phase B + FrontendHandle + lifecycle (consolidated cutover)

- **What.** scope §F3 Phase B (all 15 steps) + §F4 +
  scope §F1 FrontendHandle methods (pi-2 B1 + m0
  retro §4.1: "workspace-wide cutover commits are
  unavoidable for breaking surface introductions" —
  Phase B cannot be split into separately-green
  commits because `signal_ready` integration tests
  need register/serve/handle to land together).
  One consolidated commit covers:
  - Phase B steps 1-8 (socketpair, env, private state
    dir create_dir_all per pi-17 #3, stderr piped per
    pi-10 Blocker 1, spawn, child.stderr.take()).
  - Phase B steps 9-12 (readiness watch, reaper-
    outcome watch, reaper task, reaper-watcher task
    pushing `ReaperPanicked` on JoinError, server
    build with `FrontendBusPublishService` +
    `FrontendReadyService` composed via
    `FrontendExtraServiceFactory`).
  - Phase B steps 13-15 (register_frontend → guard
    on handle, `tokio::spawn(server.serve())` →
    serve_handle on handle, return populated
    `FrontendHandle` with all 9 fields).
  - `FrontendHandle::wait`, `wait_ready`,
    `take_child_stderr`, `has_signalled_ready`,
    `shutdown(mut self)` calls `shutdown_with_outcome`
    from c19.
  - `FrontendHandle::Drop` per scope §F4
    (Exited-skip / abnormal-best-kill /
    None-best-kill).
- **Why.** scope §F3 + §F4 + m0 retro §4.1; pi-2 B1.
- **Depends on.** c13 (BrokerAcl frontends), c14
  (register_frontend + try_reserve), c15
  (handle_frontend_publish), c16 (fixture modes
  signal_ready / exit_immediately /
  respond_peer_call MAX_LIFETIME), c17 (types),
  c18 (Phase A validation), c19 (shutdown_with_outcome).
- **Acceptance.** Nine tests (pi-4 M2 — count
  updated for the round-3 wiring test addition;
  drops the unowned `inject_reaper_panic` test —
  pi-2 B2):
  - `frontend_spawn_creates_private_state_dir.rs`.
  - `frontend_reaper_publishes_exited_on_clean_exit.rs`
    — spawn fixture in `signal_ready` mode with
    `RFL_FIXTURE_MAX_LIFETIME=1`; await outcome;
    assert `Exited(status)` with
    `status.success()`.
  - `frontend_handle_wait_ready_resolves_on_signal.rs`.
  - `frontend_handle_wait_ready_errors_on_child_exit_before_signal.rs`
    (uses `exit_immediately`).
  - `frontend_handle_wait_resolves_on_child_exit.rs`.
  - `frontend_handle_drop_does_not_leak_zombie.rs`
    (Linux-only; uses `respond_peer_call`).
  - `frontend_handle_shutdown_skips_kill_on_exited.rs`.
  - `frontend_handle_shutdown_kills_on_waitfailed.rs`
    (uses `shutdown_with_outcome` directly with
    mock signal_fn / probe_fn).
  - `frontend_bus_publish_service_routes_to_handle_frontend_publish.rs`
    (pi-3 M1 + pi-4 H2 — proves the
    `FrontendBusPublishService` that c20 composes
    actually routes inbound `bus.publish`
    notifications to
    `Broker::handle_frontend_publish` with the
    spawned frontend's `AttachId`). Mechanism
    (pi-4 H2 — round 3 left this ambiguous):
    - Spawn the TUI as `rfl-bus-fixture` in
      `frontend_bus_publish` mode (c16 pi-4 H2)
      with `RFL_FIXTURE_PUBLISH_TOPIC=
      "frontend.tui.confirm_answer"` (a topic
      OUTSIDE the m3 empty grant; intentionally
      outside).
    - Register an in-process observer plugin
      subscribed to `core.lifecycle.**` via the
      m3 harness `record_subscriber` pattern.
    - Assert the observer receives a
      `core.lifecycle.publish_rejected` event with
      `code = "publish_outside_grant"` and
      `canonical = null` (frontend publisher;
      m2's `core.lifecycle.publish_rejected`
      schema sets canonical to null when the
      publisher is a frontend).

---

## Group 8 — Session store + controller

### c21 — feat(rafaello-core::session): SessionStore::open with Flock<File> + lock-first ordering

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
    c23 with SessionController).
- **Why.** scope §S1 + §S5 + §S3.
- **Depends on.** c02, c16 (uses `probe_fd_closed`
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
    (uses `probe_fd_closed` mode from c16).

### c22 — feat(rafaello-core::session): append_entry + load_entries + StoredEntry

- **What.** scope §S1 + §S2:
  - SQL schema population (tables created at c21;
    rows + queries fill in here).
  - `StoredEntry { seq: u64, entry: Entry }`.
  - `append_entry`, `load_entries` returning
    `Vec<StoredEntry>` ordered by seq.
- **Why.** scope §S1 + §S2.
- **Depends on.** c08 (Entry), c21.
- **Acceptance.** Two tests:
  - `session_store_round_trip.rs`.
  - `session_store_seq_monotonic.rs`.

### c23 — feat(rafaello-core::session): SessionController + finalize_entry + replay_history + SessionError::Publish

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
- **Depends on.** c10 (RenderPipeline), c15 (broker
  publish_core), c22.
- **Acceptance.** Two tests using
  `in_memory_broker_with_tui_and_observer_acl()`
  helper (defined inline in this commit; `m3_harness`
  formal harness lands later in c30):
  - `session_controller_finalize_entry.rs` — assert
    SQLite row + one `entry.finalized` event with
    `replay: false`.
  - `session_controller_replay_history.rs` — three
    pre-seeded entries → three events with `replay:
    true` in seq order.

---

## Group 9 — TUI binary

### c24 — feat(rafaello-tui::bin): rfl-tui core — env parsing, fd adoption, fittings client, frontend.ready RPC

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
- **Depends on.** c03, c15 (frontend publish).
- **Acceptance.** Build-only at c24 (pi-3 H1: the
  `rafaello-tui/tests/tui_handler_calls_frontend_ready.rs`
  integration test deferred to c25 because it needs
  the headless-mode early exit semantics to keep
  the test bounded; c24 itself has no integration
  test — its `What` is exercised end-to-end via c25
  + downstream tests. The c24 commit pre-flight
  cargo build + cargo test suite continues to be
  green from c23).

### c25 — feat(rafaello-tui::bin): headless test mode + stderr sentinels + RFL_TUI_MAX_LIFETIME self-timeout

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
- **Depends on.** c24.
- **Acceptance.** Five tests (pi-3 H1: the
  `tui_handler_calls_frontend_ready.rs` integration
  test moves here from c24):
  - `tui_handler_calls_frontend_ready.rs` —
    spawn rfl-tui in `RFL_TUI_TEST_MODE=1` with
    `RFL_TUI_MAX_LIFETIME=2`; assert
    `frontend.ready` arrives parent-side via test-
    side `FrontendReadyService` mock; child exits
    cleanly via the self-timeout.
  - `tui_test_mode_logs_bus_events_to_stderr.rs`.
  - `tui_test_mode_exits_on_test_done.rs`.
  - `tui_test_mode_self_timeout_exits_zero.rs`.
  - `tui_sends_frontend_ready_after_handler_registration.rs`
    (deterministic-callback ordering, scope §I).

### c26 — feat(rafaello-tui::paint): Painter::draw_with_panic_isolation + paint_node + lib unit test

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
- **Depends on.** c08 (RenderNode — pi-5 M3:
  RenderNode lives in c08 after the round-5
  collapse), c17 (PaintError
  type — pi-1 high #2).
- **Acceptance.** Unit test passes; assert that a
  panicking `PaintAction::RunPanicking` produces
  `[render error: ...]` on the test backend AND the
  next render call proceeds normally.

### c27 — feat(rafaello-tui::bin): production-mode UI loop with crossterm + raw mode + ratatui

- **What.** scope §T2 step 5-7 + §T6: terminal init,
  alternate-screen, raw-mode, redraw on event arrival,
  q-to-quit, arrow-key scroll, restore-on-exit. Uses
  `Painter::draw_with_panic_isolation` from c26
  (pi-1 Blocker #6: this commit now properly depends
  on c26 instead of forward-referencing it).
- **Why.** scope §T2 step 5-7 + §T6.
- **Depends on.** c25, c26.
- **Acceptance.** No automated production-mode tests
  (production mode is exercised by humans only;
  scope §I `rfl_chat_demo_bar` uses headless mode).
  A manual smoke recording lands in the **Phase-4
  driver-owned `manual-validation.md`** (pi-4 M3
  — was incorrectly labelled "c31" in round 4;
  manual-validation.md is NOT a per-commit task,
  it's a milestone-close driver artifact).

---

## Group 10 — `rfl chat` subcommand

### c28 — feat(rafaello::lib): RflChatCli + RflChatError + resolve_tui_path + workspace_bin_path test helper

- **What.** scope §C1 + §C3 + pi-2 B3 (move
  `workspace_bin_path` helper here so c29 / c30 CLI
  tests can use it):
  - clap `Cli` with `Chat { project_root:
    Option<PathBuf> }` subcommand.
  - `RflChatError` with all variants used in §C2.
  - `resolve_tui_path(env, current_exe)` pure
    function per scope §C3.
  - **`rafaello/tests/common/workspace_bin_path.rs`**
    test helper resolving target-dir + binary name
    via `CARGO_TARGET_DIR` env / cargo metadata
    walk-up (scope §H workspace_bin_path).
- **Why.** scope §C1 + §C3 + pi-15 #5 + pi-2 B3.
- **Depends on.** c04.
- **Acceptance.** Three resolve_tui_path unit tests
  (per scope §C3): `_env_override.rs`,
  `_sibling_lookup.rs`, `_unresolved.rs`. Plus
  `workspace_bin_path_resolves_rfl_tui.rs` and
  `workspace_bin_path_resolves_rfl_bus_fixture.rs`
  smoke tests that cargo-build the workspace then
  call the helper and assert the resolved path
  exists and is executable.

### c29 — feat(rafaello::lib): rfl chat orchestration steps 1-7 — open/spawn/wait_ready

- **What.** scope §C2 steps 1-7. Includes the EnvPlan
  `pass` allowlist construction (scope §C2 step 5),
  child stderr forwarder task with serialised writer,
  `wait_ready` three-outcome mapping with parent-side
  `"rfl-chat: frontend-ready-observed"` sentinel.
- **Why.** scope §C2 steps 1-7.
- **Depends on.** c16 (fixture modes used by tests),
  c20 (FrontendHandle — round-3 consolidated),
  c23 (SessionController),
  c25 (rfl-tui headless mode + sentinels including
  `project-root=` and `RFL_TUI_MAX_LIFETIME` —
  pi-2 H1: c24 only had the basic RPC, c25 has the
  test-mode sentinels these tests assert on),
  c28.
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

### c30 — feat(rafaello::lib): rfl chat orchestration steps 8-10 — replay/harness/wait + cleanup guard

- **What.** scope §C2 steps 8-10 + cleanup guard:
  Option/take ownership pattern, single shutdown +
  forwarder.await regardless of result, replay,
  in-test fixture-entry harness (when
  `RFL_HARNESS_FIXTURES=1`), step-10 outcome
  reading from `frontend_handle.wait().await`,
  RflChatError mapping. **The harness uses c08's
  `Entry::new_*` constructors** (pi-2 H3) and
  `serde_json::json!` for `tool_call.args` and the
  unknown-kind payload (pi-4 B2 — `serde_json` is a
  direct dep of the rafaello bin per c04 round-5
  update; `ulid` and `chrono` stay rafaello-core-
  only because the constructors hide them).
- **Why.** scope §C2 steps 8-10 + cleanup guard +
  pi-2 H3.
- **Depends on.** c08 (Entry constructors), c29, c16
  (uses `signal_ready_then_exit_n` for the post-
  ready test).
- **Acceptance.** Two CLI tests:
  - `rfl_chat_replay_withheld_until_frontend_ready.rs`
    (single combined stderr stream, line-order
    assertion).
  - `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
    (uses `signal_ready_then_exit_n`).

---

## Group 11 — Demo-bar headline + manual validation

### c31 — test(rafaello): rfl_chat_demo_bar.rs

- **What.** scope §I `rfl_chat_demo_bar.rs` only
  (pi-2 M1 — `manual-validation.md`, the macOS CI
  URL capture, and the interactive recording are
  driver-owned Phase-4 artifacts, not per-commit
  acceptance gates; this commit covers the
  automated headline test only):
  - `rfl_chat_demo_bar.rs` end-to-end: spawn
    `rfl chat` with `RFL_HARNESS_FIXTURES=1` +
    `RFL_TUI_TEST_MODE=1` against tempdir; assert
    nine SQLite rows + nine `"rfl-tui: bus.event"`
    lines.
- **Why.** scope §I + §"Acceptance summary".
- **Depends on.** c28 (workspace_bin_path), c30.
- **Acceptance.** `rfl_chat_demo_bar.rs` passes on
  Linux. macOS CI green via the Phase-4 driver
  push (NOT a c31 per-commit gate; pi-2 M1).

---

## Phase 4 — driver-owned artifacts (NOT per-commit tasks)

After all 31 commits land, the milestone driver writes:

1. **`manual-validation.md`** capturing cargo-test-all
   output (Linux + macOS CI URLs), the real interactive
   `rfl chat` recording, and the macOS CI run URL
   (pi-2 M1 — moved out of c31 because these are
   external ratification artifacts, not local
   per-commit-green gates).
2. **`retrospective.md`** with pi review (m1: 4 rounds;
   m2: 2 rounds). Drift fixes land before retrospective
   ratifies. Then merge to `rafaello-v0.1`
   linear-history.

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

The status banner above tracks the high-level round-by-
round changes (pi-4 M4 — round-1/round-2/round-3 detailed
changelogs were retracted because the round-3 renumber +
round-5 c08+c09 collapse made the original bullet
references stale). Each pi-review-N.md file is the
authoritative record of what each round changed.
