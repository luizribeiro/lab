# m4-provider-agent-loop — commits

> **Status:** round-3 — pi-2 b/1 h/1 l/1 closed.
> Trajectory: r1 7/4/3/2 → r2 1/1/0/1.
>
> Round-3 fixes (by pi-2 number):
> - **B-1** c10 grows a test-only seed seam
>   `Broker::seed_provider_observed_user_message_for_test(
>   canonical: &CanonicalId, id: JsonRpcId)` symmetric to
>   the c11 result-id seed accessor, cfg-gated by
>   `any(test, feature = "test-fixture")`. The two tests
>   `provider_tool_request_in_reply_to_user_message_id_rejected.rs`
>   and
>   `provider_assistant_message_in_reply_to_user_message_id_accepted.rs`
>   stay in c10 and use the new seam to populate
>   `provider_observed_user_messages` directly — no
>   dependency on c12's `publish_core_with_taint` fan-out
>   side-effect. c12 deletes the seed seam when
>   `publish_core_with_taint`'s user_message-fan-out side
>   effect can populate the set naturally.
> - **H-1** c17 grows a test-only fault-injection seam on
>   `ReemitRouter`: `ReemitRouter::with_test_fault_injector(
>   inject: Arc<dyn Fn(&BusEvent) ->
>   Option<BrokerError> + Send + Sync>) -> Self`,
>   cfg-gated by `any(test, feature = "test-fixture")`.
>   When set, each inbound event is offered to the
>   injector before per-direction dispatch; on
>   `Some(err)` the router returns the error from the
>   re-emit path. c17's acceptance now includes the
>   seam in the router type. c18's
>   `reemit_invalid_taint_emits_reemit_rejected_event.rs`
>   uses the seam to inject
>   `BrokerError::InvalidTaint { ... }` on the next
>   matching event, drives a provider publish through
>   the **real** ReemitRouter path, and asserts the
>   router emits `core.lifecycle.reemit_rejected`.
>   The previous "directly call `publish_core`"
>   alternative is removed.
> - **L-1** c01 description rewritten: the contradicting
>   "members edit only" / "crate directories land in
>   c03/c04" sentence is deleted. Round-3 wording is
>   simply "workspace-member placeholder cutover —
>   `members` edit + minimal `Cargo.toml` + `src/lib.rs`
>   placeholders in the two new crate dirs land in this
>   commit so the workspace resolves cleanly. Full deps +
>   bin targets land in c03/c04."
>
> ---
>
> Round-2 history (kept for trajectory; addresses
> `commits-pi-review-1.md` b/7 h/4 m/3 l/2):
>
> Round-2 fixes (by pi-1 number):
>
> Blockers:
> - **B-1** c20/c22 manifest-compile tests need a real
>   `entry` file inside the package dir; the live
>   `manifest::validate_with_package` calls
>   `resolve_inside_package` which canonicalises the entry
>   path — non-existent files fail
>   (`validate_with_package.rs:18-34`). c20 now creates
>   `rafaello/fixtures/rafaello-mockprovider/bin/rfl-mockprovider`
>   as an executable placeholder shim (POSIX shell file
>   committed with `chmod +x`, body `#!/bin/sh\nexec "$@"`
>   — never run during the compile test, but exists as a
>   file at the entry path). c22 does the same for
>   `rafaello/fixtures/rafaello-readfile/bin/rfl-readfile`.
>   Runtime spawn (c25) points the actual lock at the
>   cargo-built bin under the workspace target dir, not
>   the fixture shim.
> - **B-2** c10's positive
>   `broker_publish_provider_topic_to_internal_subscriber.rs`
>   moved to c11 (which lands the real
>   `subscribe_internal`); c10 keeps only the publishing-
>   path negatives that don't need the internal subscriber
>   (they observe `BrokerError` results directly without
>   any subscriber). c10's `notify_internal_subscribers`
>   placeholder is now an in-commit `Vec<BusEvent>` drain
>   accessible via a `#[cfg(any(test, feature =
>   "test-fixture"))]` accessor on `Broker` — wired solely
>   so the c10 in-commit acceptance can observe inbound
>   provider events without the c11 channel API; c11
>   replaces the drain with the real
>   `mpsc::Sender<BusEvent>` flow.
> - **B-3** §B0 `request_id` enforcement extended to all
>   three publish handlers in one commit (c10): the same
>   topic-suffix check fires inside
>   `handle_plugin_publish`, `handle_frontend_publish`,
>   and `handle_provider_publish`. Scope-named
>   `broker_plugin_tool_result_missing_in_reply_to_rejected.rs`
>   added to c10's acceptance (m2 already enforces
>   `in_reply_to` on plugin tool_results; the test is a
>   regression check under the m4 broker reshape that
>   adds the `Publisher::Provider` arm).
> - **B-4** c15's test renamed
>   `frontend_publish_user_message_accepted_by_broker.rs`
>   (grant-only — observes that the broker emits a
>   `BusEvent` without `PublishOutsideGrant`); the
>   scope-named re-emit test
>   `frontend_publish_user_message_reemitted_as_core_session_user_message.rs`
>   moved to c18 (where the re-emit pipeline exists).
> - **B-5** `AgentLoop::new` signature grows
>   `caps: Capabilities` (c19). c26 wires
>   `Capabilities::tui_default()` into it. AL3/AL4/AL5/AL6
>   call `controller.finalize_entry(entry, &caps)` against
>   the loop's stored caps. The existing `Capabilities`
>   type lives in m3's `rafaello-core::renderer`; no API
>   change to the controller.
> - **B-6** c26 grows a same-commit smoke test
>   `rfl_chat_eager_spawns_provider_and_tool_then_shuts_down_cleanly.rs`
>   that starts the chat with a fixture lock + a
>   non-matching `RFL_TUI_TEST_MESSAGE="hello"`, waits
>   for the echo `assistant_message` to surface (no tool
>   round-trip), then triggers shutdown and asserts every
>   plugin child reaped cleanly. The headline c27 test
>   stays as the demo bar; c26 covers wire-up correctness
>   per the "tests with code" rule.
> - **B-7** c24 adds a small migration step for m3's
>   `rfl chat` tests: every m3 test tempdir setup now
>   writes a **minimal stub `rafaello.lock`** (empty
>   `plugins` table, `session.provider_active = None`),
>   and the corresponding test assertions are updated
>   from "exit 0" to "exit non-zero with
>   `NoActiveProvider`" since m4 makes a provider
>   load-bearing. Tests that previously asserted clean
>   exit (e.g. `rfl_chat_relative_project_root_canonicalises.rs`)
>   migrate to a new shape that checks the
>   canonicalisation behaviour against the failure mode
>   — the canonicalisation runs BEFORE lock load (§C1
>   step 1a), so the test asserts the absolute path is
>   visible on stderr even though the run terminates with
>   `NoActiveProvider`. Listed verbatim in c24
>   acceptance below.
>
> Highs:
> - **H-1** c17 grows a test
>   `reemit_invalid_taint_emits_reemit_rejected_event.rs`
>   (a re-emit failure path emits
>   `core.lifecycle.reemit_rejected`). c18 grows
>   `reemit_unknown_tool_emits_tool_dispatch_rejected_event.rs`
>   (provider publishes a `tool_request` for a tool
>   absent from `acl.tool_routes`; the re-emit path
>   skips the canonical re-emit and emits
>   `core.lifecycle.tool_dispatch_rejected` with
>   `reason: "unknown_tool"`).
> - **H-2** c14's `provider_bus_publish` fixture mode
>   acceptance spells out the publish shape:
>   `request_id: Some(JsonRpcId::String(<fresh ULID>))`
>   and `in_reply_to: Some(vec![])` (empty array per
>   §7.2.6 row 2 first-turn). Without these the m4
>   broker rejects with `MissingRequestId` /
>   `InvalidInReplyTo`.
> - **H-3** c10 sizing waiver expanded explicitly. The
>   commit body cites: ~300 LoC handler + ~150 LoC tests
>   + the maps/bookkeeping. Splitting maps/bookkeeping
>   from handler dispatch is not viable because dispatch
>   consumes the maps inline. The B-2 + B-3 fixes shrink
>   the test count slightly (positive moves to c11; B0
>   enforcement is shared across handlers but the new
>   tests are small `MissingRequestId` shape checks).
>   m0 c08 precedent.
> - **H-4** c25 grows
>   `rfl_chat_tool_spawn_failure_propagates.rs` —
>   fixture lock points the readfile plugin at a
>   `/nonexistent/binary`; `rfl chat` exits non-zero
>   with `ToolSpawnFailed`. The provider spawn must
>   succeed first (so the failure isn't shadowed by
>   `ProviderSpawnFailed`).
>
> Mediums:
> - **M-1** c05 picks **one** test file:
>   `rafaello-core/tests/env_scrubber_rejects_rfl_provider_id.rs`
>   (new file). The "or extend
>   `env_scrubber_reserved_m2_names.rs`" alternative is
>   removed.
> - **M-2** c17 `ReemitRouter::start` lookup pinned: the
>   router resolves `acl.plugins[active_provider]
>   .provider_id` (the public id string from the
>   `PluginAcl.provider_id: Option<String>` field at
>   `broker_acl.rs:81`) and subscribes to
>   `provider.<provider_id>.**`. The constructor takes
>   `active_provider: CanonicalId` because the lock
>   stores the canonical id; the lookup happens inside
>   `start` so the active-provider field stays
>   unambiguous.
> - **M-3** Harness placements made explicit per
>   commit: c18 grows the
>   `rafaello-core/tests/common/reemit_test_kit.rs`
>   shared helper module (`assert_origin_taint`,
>   `subscribe_router_test_receiver`); c21 grows
>   `rafaello-mockprovider/tests/common/mock_provider_handle.rs`
>   wrapping a spawned `rfl-mockprovider`; c23 grows
>   `rafaello-readfile/tests/common/read_file_tool_handle.rs`.
>
> Lows:
> - **L-1** c01 description reworded from
>   "members-list-only" to **"workspace-member
>   placeholder cutover"** to reflect that two new
>   `Cargo.toml` + `src/lib.rs` placeholders land
>   alongside the `members` edit.
> - **L-2** c10 negative-matrix files enumerated
>   verbatim (no "driver may collapse" wording);
>   exact filenames pinned.

Drafted from `scope.md` (round 6 — CONVERGED at 0 blockers
after six pi rounds; commit `c04e894`). Each commit is one
logical idea **and leaves the workspace green** — pre-commit
hooks (rustfmt + clippy + cargo test) gate every commit;
intermediate non-green states are not allowed. Commits land
sequentially on per-commit branches `agents/m4/c<NN>` rebased
onto `rafaello-v0.1`, no merge commits, no force pushes. Tests
land with the code that exercises them per `~/.claude/CLAUDE.md`.

The list has **28 commits** across eight phases:

- **Phase A** — Foundation (c01-c06): workspace deps + crate
  scaffolds for `rafaello-mockprovider` + `rafaello-readfile`,
  reserved env-var extension to `RFL_PROVIDER_ID`, and the m1
  back-reach for the `plugin.<topic-id>.tool_result`
  compiler-inserted auto-publish grant (§M1.1, §M1.3, §W1-W5).
- **Phase B** — Broker surface (c07-c13): the m4 broker
  extensions in dependency order. c07 is the **monolithic
  workspace cutover** for `BusEvent.request_id` +
  `Publisher::Provider` + `PublisherIdentity::Provider`
  (scope §"Internal split" calls this out; m0 c08 precedent).
  c08-c13 land typed errors, provider registration,
  `handle_provider_publish` + observed-id maps, internal
  subscriber primitive, `publish_core_with_taint` +
  `origin_provider` exclusion, defence-in-depth ACL checks.
  Covers §B0/§B1-§B11.
- **Phase C** — m2 row-39 removal + provider supervisor path
  (c14): synthetic-stub-test successor lands here (delete
  `supervisor_spawn_provider_lock_refused.rs`; add
  `provider_plugin_spawns_through_supervisor.rs`). Covers
  §PS1-§PS8 + §M2.
- **Phase D** — Frontend ACL + TUI test-mode hook (c15-c16):
  extends m3's `tui` frontend ACL with
  `frontend.tui.user_message` publish authority + retro §5.9
  granularity test; adds `RFL_TUI_TEST_MESSAGE` env hook in
  `rafaello-tui` with `ulid`-based id generation. Covers
  §F1-§F4, §T1.
- **Phase E** — Re-emit pipeline + agent loop (c17-c19):
  `ReemitRouter` (§CR1-§CR7) and `AgentLoop`
  (§AL1-§AL8 + §TD1-§TD3) wired through
  `subscribe_internal` and `publish_core_with_taint`.
- **Phase F** — Plugin fixtures (c20-c23): `rafaello-mockprovider`
  and `rafaello-readfile` manifests + openrpc.json
  siblings + bin implementations + integration tests
  (incl. pi-3 H-1 multi-turn + pi-3 H-3 lockin denial).
  Covers §PR1-§PR5, §TP1-§TP6.
- **Phase G** — `rfl chat` orchestration (c24-c26): lock
  load + V3 + `compile_plugin` per plugin + supervisor
  construction + eager spawn provider + tool + reemit +
  agent + TUI + wait + shutdown, with the orchestration
  negative matrix. Covers §C1-§C14.
- **Phase H** — Demo bar + manual validation (c27-c28):
  headline `rfl_chat_demo_bar_read_file.rs` (pinned README
  bytes, exact assistant text) + `manual-validation.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes:
  `rafaello-core`, `rafaello-tui`, `rafaello`,
  `rafaello-mockprovider`, `rafaello-readfile`,
  `rafaello` (workspace), `rafaello-m4` (docs).
- "Acceptance" lists new tests + the pre-commit invariants
  the commit must keep green.
- "Depends on" cites the *lowest* commit numbers whose code
  or types this commit references. A commit only lands after
  every declared dependency has landed on `rafaello-v0.1`.
- Test files live per scope §I's placement rules:
  - `rafaello-core/tests/` — broker, reemit, agent loop,
    m2 supervisor changes, m1 broker_acl change.
  - `rafaello-mockprovider/tests/` — anything spawning
    `rfl-mockprovider` (uses
    `env!("CARGO_BIN_EXE_rfl-mockprovider")`).
  - `rafaello-readfile/tests/` — anything spawning
    `rfl-readfile` (uses
    `env!("CARGO_BIN_EXE_rfl-readfile")`).
  - `rafaello/tests/` — `rfl chat` end-to-end tests
    (uses `env!("CARGO_BIN_EXE_rfl")`; resolves
    `rfl-mockprovider` + `rfl-readfile` + `rfl-tui` via
    `workspace_bin_path`).
- Per-commit agents pre-flight `nix develop --impure
  --command cargo test --manifest-path rafaello/Cargo.toml
  --workspace --features rafaello-core/test-fixture` until
  green before invoking pre-commit hooks.
- Per the m1 lesson §4.2, the milestone driver inlines the
  full row text + every acceptance bullet verbatim into each
  per-commit prompt; agents do NOT re-read `commits.md`.
- **Driver-owned actions, NOT per-commit agent actions:**
  pushing branches to origin, capturing CI URLs, writing
  `retrospective.md` (Phase 4, not a per-commit task).

## m4a / m4b checkpoint

No internal split is planned (scope §"Internal split"
explicitly states this). The driver re-evaluates after
**c13** (broker surface complete) and after **c19**
(reemit + agent loop landed); if a split becomes
beneficial, the driver opens an m4a / m4b owner-ratification
request mid-milestone.

## Canonical test names

Wherever `scope.md` and `commits.md` both name a test,
this `commits.md` is canonical. The headline test is
**`rfl_chat_demo_bar_read_file.rs`** (lands at c27).

---

## Phase A — Foundation: workspace deps + crate scaffolds + m1 back-reaches

### c01 — chore(rafaello): add m4 workspace members (mockprovider + readfile)

- **What.** scope §W1: extend `rafaello/Cargo.toml`
  `members` list from
  `["crates/rafaello", "crates/rafaello-core",
  "crates/rafaello-tui"]` to
  `["crates/rafaello", "crates/rafaello-core",
  "crates/rafaello-tui", "crates/rafaello-mockprovider",
  "crates/rafaello-readfile"]`. No
  `[workspace.dependencies]` edits (m4 adds no new
  third-party crates; `ulid` was already added in m3's
  c01 at line 38). This commit is a
  **workspace-member placeholder cutover** (pi-2 L-1
  rewording — the round-2 "members edit only" /
  "directories land in c03/c04" wording contradicted
  the placeholder bullets and is deleted): the
  `members` edit AND two minimal `Cargo.toml` +
  `src/lib.rs` placeholders in the new crate dirs land
  together so the workspace resolves cleanly. Full deps
  + bin targets (and the `bin/` shim files for the
  manifest compile-tests in c20/c22) land in
  c03/c04/c20/c22. Concretely:
  - `rafaello/crates/rafaello-mockprovider/Cargo.toml`
    with `[package] name = "rafaello-mockprovider";
    version = "0.0.0"; edition = "2021"; publish =
    false; [lib] path = "src/lib.rs"` plus an
    empty `src/lib.rs` placeholder file.
  - `rafaello/crates/rafaello-readfile/Cargo.toml` —
    same shape, name `rafaello-readfile`, empty
    `src/lib.rs`.
  Full deps + bin targets land in c03/c04.
- **Why.** scope §W1.
- **Depends on.** baseline.
- **Acceptance.** `cargo metadata --manifest-path
  rafaello/Cargo.toml --format-version 1` succeeds with
  five workspace members. `cargo build --workspace`
  green. `cargo doc --workspace --no-deps` warning-free.
  m3's full test suite still passes.

### c02 — feat(rafaello-tui): add `ulid` dependency for test-mode user_message id generation

- **What.** scope §W5 + pi-2 H-2:
  - Edit `rafaello/crates/rafaello-tui/Cargo.toml`
    `[dependencies]`: add `ulid = { workspace = true }`.
    The workspace alias was added by m3 c01 at
    `rafaello/Cargo.toml:38`; this commit just wires
    rafaello-tui as a consumer.
- **Why.** scope §W5 + §T1 — the `RFL_TUI_TEST_MESSAGE`
  handler (c16) needs `Ulid::new().to_string()` for the
  `frontend.tui.user_message.request_id` field. Add
  the dep up front so c16 compiles cleanly when it lands.
- **Depends on.** c01.
- **Acceptance.** `cargo build -p rafaello-tui` green.
  `cargo doc -p rafaello-tui --no-deps` warning-free.
  `rfl-tui` bin builds.

### c03 — feat(rafaello-mockprovider): scaffold crate + bin target + deps

- **What.** scope §W2 — fill out the scaffold from c01:
  - Edit `rafaello/crates/rafaello-mockprovider/Cargo.toml`
    to add `[[bin]] name = "rfl-mockprovider"; path =
    "src/bin/rfl_mockprovider.rs"`. Add
    `[dependencies]`: `rafaello-core = { path =
    "../rafaello-core" }`, `tokio`, `tracing`,
    `tracing-subscriber`, `fittings-core`,
    `fittings-server`, `fittings-client`,
    `fittings-transport`, `serde`, `serde_json`,
    `async-trait`, `anyhow`, **`ulid`** — all
    `workspace = true` (pi-4 B-2 — `ulid` is
    required so PR2's generated request_ids
    compile).
  - Add `[dev-dependencies]`: `tempfile`,
    `serial_test`, `tracing-test` — workspace.
  - `src/lib.rs`: `//! rafaello-mockprovider
    scaffolding.` placeholder.
  - `src/bin/rfl_mockprovider.rs`: minimal `fn main()
    { eprintln!("rfl-mockprovider: scaffolding
    only."); std::process::exit(0); }`.
- **Why.** scope §W2 + §PR4.
- **Depends on.** c01.
- **Acceptance.** `cargo build -p rafaello-mockprovider`
  green. `cargo build -p rafaello-mockprovider --bin
  rfl-mockprovider` green. `cargo doc -p
  rafaello-mockprovider --no-deps` warning-free.

### c04 — feat(rafaello-readfile): scaffold crate + bin target + deps

- **What.** scope §W3 — fill out the scaffold from c01.
  Same shape as c03 but for `rafaello-readfile`:
  - Edit `rafaello/crates/rafaello-readfile/Cargo.toml`
    to add `[[bin]] name = "rfl-readfile"; path =
    "src/bin/rfl_readfile.rs"`. Add `[dependencies]`
    same set as c03 (including `ulid = { workspace =
    true }` per pi-4 B-2; TP2 generates fresh
    request_ids for every `tool_result` publish).
    Add `[dev-dependencies]` same set as c03.
  - `src/lib.rs`: `//! rafaello-readfile
    scaffolding.` placeholder.
  - `src/bin/rfl_readfile.rs`: minimal `fn main() {
    eprintln!("rfl-readfile: scaffolding only.");
    std::process::exit(0); }`.
- **Why.** scope §W3 + §TP5.
- **Depends on.** c01.
- **Acceptance.** `cargo build -p rafaello-readfile`
  green. `cargo build -p rafaello-readfile --bin
  rfl-readfile` green. `cargo doc -p rafaello-readfile
  --no-deps` warning-free.

### c05 — feat(rafaello-core): extend `RESERVED_ENV_VARS` to reject `RFL_PROVIDER_ID`

- **What.** scope §PS4 + §PS5 + §M1.1 (pi-1 H-1 — only
  `RFL_PROVIDER_ID`, not `RFL_PROVIDER_ACTIVE`). Two
  in-crate edits in the same commit (m1 v3 catches
  collisions pre-compile; m2 supervisor catches at
  spawn — the two lists must move together per scope
  §M1.1):
  - `rafaello/crates/rafaello-core/src/supervisor.rs`:
    extend `RESERVED_ENV_VARS` (lines 49-56) to add
    `"RFL_PROVIDER_ID"` to the list of six existing
    entries.
  - `rafaello/crates/rafaello-core/src/scrubber.rs`:
    add `"RFL_PROVIDER_ID"` to its `RESERVED_ENV_VARS`
    constant (m1 territory; symmetric).
- **Why.** scope §PS4 + §M1.1 (decisions row 40 mirror).
- **Depends on.** c01.
- **Acceptance.** Exactly one new file (pi-1 M-1 — no
  driver-time either/or):
  `rafaello-core/tests/env_scrubber_rejects_rfl_provider_id.rs`
  asserts the m1 scrubber rejects `RFL_PROVIDER_ID` in
  both `env.set` and `env.pass`. Plus the supervisor-side
  negative
  `rafaello-core/tests/supervisor_spawn_reserved_rfl_provider_id_in_set_refused.rs`
  exercising `env.set` collision at spawn. m3's full
  test suite still passes.

### c06 — feat(rafaello-core::broker_acl): compiler-inserted `plugin.<topic-id>.tool_result` auto-publish

- **What.** scope §M1.3 + pi-3 B-6. Extend
  `rafaello-core/src/broker_acl.rs::compile` to
  auto-insert `format!("plugin.{}.tool_result",
  topic_id_str)` into
  `PluginAcl.publish_topics` for any plugin with
  non-empty `bindings.tools` — identical shape to the
  existing `auto_subscribes` insertion at
  `broker_acl.rs:98` (`format!("plugin.{}.tool_request",
  topic_id_str)`). Deduplicate defensively via
  `sort + dedup` to be safe against hand-mutated locks.
  Concretely (in `broker_acl.rs::compile` after line
  98's auto-subscribe insertion):
  ```rust
  let mut publish_topics = entry.grant.publishes.clone();
  if !entry.bindings.tools.is_empty() {
      publish_topics.push(format!("plugin.{}.tool_result",
                                   topic_id_str));
  }
  publish_topics.sort();
  publish_topics.dedup();
  ```
- **Why.** scope §M1.3 — closes the
  "non-existent placeholder substitution" issue
  surfaced by pi-1 B-6. Tool plugins now don't need to
  declare their own tool_result topic in the manifest;
  the compiler inserts it.
- **Depends on.** c01.
- **Acceptance.** Two new tests:
  - `rafaello-core/tests/broker_acl_auto_publishes_tool_result_topic.rs`
    — fixture lock with `bindings.tools = ["read-file"]`
    compiles via `broker_acl::compile`; the resulting
    `PluginAcl.publish_topics` contains
    `format!("plugin.{}.tool_result", topic_id)` where
    `topic_id = topic_id::derive(&canonical.to_string())`.
  - `rafaello-core/tests/broker_acl_auto_publish_absent_for_non_tool_plugin.rs`
    — fixture lock with `bindings.tools = []`; the
    resulting `PluginAcl.publish_topics` does NOT
    contain any `*.tool_result` topic.
  Plus a regression run of the existing m1
  `broker_acl_extraction.rs` test suite. `cargo doc -p
  rafaello-core --no-deps` warning-free.

---

## Phase B — Broker surface (B0/B1-B11)

### c07 — feat(rafaello-core): BusEvent.request_id + Publisher::Provider + PublisherIdentity::Provider — workspace cutover

- **What.** scope §B1 + §B2 + §B3.
  **MONOLITHIC WORKSPACE CUTOVER COMMIT** per scope's
  Internal-split phase 2 (m0 c08 precedent). Breaking
  changes to `BusEvent`, `Publisher`, and
  `PublisherIdentity` cannot be staged across multiple
  commits without breaking the per-commit green-bar:
  - `error.rs:289-293` — extend `Publisher`:
    ```rust
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Publisher {
        Core,
        Plugin(CanonicalId),
        Frontend(AttachId),
        Provider {
            canonical: CanonicalId,
            provider_id: String,
        },
    }
    ```
  - `bus.rs:17-26` — extend `PublishMsg`:
    add `request_id: Option<JsonRpcId>` with
    `#[serde(default)]`.
  - `bus.rs:35-44` — extend `BusEvent`:
    add `pub request_id: Option<JsonRpcId>` field with
    `#[serde(skip_serializing_if = "Option::is_none")]`.
  - `bus.rs:46-52` — extend `PublisherIdentity`:
    ```rust
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum PublisherIdentity {
        Core,
        Plugin { canonical: String, topic_id: String },
        Frontend { attach_id: String },
        Provider { canonical: String,
                   provider_id: String,
                   topic_id: String },
    }
    ```
  - Migrate every in-tree `BusEvent` literal +
    `Publisher` exhaustive match + `PublisherIdentity`
    exhaustive match in `rafaello-core`,
    `rafaello-tui`, `rafaello`, and tests to the new
    shapes. The `request_id` field defaults to `None`
    on the cutover; per-topic-class enforcement lands
    in c10. The new `Provider` arm is the
    `non_exhaustive` extension — most matches catch it
    with a wildcard.
  - Document the size in the commit body: "monolithic
    workspace cutover; ~XX files; m0 c08 precedent. The
    `BusEvent.request_id` field, `Publisher::Provider`
    variant, and `PublisherIdentity::Provider` variant
    are interdependent — splitting across multiple
    commits leaves intermediate non-green states."
- **Why.** scope §B1-§B3; scope §"Internal split"
  step 2 explicitly designates this as the m4
  monolithic cutover (overview §4.5 banner; row 42
  follow-through).
- **Depends on.** c01.
- **Acceptance.** `cargo build --workspace --features
  rafaello-core/test-fixture` green. `cargo test
  --workspace --features rafaello-core/test-fixture`
  green — every pre-existing m2/m3 test passes
  unchanged (cutover is source-compatible at
  pattern-match sites that bind by name or use
  wildcards). New test
  `rafaello-core/tests/broker_publish_provider_carries_request_id.rs`
  exercises `PublishMsg → BusEvent.request_id`
  round-trip via the existing
  `handle_plugin_publish` path (the provider-publish
  handler doesn't exist yet — that lands in c10; the
  round-trip is observable on plugin publishes
  meanwhile). New test
  `rafaello-core/tests/bus_event_serializes_provider_publisher_identity.rs`
  asserts the new wire `kind: "provider"` tag.

### c08 — feat(rafaello-core): BrokerError variants for provider + MissingRequestId + InvalidTaint + StaleRequestId

- **What.** scope §B4 + §B7b §B0 alignment + pi-1 H-5:
  - `error.rs::InReplyToReason` (lines 312-316) gains a
    new variant `StaleRequestId { id: JsonRpcId }`
    (`#[non_exhaustive]`).
  - New enum `error.rs::TaintReason`:
    ```rust
    #[non_exhaustive]
    pub enum TaintReason {
        Missing,
        EmptyArray,
        UnknownSource { source: String },
    }
    ```
  - `error.rs::BrokerError` (lines 324+) gains:
    - `ProviderNotInAcl(CanonicalId)`,
    - `ProviderNotRegistered(CanonicalId)`,
    - `ProviderAlreadyRegistered(CanonicalId)`,
    - `MissingRequestId { publisher: Publisher,
      topic: String }`,
    - `InvalidTaint { publisher: Publisher,
      topic: String, reason: TaintReason }`.
    Each with `thiserror` `#[error("...")]` strings.
  - Re-exports from `lib.rs:38-43` extended with the
    new public types (`TaintReason`).
  - No new `BrokerError::ProviderIdMismatch` variant
    (pi-1 H-5 — round-2 banner is canonical).
- **Why.** scope §B4 — the typed error surface the
  provider registration + handle_provider_publish
  paths consume. Lands before c09/c10 so the
  registration code compiles cleanly.
- **Depends on.** c07.
- **Acceptance.** New test
  `rafaello-core/tests/broker_error_provider_variants_round_trip.rs`
  — instantiate each new variant + its
  `Display`/`Debug` impl is non-panicking. `cargo build
  -p rafaello-core --features test-fixture` green.
  Existing m2/m3 tests still pass.

### c09 — feat(rafaello-core::bus): Broker provider registration surface (register_provider + RAII guard)

- **What.** scope §B5 + pi-1 H-5 (`register_provider`
  reads `provider_id` from ACL — no caller-supplied
  arg):
  - `bus.rs::BrokerState` (lines 62-65) gains a
    third map: `providers: BTreeMap<CanonicalId,
    ProviderConn>` alongside `registry` and
    `frontends`. `ProviderConn { peer: PeerHandle }`
    parallel to `PluginConn` / `FrontendConn`.
  - New methods on `Broker`:
    - `try_reserve_provider_registration(&self,
      canonical: &CanonicalId) -> Result<(),
      BrokerError>` — symmetric to
      `try_reserve_registration` (existing) and
      `try_reserve_frontend_registration` (m3).
      Errors `ProviderNotInAcl` if the canonical's
      `PluginAcl.provider_id == None` or the
      canonical is absent; `ProviderAlreadyRegistered`
      if a slot already exists.
    - `register_provider(&self, canonical: CanonicalId,
      peer: PeerHandle) -> Result<RegisteredProvider,
      BrokerError>` — derives `provider_id` from the
      `PluginAcl.provider_id` field (line 81); errors
      `ProviderNotInAcl` if `None`. Inserts a
      `ProviderConn` and returns a `RegisteredProvider`
      RAII guard.
    - `contains_provider(&self, canonical:
      &CanonicalId) -> bool`.
  - New struct `pub struct RegisteredProvider {
    broker: Arc<BrokerInner>, canonical:
    Option<CanonicalId> }` with a `Drop` impl that
    removes the canonical from
    `state.providers` (mirroring the existing
    `RegisteredPlugin` / `RegisteredFrontend` Drop
    patterns at `bus.rs:645-651` / `bus.rs:668-673`).
  - `Broker::shutdown` (line 210) extended to clear
    `state.providers` too.
  - Re-exports from `lib.rs` extended with
    `RegisteredProvider`.
- **Why.** scope §B5 — provider as a third broker
  principal class, symmetric to the plugin and
  frontend paths landed in m2 / m3.
- **Depends on.** c07, c08.
- **Acceptance.** Three new tests:
  - `rafaello-core/tests/broker_register_provider_happy_path.rs`
    — construct a `BrokerAcl` with a `PluginAcl`
    carrying `provider_id = Some("mock")`; call
    `register_provider`; assert `contains_provider`
    is true; drop the guard; assert
    `contains_provider` is false.
  - `rafaello-core/tests/broker_register_provider_unknown_canonical_rejected.rs`
    — `ProviderNotInAcl` for unknown canonical and
    for known canonical with `provider_id = None`.
  - `rafaello-core/tests/broker_register_provider_duplicate_rejected.rs`
    — `ProviderAlreadyRegistered`.
  m2/m3 test suite green.

### c10 — feat(rafaello-core::bus): handle_provider_publish + observed-id maps + in_reply_to enforcement

- **What.** scope §B6 + §B7b + §B0 cross-handler
  enforcement (pi-1 B-3). The single largest c-surface
  in this milestone. **Sizing waiver (pi-1 H-3 expanded):
  ~300 LoC handler + ~150 LoC test bodies + the maps /
  bookkeeping + §B0 enforcement applied symmetrically
  inside `handle_plugin_publish` and
  `handle_frontend_publish`.** Splitting maps from
  handler dispatch is not viable (dispatch consumes the
  maps inline); splitting the three handler patches is
  not viable either (the same topic-suffix-keyed
  enforcement runs in all three and must arrive
  atomically to keep the §B0 invariant true for every
  inbound class). m0 c08 precedent.
  - **`BrokerInner`** gains two mutexes:
    ```rust
    provider_observed_results:
        Mutex<BTreeMap<CanonicalId, BTreeSet<JsonRpcId>>>,
    provider_observed_user_messages:
        Mutex<BTreeMap<CanonicalId, BTreeSet<JsonRpcId>>>,
    ```
    Inserted on every `core.session.tool_result` /
    `core.session.user_message` fan-out delivered to a
    given provider; **never removed** (pi-2 B-4
    retained-context semantics).
  - **New method `Broker::handle_provider_publish(&self,
    canonical: &CanonicalId, raw_params: &Value) ->
    Result<(), BrokerError>`** mirroring m2's
    `handle_plugin_publish` (bus.rs:216) and m3's
    `handle_frontend_publish` (bus.rs:324). Steps per
    scope §B6:
    1. `ProviderNotRegistered` if not in
       `state.providers`.
    2. Parse `PublishMsg` (including `request_id` field
       from c07).
    3. `validate_topic`.
    4. Namespace dispatch on `segments[0]`:
       - `"core" | "plugin" | "frontend"` →
         `PublishOnReservedNamespace { publisher:
         Publisher::Provider { canonical, provider_id }, … }`.
       - `"provider"` — ≥3 segments AND
         `segments[1] == provider_id` (from
         `PluginAcl.provider_id`) → continue; else
         `PublishOnReservedNamespace`.
       - other → `UnknownNamespace`.
    5. Exact-string match against
       `PluginAcl.publish_topics`; miss →
       `PublishOutsideGrant`.
    6. **§B0 table-of-truth `request_id` enforcement**:
       topic suffix in `{.tool_request, .tool_result,
       .assistant_message, .user_message}` AND
       `msg.request_id.is_none()` →
       `MissingRequestId { publisher, topic }`.
    7. **§B0 `in_reply_to` enforcement** per security
       RFC §7.2.6:
       - `.tool_request` suffix: required, ≥0 entries
         (missing → `InvalidInReplyTo {reason:
         Missing}`). **Each entry MUST be in
         `provider_observed_results[canonical]`** —
         **never** in user_messages (pi-3 B-2). Stale →
         `InvalidInReplyTo {reason: StaleRequestId{id}}`.
       - `.assistant_message` suffix: required, ≥0
         entries. Each entry MUST be in the **union**
         of `provider_observed_results[canonical]` and
         `provider_observed_user_messages[canonical]`.
       - `.tool_result` / `.rpc_reply` from a provider
         don't apply (providers publish requests, not
         results).
    8. **Taint discard** — `msg.taint` is stripped;
       the emitted inbound `BusEvent.taint = None`
       (pi-3 B-1 — discard+replace rule).
    9. Build `BusEvent` with `PublisherIdentity::Provider
       { canonical, provider_id, topic_id }` and hand to
       a new in-crate test-only seam:
       `Broker::drain_inbound_provider_events_for_test()
       -> Vec<BusEvent>` (cfg-gated by `any(test, feature
       = "test-fixture")`). Inbound provider events
       accumulate in an internal `Mutex<Vec<BusEvent>>`
       that the c10 in-commit acceptance reads from
       directly (pi-1 B-2 fix — c10's tests are now
       self-sufficient and don't depend on c11's real
       `subscribe_internal`). c11 deletes the drain-Vec
       seam and replaces it with the `mpsc::Sender`-based
       `notify_internal_subscribers` real path. External
       fan-out is **not** invoked (B7 internal-intake-only).
  - **Two new test-only seed accessors** (pi-2 B-1)
    for the observed-id maps, cfg-gated by `any(test,
    feature = "test-fixture")`:
    ```rust
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn seed_provider_observed_result_for_test(
        &self, canonical: &CanonicalId, id: JsonRpcId);
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn seed_provider_observed_user_message_for_test(
        &self, canonical: &CanonicalId, id: JsonRpcId);
    ```
    Both insert the supplied `JsonRpcId` into the
    matching `Mutex<BTreeMap<…, BTreeSet<JsonRpcId>>>`
    field. The accessors let c10's stale-id /
    user-message-id-rejected tests populate the maps
    without depending on c12's
    `publish_core_with_taint` fan-out side-effect
    (which is the runtime path that populates them in
    production). c12 deletes BOTH accessors when
    `publish_core_with_taint`'s
    `core.session.tool_result` /
    `core.session.user_message` fan-out can populate
    the maps naturally.
  - **Extend `handle_plugin_publish` and
    `handle_frontend_publish`** with the same §B0
    `request_id` topic-suffix check (pi-1 B-3 — the
    enforcement must arrive in every handler or the
    invariant is broken for inbound `plugin.*.tool_result`
    / `frontend.tui.user_message`):
    - In `handle_plugin_publish` (existing
      `bus.rs:228-322` body), after step 3
      (`validate_topic`) and before step 4 (namespace
      dispatch), check: topic suffix in
      `{.tool_request, .tool_result, .assistant_message,
      .user_message}` AND `msg.request_id.is_none()` →
      `MissingRequestId { publisher: Publisher::Plugin(canonical), topic }`.
    - In `handle_frontend_publish` (existing
      `bus.rs:336-402` body), same check with
      `publisher: Publisher::Frontend(attach_id)`.
    - Both call sites use the same helper:
      `fn enforce_b0_request_id(publisher, topic,
      msg_request_id) -> Result<(), BrokerError>`
      lives near `handle_plugin_publish` for reuse.
- **Why.** scope §B6 + §B7b — provider inbound handler
  with topic-class-specific stale-id enforcement
  matching security RFC §7.2.6.
- **Depends on.** c07, c08, c09.
- **Acceptance.** Eighteen new tests — every filename
  enumerated verbatim (pi-1 L-2; no driver-time
  collapsing). The positive end-to-end internal-
  subscriber test lives in c11 (pi-1 B-2); c10 keeps
  only tests that observe through the drain-Vec seam
  or through direct `BrokerError` results.
  - **Negatives (provider-side, observed via
    `BrokerError` return values — no subscriber
    needed):**
    - `rafaello-core/tests/broker_publish_provider_id_segment_mismatch_rejected.rs`
      — `provider.other.foo` →
      `PublishOnReservedNamespace`.
    - `rafaello-core/tests/broker_publish_provider_two_segment_topic_rejected.rs`
      — `provider.mock` →
      `PublishOnReservedNamespace`.
    - `rafaello-core/tests/broker_publish_provider_unknown_namespace_rejected.rs`
      — `evil.foo` → `UnknownNamespace`.
    - `rafaello-core/tests/broker_publish_provider_outside_grant_rejected.rs`
      — `provider.mock.confidential` →
      `PublishOutsideGrant`.
    - `rafaello-core/tests/broker_provider_tool_request_missing_in_reply_to_rejected.rs`
      — missing → `InvalidInReplyTo {Missing}`.
    - `rafaello-core/tests/broker_provider_tool_request_stale_id_rejected.rs`
      — id never observed in either set →
      `StaleRequestId`.
    - `rafaello-core/tests/provider_assistant_message_in_reply_to_missing_rejected.rs`
      — missing field → `InvalidInReplyTo {Missing}`.
    - `rafaello-core/tests/provider_assistant_message_in_reply_to_stale_id_rejected.rs`
      — unknown id → `StaleRequestId`.
    - `rafaello-core/tests/provider_tool_request_in_reply_to_stale_id_rejected.rs`
      — same shape, tool_request topic.
    - `rafaello-core/tests/provider_tool_request_in_reply_to_user_message_id_rejected.rs`
      (pi-3 B-2 + pi-2 B-1 seed seam) — call
      `broker.seed_provider_observed_user_message_for_test(
      &provider_canonical, user_msg_id)` to populate
      `provider_observed_user_messages` without any
      fan-out side-effect (c12's
      `publish_core_with_taint` is not available at
      c10; the seam is the c10-only test path); the
      provider then publishes `tool_request` citing
      `[user_msg_id]` → `StaleRequestId` (per §7.2.6
      row 2 — tool_requests may cite only results).
  - **Positives (small, observed via the drain-Vec
    seam):**
    - `rafaello-core/tests/provider_assistant_message_in_reply_to_user_message_id_accepted.rs`
      (pi-2 B-1 seed seam) — same seed setup as the
      preceding tool_request test
      (`seed_provider_observed_user_message_for_test`)
      but provider publishes
      `provider.mock.assistant_message` citing the
      same id; broker accepts (row 3 union); the
      drain-Vec inbound event carries
      `in_reply_to: Some(vec![user_msg_id])`,
      `taint: None`.
    - `rafaello-core/tests/broker_provider_tool_request_with_supplied_taint_discards.rs`
      — provider publishes with `taint: [{source:
      "user"}]`; drain-Vec inbound event carries
      `taint: None`.
    - `rafaello-core/tests/cross_plugin_tool_request_blocked_at_broker.rs`
      — a non-provider plugin publishes on
      `plugin.<other-topic-id>.tool_request`; m2's
      existing `PublishOnReservedNamespace` rejects;
      m4 adds this test to explicitly assert the
      dispatch-path violation under the new
      `Publisher::Provider`-aware error machinery.
  - **§B0 cross-handler enforcement (pi-1 B-3):**
    - `rafaello-core/tests/broker_provider_tool_request_missing_request_id_rejected.rs`
      — provider publishes
      `provider.mock.tool_request` with
      `request_id: None` → `MissingRequestId`.
    - `rafaello-core/tests/broker_plugin_tool_result_missing_request_id_rejected.rs`
      — plugin publishes
      `plugin.<topic-id>.tool_result` with
      `request_id: None` →
      `MissingRequestId { publisher: Plugin(...) }`.
    - `rafaello-core/tests/broker_frontend_user_message_missing_request_id_rejected.rs`
      — frontend publishes
      `frontend.tui.user_message` with
      `request_id: None` →
      `MissingRequestId { publisher: Frontend(...) }`.
    - **`rafaello-core/tests/broker_plugin_tool_result_missing_in_reply_to_rejected.rs`**
      (scope §I — pi-1 B-3 named the gap): plugin
      publishes `plugin.<topic-id>.tool_result`
      without `in_reply_to` → m2's existing
      `InvalidInReplyTo {Missing}`. This is a
      regression check that m2's enforcement
      continues to fire under the m4 reshape
      (the `Publisher::Plugin` arm flows through
      the new error machinery).
  Workspace test suite green.

### c11 — feat(rafaello-core::bus): subscribe_internal RAII primitive + internal-intake fan-out

- **What.** scope §B7 + §CR1 + pi-2 M-1. Land the
  internal-subscriber primitive — used by the
  `ReemitRouter` in Phase E:
  - `BrokerInner` gains
    `internal_subscribers: Mutex<Vec<InternalSlot>>`
    where `InternalSlot { id: u64, patterns:
    Vec<String>, sender: mpsc::Sender<BusEvent> }`,
    plus a `next_slot_id: AtomicU64`.
  - New public method on `Broker`:
    ```rust
    pub fn subscribe_internal(
        &self,
        patterns: Vec<String>,
        capacity: usize,
    ) -> (mpsc::Receiver<BusEvent>, InternalSubscription);
    ```
    Bounded `tokio::sync::mpsc` channel; default
    capacity in caller path is 256 (caller picks; the
    primitive is generic). Returns the receiver +
    RAII guard.
  - New public struct `pub struct InternalSubscription
    { broker: Arc<BrokerInner>, slot_id: u64 }` with a
    `Drop` impl that removes the matching slot from
    `internal_subscribers`. The Drop is no-op if the
    slot is already gone (broker shutdown cleared it).
  - **Replace c10's drain-Vec seam** (pi-1 B-2) with
    the real `notify_internal_subscribers` dispatch.
    `handle_provider_publish` now calls the channel-
    based notifier (below) instead of pushing to the
    `Mutex<Vec<BusEvent>>` from c10. The drain-Vec
    field and its `drain_inbound_provider_events_for_test`
    accessor are deleted in this commit (the c10 tests
    that consumed them have moved to c11 or remain
    on the `BrokerError`-return path which is
    unchanged). Concretely the helper is:
    ```rust
    fn notify_internal_subscribers(&self, event: &BusEvent) {
        let slots = self.0.internal_subscribers.lock();
        for slot in slots.iter() {
            let matches = slot.patterns.iter()
                .any(|p| pattern_matches_topic(p, &event.topic));
            if !matches { continue; }
            if let Err(e) = slot.sender.try_send(event.clone()) {
                tracing::warn!(slot_id = slot.id,
                    topic = %event.topic, error = ?e,
                    "internal subscriber dropped event");
            }
        }
    }
    ```
  - **Connect `handle_plugin_publish` tool_result branch**:
    m2's `bus.rs:310-318` "result-routing protection"
    skips external fan-out for `tool_result` / `rpc_reply`
    topics; m4 extends that branch to **also** call
    `notify_internal_subscribers(&event)` so the
    ReemitRouter sees inbound tool_results (scope §B7
    second bullet). External-fan-out skip is unchanged
    (additive only).
  - **Ordering rule** (pi-2 M-1): inside the broker's
    fan-out body, the internal-subscriber notify runs
    **before** external recipient loops. Document this
    rule inline.
- **Why.** scope §B7 + §CR1 — trusted core read path.
- **Depends on.** c10.
- **Acceptance.** Five new tests:
  - `rafaello-core/tests/broker_internal_subscriber_unregister_on_drop.rs`
    — subscribe; drop the guard; the subscriber no
    longer receives events.
  - `rafaello-core/tests/broker_internal_subscriber_drops_event_when_full.rs`
    — capacity 1; publish two events; second one
    drops; a `tracing::warn!` line surfaces under
    `tracing-test`.
  - `rafaello-core/tests/broker_internal_subscriber_fires_before_external_fan_out.rs`
    — register both an external plugin subscriber and
    an internal subscriber on the same pattern; publish;
    observe a strict ordering via timestamps captured
    inside the recipient closures.
  - `rafaello-core/tests/broker_provider_event_not_fanned_to_external_subscribers.rs`
    (pi-1 B-5) — register a plugin with subscribe
    pattern `provider.mock.**`; provider publishes
    `provider.mock.assistant_message`; the
    `notify` count on that plugin's peer stays at zero
    (internal subscriber observes; external does not).
  - `rafaello-core/tests/broker_publish_provider_topic_to_internal_subscriber.rs`
    **(moved from c10 — pi-1 B-2)**. Setup (pi-2 L-2 +
    pi-2 B-1 seed seam reused from c10): call
    `broker.seed_provider_observed_result_for_test(
    &provider_canonical, result_id)` (the seam landed
    in c10; still available here) to populate
    `provider_observed_results` directly —
    `publish_core_with_taint` isn't available until
    c12, and the seam is the canonical c10-introduced
    way to populate the map for tests at c10/c11.
    **Then** the provider publishes
    `provider.mock.assistant_message` with `in_reply_to:
    [<that-id>]`; the `subscribe_internal` receiver
    observes a `BusEvent` with `publisher: Provider {...}`,
    `request_id: Some(_)`, `taint: None`; **no external
    plugin / frontend / other-provider peer's `notify`
    count increments**.

### c12 — feat(rafaello-core::bus): publish_core_with_taint + origin_provider exclusion + extended fan-out

- **What.** scope §B8 + §B10 + pi-3 H-2:
  - New method on `Broker`:
    ```rust
    pub fn publish_core_with_taint(
        &self,
        topic: &str,
        payload: serde_json::Value,
        request_id: Option<JsonRpcId>,
        in_reply_to: Option<Vec<JsonRpcId>>,
        taint: Option<Vec<TaintEntry>>,
        origin_provider: Option<CanonicalId>,
    ) -> Result<(), BrokerError>;
    ```
    Validates per scope §B8:
    - topic is `core.*`; else returns
      `PublishOnReservedNamespace` /
      `UnknownNamespace` (per m2's existing rules);
    - **§B0 `request_id` requirement**: topic suffix
      in `{.tool_request, .tool_result,
      .assistant_message, .user_message}` →
      `request_id` MUST be `Some(_)`; else
      `MissingRequestId { publisher: Publisher::Core,
      topic }`;
    - for `core.session.tool_request` /
      `core.session.tool_result`: `taint =
      Some(non_empty_vec)` AND every entry's `source ∈
      {"user", "provider", "tool", "system"}`; else
      `InvalidTaint`.
  - `publish_core` (existing, line 409) becomes a thin
    wrapper: `publish_core(topic, payload) →
    publish_core_with_taint(topic, payload, None, None,
    None, None)`. Publishing
    `publish_core("core.session.tool_request", _)`
    therefore errors with `InvalidTaint{Missing}` — DiD
    against a core path that forgot the taint-aware
    variant.
  - **Extend `fan_out`** (bus.rs:546-625): build a
    `provider_recipients` band parallel to
    `plugin_recipients` / `frontend_recipients` for
    canonical `core.*` events. Provider subscribers
    receive on the patterns their manifest declares
    (e.g. `core.session.user_message`,
    `core.session.tool_result`). On
    `core.session.tool_request` fan-out, when
    `origin_provider == Some(c)`, the recipient set
    **excludes** provider `c` (the pi-3 H-2 mechanical
    exclusion hook).
  - **Side effect on observed-id maps**: fan-out of a
    `core.session.tool_result` to a provider inserts
    the event's `request_id` into
    `provider_observed_results[provider_canonical]`;
    fan-out of `core.session.user_message` inserts into
    `provider_observed_user_messages[provider_canonical]`
    — the per-recipient bookkeeping the B7b enforcement
    consumes. This is where the maps get populated; the
    consumer side is c10's `handle_provider_publish`.
- **Why.** scope §B8 + §B10 — the canonical core-only
  publish path with envelope synthesis and the
  provider-exclusion hook.
- **Depends on.** c10, c11.
- **Acceptance.** New tests:
  - `rafaello-core/tests/broker_publish_core_with_taint_happy_path.rs`
    — `publish_core_with_taint("core.session.tool_request",
    …, taint=[{source: "provider", detail: "mock"}],
    origin_provider=Some(<provider_canonical>))`
    succeeds; subscribing plugin observes the event;
    the originating provider does not.
  - `rafaello-core/tests/broker_publish_core_with_taint_excludes_origin_provider.rs`
    (pi-3 H-2) — two providers registered;
    `origin_provider=Some(provider_a)`; provider A's
    notify count stays at zero; provider B's
    increments.
  - `rafaello-core/tests/broker_publish_core_session_tool_request_missing_taint_rejected.rs`
    — `publish_core("core.session.tool_request", _)`
    → `InvalidTaint{Missing}`.
  - `rafaello-core/tests/broker_publish_core_session_tool_result_missing_taint_rejected.rs`
    — symmetric for `core.session.tool_result`.
  - `rafaello-core/tests/broker_publish_core_session_user_message_missing_request_id_rejected.rs`
    — `publish_core_with_taint("core.session.user_message",
    _, request_id=None, …)` → `MissingRequestId`.
  - `rafaello-core/tests/broker_fan_out_populates_provider_observed_results.rs`
    — fan out a `core.session.tool_result` to a
    registered provider; provider publishes a
    `tool_request` citing the result id; broker
    accepts (round-trips through B7b).

### c13 — feat(rafaello-core::broker_acl): defence-in-depth provider publish-id check

- **What.** scope §B11. m2's `BrokerAcl` construction
  (bus.rs:85-122) already revalidates pattern + topic
  grammar; m4 extends it with a structural check: for
  any `PluginAcl` with `provider_id = Some(id)`, every
  entry in `publish_topics` must be either
  `provider.<id>.<segment>+` or `plugin.<topic-id>.*`
  (the latter is the m1 compiler-inserted self-publish
  set from c06). A publish_topic that starts with
  `provider.` whose second segment is NOT the
  registered `provider_id` is rejected at construction
  time with `BrokerError::InvalidTopic` (a hand-mutated
  ACL bypassing the m1 compile path can't smuggle a
  cross-provider publish grant). m1's
  `check_lock_publish_topic` already does this on the
  lock side — this is the broker-side defence in depth.
- **Why.** scope §B11 — pi-1 round-1 defence-in-depth
  belt-and-braces.
- **Depends on.** c07 (Publisher::Provider), c09
  (`provider_id` in ACL is now load-bearing for
  registration).
- **Acceptance.** New test
  `rafaello-core/tests/broker_construct_with_provider_publish_id_mismatch_rejected.rs`
  — construct a `BrokerAcl` whose `PluginAcl` has
  `provider_id = Some("mock")` and `publish_topics`
  containing `"provider.other.foo"`; `Broker::new`
  returns `InvalidTopic` with the mismatched topic
  cited.

---

## Phase C — m2 supervisor row-39 removal + provider supervisor path

### c14 — refactor(rafaello-core::supervisor): remove row-39 refusal + wire provider broker registration

- **What.** scope §PS1-§PS8 + §M2.1-§M2.3.
  Synthetic-stub-successor pattern (per
  `plans/README.md`):
  - **Delete** `rafaello/crates/rafaello-core/tests/supervisor_spawn_provider_lock_refused.rs`
    (the m2 negative test for the row-39 refusal —
    confirmed live filename via `ls
    rafaello/crates/rafaello-core/tests/` at the
    branch tip).
  - **Delete** `InvalidPlanReason::ProviderNotInM2`
    from `error.rs:401-403`.
  - **Delete** the row-39 refusal block at
    `supervisor.rs:414-419` (the
    `if let Some(provider_id) = acl_provider_id`
    block that returns `SpawnError::InvalidPlan`).
  - **Wire provider-bound registration**: at the
    broker-registration step in
    `PluginSupervisor::spawn`, branch on
    `acl_provider_id`:
    ```rust
    let registered = match acl_provider_id {
        Some(_) => ProviderOrPlugin::Provider(
            self.broker.register_provider(
                plan.canonical.clone(), peer.clone())?,
        ),
        None => ProviderOrPlugin::Plugin(
            self.broker.register_plugin(
                plan.canonical.clone(), peer.clone())?,
        ),
    };
    ```
    Define `ProviderOrPlugin` enum (or
    `enum SpawnRegistration { Plugin(RegisteredPlugin),
    Provider(RegisteredProvider) }`) and update
    `ManagedSpawn.registered` (supervisor.rs:155-168)
    to hold it. Drop unconditionally releases the
    right registry slot via the appropriate RAII
    guard. `SpawnError::BrokerRegister` shape unchanged.
  - **Inject `RFL_PROVIDER_ID` env**: when
    `acl_provider_id = Some(id)`, the child's env
    receives `RFL_PROVIDER_ID = <id>`. The
    `RESERVED_ENV_VARS` already contains the name (c05)
    so plan authors can't shadow it.
  - **Extend `SupervisorConnectionService`** (or
    `BusPublishService` at `supervisor.rs:1005`) to
    dispatch `bus.publish` calls from a
    provider-bound peer to `handle_provider_publish`
    instead of `handle_plugin_publish`: check
    `broker.contains_provider(canonical)` first.
  - **Add** the named successor test (synth-stub
    successor):
    `rafaello-core/tests/provider_plugin_spawns_through_supervisor.rs`
    — build a fixture `CompiledPlugin` with
    `bindings.provider = true, provider_id = "mock"`;
    spawn via `PluginSupervisor::spawn`; assert spawn
    succeeds; `broker.contains_provider(canonical) ==
    true` while the spawn is live; `SpawnHandle::try_wait()`
    initially `None`; `SpawnHandle::wait()` resolves with
    `ReaperOutcome::Exited(_)` after shutdown.
- **Why.** scope §M2 + decisions row 39 — m2 retro §2.1
  records this as the m4-owned removal. Synthetic-stub
  successor per `plans/README.md` Patterns.
- **Depends on.** c05 (reserved env), c08
  (BrokerError::Provider* variants), c09
  (register_provider), c10
  (handle_provider_publish dispatch).
- **Acceptance.** The m2 test
  `supervisor_spawn_provider_lock_refused.rs` is
  deleted (commit body cites the synth-stub-successor
  rule). New positive test
  `provider_plugin_spawns_through_supervisor.rs`
  passes. The `rfl-bus-fixture` bin gains a new mode
  `provider_bus_publish` that publishes a synthetic
  `provider.<RFL_PROVIDER_ID>.tool_request` to exercise
  the new dispatch path in the successor test (pi-1
  H-2 — publish shape pinned: payload `{tool:
  "noop", args: {}}`, `request_id:
  Some(JsonRpcId::String(<fresh ULID>))`,
  `in_reply_to: Some(vec![])` — empty array is valid
  per §7.2.6 row 2 first-turn. Without these the m4
  broker rejects with `MissingRequestId` or
  `InvalidInReplyTo`).
  Workspace test suite green.

---

## Phase D — Frontend ACL extension + TUI test-mode hook

### c15 — feat(rafaello): extend `tui` frontend ACL with `frontend.tui.user_message` publish authority

- **What.** scope §F1-§F4 + m3 retro §2.10 + §5.9:
  - Edit `rafaello/crates/rafaello/src/lib.rs` at the
    `run_chat` ACL construction (lines 142-153):
    extend the `tui` `FrontendAcl.publish_topics` from
    `BTreeSet::new()` to include
    `"frontend.tui.user_message"`.
  - The retro §5.9 file granularity gap closer: NEW
    test `rafaello-core/tests/frontend_register_with_broker.rs`
    — stand-alone positive test for the frontend
    registration happy path (m3 retro recorded the
    file gap; m4 lands the file).
- **Why.** scope §F1-§F4 + m3 retro §2.10 + §5.9
  carryover.
- **Depends on.** c07 (BusEvent.request_id field exists
  on the wire).
- **Acceptance.** Two new tests (pi-1 B-4 — grant-only
  in this commit; the re-emit-named test moves to c18):
  - `rafaello-core/tests/frontend_register_with_broker.rs`
    — m3 retro §5.9 granularity gap closer (stand-alone
    positive test for frontend registration).
  - `rafaello-core/tests/frontend_publish_user_message_accepted_by_broker.rs`
    (pi-1 B-4 rename from
    `frontend_publish_user_message_reemitted_as_core_session_user_message.rs`):
    drive a frontend `bus.publish` for
    `frontend.tui.user_message` with `request_id:
    Some(<ULID JsonRpcId>)` (required per §B0);
    assert `handle_frontend_publish` returns `Ok(())`
    — i.e. no `PublishOutsideGrant` and no
    `MissingRequestId`. The test only observes the
    `BrokerError` return value (or absence thereof);
    the canonical re-emit to
    `core.session.user_message` is c18 territory.
  - Existing m3 negative
    `frontend_publish_outside_grant_rejected.rs` MUST
    still pass — `confirm_answer` remains outside
    grant.

### c16 — feat(rafaello-tui): RFL_TUI_TEST_MESSAGE env hook with ulid-based request_id

- **What.** scope §T1 + pi-2 M-2 + pi-4 B-2:
  - `rafaello/crates/rafaello-tui/src/bin/rfl_tui.rs`
    (or a new `env.rs` if the bin grows large)
    reads `RFL_TUI_TEST_MESSAGE` at startup. If set
    and non-empty:
    - After the existing `peer.call("frontend.ready",
      json!({}))` resolves AND the BusEventHandler
      is registered, publish a single
      `bus.publish` notification with topic
      `frontend.tui.user_message`, payload
      `{text: <env-value>}`, and `request_id:
      Some(JsonRpcId::String(Ulid::new().to_string()))`.
    - If unset or empty, the TUI runs the normal
      interactive prompt (m3 path, unchanged).
  - Extend `rafaello-tui`'s
    `ENV_PASS_ALLOWLIST` constant to include
    `"RFL_TUI_TEST_MESSAGE"`.
  - Extend `rafaello/src/lib.rs`'s `ENV_PASS_ALLOWLIST`
    (the `CompiledFrontend.env.pass` list at line ~100)
    to include `"RFL_TUI_TEST_MESSAGE"` so `rfl chat`
    propagates it to the spawned child.
- **Why.** scope §T1 — the test-mode driver for the
  headline demo bar test (c27).
- **Depends on.** c02 (ulid dep), c07 (BusEvent +
  PublishMsg `request_id` field), c15 (frontend ACL
  grants the topic).
- **Acceptance.** New test
  `rafaello-tui/tests/tui_sends_test_message_after_ready.rs`
  — spawn `rfl-tui` in `RFL_TUI_TEST_MODE=1`
  + `RFL_TUI_TEST_MESSAGE="what's in README.md"`;
  in the parent-side broker fixture, register a
  callback on the `FrontendReadyService`; await
  ready; await the `frontend.tui.user_message`
  publish; assert payload `text` matches and
  `request_id` is a valid `JsonRpcId::String`. m3's
  TUI test suite still passes.

---

## Phase E — Re-emit pipeline + agent loop

### c17 — feat(rafaello-core::reemit): ReemitRouter module + active-provider scoping

- **What.** scope §CR1 + §CR6 + §CR7. Land the routing
  task structure but NOT the per-direction re-emit
  logic (that's c18):
  - New module `rafaello/crates/rafaello-core/src/reemit/mod.rs`
    + `pub mod reemit;` in `lib.rs:14` (after
    `pub mod renderer;`).
  - New public struct (with the pi-2 H-1
    fault-injection seam):
    ```rust
    pub struct ReemitRouter {
        broker: Broker,
        acl: BrokerAcl,
        active_provider: CanonicalId,
        shutdown_rx: watch::Receiver<bool>,
        #[cfg(any(test, feature = "test-fixture"))]
        fault_injector: Option<TestFaultInjector>,
    }

    #[cfg(any(test, feature = "test-fixture"))]
    pub type TestFaultInjector = std::sync::Arc<
        dyn Fn(&BusEvent) -> Option<BrokerError>
            + Send + Sync,
    >;

    impl ReemitRouter {
        pub fn new(broker: Broker, acl: BrokerAcl,
                   active_provider: CanonicalId,
                   shutdown_rx: watch::Receiver<bool>)
                   -> Self;
        pub fn start(self) -> JoinHandle<()>;

        #[cfg(any(test, feature = "test-fixture"))]
        pub fn with_test_fault_injector(
            mut self, inject: TestFaultInjector
        ) -> Self {
            self.fault_injector = Some(inject);
            self
        }
    }
    ```
    Per-direction dispatch (c17 placeholders; c18
    lights them up) calls
    `self.fault_injector.as_ref().and_then(|f| f(event))`
    BEFORE the real re-emit; on `Some(err)` the
    handler skips the canonical publish and runs the
    same §CR7 failure path (log at
    `tracing::error!`, emit
    `core.lifecycle.reemit_rejected` with the
    structured reason). This makes the failure path
    drivable from tests through the **real** router
    body, not a side-channel that bypasses it (pi-2
    H-1).
    `start` resolves the public `provider_id` segment
    via `acl.plugins[&active_provider].provider_id`
    (the `Option<String>` field at `broker_acl.rs:81`;
    panics with a clear message if `None`, which
    indicates `rfl chat` constructed an `AgentLoop`
    against an ACL whose active-provider plugin has no
    `provider = true` binding — that should already
    error at `validate::lock`-time but defence in depth
    is cheap; pi-1 M-2 made this lookup explicit because
    the round-1 `"provider.<active-provider-id>.**"`
    wording conflated the canonical id with the public
    `provider_id`). Then subscribes via
    `broker.subscribe_internal` on three patterns:
    `["frontend.tui.user_message",
      format!("provider.{}.**", provider_id),
      "plugin.*.tool_result"]`
    (the third pattern uses `*` for the topic-id
    segment so every installed tool plugin's
    tool_result reaches the router). Spawns a tokio
    task that selects on the receiver + the
    shutdown_rx and dispatches to per-direction
    handlers (c18 fills them in).
  - The per-direction handlers in this commit are
    placeholders that log + drop the event (so the
    crate builds green at this commit). c18 lights
    them up.
  - **Failure semantics** (§CR7): a re-emit that hits
    `BrokerError::InvalidTaint` etc. logs at
    `tracing::error!` and emits a
    `core.lifecycle.reemit_rejected` event via
    `publish_core` for observability. No process kill.
- **Why.** scope §CR1 — the module structure /
  task wiring lives in its own commit so the c18
  per-direction logic can land in a focused diff.
- **Depends on.** c11 (subscribe_internal), c12
  (publish_core_with_taint), c14 (Provider broker
  surface complete).
- **Acceptance.** New test
  `rafaello-core/tests/reemit_router_subscribes_and_shuts_down.rs`
  — construct a router; call `start`; trigger the
  shutdown watch; assert the join handle resolves
  within 2 s. `cargo build -p rafaello-core
  --features test-fixture` green.

### c18 — feat(rafaello-core::reemit): per-direction re-emit logic (CR2-CR5)

- **What.** scope §CR2 + §CR3 + §CR4 + §CR5. Light up
  the four re-emit directions in the placeholder
  handlers from c17:
  - **CR2** — `provider.<id>.tool_request` →
    `core.session.tool_request`: validate payload as
    `{tool: String, args: Value}`; look up
    `acl.tool_routes.get(&tool)`; on miss, emit
    `core.lifecycle.tool_dispatch_rejected` with
    `reason: "unknown_tool"`; on hit, synthesise
    `taint = [{source: "provider", detail:
    Some(provider_id)}]`; call
    `publish_core_with_taint(
       "core.session.tool_request",
       json!({tool, args,
              dispatch_target: <canonical>}),
       Some(inbound.request_id),
       Some(inbound.in_reply_to),
       Some(taint),
       Some(source_provider_canonical))`
    (pi-3 H-2 — origin_provider passes the source
    provider for fan-out exclusion).
  - **CR3** — `plugin.<topic-id>.tool_result` →
    `core.session.tool_result`: payload forwarded
    byte-for-byte (the canonical wire shape `{ok:
    bool, content: String}` for the readfile tool;
    pi-3 B-4 — no serialisation into
    `ToolResultPayload`); taint = `[{source:
    "tool", detail: Some(canonical.to_string())}]`;
    `publish_core_with_taint(
       "core.session.tool_result",
       payload, Some(rid),
       Some([tool_request_id]),
       Some(taint), None /* no origin exclusion */)`.
    The inbound `request_id` is enforced present by
    the broker (§B6 step 7); CR3 forwards it.
  - **CR4** — `provider.<id>.assistant_message` →
    `core.session.assistant_message`: payload
    pass-through; taint = `[{source: "provider",
    detail: Some(provider_id)}]`; `request_id` /
    `in_reply_to` forwarded;
    `origin_provider = None` (the assistant_message
    is informational, not a request the source
    provider needs to be shielded from re-receiving).
  - **CR5** — `frontend.tui.user_message` →
    `core.session.user_message`: discard inbound
    `taint`; synthesise `taint = [{source: "user",
    detail: None}]`; forward `request_id` (broker
    enforces presence per §B0); `in_reply_to = None`
    (user messages are roots); `origin_provider =
    None`.
- **Why.** scope §CR2-§CR5 — the canonical re-emit
  pipeline.
- **Depends on.** c17.
- **Acceptance.** Eleven new tests + one new common
  module (pi-1 M-3 harness placement explicit):
  - **New common module**
    `rafaello-core/tests/common/reemit_test_kit.rs`
    exposing `assert_origin_taint(event, source,
    detail)` and
    `subscribe_router_test_receiver(broker) ->
    (Receiver<BusEvent>, InternalSubscription)` (a
    tight wrapper around `broker.subscribe_internal`
    pre-configured with the four router patterns).
    Every test below imports from it.
  - `rafaello-core/tests/reemit_provider_tool_request_to_core_session_tool_request.rs`
    — observe canonical taint `[{source: "provider",
    detail: "mock"}]`, `dispatch_target` field.
  - `rafaello-core/tests/reemit_plugin_tool_result_to_core_session_tool_result.rs`
    — observe canonical taint `[{source: "tool",
    detail: <canonical>}]`, `in_reply_to` forwarded,
    payload byte-equal to inbound.
  - `rafaello-core/tests/reemit_frontend_user_message_to_core_session_user_message.rs`
    — observe canonical taint `[{source: "user"}]`,
    `in_reply_to: None`, request_id forwarded.
  - `rafaello-core/tests/reemit_user_message_synthesises_user_taint.rs`
    (pi-1 L-3) — same as above but explicitly
    asserts the synthesis happens regardless of
    inbound taint.
  - `rafaello-core/tests/reemit_user_message_discards_frontend_supplied_taint.rs`
    (pi-1 L-3) — TUI publishes with `taint:
    [{source: "provider"}]`; canonical event carries
    `[{source: "user"}]` only.
  - `rafaello-core/tests/reemit_discards_plugin_supplied_taint_on_core_session_tool_request.rs`
    (round-1 / round-2 negative) — provider publishes
    `tool_request` with `taint: [{source: "user"}]`;
    canonical `core.session.tool_request` carries
    `[{source: "provider", detail: "mock"}]` only.
  - `rafaello-core/tests/reemit_plugin_tool_result_missing_in_reply_to_rejected.rs`
    — broker already rejects with `InvalidInReplyTo`
    on inbound (m2-enforced); this test confirms the
    canonical re-emit does NOT fire for such an
    event (no `core.session.tool_result` is emitted).
  - `rafaello-core/tests/frontend_user_message_missing_request_id_rejected.rs`
    (round-4 — replaces the round-3 synthesise test)
    — broker rejects the inbound frontend publish
    with `MissingRequestId`; canonical re-emit does
    not fire.
  - `rafaello-core/tests/frontend_publish_user_message_reemitted_as_core_session_user_message.rs`
    **(moved from c15 — pi-1 B-4)**. Drive a frontend
    `bus.publish` for `frontend.tui.user_message` with
    a valid `request_id`; the router's
    `subscribe_internal` receiver observes the inbound
    event AND the broker subsequently fans out the
    canonical `core.session.user_message` to external
    subscribers (registered via a separate
    `register_plugin` with `subscribe_patterns =
    ["core.session.**"]`); assert the canonical
    event's `payload.text` matches the inbound's
    `text`, the canonical `request_id` is forwarded,
    and `taint = [{source: "user"}]`.
  - `rafaello-core/tests/reemit_invalid_taint_emits_reemit_rejected_event.rs`
    (pi-1 H-1 + pi-2 H-1 — uses the c17
    `with_test_fault_injector` seam to drive the
    failure through the **real** router body): build
    a `ReemitRouter::new(...)` then
    `.with_test_fault_injector(Arc::new(|event| {
    if event.topic == "provider.mock.tool_request" {
        Some(BrokerError::InvalidTaint {
            publisher: Publisher::Core,
            topic: "core.session.tool_request".into(),
            reason: TaintReason::Missing,
        })
    } else { None }
    }))`; spawn the router; the provider publishes
    `provider.mock.tool_request` through
    `handle_provider_publish`; the router observes
    the inbound event via its internal subscriber,
    consults the injector, receives `Some(err)`,
    runs the §CR7 failure path, and emits
    `core.lifecycle.reemit_rejected` whose payload
    carries the structured reason. Assert: no
    canonical `core.session.tool_request` is fanned
    out (a subscriber registered on
    `core.session.**` sees the
    `core.lifecycle.reemit_rejected` event but not
    the tool_request). The previous "directly call
    `publish_core`" alternative is removed (pi-2
    H-1).
  - `rafaello-core/tests/reemit_unknown_tool_emits_tool_dispatch_rejected_event.rs`
    (pi-1 H-1) — provider publishes
    `provider.mock.tool_request` for a tool name
    absent from `acl.tool_routes`; the router
    observes the inbound event, fails the
    `tool_routes` lookup, emits
    `core.lifecycle.tool_dispatch_rejected` with
    `reason: "unknown_tool"`, and does NOT publish a
    canonical `core.session.tool_request`.

### c19 — feat(rafaello-core::agent): AgentLoop + tool dispatch + entry persistence

- **What.** scope §AL1-§AL8 + §TD1-§TD3:
  - New module `rafaello/crates/rafaello-core/src/agent/mod.rs`
    + `pub mod agent;` in `lib.rs`.
  - New public struct (pi-1 B-5 — `caps` is required
    because `SessionController::finalize_entry` takes
    `&Capabilities` per `session/mod.rs:275`; m3's
    `run_chat` constructs `Capabilities::tui_default()`
    at `lib.rs:166` and passes it to the controller —
    m4's AgentLoop holds it for the same reason):
    ```rust
    pub struct AgentLoop {
        broker: Broker,
        acl: BrokerAcl,
        controller: Arc<SessionController>,
        caps: Capabilities,
        shutdown_rx: watch::Receiver<bool>,
    }
    impl AgentLoop {
        pub fn new(broker: Broker, acl: BrokerAcl,
                   controller: Arc<SessionController>,
                   caps: Capabilities,
                   shutdown_rx: watch::Receiver<bool>)
                   -> Self;
        pub fn start(self) -> JoinHandle<()>;
    }
    ```
    `start` subscribes via `broker.subscribe_internal`
    on patterns:
    `["core.session.user_message",
      "core.session.assistant_message",
      "core.session.tool_request",
      "core.session.tool_result"]`. Spawns a tokio
    task that dispatches per event:
    - **AL3** `core.session.user_message`: persist
      via `controller.finalize_entry(Entry { kind:
      "text", metadata.author = User, payload =
      TextPayload { text, markdown: false }, … },
      &caps)`.
    - **AL4** `core.session.assistant_message`:
      persist `Entry { kind: "text", author:
      Assistant, payload = TextPayload {...} }`.
    - **AL5** `core.session.tool_request`: persist
      `Entry { kind: "tool_call", author:
      Assistant, payload = ToolCallPayload { id:
      <request_id-as-string>, name, args, status:
      Pending } }` AND publish the per-plugin
      dispatch via a new
      `Broker::publish_for_tool_dispatch(canonical:
      &CanonicalId, payload, request_id,
      in_reply_to, taint)` method that mirrors
      `publish_core_with_taint` but emits on
      `plugin.<topic-id>.tool_request` with
      publisher `PublisherIdentity::Core`. The
      method looks up the topic-id via
      `BrokerAcl.plugins[canonical].topic_id` and
      validates the canonical is in `acl.plugins`;
      it does NOT validate `taint` (the plugin path
      isn't gated like `core.session.tool_*`).
    - **AL6** `core.session.tool_result`: persist
      `Entry { kind: "tool_result", author: Tool,
      payload = ToolResultPayload { call_id:
      <in_reply_to[0]-as-string>, ok, content:
      RenderNode::Code { code: <content>, lang:
      None } /* wire→persistence wrap per pi-3 B-4 */,
      details: None } }`. No in-place update of the
      prior `tool_call` entry (round-2 cut — append-only).
  - **TD1** new method `Broker::tool_route(name:
    &str) -> Option<CanonicalId>` — thin accessor
    over `self.0.acl.tool_routes.get(name)`.
- **Why.** scope §AL + §TD — the dispatch half of the
  canonical 5-step path (overview §7).
- **Depends on.** c12 (publish_core_with_taint), c17
  (reemit module — agent runs alongside),
  c18 (canonical re-emit produces the events agent
  consumes).
- **Acceptance.** Seven new tests:
  - `rafaello-core/tests/agent_loop_dispatches_tool_request_to_target_plugin.rs`
    — drive a `core.session.tool_request` with
    `dispatch_target` set; observe the resulting
    `plugin.<topic-id>.tool_request` publish via an
    in-process subscriber registered on that plugin's
    inbox.
  - `rafaello-core/tests/agent_loop_persists_user_message_entry.rs`
    — observe a SQLite row with `kind = "text"`,
    `metadata.author = User`, `payload.text`
    matching.
  - `rafaello-core/tests/agent_loop_persists_assistant_message_entry.rs`
    — analogous, `author = Assistant`.
  - `rafaello-core/tests/agent_loop_persists_tool_call_entry.rs`
    — `kind = "tool_call"`, `payload =
    ToolCallPayload { id, name, args, status:
    Pending }`.
  - `rafaello-core/tests/agent_loop_persists_tool_result_entry.rs`
    — `kind = "tool_result"`, `payload =
    ToolResultPayload { call_id, ok, content:
    RenderNode::Code { … }, details: None }`.
  - `rafaello-core/tests/broker_tool_route_lookup.rs`
    — `Broker::tool_route("read-file")` returns the
    expected canonical from a fixture `BrokerAcl`.
  - `rafaello-core/tests/cross_provider_request_to_tool_only_routes_via_core.rs`
    (pi-2 M2-2) — construct a lock where the
    readfile tool plugin's `bus.subscribes`
    explicitly includes `"core.session.tool_request"`;
    drive a tool request; assert the tool plugin
    observes the canonical event (via its subscribe
    pattern) but does NOT execute a second tool
    invocation — only the agent-loop-published
    `plugin.<topic-id>.tool_request` triggers
    execution. Assert exactly one `tool_result` per
    dispatch.

---

## Phase F — Plugin fixtures: `rafaello-mockprovider` + `rafaello-readfile`

### c20 — feat(rafaello-mockprovider): manifest fixture + openrpc.json + compile-test

- **What.** scope §PR1 + §PR3:
  - Create `rafaello/fixtures/rafaello-mockprovider/rafaello.toml`
    in the **live m1 schema** (pi-1 B-2): top-level
    `schema = 1; name = "mockprovider"; version =
    "0.0.0"; entry = "bin/rfl-mockprovider"; rafaello
    = ">=0.1, <0.2"`; `[provides] provider = "mock"`;
    `[bus] subscribes =
    ["core.session.user_message",
    "core.session.tool_result"]; publishes =
    ["provider.mock.tool_request",
    "provider.mock.assistant_message"]`;
    `[capabilities.default.filesystem] read_dirs = [];
    write_dirs = []`;
    `[capabilities.default.network] mode = "deny"`;
    `[load] eager = true`.
  - Create `rafaello/fixtures/rafaello-mockprovider/openrpc.json`
    — minimum valid shape per m1 §M10: `{"openrpc":
    "1.2.6", "info": {"title": "mockprovider",
    "version": "0.0.0"}, "methods": []}`.
  - **Create `rafaello/fixtures/rafaello-mockprovider/bin/rfl-mockprovider`**
    (pi-1 B-1 — the live
    `manifest::validate_with_package` calls
    `resolve_inside_package` which canonicalises the
    `entry` path inside the package; a non-existent
    entry file fails `validate_with_package.rs:18-34`).
    Contents: a placeholder POSIX shell script
    `#!/bin/sh\nexec "$@"\n` committed with `chmod +x`.
    The shim never executes during the compile test —
    its presence as a regular file at `entry` is what
    the validator requires. The actual runtime binary
    (used by `rfl chat` at c25 spawn time) lives at
    `<workspace_target>/debug/rfl-mockprovider`,
    resolved separately via `CARGO_BIN_EXE_*`; the
    fixture lock's `entry_absolute` points at the
    cargo-built binary, not at this shim.
- **Why.** scope §PR1 + §PR3.
- **Depends on.** c03 (mockprovider crate exists).
- **Acceptance.** New test
  `rafaello-mockprovider/tests/mockprovider_manifest_compiles.rs`
  (pi-1 B-2 + pi-2 H-5) loading the fixture via the
  **live** API sequence:
  ```rust
  let fixture_dir = /* workspace path to the fixture dir */;
  let manifest_path = fixture_dir.join("rafaello.toml");
  let raw = std::fs::read_to_string(&manifest_path)?;
  let manifest = Manifest::parse(&raw)?;
  manifest::validate_with_package(&manifest_path,
      &fixture_dir, &manifest)?;
  ```
  per `manifest/top_level.rs:68` + `validate_with_package.rs:18-22`.

### c21 — feat(rafaello-mockprovider): bin implementation + multi-turn-correlation integration tests

- **What.** scope §PR2 + §PR4 — the deterministic
  content-pattern matcher per scope §PR2 (hand-written,
  no regex — pi-1 M-3 / pi-2 M2-3 multibyte-safe
  slicing) wired through fittings as the
  `rfl-mockprovider` bin's main loop. Reads env per
  scope (no `RFL_PROVIDER_ACTIVE` — pi-1 H-1). Holds
  per-session state: `outstanding`,
  `last_user_message`, `seen_tool_results`. Generates
  fresh `request_id`s via `Ulid::new().to_string()`
  for each emitted event.
- **Why.** scope §PR2 — the mock provider bin.
- **Depends on.** c14 (provider supervisor path so the
  bin can be spawned and registered), c17/c18 (reemit
  delivers `core.session.user_message` /
  `core.session.tool_result` to the provider), c20
  (manifest fixture exists).
- **Acceptance.** Seven new tests + one new common
  module (pi-1 M-3 — harness placement explicit) in
  `rafaello-mockprovider/tests/`:
  - **New common module**
    `rafaello-mockprovider/tests/common/mock_provider_handle.rs`
    exposing `MockProviderHandle` — a struct wrapping
    a spawned `rfl-mockprovider` child + the in-test
    broker fixture; exposes
    `publish_user_message(&self, text: &str) ->
    JsonRpcId` and `recv_event(&self) -> BusEvent`
    (scope §H1). Imported by every test below.
  - `mockprovider_emits_tool_request_for_read_file_pattern.rs`
    — `"what's in README.md"` →
    `provider.mock.tool_request` with `{tool:
    "read-file", args: {path: "README.md"}}`,
    `request_id` present, `in_reply_to: []`
    (first-turn, no prior results).
  - `mockprovider_strips_trailing_punctuation_from_path.rs`
    (pi-1 H-4) — `"what's in README.md?"` →
    `args.path = "README.md"`.
  - `mockprovider_records_request_id_to_path_mapping.rs`
    (pi-1 H-4) — two consecutive user_messages for
    different paths; outstanding map records both;
    a `tool_result` for the second resolves to the
    second path's assistant text.
  - `mockprovider_emits_echo_assistant_message_on_no_match.rs`
    — `"hello"` → `{text: "echo: hello"}` with
    `in_reply_to: [<user_message.request_id>]`.
  - `mockprovider_emits_assistant_message_on_tool_result.rs`
    — drive a request; inject a `tool_result` with
    `payload.content = "Hello!"`; assert
    assistant text begins `"Here's what's in"` and
    `in_reply_to = [<tool_result.request_id>]`.
  - `mockprovider_handles_multibyte_utf8_path.rs`
    (pi-1 H-4) — `"what's in données.txt"` →
    `args.path = "données.txt"`.
  - `mockprovider_multi_turn_cites_prior_tool_result_id.rs`
    (pi-3 H-1) — the canonical multi-turn coverage:
    turn 1 user_message → tool_request →
    tool_result; turn 2 user_message → second
    tool_request whose `in_reply_to` **contains**
    the turn-1 tool_result's `request_id`; broker
    accepts (retained-context semantics — no
    `StaleRequestId`).

### c22 — feat(rafaello-readfile): manifest fixture + openrpc.json + compile-test

- **What.** scope §TP1 + §TP3:
  - Create `rafaello/fixtures/rafaello-readfile/rafaello.toml`:
    top-level `schema = 1; name = "readfile";
    version = "0.0.0"; entry = "bin/rfl-readfile";
    rafaello = ">=0.1, <0.2"`; `[provides] tools =
    ["read-file"]`; `[provides.tool.read-file] sinks
    = []; always_confirm = false`; `[bus]
    subscribes = []; publishes = []` (the m1
    compiler auto-inserts the tool_request
    subscribe and the c06 tool_result publish);
    `[capabilities.default.filesystem] read_dirs =
    ["${project}"]; write_dirs = []`;
    `[capabilities.default.network] mode = "deny"`;
    `[load] eager = true` (pi-3 B-6 — m4 eager-loads
    every tool).
  - Create `rafaello/fixtures/rafaello-readfile/openrpc.json`
    — same minimal shape as c20.
  - **Create `rafaello/fixtures/rafaello-readfile/bin/rfl-readfile`**
    (pi-1 B-1) — same placeholder shim pattern as
    c20: `#!/bin/sh\nexec "$@"\n` with `chmod +x`.
    Required so
    `manifest::validate_with_package` finds the
    `entry` path. Runtime spawn uses the cargo-built
    `rfl-readfile` from the workspace target dir,
    not this shim.
- **Why.** scope §TP1 + §TP3.
- **Depends on.** c04 (readfile crate exists), c06
  (auto-publish grant — without c06 the manifest's
  empty `publishes` list would not satisfy the
  runtime's `plugin.<topic-id>.tool_result` publish
  authority).
- **Acceptance.** New test
  `rafaello-readfile/tests/readfile_manifest_compiles.rs`
  (pi-1 B-2 + pi-2 H-5) — same live API sequence as
  c20.

### c23 — feat(rafaello-readfile): bin implementation + readfile integration tests + lockin denial

- **What.** scope §TP2 + §TP4 + pi-4 B-1 + pi-3 H-3 —
  the readfile tool bin's main loop:
  - Reads env: `RFL_BUS_FD`, `RFL_TOPIC_ID`,
    `RFL_PROJECT_ROOT`, `RFL_PRIVATE_STATE_DIR`,
    `RFL_PLUGIN`.
  - On `bus.event` with topic
    `plugin.<own-topic-id>.tool_request`, parses
    payload as `{tool: "read-file", args: {path:
    String}}`. Resolves `path` against
    `RFL_PROJECT_ROOT` if relative. Rejects
    paths that escape (canonicalize + ancestor
    check).
  - Reads the file (utf8-only — m4 cut).
  - Publishes `plugin.<topic-id>.tool_result` with:
    - payload `{ok: true, content: <utf8>}` (or
      `{ok: false, error: <reason>}`) — canonical
      wire shape (pi-3 B-4);
    - **`request_id: Some(JsonRpcId::String(Ulid::new()
      .to_string()))`** — pi-4 B-1, §B0 table-of-truth
      requires `request_id` on every `.tool_result`;
    - `in_reply_to: Some(vec![<tool_request.request_id>])`.
  - Test-only env hook
    `RFL_READFILE_TEST_BYPASS_GUARD=1` (per scope
    §TP3 / pi-1 H-3) skips the in-plugin ancestor
    check and calls `std::fs::read` directly on the
    raw input — used by the lockin-level negative
    test below.
- **Why.** scope §TP2 — the read-file tool.
- **Depends on.** c14 (tool plugin spawn path),
  c19 (agent loop dispatches to it),
  c22 (manifest fixture exists).
- **Acceptance.** Five new tests + one new common
  module (pi-1 M-3) in `rafaello-readfile/tests/`:
  - **New common module**
    `rafaello-readfile/tests/common/read_file_tool_handle.rs`
    exposing `ReadFileToolHandle` — analogous to c21's
    `MockProviderHandle`; wraps a spawned
    `rfl-readfile` child + an in-test broker fixture;
    exposes `publish_tool_request(&self, path:
    &str) -> JsonRpcId` and `recv_event(&self) ->
    BusEvent` (scope §H2). Imported by every test
    below.
  - `readfile_returns_content_for_existing_file.rs`
    — tempdir project root with a `README.md`;
    synthetic `plugin.<topic-id>.tool_request`
    `{path: "README.md"}` → `tool_result {ok: true,
    content: "<body>"}`, `request_id` present,
    `in_reply_to` matches.
  - `readfile_errors_for_missing_file.rs`.
  - `readfile_errors_for_non_utf8.rs`.
  - `readfile_errors_for_outside_project_root.rs`
    — plugin-level ancestor check rejects.
  - `readfile_lockin_denies_outside_grant.rs` (pi-1
    H-3) — sandbox-level negative: spawn with
    `RFL_READFILE_TEST_BYPASS_GUARD=1` and a
    `read_dirs` restricted to a tempdir A; request
    a file in tempdir B; the lockin sandbox denies
    with `io::ErrorKind::PermissionDenied` rendered
    into the `tool_result.error`.

---

## Phase G — `rfl chat` orchestration

### c24 — feat(rafaello): rfl chat lock-load + V3 validation + compile_plugin per plugin + orchestration negatives

- **What.** scope §C1-§C3 + §C13 + §C14:
  - Edit `rafaello/crates/rafaello/src/lib.rs::run_chat`
    to add steps C1-C3 (live API sequence per pi-2
    B2 / pi-3 B3): `Lock::from_toml(&fs::read_to_string(
    project_root.join("rafaello.lock"))?)?`,
    `validate::lock(&lock, &LockValidationContext
    { project_root, home, plugin_dirs, cache_root,
    state_root })?`, per-plugin
    `compile_plugin(&lock, &canonical, &path_ctx,
    &recomputed_digests)?` building
    `RecomputedDigests` via `digest::content_digest`
    + `digest::manifest_digest(&manifest.canonical_bytes())`
    (no `.as_bytes()` — `canonical_bytes()` returns
    `Vec<u8>`; pi-3 B-3) and `topic_id::derive(
    &canonical.to_string())` for cache/state-dir
    naming (NOT an invented `topic_id_of`; pi-3 B-3).
  - Extend `RflChatError` (existing in `rafaello/src/lib.rs`)
    with the new variants from scope §C13:
    `LockNotFound`, `LockIo`, `LockParse`,
    `LockValidation`, `NoHomeDir`, `ManifestIo`,
    `ManifestParse`, `Digest`, `CompilePlugin`,
    `NoActiveProvider`, `ProviderSpawnFailed`,
    `ToolSpawnFailed`. Each maps to a distinct
    non-zero exit code with a stderr message.
  - **Stop short of supervisor construction + spawn**
    — that's c25. This commit only lands the lock
    pipeline + the error surface.
- **Why.** scope §C1-§C3 + §C13 — the load half.
- **Depends on.** c07 (BusEvent.request_id field exists,
  for downstream compatibility), c14 (Provider broker
  surface needed because compile_plugin emits
  provider-shaped ACLs that the broker must accept),
  c15 (frontend ACL extension lives in run_chat).
- **Acceptance.** Four new tests + the m3 test
  migration in the same commit (pi-1 B-7 — the
  `LockNotFound` path lands here, so the m3 tests
  that previously asserted no-lock-clean-exit get
  updated in this commit):
  - `rafaello/tests/rfl_chat_missing_lock_errors.rs`
    — no `rafaello.lock` at project root; exit
    non-zero with stderr citing `LockNotFound`.
  - `rafaello/tests/rfl_chat_invalid_lock_errors.rs`
    — corrupt TOML; `LockParse`.
  - `rafaello/tests/rfl_chat_lock_validation_fails.rs`
    — lock with an invalid `bindings.tools` entry;
    `LockValidation`.
  - `rafaello/tests/rfl_chat_no_active_provider_errors.rs`
    — valid lock with `session.provider_active =
    None`; `NoActiveProvider`.
  - **m3 `rfl chat` test migration** (pi-1 B-7).
    Every m3 `rfl chat` test that previously
    constructed a tempdir without a `rafaello.lock`
    file now uses a shared helper
    `rafaello/tests/common/m4_lock_fixture.rs::write_stub_lock(dir: &Path)`
    that materialises a **minimal stub
    `rafaello.lock`** (empty `plugins` table,
    `session.provider_active = None`,
    `session.tool_owner = {}`) before invoking
    `rfl chat`. Affected files (verified at agent
    time via `ls rafaello/crates/rafaello/tests/`):
    `rfl_chat_demo_bar.rs`,
    `rfl_chat_resolves_tui_via_env_override.rs`,
    `rfl_chat_locked_session_errors_with_holder_pid.rs`,
    `rfl_chat_relative_project_root_canonicalises.rs`,
    `rfl_chat_nonexistent_project_root_errors.rs`,
    `rfl_chat_locked_session_unknown_holder_errors.rs`,
    `rfl_chat_replay_withheld_until_frontend_ready.rs`,
    `rfl_chat_frontend_exits_before_ready_errors.rs`,
    `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`,
    `rfl_chat_frontend_ready_timeout_errors.rs`.
    Migration rule per file:
    - Tests previously asserting **clean exit 0**
      now assert **exit non-zero with
      `NoActiveProvider` on stderr** (m4 makes a
      provider load-bearing).
    - Tests asserting failure modes that fire
      **BEFORE** lock load (project-root
      canonicalisation, nonexistent-project-root,
      flock-held-by-other) keep their existing
      assertions — the m4 path canonicalises BEFORE
      `Lock::from_toml`, so the EARLIER failure is
      still surfaced. The stub-lock fixture is still
      written so the test doesn't depend on the
      lock-load happening at all.
    - `rfl_chat_demo_bar.rs` (the m3 legacy demo
      bar) becomes a `NoActiveProvider`-expecting
      smoke test under the stub lock; the m4
      headline demo bar is c27's new
      `rfl_chat_demo_bar_read_file.rs`.
  Workspace test suite green.

### c25 — feat(rafaello): rfl chat supervisor + eager spawn provider + tool + ProviderSpawnFailed negative

- **What.** scope §C4-§C8 + tail of §C14:
  - Continue `run_chat` after C3: build
    `BrokerAcl` via `broker_acl::compile(&lock)`,
    extend `acl.frontends` with the `tui` entry
    from c15; construct `Broker::new(acl)?`;
    construct `PluginSupervisor::new(broker.clone(),
    SupervisorConfig::default())`; compute
    `SpawnPaths` per plugin (project_root +
    `.rafaello-plugin-data/<topic-id>/`).
  - **Eager spawn the active provider** (C7): look
    up `lock.session.provider_active`; spawn via
    `supervisor.spawn(&plan, &paths).await`;
    collect `SpawnHandle` clones. On
    `None` → `NoActiveProvider` (c24 enum already
    has it). On spawn failure →
    `ProviderSpawnFailed`.
  - **Eager spawn every tool plugin** (C8): iterate
    `lock.plugins` filtering
    `!entry.bindings.tools.is_empty() &&
    !entry.bindings.provider`; spawn each; collect
    `SpawnHandle`s. On failure → `ToolSpawnFailed`.
  - **No lazy spawn** (pi-2 B-6). Out of scope row
    in scope.md is the canonical record.
- **Why.** scope §C4-§C8.
- **Depends on.** c14, c20 (mockprovider manifest +
  bin so a fixture lock can point at it), c21
  (mockprovider bin compiles), c22, c23 (readfile
  bin compiles).
- **Acceptance.** Two new tests in `rafaello/tests/`:
  - `rfl_chat_provider_spawn_failure_propagates.rs`
    — fixture lock points the provider plugin at
    `/nonexistent/binary`; `rfl chat` exits
    non-zero with stderr citing
    `ProviderSpawnFailed`.
  - `rfl_chat_tool_spawn_failure_propagates.rs`
    (pi-1 H-4) — fixture lock points the **readfile
    tool** plugin at `/nonexistent/binary` while the
    mockprovider plugin's entry is valid (so the
    provider spawn succeeds first); `rfl chat` exits
    non-zero with stderr citing `ToolSpawnFailed`
    (NOT `ProviderSpawnFailed` — the order matters,
    and the failure must surface from the tool-spawn
    iteration in §C8, not the provider spawn in §C7).

### c26 — feat(rafaello): rfl chat reemit + agent + TUI spawn + wait loop + shutdown

- **What.** scope §C9-§C12:
  - After C8 (spawning complete), construct
    `ReemitRouter::new(broker.clone(), acl.clone(),
    provider_active_canonical, shutdown_rx.clone())`
    and spawn its task. Construct `AgentLoop::new(
    broker.clone(), acl.clone(), controller.clone(),
    Capabilities::tui_default(), shutdown_rx.clone())`
    (pi-1 B-5 — `caps` passed explicitly; matches the
    c19 struct shape) and spawn its task. Both
    subscribe BEFORE the TUI starts (so the
    user_message → reemit → tool_request → tool_result
    chain is wired before any input).
  - Frontend (TUI) spawn — unchanged from m3 except
    the `CompiledFrontend.env.pass` allowlist
    extended to include `RFL_TUI_TEST_MESSAGE`
    (c16 already extended the constant in
    `rafaello/src/lib.rs`).
  - Wait loop: `tokio::select!` on
    `handle.wait_ready()` (run
    `controller.replay_history` on `Ok(Ok(()))`),
    TUI exit, shutdown signal.
  - Shutdown: signal the shutdown watch →
    `router_join.await` + `agent_join.await` →
    `supervisor.shutdown().await` (no grace arg per
    `supervisor.rs:812`) → drain stderr forwarder.
- **Why.** scope §C9-§C12 — the wire-up half.
- **Depends on.** c17 (ReemitRouter), c19
  (AgentLoop), c25 (supervisor + plugin spawns
  populate the broker registry before reemit/agent
  start).
- **Acceptance.** One new same-commit smoke test
  (pi-1 B-6 — the "code + tests in same commit"
  rule):
  - `rafaello/tests/rfl_chat_eager_spawns_provider_and_tool_then_shuts_down_cleanly.rs`
    — pre-materialise a fixture `rafaello.lock` with
    `rfl-mockprovider` (active) + `rfl-readfile`
    installed; spawn `rfl chat` with
    `RFL_TUI_TEST_MESSAGE="hello"` (non-matching
    pattern → mockprovider echoes back via
    `provider.mock.assistant_message`, no tool
    round-trip); wait for the `core.session.entry.finalized`
    line on stderr corresponding to the assistant's
    "echo: hello" text; trigger Ctrl-C; assert
    `rfl chat` exits 0 and every spawned plugin
    child reaped cleanly (no zombies under
    `/proc/self/task` on Linux; verified via the m2
    fd-baseline pattern). 6-second `serial_test`
    gate. The c27 headline test exercises the
    full tool-dispatch path; this smoke covers the
    wire-up correctness without depending on c27.
  The m3 `rfl chat` test migration landed in c24
  (pi-1 B-7); c26 does not re-migrate them. Existing
  m3 frontend-handle / SessionStore / renderer-pipeline
  tests are unchanged (they don't spawn `rfl chat`).

---

## Phase H — Demo bar + manual validation

### c27 — test(rafaello): rfl_chat_demo_bar_read_file.rs headline test (pinned bytes + exact assistant text)

- **What.** scope §I `rafaello/tests/`
  `rfl_chat_demo_bar_read_file.rs` (pi-2 H-4 pinned
  bytes + pi-3 B-4 wire shape). Setup:
  ```rust
  const README_BODY: &str = "m4 demo readme\n";
  let tempdir = tempfile::TempDir::new()?;
  let project_root = tempdir.path();
  std::fs::write(project_root.join("README.md"),
      README_BODY)?;
  // pre-materialise rafaello.lock with rfl-mockprovider
  // (active) + rfl-readfile installed, both
  // `[load] eager = true`.
  ```
  Drive via `RFL_TUI_TEST_MESSAGE="what's in
  README.md"` per §T1. Assert (in order):
  - SQLite `entries` table contains rows of kinds
    `text` (user), `tool_call`, `tool_result`,
    `text` (assistant) in `seq` order, distinguished
    by `metadata.author` (User / Assistant / Tool).
    Test asserts via the canonical `Entry` shape,
    not via the kind string alone (pi-1 B-8).
  - Combined stderr stream contains the four
    `"rfl-tui: bus.event topic=core.session.entry.finalized
    seq=N"` lines for `N = 0..=3`.
  - The assistant message's text equals **exactly**
    `"Here's what's in README.md:\nm4 demo readme\n"`
    (string equality, not `starts_with`).
- **Why.** scope demo bar — the headline test.
- **Depends on.** c26 (full orchestration path).
- **Acceptance.** Test passes on Linux + macOS CI.
  6-second `serial_test` gate caps wall-clock.

### c28 — docs(rafaello-m4): manual-validation.md

- **What.** Driver-owned (per the m3 precedent —
  the per-commit agent writes the file, but only
  AFTER the full c27 cargo-test passes). Create
  `rafaello/plans/milestones/m4-provider-agent-loop/manual-validation.md`
  documenting:
  - **Interactive `rfl chat` run** against a
    real-tempdir fixture lock — user types
    `"what's in README.md"`, sees the file body
    rendered as an assistant `text` entry, exits
    cleanly via Ctrl-C; SQLite contains the four
    entries from c27.
  - **macOS CI green URL** (driver-captured at
    retrospective time; per-commit prompt writes a
    placeholder).
  - **Negative validation list**: each of the six
    roadmap-row negatives walked through with the
    exact test file that covers it (cross-reference
    to scope.md §"Acceptance summary").
- **Why.** scope §"Acceptance summary" requires
  `manual-validation.md`.
- **Depends on.** c27.
- **Acceptance.** File exists at the path; markdown
  validates; cross-references to test files in
  scope.md §"Acceptance summary" are accurate.

---

## Coverage check

Every scope.md §"Acceptance summary" matrix file maps to
exactly one commit above. Every scope.md In-scope letter
section (W / B0 / B / F / PS / CR / AL / PR / TP / TD / M2 /
H6 / M1 / I / H / C / T) is covered:

- W → c01, c02, c03, c04
- B0 (table-of-truth) → c07 (cited by every later commit)
- B1-B11 → c07 (B1-B3 monolithic), c08 (B4 errors), c09
  (B5 registration), c10 (B6 + B7b), c11 (B7), c12 (B8 +
  B10), c13 (B11), c09/c10 (B9 topic validation —
  enforced inside handlers)
- F → c15
- PS → c14
- CR → c17 (CR1, CR6, CR7), c18 (CR2-CR5)
- AL → c19
- PR → c20, c21
- TP → c22, c23
- TD → c19
- M2 → c14
- H6 → no new inject points (round-1 explicit "no
  TestHooks taxonomy expansion"); the existing m3
  three-point machinery is reused by c14's positive
  successor test
- M1 → c05 (M1.1), c06 (M1.3); M1.2 is the no-commit
  default (m3 retro §2.7 deferral)
- I → tests land alongside the code in each commit
- H → harness extensions land alongside the tests that
  consume them (driver picks scope-shaped placement at
  agent time — round-1 default is the existing m3
  test-kit modules extended in-place; no separate "harness"
  commit needed)
- C → c24, c25, c26
- T → c16

**Total: 28 commits.** Inside the scope §"Internal split"
budget of 26-32.

---

## What changed from prior drafts

Round-1 draft only. No prior draft to diff against.
