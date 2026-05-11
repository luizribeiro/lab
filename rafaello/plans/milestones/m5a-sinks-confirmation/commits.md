# m5a-sinks-confirmation — commits

> **Status:** round 5 draft — folds `commits-pi-review-4.md`
> (2 B / 1 M / 1 N). Pi-4 expects zero-blocker convergence
> next round.
>
> Round-5 fixes by pi-4 finding:
> - **B-1** c34's m5a fixture lock drops the
>   `session.tool_owner.send-mail` and
>   `session.tool_owner.read-file` entries — live
>   `validate::lock` rejects them as
>   `ToolOwnerRedundant` when only one claimant
>   exists. `session.tool_owner` is conflict-resolution
>   state, not an active-pin table. Acceptance test
>   renamed `m5a_fixture_lock_session_pins_provider_active.rs`
>   and now asserts `session.tool_owner.is_empty()`.
> - **B-2** c18's `/grant` default-plugin resolution
>   reworded to use `BrokerAcl::tool_route(tool)` (a
>   small wrapper over the live `tool_routes`
>   BTreeMap m4 populates as the dispatch-target
>   source of truth), NOT `session.tool_owner[tool]`
>   (which is empty in the no-conflict case).
>   `SlashHandler` gains an `Arc<BrokerAcl>` field.
>   New tests for `/grant send-mail` with default
>   plugin resolution and `/grant nonexistent` with
>   unknown tool.
> - **M-1** c38's inactive-provider spawn bullet
>   reworded: the loop excludes the active provider
>   by **canonical id** comparison (the
>   unique-per-install identifier), while the
>   `ReemitRouter` topic scope is
>   `provider.<active.provider_id>.**` (resolved
>   from the active canonical's `PluginAcl.provider_id`
>   — `provider.openai.**` in the fixture). The
>   round-3 wording conflated the two.
> - **N-1** Cross-checks / sizing / convergence-note
>   tail updated to round-5 wording.
>
> ---
>
> Round-4 fixes by pi-3 finding (kept for trajectory):
> - **B-1 (r3)** c34 lock fixture renamed
>   `session.active_provider` → `session.provider_active`
>   (live field per `lock/session.rs:11`).
> - **M-1 (r3)** c11's wire-shape positive reworded to
>   a direct broker check; gate-publisher positive
>   stays in c24.
> - **M-2 (r3)** c38 spawn-loop spelled out.
> - **M-3 (r3)** c34 catalog-build assertion extended.
> - **N-1 (r3)** Stale `c37` → `c38` references fixed.
>
> Drafted against scope.md round 6 (ratified `95d6f12`). Pi-6's
> sole nit (OP6 vs Tr1 wording mismatch on the unused-`allow_secrets`
> stderr line) was folded in round 2: c27 pins the canonical
> stderr string `warning: unused allow_secrets entry '<name>' (no
> matching env.pass entry)` from §Tr1 step 10.
>
> Total: **41 commits** (round 1 had 42; old c25 agent-loop pivot
> bundled into c38 per pi-1 B-3). Sizing: 30 small, 8 medium,
> 1 large, 3 unsplittable cutovers (c06 + c10 + c38).
>
> Per-commit agent prompts MUST inline the full row text below
> verbatim (m1 §4.2): cite-by-row delegates granularity to the
> per-commit agent and risks bundling.
>
> ---
>
> Round-3 fixes by pi-2 finding (one line each):
>
> - **B-1** c04 feature stanza simplified to
>   `test-fixture = []` + `required-features =
>   ["test-fixture"]` on the bin; the feature
>   dependency list is dropped.
> - **B-2** c31 backfills
>   `rafaello/fixtures/rafaello-readfile/openrpc.json`
>   to declare the `read-file` method (currently
>   empty `methods: []` would fail
>   `ToolSchemaCatalog::build`'s method-vs-tool
>   consistency check). Mockprovider stays
>   `methods: []` (no `provides.tools`). Adds an m4
>   readfile-demo-still-starts regression test.
> - **B-3** c34's combined fixture lock now includes
>   **all four plugins** (openai + mailcat + readfile
>   + mockprovider). c34 deps add c31; c34 acceptance
>   gains a four-plugin validate-and-compile test plus
>   a session-pin assertion.
> - **B-4** c24's stale-answer-after-short-circuit
>   test renamed and re-anchored:
>   `gate_duplicate_answer_after_grant_short_circuit_audit_logged.rs`
>   audits `confirm_duplicate` (NOT `confirm_late`).
>   Rationale: short-circuit transitions
>   `Active → ResolvedByAnswer`; `prior_outcome` then
>   classifies as `Duplicate`. `Late` only applies to
>   `TimedOut`.
> - **M-1** c11 gains a `confirm_resolved` wire-contract
>   table (envelope `request_id`, payload
>   `request_id`, `in_reply_to`, stale meaning) plus
>   a positive `broker_publish_*_wire_shape_positive.rs`
>   test asserting the relationship.
> - **M-2** c27's `AuditWriter::open_for_install`
>   now calls `std::fs::create_dir_all(project_root.join(".rafaello/state"))`
>   before opening the SQLite file. New fresh-tempdir
>   test verifies the directory + table appear.
> - **M-3** c38 deps add c16 and c18 (the slash
>   handler and its jsonschema transitive dep — the
>   handler is registered as an internal subscriber
>   at run_chat in this commit).
> - **M-4** c34's lock fixture text pins
>   `bindings.tool_meta.send-mail.grant_match =
>   "schemas/send-mail-grant.json"` explicitly
>   (NOT openrpc.json; openrpc carries the tool's
>   param schema for the model-facing catalog, while
>   grant_match validates the user's `/grant`
>   template).
> - **M-5** c14 acceptance gains
>   `reemit_confirm_answer_without_confirm_state_warns_and_drops.rs`
>   asserting the transitional drop contract (no
>   `confirm_reply` emitted when `confirm_state` is
>   `None`; tracing warn captured; other re-emit
>   arms unaffected).
> - **N-1/N-2/N-3** Renumber-and-cross-check pass:
>   round-2 preamble used stale numbers (c26/c33/c37
>   etc. that did not match the post-renumber file
>   after the round-2 sed pass). Round-3 preamble +
>   cross-check + sizing summary all use round-3
>   final numbers verified against the
>   `### cNN — ...` headings.

## Reading order for per-commit agents

Every per-commit agent receives:

1. `rafaello/plans/overview.md`.
2. `rafaello/plans/decisions.md` rows 4, 9, 10, 11, 12, 13, 17,
   26, 27, 28, 29, 38, 42, 43, 45.
3. `rafaello/plans/glossary.md`.
4. The streams RFCs relevant to the row's scope.
5. `rafaello/plans/milestones/m5a-sinks-confirmation/scope.md`.
6. The inlined row text below — full prose, every acceptance
   bullet — passed in the prompt body, not cited by number.

`tests-with-code`: every acceptance row names the test files it
adds. Per `~/.claude/CLAUDE.md`, tests land in the same commit
as the surface they cover unless explicitly called out as a
follow-up extension (two-stage tests, m0 retrospective §4.3).

---

## Phase A — Workspace + scaffolds + `allow_secrets` cutover

Scope §W1-W4 + §OP6 schema landing. Six commits: the three new
crate skeletons + workspace dep additions land separately from
logic, and the `allow_secrets` schema cuts across manifest +
lock + effective-grant + scrubber + compile-time caller in one
unsplittable workspace cutover.

### c01 — chore(rafaello): add m5a workspace members (rafaello-openai, rafaello-openai-stub, rafaello-mailcat)

- **What.** Scope §W1-W3. Extend `rafaello/Cargo.toml` `members`
  list from `["crates/rafaello", "crates/rafaello-core",
  "crates/rafaello-tui", "crates/rafaello-mockprovider",
  "crates/rafaello-readfile"]` to also include
  `crates/rafaello-openai`, `crates/rafaello-openai-stub`,
  `crates/rafaello-mailcat`. Workspace-member placeholder
  cutover (m4 c01 precedent): the `members` edit AND three
  minimal `Cargo.toml` + `src/lib.rs` placeholders land
  together so the workspace resolves cleanly. Full deps + bin
  targets land in c03/c04/c05. Concretely:
  - `rafaello/crates/rafaello-openai/Cargo.toml` with
    `[package] name = "rafaello-openai"; version = "0.0.0";
    edition = "2021"; publish = false; [lib] path = "src/lib.rs"`
    plus an empty `src/lib.rs`.
  - `rafaello/crates/rafaello-openai-stub/Cargo.toml` — same
    shape, name `rafaello-openai-stub`, empty `src/lib.rs`.
  - `rafaello/crates/rafaello-mailcat/Cargo.toml` — same
    shape, name `rafaello-mailcat`, empty `src/lib.rs`.
- **Why.** Scope §W1-W3. Pure scaffolding so c02-c06 add
  real content without churning the workspace shape.
- **Depends on.** baseline (m4 retro merged).
- **Acceptance.** `cargo metadata --manifest-path
  rafaello/Cargo.toml --format-version 1` succeeds with eight
  workspace members. `cargo build --workspace` green.
  `cargo doc --workspace --no-deps` warning-free. m4's full
  test suite still passes.
- **Size.** small.

### c02 — feat(rafaello): add `reqwest` and `jsonschema` to `[workspace.dependencies]`

- **What.** Scope §W1 + §UG2. Edit `rafaello/Cargo.toml`
  `[workspace.dependencies]`:
  - `reqwest = { version = "0.12", default-features = false,
    features = ["rustls-tls", "json"] }` — used by
    `rafaello-openai` for the OpenAI Chat Completions HTTP
    client. `rustls-tls` keeps the macOS / Linux builds
    identical (no platform-specific TLS backend).
  - `jsonschema = "0.18"` — used by `rafaello-core` to
    validate `/grant` user-supplied matcher templates against
    the lock's `bindings.tool_meta.<n>.grant_match` schema at
    slash-command processing time (§UG2). Pure-Rust, no C
    deps; MSRV compatible with the workspace
    `rust-toolchain.toml`.
- **Why.** Land workspace aliases up front so c03 and c20
  compile cleanly when they wire the actual dependents.
- **Depends on.** c01.
- **Acceptance.** `cargo metadata --manifest-path
  rafaello/Cargo.toml --format-version 1` shows both new
  workspace deps. `cargo build --workspace` green
  (placeholders don't consume them yet; transitively fetched
  only). `cargo doc --workspace --no-deps` warning-free.
  Linux + macOS CI green.
- **Size.** small.

### c03 — feat(rafaello-openai): scaffold crate + bin target + deps

- **What.** Scope §W1 — fill out the c01 placeholder. Edit
  `rafaello/crates/rafaello-openai/Cargo.toml`:
  - Add `[[bin]] name = "rfl-openai"; path =
    "src/bin/rfl_openai.rs"`.
  - `[dependencies]`: `rafaello-core = { path =
    "../rafaello-core" }`, `tokio`, `tracing`,
    `tracing-subscriber`, `fittings-core`, `fittings-server`,
    `fittings-client`, `fittings-transport`, `serde`,
    `serde_json`, `async-trait`, `anyhow`, `ulid`,
    `reqwest = { workspace = true }` (the new alias from c02)
    — all the others `workspace = true` per the m4
    `rafaello-mockprovider/Cargo.toml:19-22` shape.
  - `[dev-dependencies]`: `tempfile`, `serial_test`,
    `tracing-test` (all workspace).
  - `src/lib.rs`: `//! rafaello-openai scaffolding.`
    placeholder.
  - `src/bin/rfl_openai.rs`: minimal `fn main() {
    eprintln!("rfl-openai: scaffolding only.");
    std::process::exit(0); }`.
- **Why.** Scope §W1 + §OP. Lay the build tracks so c31-c35
  fill in the wire client, bus adapter, and tests without
  fighting the build system.

  **§W4 disposition** (pi-1 N3): c03 also appends a one-line
  "m5a adds rfl-openai (OpenAI-compatible provider plugin)"
  entry to `rafaello/README.md`'s crate list (the workspace
  README exists; verified at round-2 drafting). This
  satisfies scope §W4 inside the openai scaffold commit
  rather than dangling as an unassigned bullet.
- **Depends on.** c01, c02.
- **Acceptance.** `cargo build -p rafaello-openai` green.
  `cargo build -p rafaello-openai --bin rfl-openai` green.
  `cargo doc -p rafaello-openai --no-deps` warning-free.
- **Size.** small.

### c04 — feat(rafaello-openai-stub): scaffold crate + bin target + test-fixture feature

- **What.** Scope §W2. Fill out the c01 placeholder. Edit
  `rafaello/crates/rafaello-openai-stub/Cargo.toml`:
  - Add `[[bin]] name = "rfl-openai-stub"; path =
    "src/bin/rfl_openai_stub.rs"`.
  - `[features]`: `default = []`; `test-fixture = []`
    (no feature dependencies — pi-2 B-1 smallest fix;
    the deps tokio/serde_json are unconditional in the
    `[dependencies]` table). The `[[bin]]` entry
    declares `required-features = ["test-fixture"]` so
    the stub is only built when the workspace
    `test-fixture` feature is on (mirrors m4's
    `rafaello-bus-fixture` pattern). No `hyper`
    workspace dep (pi-1 B-1 — the workspace has no
    `hyper` alias today and `reqwest`'s transitive
    copy cannot be referenced as `workspace = true`).
  - `[dependencies]`: `tokio` (with `workspace = true` and
    feature subset `net`, `macros`, `rt-multi-thread`, `io-util`),
    `serde`, `serde_json`, `anyhow`, `tracing`. **The HTTP
    server is hand-rolled** on top of `tokio::net::TcpListener`
    with manual request-line + header parsing and a single
    fixed `Content-Length`-based body read (~80 LoC; pi-1
    B-1 — no new workspace dep, no per-commit-agent design
    choice). The bin lives at `src/bin/rfl_openai_stub.rs`
    and the parser sits in `src/server.rs` for ease of unit
    testing.
  - `src/lib.rs`: `//! rafaello-openai-stub scaffolding.`
    placeholder.
  - `src/bin/rfl_openai_stub.rs`: minimal `fn main() {
    eprintln!("rfl-openai-stub: scaffolding only.");
    std::process::exit(0); }`.
- **Why.** Scope §W2 + §A8. Defer the real `/v1/chat/completions`
  stub to c35 once we know what wire shape the openai
  client expects.
- **Depends on.** c01, c02.
- **Acceptance.** `cargo build -p rafaello-openai-stub
  --features test-fixture` green. `cargo build
  -p rafaello-openai-stub` (default features) succeeds
  (lib only; bin gated). `cargo doc -p rafaello-openai-stub
  --no-deps` warning-free.
- **Size.** small.

### c05 — feat(rafaello-mailcat): scaffold crate + bin target + deps

- **What.** Scope §W3 — fill out the c01 placeholder. Same
  shape as c03 but for `rafaello-mailcat`:
  - Edit `rafaello/crates/rafaello-mailcat/Cargo.toml`:
    add `[[bin]] name = "rfl-mailcat"; path =
    "src/bin/rfl_mailcat.rs"`. `[dependencies]`:
    `rafaello-core = { path = "../rafaello-core" }`,
    `tokio`, `tracing`, `tracing-subscriber`,
    `fittings-core`, `fittings-server`, `fittings-client`,
    `fittings-transport`, `serde`, `serde_json`,
    `async-trait`, `anyhow`, `ulid` — all `workspace =
    true`.
  - `[dev-dependencies]`: `tempfile`, `serial_test`,
    `tracing-test` (workspace).
  - `src/lib.rs`: `//! rafaello-mailcat scaffolding.`
    placeholder.
  - `src/bin/rfl_mailcat.rs`: minimal `fn main() {
    eprintln!("rfl-mailcat: scaffolding only.");
    std::process::exit(0); }`.
- **Why.** Scope §W3 + §TP. Sink-declaring fixture-tool
  scaffolding; c30 fills in the manifest, openrpc.json,
  and bin.
- **Depends on.** c01.
- **Acceptance.** `cargo build -p rafaello-mailcat` green.
  `cargo build -p rafaello-mailcat --bin rfl-mailcat`
  green. `cargo doc -p rafaello-mailcat --no-deps`
  warning-free.
- **Size.** small.

### c06 — feat(rafaello-core): `env.allow_secrets` schema + scrubber-signature cutover

- **What.** Scope §OP6 — the additive m1 schema extension
  and scrubber-signature change. This is an **unsplittable
  workspace cutover** (m0 c08 / m4 c07 class): the new
  field has to land in manifest `EnvCapabilities`, lock
  `GrantEnv`, `compile.rs::effective_grant`'s
  union/dedup loop, the `scrubber::strip` signature, and
  every existing call site of `scrubber::strip` in one
  commit. Splitting fails per-commit green.
  Concretely:
  - `crates/rafaello-core/src/manifest/capabilities.rs` —
    extend `EnvCapabilities` (currently lines 69-73) with
    `pub allow_secrets: Vec<String>` annotated
    `#[serde(default)]`. Keep both `Deserialize` and
    `Serialize` derives (pi-5 N-4 / live capabilities.rs).
  - `crates/rafaello-core/src/lock/grant.rs` — extend
    `GrantEnv` (currently lines 66-70) with
    `pub allow_secrets: Vec<String>` annotated
    `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
  - `crates/rafaello-core/src/compile.rs::effective_grant`
    (lines 248-312) — extend the existing
    `for bundle in grant.bundles.values()` loop to also
    accumulate `allow_secrets`; dedup with the same
    pattern as `env.pass`. Add `allow_secrets:
    Vec<String>` to `EffectiveGrant.env`.
  - `crates/rafaello-core/src/scrubber.rs::strip` —
    change signature from
    `pub fn strip(env_pass: &[String],
    i_know_what_im_doing: bool) -> Vec<String>` to
    `pub fn strip(env_pass: &[String], allow_secrets:
    &[String], i_know_what_im_doing: bool) ->
    Vec<String>`. Retention rule: a name in `env_pass`
    is retained iff (a) `i_know_what_im_doing == true`,
    OR (b) it does not match `SECRET_PATTERNS`, OR (c)
    it appears in `allow_secrets` (case-sensitive exact
    match).
  - `crates/rafaello-core/src/compile.rs` (~line 191) —
    update the single caller to
    `scrubber::strip(&eff.env.pass,
    &eff.env.allow_secrets,
    entry.flags.i_know_what_im_doing)`.
  - `crates/rafaello-core/src/validate/mod.rs` — add
    `ValidationError::AllowSecretsInvalidName { name }`
    and `ValidationError::AllowSecretsReservesCoreName
    { name }` variants. Wire two explicit `validate::lock`
    checks: every entry in any bundle's `allow_secrets`
    must match `^[A-Za-z_][A-Za-z0-9_]*$`; no entry may
    overlap `RESERVED_ENV_VARS`. The
    `reject_reserved` step (which only checks `pass` and
    `set`) is **not** sufficient for `allow_secrets` per
    pi-4 M-4.
- **Why.** Scope §OP6 / §A11. The bundled `rfl-openai`
  plugin needs to accept `LITELLM_API_KEY` (a `*_KEY`-pattern
  name) without forcing operators into
  `flags.i_know_what_im_doing`'s scary red marker. The
  `allow_secrets` opt-in is the narrow, per-name path.
  Lands in c06 — before openai (c32+) — so the openai
  manifest and lock fixtures in c34 reference live
  schema fields.
- **Depends on.** c01.
- **Acceptance.** Tests (all in
  `rafaello-core/tests/`):
  - `manifest_env_capabilities_allow_secrets_parses.rs` —
    parse a manifest with `[capabilities.default.env]
    allow_secrets = ["LITELLM_API_KEY", "OPENAI_API_KEY"]`;
    assert round-trip through `Manifest::parse` ↔
    `to_string` preserves the list.
  - `lock_env_grant_allow_secrets_parses_and_serialises.rs`
    — same for the lock-side `GrantEnv`.
  - `compile_effective_grant_unions_allow_secrets_across_bundles.rs`
    — a lock with `allow_secrets` in `bundles.default` and
    `bundles.send-mail` produces a unioned+deduped list in
    the `EffectiveGrant.env`.
  - `scrubber_strip_honours_allow_secrets_for_listed_names.rs`
    — `env_pass = ["LITELLM_API_KEY"]`,
    `allow_secrets = ["LITELLM_API_KEY"]`,
    `i_know_what_im_doing = false` → retained.
  - `scrubber_strip_strips_unlisted_secrets_when_allow_secrets_present.rs`
    — `env_pass = ["LITELLM_API_KEY", "RANDOM_API_KEY"]`,
    `allow_secrets = ["LITELLM_API_KEY"]` → only
    `LITELLM_API_KEY` retained; `RANDOM_API_KEY` stripped.
  - `compile_passes_allow_secrets_into_scrubber.rs` —
    end-to-end via `compile_plugin` against a fixture
    lock; assert the compiled `EnvPlan.pass` matches the
    scrubber's expected output.
  - `validate_lock_rejects_invalid_allow_secrets_name_shape.rs`
    — `allow_secrets = ["1bad"]` → `AllowSecretsInvalidName`.
  - `validate_lock_rejects_reserved_name_in_allow_secrets.rs`
    — `allow_secrets = ["RFL_BUS_FD"]` →
    `AllowSecretsReservesCoreName`.
  - `scrubber_reject_reserved_unchanged_for_seven_core_names.rs`
    (pi-1 B-8 / scope §M1.1) — assert `scrubber::reject_reserved`
    still rejects each of the live seven reserved names
    (`RFL_BUS_FD`, `RFL_PLUGIN`, `RFL_HELPER_FD`,
    `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`,
    `RFL_PRIVATE_STATE_DIR`, `RFL_PROVIDER_ID`) in both
    `env.pass` and `env.set`; assert m5a adds **no new
    names** to `RESERVED_ENV_VARS` (the list size remains
    7).
  - Existing m4 tests that touch `scrubber::strip`
    indirectly continue passing (no semantic change for
    callers that pass an empty `allow_secrets` slice).
  - `cargo build --workspace` green; `cargo doc --workspace
    --no-deps` warning-free. m4's full test suite passes.
- **Size.** medium-to-large. ~300 LoC (40 LoC schema + 80
  LoC scrubber rule + ~30 LoC effective-grant + ~80 LoC
  validator + ~70 LoC tests). Unsplittable cutover; commit
  body cites m0 c08 precedent.

---

## Phase B — m1 lock-side namespace tightening

Scope §M1.2. One commit, closes m4 §2.6 / m3 §2.7.

### c07 — feat(rafaello-core::validate): `check_lock_publish_topic` unknown-namespace rejection

- **What.** Scope §M1.2. Extend
  `crates/rafaello-core/src/validate/mod.rs` —
  `check_lock_publish_topic` currently accepts any
  grammatically valid topic in `entry.grant.publishes`; the
  broker rejects unknown top-level segments at runtime, but
  the lock validator is silent. Add the compile-time check:
  top-level segment of each publish entry must be one of
  `core`, `provider`, `plugin`, or `frontend`. Add a new
  `m1::ValidateError::PublishUnknownNamespace { topic: String,
  top: String }` variant. Deeper-segment shape
  (`provider.<id>.x`, `plugin.<topic-id>.x`,
  `frontend.<id>.x`) is unchanged: existing
  `PublishOnReservedNamespace` / `PublishOnFrontendNamespace`
  / `ProviderNamespaceMismatch` arms continue to fire for
  shape violations within the known namespaces.
- **Why.** Closes m4 retro §2.6 / m3 retro §2.7. The wire
  surface m5a introduces (`core.session.confirm_*`,
  `frontend.tui.confirm_answer`, `frontend.tui.slash_command`,
  `core.session.command_result`) is the natural moment to
  tighten because the install-time trifecta refusal also runs
  in the same V3 pass.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `lock_validate_publish_unknown_namespace_rejected.rs` —
    a lock with `publishes = ["weird.topic.here"]` fails
    `validate::lock` with `PublishUnknownNamespace { top:
    "weird" }`.
  - `lock_validate_publish_evil_top_segment_rejected.rs` —
    `publishes = ["frontends.tui.x"]` (typo'd plural)
    rejected.
  - `lock_validate_publish_known_namespaces_accepted.rs` —
    `publishes = ["core.x", "provider.foo.x",
    "plugin.id_abc.x", "frontend.tui.x"]` accepted (each
    against its existing shape rule).
  - m1's existing validator tests still pass; m4 fixtures
    that build valid locks still compile.
- **Size.** small.

---

## Phase C — Audit log infrastructure

Scope §AL. Land the audit table + writer early so gate (Phase
F), slash (Phase G), and install (Phase J) wire it inline at
land-time per the tests-with-code rule.

### c08 — feat(rafaello-core::audit): `audit_events` SQLite table + `AuditWriter`

- **What.** Scope §AL1 + §AL2. New module
  `crates/rafaello-core/src/audit/mod.rs` exposing:
  - SQLite migration adding the table
    ```sql
    CREATE TABLE audit_events (
        seq        INTEGER PRIMARY KEY AUTOINCREMENT,
        at         TEXT NOT NULL,
        kind       TEXT NOT NULL,
        request_id TEXT,
        payload    TEXT NOT NULL
    );
    ```
    Migration runs at session-store open. The migration
    plugs into the existing `${PROJECT_ROOT}/.rafaello/state/`
    SQLite database from m3 — no new database file.
  - `pub struct AuditWriter { conn: Arc<Mutex<Connection>>
    }` with a single API
    `pub fn record(&self, kind: AuditKind, request_id:
    Option<&JsonRpcId>, payload: &serde_json::Value) ->
    Result<i64, AuditError>` returning the inserted `seq`.
  - `pub enum AuditKind` with every variant scope §AL1
    lists (gate kinds, slash kinds, install kinds), each
    serialised as a `&'static str` matching scope's `kind`
    column verbatim.
  - The connection is shared with m3's session store via
    the existing `Arc<SessionController>` pool — c08 hands
    out the `AuditWriter` from a new
    `SessionController::audit_writer(&self) ->
    Arc<AuditWriter>` accessor that lazily initialises the
    writer on first call.
- **Why.** Scope §AL1-§AL3. The audit log is a **passive
  sink** (no bus topic — scope §AL3 + decision A6). Reading
  is via raw SQLite in m5a; an `rfl audit` read CLI is m6.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `audit_table_migration_creates_audit_events.rs` —
    open a fresh session store; assert the table exists
    with the four columns.
  - `audit_writer_record_returns_monotonic_seq.rs` —
    insert three rows; assert `seq` is `1, 2, 3` in
    insertion order (i.e. `seq_monotonic_per_session`
    contract from AL4).
  - `audit_writer_record_persists_payload_json.rs` —
    insert with `payload = json!({"foo": "bar"})`; read
    back with raw SQLite; assert the JSON round-trips.
  - `audit_writer_record_persists_request_id_optional.rs`
    — one row with `Some(id)`, one with `None`; both
    persist correctly.
  - m3 / m4 tests using the session store continue
    passing (no breaking change to existing tables; the
    new `audit_events` table is additive).
- **Size.** small.

---

## Phase D — Sink-class consumer + broker outstanding-dispatched map

Scope §Si + §OM. Two commits; both add small data
structures on top of existing m4 surfaces.

### c09 — feat(rafaello-core::sinks): `SinkClass` enum + `CompiledPlugin` accessors

- **What.** Scope §Si1 + §Si2 + §Si3.
  - Extend `crates/rafaello-core/src/sinks.rs` with:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SinkClass {
        Network,
        VcsPush,
        Mail,
        WorkspaceWrite,
        Other(String),
    }
    impl SinkClass {
        pub fn parse(s: &str) -> Self { /* network/vcs_push/mail/workspace_write match; else Other */ }
    }
    ```
  - Extend `crates/rafaello-core/src/compile.rs::CompiledPlugin`
    with three accessor methods. **The underlying
    `Vec<String>` storage in
    `bindings.tool_meta.<n>.sinks`,
    `compile.rs::ToolMeta.sinks`, and the m1 validator's
    acceptance set is unchanged** (pi-1 M-8 — no
    cross-crate cutover). Only the parser is layered over:
    - `pub fn tool_sinks(&self, name: &str) ->
      Option<&[String]>`
    - `pub fn tool_sink_classes(&self, name: &str) ->
      Vec<SinkClass>` — maps each stored string through
      `SinkClass::parse`.
    - `pub fn tool_always_confirm(&self, name: &str) ->
      bool`.
- **Why.** Scope §Si. The gate (Phase F) consumes
  `tool_sink_classes` to decide `gate_required` and
  forwards the classes verbatim into the
  `ConfirmRequestPayload.summary` for TUI display. Storage
  stays `Vec<String>` because m5b's taint-matching layer
  matches on the *string* sink class names from manifests
  and locks; promoting storage to the enum prematurely
  forces an m5b refactor.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `tool_meta_with_sinks_drives_gate_decision.rs` — a
    `CompiledPlugin` with
    `tool_meta["send-mail"].sinks = ["mail"]` returns
    `vec![SinkClass::Mail]` from `tool_sink_classes`, and
    `tool_sinks` returns the underlying string slice
    `&["mail".to_string()]`.
  - `tool_sink_classes_maps_unknown_string_to_other.rs` —
    `sinks = ["exec"]` → `vec![SinkClass::Other("exec".into())]`.
  - `tool_always_confirm_reads_compiled_value.rs` — true
    and false manifests both round-trip.
- **Size.** small.

### c10 — feat(rafaello-core::bus): broker `outstanding_dispatched` map + atomic intake check

- **What.** Scope §OM1 + §OM2 + §OM3. **Unsplittable
  cutover**: the populator
  (`publish_for_tool_dispatch`) and the consumer
  (`handle_plugin_publish` for `tool_result`) are coupled
  at the broker-state level — splitting them leaves a
  window where the populator is dead code or the
  consumer reads an empty map, either of which fails
  per-commit green-bar.
  - `crates/rafaello-core/src/bus.rs` — extend
    `BrokerState` with
    `outstanding_dispatched: BTreeMap<CanonicalId,
    BTreeMap<JsonRpcId, OutstandingDispatch>>`.
    `OutstandingDispatch` carries
    `{request_id: JsonRpcId, dispatched_at: Instant}` (the
    instant is unused in m5a but stored for an m6 metrics
    hook).
  - Inside `Broker::publish_for_tool_dispatch`: in the same
    critical section that hands the event to the fittings
    transport, insert
    `(target_canonical, event.request_id) ->
    OutstandingDispatch { request_id, dispatched_at:
    Instant::now() }`.
  - Inside `Broker::handle_plugin_publish` for topic suffix
    `tool_result`:
    - extract publisher canonical from
      `Publisher::Plugin { canonical, .. }`;
    - extract `id = event.in_reply_to[0]`;
    - check
      `outstanding_dispatched[canonical].contains_key(&id)`;
      if absent → return
      `BrokerError::StaleRequestId { canonical, id }`
      (m4 already has the variant). The publish is
      rejected before fan-out, before re-emit, before any
      subscriber sees the event;
    - if present → `remove` the entry before fan-out so a
      duplicate `tool_result` from the same plugin fails
      the next time.
  - Add `#[cfg(test)]` accessor on `BrokerState` for
    `outstanding_dispatched_count(canonical: &CanonicalId)
    -> usize` so c10's tests can observe the map shape
    directly.
- **Why.** Scope §OM. Closes m4 retro §5.1 / pi-3 M-2 / m4
  §"Out of scope" carryover. Atomic intake check is the
  invariant — a duplicate or routed-to-other-plugin
  tool_result must never reach the agent loop or the gate.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`
    — plugin A publishes `tool_result` citing an id
    nothing was dispatched for; broker rejects with
    `StaleRequestId`.
  - `broker_plugin_tool_result_in_reply_to_routed_to_other_plugin_rejected.rs`
    — id N was dispatched to plugin A; plugin B
    publishing `tool_result` citing N fails closed.
  - `broker_plugin_tool_result_duplicate_publish_rejected.rs`
    — plugin A publishes twice with the same id; the
    second publish fails at intake with `StaleRequestId`
    (the first drained the entry).
  - `broker_plugin_tool_result_race_two_concurrent_publishes.rs`
    — spawn two tasks publishing `tool_result` with the
    same id concurrently from the same plugin; assert
    exactly one succeeds, exactly one fails with
    `StaleRequestId`.
  - `broker_outstanding_dispatched_populated_by_publish_for_tool_dispatch.rs`
    — `publish_for_tool_dispatch` followed by the
    `#[cfg(test)]` accessor shows count = 1.
- **Size.** medium. ~150 LoC handler + ~100 LoC tests.

---

## Phase E — Confirmation topics + ACL + `ConfirmState` + reemit arm

Scope §CT + §CT0 + §CG1a (shared type) + §CT5 (reemit arm).
Four commits.

### c11 — feat(rafaello-core::bus): confirm topic constants + suffix-list extensions

- **What.** Scope §CT0 + §CT1 + §CT2 + §CT3.
  - `crates/rafaello-core/src/bus.rs` — add the three
    topic literals as constants (or via a new
    `crates/rafaello-core/src/bus/topics.rs` module if a
    separate file is preferred — pick the lighter
    in-place option unless pi argues for hoisting):
    `pub const CORE_SESSION_CONFIRM_REQUEST: &str =
    "core.session.confirm_request";`
    `pub const CORE_SESSION_CONFIRM_REPLY: &str =
    "core.session.confirm_reply";`
    `pub const FRONTEND_TUI_CONFIRM_ANSWER: &str =
    "frontend.tui.confirm_answer";`
    `pub const FRONTEND_TUI_SLASH_COMMAND: &str =
    "frontend.tui.slash_command";`
    `pub const CORE_SESSION_COMMAND_RESULT: &str =
    "core.session.command_result";`
    `pub const CORE_SESSION_CONFIRM_RESOLVED: &str =
    "core.session.confirm_resolved";` (pi-1 M-1 — new
    bus-visible resolution signal for short-circuited
    confirms; gate publishes one per short-circuit
    resolution so the TUI's overlay queue can drop the
    pending entry. Distinct from `confirm_reply` so the
    gate's CG4 doesn't observe its own signal.)
  - Extend `REQUEST_ID_REQUIRED_SUFFIXES` (live `bus.rs:17-22`,
    stores last-segment-without-dot) to append the six
    literal strings `"confirm_request"`, `"confirm_reply"`,
    `"confirm_answer"`, `"slash_command"`,
    `"command_result"`, `"confirm_resolved"`. **No
    leading dot** (pi-3 M-5 — the list compares against
    `topic.rsplit('.').next()`).
  - Extend the `in_reply_to`-mandatory enforcement
    (security RFC §7.2.6 row 5) to
    `frontend.tui.confirm_answer`,
    `core.session.confirm_reply`,
    `core.session.command_result`, and
    `core.session.confirm_resolved`. **Not**
    `frontend.tui.slash_command` (root event). Cardinality
    is exactly one on the four mandatory-`in_reply_to`
    suffixes; the broker rejects missing or
    multi-element arrays with the existing
    `InvalidInReplyTo` variant.
- **Why.** Scope §CT2 + §CT3 + §SL0 implication 2. The
  broker has to reject malformed publishes at intake before
  the gate or re-emit pipeline run.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `broker_publish_core_session_confirm_request_missing_request_id_rejected.rs`
  - `broker_publish_core_session_confirm_reply_missing_request_id_rejected.rs`
  - `broker_publish_frontend_tui_confirm_answer_missing_request_id_rejected.rs`
  - `broker_publish_frontend_tui_slash_command_missing_request_id_rejected.rs`
  - `broker_publish_core_session_command_result_missing_request_id_rejected.rs`
  - `broker_publish_frontend_tui_confirm_answer_missing_in_reply_to_rejected.rs`
  - `broker_publish_frontend_tui_confirm_answer_in_reply_to_too_many_rejected.rs`
    (cardinality exactly one)
  - `broker_publish_core_session_confirm_reply_missing_in_reply_to_rejected.rs`
  - `broker_publish_core_session_command_result_missing_in_reply_to_rejected.rs`
  - `broker_publish_core_session_confirm_reply_in_reply_to_too_many_rejected.rs`
    (pi-1 M-2 — exact-one cardinality, parallel to the
    `confirm_answer` test).
  - `broker_publish_core_session_command_result_in_reply_to_too_many_rejected.rs`
    (pi-1 M-2 — same).
  - `broker_publish_core_session_confirm_resolved_missing_request_id_rejected.rs`
    (pi-1 M-1 — new short-circuit signal topic).
  - `broker_publish_core_session_confirm_resolved_missing_in_reply_to_rejected.rs`
  - `broker_publish_core_session_confirm_resolved_in_reply_to_too_many_rejected.rs`
  - `broker_publish_core_session_confirm_resolved_wire_shape_positive.rs`
    (pi-2 M-1 + pi-3 M-1) — broker-level test only:
    a direct synthetic publish via
    `Broker::publish_core_with_taint("core.session.confirm_resolved",
    ...)` is accepted; an internal subscriber observes
    envelope `request_id` = the published ULID,
    payload `request_id` matches `in_reply_to[0]`, and
    no broker-side rejection fires. **The
    gate-publisher positive (does the short-circuit
    code path actually publish this event with the
    right `reason`?) lives in c24** alongside the
    gate's short-circuit logic — pi-3 M-1.

  **`core.session.confirm_resolved` wire contract**
  (pi-2 M-1, mirrors §CT0):

  | Topic                            | Envelope `request_id`                                  | Payload `request_id`                          | Envelope `in_reply_to`                              | Stale / duplicate / late                                                                       |
  |----------------------------------|--------------------------------------------------------|-----------------------------------------------|-----------------------------------------------------|-----------------------------------------------------------------------------------------------|
  | `core.session.confirm_resolved`  | fresh ULID = the resolution event's id (gate-allocated) | the confirmation correlation id of the resolved confirm | exactly `[payload.request_id]` (the resolved confirm id) | n/a — gate publishes once per short-circuit resolution; the TUI tracks correlation by `payload.request_id` and drops the matching queue entry; receiving a `confirm_resolved` for an unknown confirm id is silently ignored by the TUI (operator-visible cost: nil) |

  The gate does NOT subscribe internally to
  `confirm_resolved` (only the TUI consumes it), so
  there is no self-handling race with CG4.
- **Size.** small.

### c12 — feat(rafaello): extend `tui` frontend ACL with `confirm_answer` + `slash_command` publishes

- **What.** Scope §CT4. Extend the `BrokerAcl` construction
  site in `crates/rafaello/src/lib.rs:308-315` (the m4
  `frontend.tui.user_message` insertion) to also grant the
  `tui` frontend principal publish authority over
  `frontend.tui.confirm_answer` and
  `frontend.tui.slash_command`. The change is two lines on
  the `publish_topics` list.
- **Why.** Scope §CT + §SL. The TUI's overlay (Phase H)
  publishes `confirm_answer` on user key events; its input
  parser (Phase G) publishes `slash_command` when the user
  types `/...`. Both need an ACL grant or the broker
  rejects with `PublishOutsideGrant`.
- **Depends on.** c01, c11.
- **Acceptance.** Tests in `rafaello/tests/`:
  - `frontend_publish_confirm_answer_accepted_by_broker.rs`
    — fabricate a well-formed `confirm_answer` publish
    from a fake TUI principal; assert the broker emits a
    `BusEvent` without `PublishOutsideGrant`.
  - `frontend_publish_slash_command_accepted_by_broker.rs`
    — same shape.
  - `frontend_publish_unknown_topic_rejected.rs` —
    `frontend.tui.evil_topic` publish fails with
    `PublishOutsideGrant`.
- **Size.** small.

### c13 — feat(rafaello-core::gate): `ConfirmState` shared type + atomic methods

- **What.** Scope §CG1a. New module
  `crates/rafaello-core/src/gate/mod.rs` (file) with an
  inner `crates/rafaello-core/src/gate/confirm_state.rs`
  containing the named shared map. Types:
  ```rust
  pub struct ConfirmState {
      inner: parking_lot::Mutex<BTreeMap<JsonRpcId, HeldEntry>>,
  }
  enum HeldEntry {
      Active { held: HeldConfirmation, session_grant_requested: bool },
      ResolvedByAnswer,
      TimedOut,
  }
  pub struct HeldConfirmation {
      pub tool_request: BusEvent,
      pub deadline: std::time::Instant,
      pub dispatch_target: CanonicalId,
  }
  pub enum PriorOutcome { Held, Duplicate, Late, Unknown }
  #[derive(Debug, thiserror::Error)]
  pub enum MarkError { #[error("entry not active")] NotActive }
  ```
  Atomic methods (each acquires the mutex once, mutates,
  drops) matching scope §CG1a's table verbatim:
  - `reserve(confirm_id: JsonRpcId, held:
    HeldConfirmation)` — insert
    `Active { held, session_grant_requested: false }`;
    panic on collision (gate-allocated `confirm_id` is
    fresh per call).
  - `is_held(&self, confirm_id: &JsonRpcId) -> bool` —
    true iff `Active`.
  - `mark_session_grant_requested(&self, confirm_id:
    &JsonRpcId) -> Result<(), MarkError>` — flip the
    flag on `Active` without consuming; idempotent
    re-call; `Err(NotActive)` on `ResolvedByAnswer` /
    `TimedOut` / absent.
  - `try_resolve(&self, confirm_id: &JsonRpcId) ->
    Option<(HeldConfirmation, bool)>` — `Active →
    ResolvedByAnswer`, return inner; else `None`.
  - `try_take_for_timeout(&self, confirm_id:
    &JsonRpcId) -> Option<HeldConfirmation>` — `Active →
    TimedOut`, return inner `HeldConfirmation`
    (`session_grant_requested` discarded — call timed out
    before dispatch); else `None`.
  - `prior_outcome(&self, confirm_id: &JsonRpcId) ->
    PriorOutcome` — read-only classifier (Active → Held;
    ResolvedByAnswer → Duplicate; TimedOut → Late;
    absent → Unknown).
  - **No `re_hold` method** (pi-3 M-3 — malformed
    validation in re-emit happens before any state
    mutation).
  - **No `take_for_publish` method** (pi-3 B-1 — round-3
    name; renamed to `try_resolve` and moved from re-emit
    to gate's CG4).
- **Why.** Scope §CG1a. The named shared structure pi-2
  M-5 required: re-emit (Phase E c14) and the gate (Phase
  F) clone an `Arc<ConfirmState>` and share a single
  coherent map. Round-4 ownership inversion (pi-3 B-1)
  put consumption on the gate's side; round-5 pi-4 B-1
  added `mark_session_grant_requested` to keep grant
  creation on the gate too.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `confirm_state_reserve_then_try_resolve_returns_held_with_false_flag.rs`
  - `confirm_state_try_resolve_twice_returns_none_second_time.rs`
  - `confirm_state_try_take_for_timeout_then_try_resolve_returns_none.rs`
  - `confirm_state_try_resolve_then_try_take_for_timeout_returns_none.rs`
  - `confirm_state_prior_outcome_distinguishes_held_duplicate_late_unknown.rs`
  - `confirm_state_concurrent_try_resolve_and_try_take_for_timeout_exactly_one_winner.rs`
    — spawn two tasks per `confirm_id`; loop 100x; assert
    invariant.
  - `confirm_state_no_re_hold_method_exists.rs` —
    compile-time / type-level assertion that the symbol
    does not exist (a `trybuild` or `proc-macro` check;
    or fall back to a doc-test that fails compilation if
    `re_hold` is re-introduced).
  - `confirm_state_mark_session_grant_requested_flips_flag_without_consuming.rs`
  - `confirm_state_mark_session_grant_requested_twice_is_idempotent.rs`
  - `confirm_state_mark_session_grant_requested_on_resolved_returns_mark_error.rs`
  - `confirm_state_mark_session_grant_requested_on_timed_out_returns_mark_error.rs`
- **Size.** medium. ~200 LoC type + ~150 LoC tests.

### c14 — feat(rafaello-core::reemit): canonicalise `frontend.tui.confirm_answer` → `core.session.confirm_reply`

- **What.** Scope §CT5. Extend
  `crates/rafaello-core/src/reemit/mod.rs::ReemitRouter`
  with **two new optional fields** (pi-1 B-2 — keep
  `ReemitRouter::new` backward-compatible so no live
  reemit test or `run_chat` call site breaks; the
  catalog of existing call sites is documented at
  `crates/rafaello/src/lib.rs::run_chat` and across
  `rafaello-core/tests/reemit_*`):
  ```rust
  pub struct ReemitRouter {
      // existing m4 fields...
      confirm_state: Option<Arc<ConfirmState>>,
      audit:         Option<Arc<AuditWriter>>,
  }
  impl ReemitRouter {
      // unchanged from m4
      pub fn new(broker: Broker, acl: BrokerAcl,
                 active_provider: CanonicalId,
                 shutdown_rx: watch::Receiver<bool>) -> Self { ... }
      // NEW (c14): builder for m5a confirm_answer arm
      pub fn with_confirm_state_and_audit(
          mut self,
          confirm_state: Arc<ConfirmState>,
          audit: Arc<AuditWriter>,
      ) -> Self {
          self.confirm_state = Some(confirm_state);
          self.audit = Some(audit);
          self
      }
  }
  ```
  The `confirm_answer` arm checks
  `self.confirm_state.as_ref()` and `self.audit.as_ref()`
  at dispatch time; if either is `None`, the arm
  **drops the event silently with a tracing warning**
  (m5a's m4-shaped paths continue to work without
  confirm wiring during the gradual rollout between
  c14 and c38). When `Some`, the algorithm runs in
  full.
  c38 (`rfl chat` orchestration) calls
  `.with_confirm_state_and_audit(...)` on the
  `ReemitRouter` instance during run_chat construction
  — no other live call site changes in c14.

  Add a fourth `confirm_answer` arm to the per-direction
  dispatch (m4 already has `user_message`,
  `tool_request`, `assistant_message`/`tool_result`
  arms — see `reemit/mod.rs`). Algorithm exactly
  matches scope §CT5 steps 1-7:
  1. Envelope `request_id` present (broker already
     checked); payload `request_id` is a valid ULID.
  2. `in_reply_to` is exactly one entry **and equals
     `payload.request_id`** — fail with
     `ReemitError::ConfirmAnswerCorrelationMismatch`;
     **never touches `ConfirmState`**.
  3. Answer string is one of `"allow" | "deny" |
     "always_allow_session"`; else →
     `ReemitError::ConfirmAnswerMalformed`; audit
     `confirm_malformed`; **never touches
     `ConfirmState`**.
  4. Classify via `prior_outcome(payload.request_id)`:
     `Held` → continue; `Duplicate` → audit
     `confirm_duplicate`, drop; `Late` → audit
     `confirm_late`, drop; `Unknown` → audit
     `confirm_unknown`, drop. `prior_outcome` is
     read-only.
  5. Special-case `always_allow_session`: call
     `state.mark_session_grant_requested(payload.request_id)`.
     `Ok(())` → rewrite outbound
     `confirm_reply.payload.answer = "allow"`, proceed to
     step 6. `Err(MarkError::NotActive)` → re-read
     `prior_outcome`; audit `confirm_late` if Late,
     `confirm_duplicate` if Duplicate, `confirm_unknown`
     if Unknown; **do NOT publish
     `core.session.confirm_reply`** (pi-5 M-1 — matches
     CT0's late-answer contract). The held call has
     already been resolved by the race winner.
     If the answer was `"allow"` or `"deny"`, this step
     is skipped; forward verbatim.
  6. Synthesise canonical taint `[{source: "user",
     detail: None}]` per security RFC §7.2.2.
  7. Publish `core.session.confirm_reply` via
     `Broker::publish_core_with_taint` with payload
     `{request_id: <correlation_id>, answer: "allow" |
     "deny"}` (Stream A §5.6 schema verbatim — answer is
     two-value enum; pi-3 B-2) and envelope
     `in_reply_to = [<correlation_id>]`.
  The grant-creation + `grant_added` audit happen in
  CG4 (c22); re-emit's audit handle (added in this
  commit via the builder above) is used **only** for
  the four classification kinds
  `confirm_malformed` / `confirm_duplicate` /
  `confirm_late` / `confirm_unknown`. Re-emit does
  NOT gain `UserGrants` handles.
- **Why.** Scope §CT5 + pi-3 B-1 + pi-3 B-2 + pi-4 B-1 +
  pi-5 M-1. Re-emit's role is canonicalisation +
  validation; the gate consumes the held entry on
  observation of the canonical `confirm_reply` (c22).
- **Depends on.** c08, c11, c13.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `reemit_frontend_confirm_answer_to_core_session_confirm_reply.rs`
    — happy `"allow"` path; assert canonical envelope
    fields per scope §CT5 step 7.
  - `reemit_confirm_answer_payload_id_neq_envelope_id.rs`
    — Stream A semantics; payload `request_id` is the
    correlation id, envelope is a fresh ULID.
  - `reemit_confirm_answer_in_reply_to_neq_payload_request_id_rejected.rs`
  - `reemit_confirm_answer_unknown_request_id_audit_logged.rs`
  - `reemit_confirm_answer_late_after_timeout_audit_logged.rs`
  - `reemit_confirm_answer_duplicate_audit_logged.rs`
  - `reemit_confirm_answer_malformed_string_does_not_touch_confirm_state.rs`
    — asserts `prior_outcome == Held` both before and
    after the malformed answer (pi-3 M-3).
  - `reemit_confirm_answer_synthesises_user_taint.rs`
  - `reemit_confirm_answer_always_allow_session_marks_state_and_emits_allow.rs`
    — outbound payload carries `answer = "allow"`; flag
    flipped.
  - `reemit_confirm_answer_always_allow_session_races_timeout_drops_without_reply.rs`
    — between step-4 `prior_outcome == Held` and
    step-5 `mark_session_grant_requested`, CG5's
    timeout fires; `MarkError::NotActive` returned;
    re-emit audits `confirm_late` and emits **no**
    `core.session.confirm_reply`. Use a test seam that
    swaps `prior_outcome` and the `mark_session_grant_requested`
    impl into a synchronised pair to deterministically
    interleave the two calls.
  - `reemit_confirm_answer_always_allow_session_does_not_consume_held_entry.rs`
    — after `mark_session_grant_requested` returns Ok,
    a subsequent `try_resolve` returns
    `Some((held, true))`.
  - `reemit_confirm_answer_without_confirm_state_warns_and_drops.rs`
    (pi-2 M-5) — construct a `ReemitRouter` via
    `ReemitRouter::new(...)` only (no
    `.with_confirm_state_and_audit(...)` call). Publish
    a well-formed `frontend.tui.confirm_answer`; assert
    no `core.session.confirm_reply` is emitted, a
    `tracing::warn!` is captured with the
    "confirm_state-not-wired" message, and m4-shaped
    callers see no behaviour change on the other
    re-emit arms (user_message / tool_request /
    assistant_message / tool_result still re-emit
    normally). This is the transitional-drop contract
    that keeps c14 backward-compatible with the
    live reemit tests + run_chat call sites until
    c38 wires `confirm_state`.

---

## Phase F — `user_grants`

Scope §UG. Two commits. Lands before the gate (Phase H)
because CG2 step 4 (grant-match passthrough) and CG4's
always_allow_session grant creation both need
`UserGrants`.

### c15 — feat(rafaello-core::user_grants): `UserGrants` + `GrantMatcher` + API

- **What.** Scope §UG1 + §UG2 (matcher data structures
  only; jsonschema template validation lands in c16) +
  §UG3 + §UG4 + §UG5 (subset of tests that don't need
  jsonschema).
  - New module `crates/rafaello-core/src/user_grants.rs`.
  - Types:
    ```rust
    pub struct UserGrants {
        entries: BTreeMap<GrantId, UserGrant>,
    }
    pub struct UserGrant {
        pub tool: String,
        pub plugin: CanonicalId,
        pub matcher: GrantMatcher,
        pub added_at: DateTime<Utc>,
        pub source: GrantSource,
    }
    pub enum GrantMatcher {
        Any,
        Structural { template: serde_json::Value },
    }
    pub enum GrantSource { SlashCommand, AlwaysAllowSession }
    pub struct GrantId(pub Ulid);
    ```
    `ProviderProposal` is **not** constructed in m5a
    (scope §UG3 — m5b/m6 territory).
  - API (scope §UG4):
    - `UserGrants::add(grant: UserGrant) -> GrantId`
    - `UserGrants::list(&self) -> Vec<(GrantId,
      &UserGrant)>`
    - `UserGrants::revoke(id: GrantId) -> Result<(),
      RevokeError>`
    - `UserGrants::matches(&self, plugin:
      &CanonicalId, tool: &str, args: &Value) ->
      Option<GrantId>` — `GrantMatcher::Any` always
      matches; `Structural { template }` requires
      structural-subset match: every key/value in the
      template is present and deep-equal in `args`;
      arrays compared element-wise; missing template key
      → no match; extra args keys → still match (subset
      semantics).
  - Pin the `plugin` field check inside `matches`: a
    grant for plugin A does NOT authorise plugin B even
    if the tool name matches (scope §UG1 second
    paragraph).
- **Why.** Scope §UG. The gate (Phase H) consumes
  `matches` for the grant-bypass passthrough; the slash
  handler (Phase G c18) and the gate's CG4
  always_allow_session path (c22) consume `add` /
  `revoke` / `list`.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `user_grants_any_matcher_matches_every_invocation_of_tool.rs`
  - `user_grants_structural_matcher_subset_match.rs`
  - `user_grants_structural_matcher_value_mismatch.rs`
  - `user_grants_structural_matcher_missing_key.rs`
  - `user_grants_structural_matcher_extra_args_still_matches.rs`
  - `user_grants_plugin_pinned_does_not_match_other_plugin.rs`
  - `user_grants_revoke_removes_entry.rs`
  - `user_grants_revoke_unknown_id_errors.rs`
- **Size.** small-to-medium. ~150 LoC + ~150 LoC tests.

### c16 — feat(rafaello-core::user_grants): `jsonschema` template validation at `/grant` time

- **What.** Scope §UG2 step 1 + §UG5 schema tests + §TP2
  manifest schema referencing.
  - Add a new method
    `UserGrants::compile_template(tool: &str,
    user_args: BTreeMap<String, serde_json::Value>,
    grant_match_schema: Option<&serde_json::Value>) ->
    Result<GrantMatcher, GrantCompileError>`. If
    `user_args` is empty AND no schema is declared →
    `GrantMatcher::Any`. If `user_args` is non-empty:
    build the template object; if `grant_match_schema`
    is `Some(s)`, validate the template object against
    `s` via the `jsonschema` workspace dep; on failure
    return `GrantCompileError::SchemaMismatch { diag:
    <jsonschema diagnostic string> }`. Schema absent →
    accept the template as-is (scope §UG2 step 1
    bullets).
  - **Lock-pinned**: the schema is whatever the gate /
    slash handler passed in from
    `bindings.tool_meta[tool].grant_match` at gate
    construction; manifest changes mid-session are not
    re-read (m1 lock-correspondence precedent, m4
    §"Lock-correspondence claim"). Caller responsibility
    to read it once and hand it in; no inner read.
  - Add `rafaello-core` dependency on the `jsonschema`
    workspace alias (declared in c02).
- **Why.** Scope §UG2 step 1 + pi-1 M-5 resolution. The
  schema validates the user's matcher *template* at
  `/grant` time (Stream A §7.2.4's "uses the matcher
  schema declared in the tool's manifest"); runtime
  matching stays cheap structural-subset (no per-call
  schema compile — scope §"Out of scope" item 5).
- **Depends on.** c02, c15.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `user_grants_template_validated_against_lock_schema_at_grant_time.rs`
    — schema `{type: "object", properties: {to: {type:
    "string"}}, required: ["to"]}`; template
    `{"to": "alice@example.com"}` → `Structural`.
  - `user_grants_template_schema_mismatch_rejected.rs`
    — same schema; template `{"to": 42}` →
    `SchemaMismatch`.
  - `user_grants_template_no_schema_declared_accepted.rs`
    — no schema; template `{"to": "x"}` →
    `Structural` (accepted as-is).
  - `user_grants_revoke_during_pending_confirmation_does_not_short_circuit.rs`
    (pi-1 M-7) — install a `gate_grant_match`
    short-circuit, then revoke the grant; the in-flight
    call (`gate_grant_match` already audited) is **not**
    retroactively un-allowed; the next call prompts
    again. (Exercised via a small in-process harness;
    no real gate task required because the test only
    asserts the `UserGrants` API surface plus an audit
    log row.)
- **Size.** small. ~80 LoC method + ~120 LoC tests.

---

## Phase G — Slash commands (bus-mediated)

Scope §SL + §SL0. Three commits: TUI parser →
core handler → TUI rendering of command_result.

### c17 — feat(rafaello-tui): `SlashCommand::parse` + publish on `frontend.tui.slash_command`

- **What.** Scope §SL1 + §SL2 + §SL5 (TUI-side tests).
  - New module `crates/rafaello-tui/src/slash.rs`.
    `pub fn parse(input: &str) -> SlashCommand` where
    `SlashCommand = { command: SlashKind, args:
    serde_json::Value }`. `SlashKind = Grant |
    ListGrants | Revoke | Unknown`. Lines beginning
    with `/` are parsed: `/grant <tool> <k>=<v>...` →
    `Grant { tool, plugin?, template:
    BTreeMap<String, Value> }`; `/grants list` →
    `ListGrants`; `/revoke <id>` → `Revoke { grant_id:
    String }`; anything else with `/` prefix →
    `Unknown { raw: <input> }`. Parse failures
    (malformed `k=v`) become `Unknown` too so core's
    audit log captures the attempt verbatim.
  - Extend the TUI's input handler in
    `crates/rafaello-tui/src/lib.rs` (m4's
    user_message-publish site is the model): when the
    user submits a line, check if it starts with `/`;
    if yes, publish on `frontend.tui.slash_command`
    (with mandatory envelope `request_id = Ulid::new()`,
    no envelope `in_reply_to` — root event per §SL0);
    if no, fall through to the existing user_message
    publish path. The slash payload schema matches
    scope §SL2 verbatim:
    ```json
    {
      "command": "grant" | "list_grants" | "revoke" | "unknown",
      "args": { ... }                              // shape per command
    }
    ```
- **Why.** Scope §SL1 + pi-1 B-1. The TUI is a separate
  process and cannot mutate core's `UserGrants` directly;
  slash commands become typed bus events.
- **Depends on.** c11, c12.
- **Acceptance.** Tests in `rafaello-tui/tests/`:
  - `tui_slash_grant_publishes_typed_event.rs`
  - `tui_slash_grant_with_args_template_object.rs`
  - `tui_slash_unknown_command_publishes_unknown_kind.rs`
  - `tui_user_message_starting_with_slash_not_published.rs`
    — input `/foo` does NOT generate a
    `frontend.tui.user_message`; produces a
    `frontend.tui.slash_command` instead.
- **Size.** small. ~100 LoC parser + ~120 LoC tests.

### c18 — feat(rafaello-core::slash): core handler subscribed to `slash_command`, mutates `UserGrants`, publishes `command_result`

- **What.** Scope §SL3 + §SL0.
  - New module `crates/rafaello-core/src/slash.rs`.
    `pub struct SlashHandler { broker, acl: Arc<BrokerAcl>,
    user_grants, audit, tool_grant_match_schemas:
    BTreeMap<String, serde_json::Value> }`.
  - Subscribe via `Broker::subscribe_internal` to
    `frontend.tui.slash_command`. For each event:
    1. Validate the payload shape; malformed →
       publish `core.session.command_result {ok: false,
       kind: "unknown", message: "malformed payload",
       ...}`; audit `slash_unknown`; envelope
       `in_reply_to = [slash_request_id]`.
    2. `command == "grant"`: extract tool, plugin
       (optional), template map. **Default plugin
       resolution** (pi-4 B-2 — round-3 wrongly used
       `session.tool_owner[tool]` which is empty in the
       no-conflict case): when the user omits the
       `plugin` field, look up the canonical via
       `BrokerAcl::tool_route(tool) -> Option<&CanonicalId>`
       (a small lookup wrapper over the existing live
       `BrokerAcl.tool_routes: BTreeMap<String,
       CanonicalId>` field — m4 already populates this
       at compile time as the dispatch-target source of
       truth, regardless of `tool_owner` state). If the
       lookup returns `None`, publish `command_result
       {ok: false, kind: "grant", message: "no plugin
       provides tool '<tool>'"}` and audit
       `grant_failed`. On success, call
       `UserGrants::compile_template(tool, template,
       lock_schema)` (c16); on success insert via
       `UserGrants::add`; audit `grant_added` with
       `payload.source: "SlashCommand"`; publish
       `command_result {ok: true, kind: "grant",
       details: {grant_id}}`.
    3. `command == "list_grants"`: enumerate via
       `UserGrants::list`; publish `command_result
       {ok: true, kind: "list_grants", details:
       {entries: [...]}}`; audit `grant_list`.
    4. `command == "revoke"`: parse `grant_id`; call
       `UserGrants::revoke`; on `Ok` publish
       `command_result {ok: true, kind: "revoke"}` +
       audit `grant_revoked`; on unknown id publish
       `command_result {ok: false, ...}`.
    5. `command == "unknown"`: no mutation; publish
       `command_result {ok: false, kind: "unknown",
       message: "unknown command: <raw>"}`; audit
       `slash_unknown`.
  - All `command_result` publishes use
    `Broker::publish_core_with_taint` with envelope
    `in_reply_to = [slash_request_id]` (cardinality
    exactly one — c11 enforces). Payload has no
    `request_id` field (scope §SL0 implication 1 —
    correlation lives in the envelope alone).
- **Why.** Scope §SL3 + pi-1 B-1 + pi-2 M-2. Core is the
  sole mutator of `UserGrants`.
- **Depends on.** c08, c11, c15, c16.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `core_slash_command_grant_handler_inserts_user_grant.rs`
  - `core_slash_command_grant_template_schema_mismatch_publishes_ok_false.rs`
  - `core_slash_command_grant_no_schema_template_accepted.rs`
  - `core_slash_command_revoke_unknown_id_publishes_ok_false.rs`
  - `core_slash_command_list_grants_returns_entries.rs`
  - `core_slash_command_malformed_payload_rejected.rs`
  - `core_slash_command_publishes_command_result_correlated.rs`
    — assert `in_reply_to = [slash_request_id]`.
  - `audit_log_records_grant_added_with_plugin_pin.rs`
    — `payload.plugin == <canonical>`, distinct from
    just-tool-name authorisation (§UG1).
  - `core_slash_command_grant_resolves_default_plugin_via_tool_route.rs`
    (pi-4 B-2) — fixture lock has empty
    `session.tool_owner` and one mailcat plugin
    providing `send-mail`; submit `/grant send-mail
    to=alice@example.com` with no `plugin` field;
    assert the inserted `UserGrant.plugin ==
    "local:mailcat@0.0.0"` (resolved via
    `BrokerAcl::tool_route("send-mail")`); assert
    `command_result {ok: true}` is published.
  - `core_slash_command_grant_unknown_tool_publishes_ok_false.rs`
    (pi-4 B-2) — submit `/grant nonexistent` against
    a lock where no plugin claims the tool; assert
    `command_result {ok: false, message: "no plugin
    provides tool 'nonexistent'"}` and `grant_failed`
    audit row.
- **Size.** medium. ~200 LoC handler + ~250 LoC tests.

### c19 — feat(rafaello-tui): render `core.session.command_result` inline as transient text

- **What.** Scope §SL4.
  - The TUI's bus subscriber already covers
    `core.session.**` (m4); no new ACL grant required.
  - Add a small per-pending-command map in the TUI
    keyed by `slash_request_id` (the envelope id of
    the issued slash command); on incoming
    `command_result` whose `in_reply_to[0]` matches,
    render the result as a **transient inline text
    line** above the input area, distinct from
    conversation entries. Not persisted as
    `core.session.entry.finalized`.
  - The user can scroll back to see prior command
    results until the TUI is restarted.
- **Why.** Scope §SL4. Slash commands are not
  conversation history; the result is a transient
  callout.
- **Depends on.** c17, c18.
- **Acceptance.** Tests in `rafaello-tui/tests/`:
  - `tui_renders_command_result_inline.rs` — drive a
    `command_result` event with the matching
    `in_reply_to`; capture the TUI's render buffer;
    assert the text appears in the callout region but
    is **not** in the entry list.
  - `tui_command_result_for_unknown_correlation_ignored.rs`
    — a `command_result` with no matching pending
    slash is silently ignored (does not crash the
    TUI).
- **Size.** small.

---

## Phase H — Confirmation gate (the largest single module)

Scope §CG1-§CG8. Five commits: gate skeleton + decision
logic (passthrough/grant-match) → hold path → CG4
(allow + deny + grant creation) → CG5 timeout →
CG7 multi-pending + short-circuit. CG6 (agent-loop
pivot) is bundled into c38 per pi-1 B-3 — see Phase M.

### c20 — feat(rafaello-core::gate): `ConfirmationGate` skeleton + CG2 passthrough / grant-match

- **What.** Scope §CG1 + §CG2 steps 1-4 + §CG8 partial.
  - New struct in `crates/rafaello-core/src/gate/mod.rs`:
    `pub struct ConfirmationGate { broker: Arc<Broker>,
    user_grants: Arc<RwLock<UserGrants>>, audit:
    Arc<AuditWriter>, state: Arc<ConfirmState>,
    compiled: BTreeMap<CanonicalId, CompiledPlugin> }`.
    Constructor takes the four `Arc`s + the compiled
    map; `pub fn spawn(self) -> tokio::task::JoinHandle<()>`
    starts the task.
  - The task subscribes internally (`Broker::subscribe_internal`)
    to `core.session.tool_request`. For each event:
    1. Resolve `dispatch_target` from the payload
       (m4-populated field); look up the
       `CompiledPlugin`.
    2. Compute `gate_required = !sinks.is_empty() ||
       always_confirm` using c09's
       `tool_sink_classes` / `tool_always_confirm`
       accessors.
    3. `!gate_required` → call
       `broker.publish_for_tool_dispatch(...)`
       directly with the held event's fields; audit
       `gate_passthrough`.
    4. `gate_required` → look up
       `user_grants.read().matches(dispatch_target,
       tool, args)`; if `Some(_)` →
       `publish_for_tool_dispatch`; audit
       `gate_grant_match`.
    Hold path (CG2 step 5) lands in c21; CG4 / CG5 in
    c22 / c23.
- **Why.** Scope §CG1 + §CG2. Skeleton + the two
  pass-through arms first so the agent-loop pivot
  bundled in c38 has a target.
- **Depends on.** c08, c09, c13, c15.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `gate_passes_through_non_sink_tool_request.rs` —
    a `tool_request` whose target's `sinks = []` and
    `always_confirm = false` is dispatched directly;
    audit row `gate_passthrough` recorded.
  - `gate_passes_through_user_grant_match.rs` — a
    `tool_request` whose target has sinks BUT a
    matching `UserGrant` is dispatched; audit row
    `gate_grant_match`.
  - `gate_construction_subscribes_internally.rs` —
    after `spawn`, a published
    `core.session.tool_request` reaches the gate's
    handler (observed via a test hook on the
    `ConfirmationGate` that exposes "events seen so
    far").
- **Size.** medium. ~180 LoC + ~180 LoC tests.

### c21 — feat(rafaello-core::gate): CG2 hold path + `reserve` + publish `confirm_request`

- **What.** Scope §CG2 step 5 + §CG3 + §CG8 partial.
  - In the `ConfirmationGate` task's
    `core.session.tool_request` handler, when neither
    passthrough arm fires:
    - allocate `confirm_id = JsonRpcId::String(Ulid::new().to_string())`;
    - call `state.reserve(confirm_id, HeldConfirmation
      { tool_request: event.clone(), deadline:
      Instant::now() + Duration::from_secs(60),
      dispatch_target: canonical })`;
    - build the `ConfirmRequestPayload` per scope §CG3:
      ```json
      {
        "request_id": "<confirm_id>",
        "what": "tool_call",
        "summary": "<tool> via <plugin> — sinks: [<class>, ...]",
        "details": {
          "tool_call_id": "<held tool_request.request_id>",
          "tool": "<tool>",
          "args": {...},
          "sinks": ["mail", ...],
          "always_confirm": false,
          "taint": [...]
        },
        "default": "deny",
        "ttl_seconds": 60
      }
      ```
      `payload.request_id == confirm_id == envelope.request_id`
      (scope §CT0 row 1 — gate-allocated confirm
      correlation id; on `.confirm_request` envelope and
      payload coincide). `details.tool_call_id` carries
      the held `tool_request.request_id` (the tool-call
      correlation, separate id space — scope §CT0
      implication 4).
    - publish via
      `Broker::publish_core_with_taint("core.session.confirm_request",
      payload, taint = [{source: "system", detail:
      "confirm_request"}], in_reply_to =
      Some(vec![event.request_id.clone()]))`;
    - audit `confirm_request` with `request_id =
      Some(confirm_id)`.
  - Spawn a `tokio::time::sleep_until(deadline)` task
    that calls back into the gate's `handle_timeout`
    method (CG5 — landing in c23); the JoinHandle is
    stored on the gate's internal task map for
    abort-on-resolve.
- **Why.** Scope §CG2 step 5 + §CG3.
- **Depends on.** c20.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `gate_holds_sink_tool_request_pending_confirm.rs`
    — a `tool_request` whose target declares
    `sinks = ["mail"]` and no matching grant: the
    gate publishes `core.session.confirm_request`;
    the held entry is `Active`; audit row
    `confirm_request` recorded; no
    `plugin.<id>.tool_request` is dispatched.
  - `gate_confirm_request_payload_matches_scope_cg3_shape.rs`
    — assert every field in §CG3's schema, with
    `payload.request_id == envelope.request_id` and
    `details.tool_call_id == held.tool_request.request_id`.
- **Size.** small-to-medium.

### c22 — feat(rafaello-core::gate): CG4 allow + deny + grant creation + `synthesise_deny_tool_result`

- **What.** Scope §CG4 + §CG4a + §CG8 partial.
  - Subscribe (in the existing gate task, second
    arm) to `core.session.confirm_reply`. For each
    event:
    1. `correlation_id = envelope.in_reply_to[0]` (=
       payload `request_id`).
    2. Call `state.try_resolve(correlation_id)`.
    3. `None` → CG4 step 1 race-loser path: audit
       `confirm_resolved_after_timeout`; drop.
    4. `Some((held, session_grant_requested))` →
       dispatch on `payload.answer`:
       - `"allow"`:
         - if `session_grant_requested == true`:
           extract `(tool, args)` from
           `held.tool_request.payload`; insert
           **exactly** `UserGrant { plugin:
           held.dispatch_target, tool, matcher:
           GrantMatcher::Structural { template:
           args.clone() }, added_at: Utc::now(),
           source: AlwaysAllowSession }` (pi-1 B-7 —
           scope §CG4 mandates the verbatim
           `Structural::from_args(args)` shape; no
           `compile_template` call, no schema
           validation, no `Any` fallback. The user
           already saw the args in the modal, so the
           grant covers exactly those args via
           structural-subset matching). Audit
           `grant_added` with `payload.source:
           "AlwaysAllowSession"`.
         - publish the held tool_request via
           `Broker::publish_for_tool_dispatch(canonical:
           held.dispatch_target, payload:
           held.tool_request.payload, request_id:
           held.tool_request.request_id, in_reply_to:
           held.tool_request.in_reply_to, taint:
           held.tool_request.taint)`.
         - audit `confirm_allowed_with_session_grant`
           if `session_grant_requested`, else
           `confirm_allowed`.
       - `"deny"`: build via
         `gate::synthesise_deny_tool_result(&held,
         DenyReason::UserDenied)` (§CG4a); call
         `broker.publish_core_with_taint(...)` with
         the result. Audit `confirm_denied`. The
         `session_grant_requested` flag is ignored
         on the deny path.
  - **`synthesise_deny_tool_result` helper** in
    `crates/rafaello-core/src/gate/mod.rs`:
    ```rust
    pub fn synthesise_deny_tool_result(
        held: &HeldConfirmation,
        reason: DenyReason,
    ) -> PublishCoreArgs { /* exactly scope §CG4a shape */ }
    pub enum DenyReason { UserDenied, ConfirmTimeout }
    ```
    The returned `PublishCoreArgs.payload` is
    `{ok: false, error: "user_denied" | "confirm_timeout",
    content: ""}`; envelope `request_id =
    Some(JsonRpcId::from(Ulid::new()))`; `in_reply_to
    = Some(vec![held.tool_request.request_id.clone()])`;
    `taint = Some(vec![TaintEntry { source: "system",
    detail: Some(<reason str>.to_string()) }])`.
  - The CG5 timeout path in c23 calls the same helper
    with `DenyReason::ConfirmTimeout`.
- **Why.** Scope §CG4 + §CG4a + pi-3 B-1 (ownership
  inversion: gate consumes the held entry) + pi-4 B-1
  (gate creates the grant when re-emit flagged
  `session_grant_requested`).
- **Depends on.** c13, c14, c15, c20, c21 (pi-1 B-7 —
  c16 dep dropped; CG4 no longer calls `compile_template`).
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `gate_dispatches_on_allow.rs` — the held
    `tool_request` is dispatched via
    `publish_for_tool_dispatch`; audit row
    `confirm_allowed`.
  - `gate_synthesises_deny_tool_result_with_pinned_shape.rs`
    — assert §CG4a wire shape exactly (`request_id
    Some`, `in_reply_to` matches held id, `taint`
    non-empty with `source: "system"`, payload has
    `ok: false`, `error`, `content`).
  - `gate_synthetic_deny_persists_through_agent_loop.rs`
    — publish the synthetic deny through
    `publish_core_with_taint`; assert the existing
    `agent/mod.rs::handle_tool_result` records a
    `tool_result` entry with `ok: false`.
  - `gate_always_allow_session_creates_grant_and_dispatches.rs`
    — `try_resolve` returns `Some((held, true))`;
    the gate inserts a `UserGrant` (assert via
    `user_grants.list()`); audits `grant_added` then
    `confirm_allowed_with_session_grant`; dispatches
    the held call.
  - `gate_cg4_race_with_timeout_audits_confirm_resolved_after_timeout.rs`
    — pre-set the entry to `TimedOut` via
    `try_take_for_timeout`; deliver a
    `confirm_reply`; assert
    `confirm_resolved_after_timeout` audit; no
    dispatch.
- **Size.** medium-to-large. ~250 LoC handler + helper
  + ~250 LoC tests.

### c23 — feat(rafaello-core::gate): CG5 60s timeout via `try_take_for_timeout`

- **What.** Scope §CG5 + §CG8 partial. The
  per-`reserve` `sleep_until(deadline)` task scheduled
  in c21 calls a new
  `ConfirmationGate::handle_timeout(confirm_id)`
  method:
  1. `state.try_take_for_timeout(confirm_id)`;
  2. on `Some(held)` → publish the synthetic deny
     `core.session.tool_result` via the c22 helper
     with `reason = DenyReason::ConfirmTimeout`;
     audit `confirm_timeout`;
  3. on `None` → the answer arm won the race; exit
     silently (no audit, no publish).
  Tests use `tokio::time::pause` + `advance` per m3's
  idiom.
- **Why.** Scope §CG5 + security RFC §5.6 `default =
  "deny"`.
- **Depends on.** c22.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `gate_times_out_to_deny_after_60s.rs` — paused
    time advanced past 60s; assert the synthetic
    `tool_result.error == "confirm_timeout"`; audit
    row `confirm_timeout`.
  - `gate_timeout_after_resolve_is_noop.rs` —
    `try_resolve` runs first; the timeout task's
    `try_take_for_timeout` returns `None`; no second
    audit or publish.
  - `gate_per_held_timeout_independent.rs` — two
    held entries with staggered deadlines; advance
    past the first; assert only the first times out;
    advance past the second; assert the second
    times out.
- **Size.** small.

### c24 — feat(rafaello-core::gate): CG7 multi-pending + `always_allow_session` short-circuit

- **What.** Scope §CG7 + §CG8 partial.
  - On every `UserGrants::add` (called from CG4's
    always_allow_session path), the gate walks its
    held map; for every `Active` entry whose
    `(plugin, tool, args)` newly matches the new
    grant via `UserGrants::matches`:
    - `state.try_resolve(entry_id)`; on `Some((held,
      _))`: dispatch via
      `publish_for_tool_dispatch`; audit
      `gate_grant_match_short_circuit`; **publish
      `core.session.confirm_resolved`** (pi-1 M-1)
      with payload `{request_id: <confirm_id>,
      reason: "grant_short_circuit"}` and envelope
      `in_reply_to = [<confirm_id>]`. This is the
      bus-visible signal the TUI subscribes to in c25
      for queue pruning; the gate does NOT subscribe
      internally to `confirm_resolved` (only the TUI
      consumes it), so there's no self-handling
      race. Distinct topic from `confirm_reply` to
      keep the CG4 subscriber's contract narrow.
  - **Per-held-entry timeout** is independent: each
    `Active` entry has its own
    `sleep_until(deadline)` task and they don't
    interfere.
  - **Hold queue is unbounded** in m5a; one entry per
    held `tool_request.request_id`; the gate makes no
    capacity ruling.
- **Why.** Scope §CG7 + pi-1 M-3. Granting bulk
  `always_allow_session` mid-prompt should not
  require the operator to re-answer the queued
  prompts.
- **Depends on.** c22.
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `gate_two_concurrent_sink_calls_serialise_in_tui.rs`
    — two `tool_request` events arrive in quick
    succession; both are held; assert `held.len() ==
    2`; resolve one and confirm the other stays
    `Active`.
  - `gate_grant_short_circuits_pending_held_entry.rs`
    — hold entry A then call `UserGrants::add` with
    a matcher that covers A; assert A is resolved
    (via the audit row
    `gate_grant_match_short_circuit`) and
    dispatched.
  - `gate_grant_short_circuit_publishes_confirm_resolved.rs`
    (pi-1 M-1) — subscribe an internal observer to
    `core.session.confirm_resolved`; assert one
    event is published with `reason:
    "grant_short_circuit"`, payload
    `request_id == <confirm_id>`, envelope
    `in_reply_to == [<confirm_id>]`.
  - `gate_duplicate_answer_after_grant_short_circuit_audit_logged.rs`
    (pi-2 B-4 — short-circuit calls `try_resolve`
    which transitions `Active → ResolvedByAnswer`;
    `prior_outcome` then classifies a subsequent
    answer as `Duplicate`, NOT `Late`. `Late` is the
    `TimedOut` arm only — see c13's `PriorOutcome`
    classifier). After the short-circuit fires, a
    stale `confirm_answer` for A arrives; re-emit
    reads `prior_outcome == Duplicate` and audits
    `confirm_duplicate` via the c14 pipeline.
- **Size.** small-to-medium.

> **(Round-1 c25 — agent-loop pivot — bundled into c38
> per pi-1 B-3.)** Splitting the agent-loop's direct
> dispatch removal from the `rfl chat` gate construction
> creates a per-commit double-dispatch window
> (constructing the gate without removing direct dispatch
> means each `tool_request` reaches the plugin twice).
> The pivot now lands inside c38's unsplittable cutover.

---

## Phase I — TUI confirmation overlay

Scope §TUI. Two commits.

### c25 — feat(rafaello-tui): `InputMode::ConfirmOverlay` + key handling + publish answer

- **What.** Scope §TUI1.
  - Extend the TUI's input mode enum in
    `crates/rafaello-tui/src/lib.rs` with
    `ConfirmOverlay { confirm_id: JsonRpcId, summary:
    String, details: ConfirmDetails, ttl_remaining:
    u32, queued_count: u32 }`. Entered when the
    TUI's bus subscriber observes a
    `core.session.confirm_request` event.
  - While in this mode the input line is
    non-editable; key events drive the answer:
    - `y` / `a` / `Enter` → publish
      `frontend.tui.confirm_answer` with payload
      `{request_id: confirm_id, answer: "allow"}`
      and envelope `in_reply_to: [confirm_id]`;
    - `n` / `d` / `Esc` → answer `"deny"`;
    - `s` → answer `"always_allow_session"`.
  - The envelope `request_id` is a fresh
    `Ulid::new()` (TUI-generated), distinct from the
    payload `request_id` (scope §CT0 row 2
    implication 2).
  - After publishing, the TUI clears the overlay
    mode and observes the resulting
    `core.session.confirm_reply` (which the gate
    will produce) to confirm; the overlay does not
    persist any entry (scope §TUI deletion of
    `RenderNode::Confirm` per pi-1 M-4).
- **Why.** Scope §TUI1.
- **Depends on.** c11, c12, c17.
- **Acceptance.** Tests in `rafaello-tui/tests/`:
  - `tui_enters_overlay_on_confirm_request.rs`
  - `tui_y_key_publishes_allow_answer.rs`
  - `tui_n_key_publishes_deny_answer.rs`
  - `tui_esc_key_publishes_deny_answer.rs`
  - `tui_s_key_publishes_always_allow_session.rs`
  - `tui_input_blocked_during_overlay.rs`
  - `tui_overlay_exits_on_confirm_reply_via_bus.rs`
  - `tui_overlay_does_not_persist_entry_for_confirm_request.rs`
    — assert no `core.session.entry.finalized` is
    emitted for the modal itself (positive m4
    regression check).
- **Size.** medium. ~150 LoC mode handling + ~200 LoC
  tests.

### c26 — feat(rafaello-tui): overlay rendering + multi-pending queue + ttl countdown

- **What.** Scope §TUI2 + §TUI3 + §TUI4 + §TUI5
  remainder.
  - Render the overlay as a framed area above the
    input line via ratatui's existing pipeline (no
    `RenderNode::Confirm`; the overlay paints
    directly).
  - Overlay shows: summary, args, sinks list, taint
    list (m5a-empty / m5b-populated), TTL countdown
    ticked from a `tokio::time::interval(1s)`. The
    countdown is purely UI; deadline enforcement is
    server-side per §CG5.
  - Maintain a `VecDeque<PendingConfirm>` of
    pending `core.session.confirm_request` events
    whose answer hasn't been published. The current
    overlay corresponds to the queue head; on
    exit (allow/deny/timeout/short-circuit), pop
    the next.
  - Short-circuited entries: the TUI subscribes to
    **`core.session.confirm_resolved`** (pi-1 M-1 —
    the new bus-visible short-circuit signal added in
    c11/c24); on arrival, drop the queue entry whose
    `confirm_id == payload.request_id`. Entries
    resolved by their own answer arm (TUI published
    the answer itself) are dropped on the matching
    `core.session.confirm_reply` (also subscribed).
    The TUI tracks both topics in parallel for queue
    pruning.
  - The `+N more pending` badge surfaces in the
    overlay frame when `queued_count > 0`.
- **Why.** Scope §TUI2-§TUI4 + §CG7 TUI side.
- **Depends on.** c25.
- **Acceptance.** Tests in `rafaello-tui/tests/`:
  - `tui_two_concurrent_confirm_requests_serialise.rs`
    — assert head shown, `+1 more pending`
    rendered.
  - `tui_short_circuited_pending_overlay_silently_dropped_on_confirm_resolved.rs`
    (pi-1 M-1) — drive a `core.session.confirm_resolved`
    event matching a queued pending; assert the queue
    drops it without rendering the overlay.
  - `tui_overlay_ttl_countdown_renders_seconds.rs` —
    capture the render buffer; assert "60s" then
    `tokio::time::advance` 10s; assert "50s".
  - `tui_tool_result_for_confirmed_call_renders_beneath_call_row.rs`
    — TUI4: when the corresponding
    `core.session.tool_result` arrives, the existing
    m3 entry-update path renders the result row
    beneath the call row.
- **Size.** medium.

---

## Phase J — Install-time trifecta refusal

Scope §Tr. Three commits.

### c27 — feat(rafaello): `rfl install --fixture` subcommand + validate-via-trifecta + `allow_secrets` warnings

- **What.** Scope §Tr1 + §Tr2 + §Tr4 (subset of tests).
  New `rfl install` subcommand on the existing `rfl`
  binary (`crates/rafaello/src/main.rs`). Signature:
  `rfl install --fixture <PACKAGE_DIR>
  [--lock <LOCK_PATH>] [--i-know-what-im-doing]
  [--allow-credential-paths]`.
  Algorithm — implement exactly scope §Tr1 steps 1-12:
  1. Parse manifest:
     `Manifest::parse(&fs::read_to_string(<PACKAGE_DIR>/rafaello.toml))`.
  2. `manifest::validate_with_package(...)`.
  3. Resolve canonical id (`local:<name>@<version>`
     for `--fixture`).
  4. Compute `manifest_digest` + `content_digest`.
  5. Synthesise default `Grant` from
     `[capabilities.default]`.
  6. Construct candidate `PluginEntry`; apply
     `--i-know-what-im-doing` /
     `--allow-credential-paths` flags **before**
     validation.
  7. Merge into existing `Lock::from_toml(...)` or
     fresh `Lock`.
  8. Run `validate::lock(&merged, ...)` (m1's V3
     path); map outcomes:
     - `Ok(())` → proceed.
     - `Err(ValidationError::TrifectaRefused { reads,
       outbound, write })` →
       `InstallError::TrifectaRefused` with the three
       booleans printed on stderr.
     - `Err(ValidationError::CarveOutRefused |
       CarveOutTooLarge)` →
       `InstallError::CarveOutRefused`; stderr
       suggests `--allow-credential-paths`.
     - other `ValidationError` arms →
       `InstallError::Validation`.
  9. Optional `--verbose` pre-validation diagnostic
     call to `trifecta::evaluate(...)`.
  10. **Unused-`allow_secrets` warning** (pi-5 M-2).
      For each `GrantEnv.allow_secrets` name in each
      bundle, compute names absent from that bundle's
      effective `env.pass`. For each unused name,
      print exactly one stderr line:
      ```
      warning: unused allow_secrets entry '<name>' (no matching env.pass entry)
      ```
      (Canonical string from §Tr1 step 10; pi-6 N-1
      fold — §OP6 references the same string here.)
      Collect the list for the audit row in step 12.
  11. Write merged lock to `<LOCK_PATH>` (default
      `${PROJECT_ROOT}/rafaello.lock`).
  12. Audit `install_accepted` (or
      `trifecta_overridden` if `--i-know-what-im-doing`)
      with `details.allow_secrets: [...]` when
      non-empty and `details.unused_allow_secrets:
      [...]` from step 10 (empty when every name is
      also in `env.pass`).

  **Audit-database opening at install time** (pi-1 M-3 —
  `rfl install` runs without a `SessionController`).
  c27 adds a new `AuditWriter::open_for_install(project_root:
  &Path) -> Result<Arc<AuditWriter>, AuditError>`
  constructor. It first calls
  `std::fs::create_dir_all(project_root.join(".rafaello/state"))`
  (pi-2 M-2 — `rfl install` may run before any
  `rfl chat` has materialised the state dir, so the
  parent must be created defensively; m3 creates this
  directory inside `SessionController::open`, but
  install bypasses that path). Then opens
  `${PROJECT_ROOT}/.rafaello/state/session.sqlite`
  directly via `rusqlite::Connection::open(...)`,
  runs the `audit_events` migration from c08 (the
  migration is idempotent — `CREATE TABLE IF NOT
  EXISTS`), and wraps the connection in
  `Arc<Mutex<...>>` matching the c08 `AuditWriter`
  shape. **No chat-session lock** is acquired; SQLite
  WAL handles concurrent writers if a `rfl chat`
  session happens to be running. c08's
  `SessionController::audit_writer` path is
  unchanged; the new `open_for_install` constructor
  is the install-side path. `rfl install` calls it
  once at startup and shares the `Arc` across the
  install run.
- **Why.** Scope §Tr1. m1 ships `trifecta::evaluate`;
  m5a turns it on at the install path. Network fetch /
  update / review-UI explicitly out of scope (m6).
- **Depends on.** c06, c08.
- **Acceptance.** Tests in `rafaello/tests/`:
  - `rfl_install_fixture_writes_lock.rs` — happy
    path against a benign `rafaello-readfile`-shaped
    fixture; lock gains the entry with the expected
    digests.
  - `rfl_install_refuses_trifecta_plugin.rs` —
    install a fixture declaring all three trifecta
    dimensions; assert exit non-zero, stderr
    contains `TrifectaRefused` + the three booleans.
  - `rfl_install_accepts_trifecta_plugin_with_override.rs`
    — same manifest + `--i-know-what-im-doing`;
    install succeeds; lock entry's
    `flags.i_know_what_im_doing == true` (asserted
    via a `#[cfg(test)]` accessor exposing the
    candidate at validation time).
  - `rfl_install_warns_on_unused_allow_secrets_entry.rs`
    (pi-5 M-2) — install a fixture whose
    `allow_secrets = ["A", "B"]` and
    `env.pass = ["A"]`; capture stderr; assert it
    contains `warning: unused allow_secrets entry
    'B' (no matching env.pass entry)`; assert the
    lock writes succeed; assert the audit row's
    `details.unused_allow_secrets == ["B"]`.
  - `audit_install_accepted_records_allow_secrets_list.rs`
    — install with non-empty `allow_secrets`; audit
    row carries `details.allow_secrets:
    ["LITELLM_API_KEY", ...]`.
  - `audit_install_accepted_records_unused_allow_secrets.rs`
    — the stderr-warning case is also recorded.
  - `audit_records_trifecta_override_at_install.rs`
  - `audit_records_install_refused_with_three_booleans.rs`
  - `audit_records_install_accepted_for_happy_path.rs`
  - `audit_writer_open_for_install_creates_state_dir.rs`
    (pi-2 M-2) — fresh tempdir with no
    `.rafaello/state/` subtree; call
    `AuditWriter::open_for_install(&tempdir)`;
    assert the directory exists, the SQLite file
    exists, the `audit_events` table is queryable,
    and a subsequent `record` call succeeds.
- **Size.** medium. ~250 LoC subcommand + ~250 LoC
  tests.

### c28 — feat(rafaello): `rfl status` subcommand + override marker + `allow_secrets` yellow marker

- **What.** Scope §Tr3 + §OP6 `rfl status` surface.
  - New `rfl status` subcommand: reads
    `${PROJECT_ROOT}/rafaello.lock`; prints one row
    per plugin with canonical id, bindings summary,
    active flags.
  - Plugins with `flags.i_know_what_im_doing == true`
    rendered red ANSI; non-TTY → `[OVERRIDE]`
    prefix.
  - Plugins whose any-bundle
    `GrantEnv.allow_secrets` is non-empty rendered
    with a yellow ANSI `explicit secret: <names>`
    suffix; non-TTY → `[SECRET: <names>]`. **Yellow,
    not red** — distinct from `[OVERRIDE]`'s
    red panic-inducing marker (scope §OP6 + §A11
    UX choice).
- **Why.** Scope §Tr3 + §OP6. Security RFC §7.1's
  "loud surfacing" requirement for overrides; §A11's
  distinct-marker UX for the bundled `rfl-openai`'s
  `allow_secrets`.
- **Depends on.** c06, c27.
- **Acceptance.** Tests in `rafaello/tests/`:
  - `rfl_status_prints_red_for_override_flag.rs` —
    TTY capture; assert ANSI red escape sequence
    around the canonical id.
  - `rfl_status_prints_override_prefix_for_non_tty.rs`
    — pipe stdout to a buffer; assert `[OVERRIDE]`
    prefix; no ANSI codes.
  - `rfl_status_yellow_marker_for_allow_secrets_lock_entry.rs`
    — install a fixture with non-empty
    `allow_secrets`; `rfl status` TTY shows yellow
    `explicit secret: ...` suffix.
  - `rfl_status_non_tty_secret_suffix_for_allow_secrets.rs`
    — non-TTY shows `[SECRET: ...]`.
  - `rfl_install_status_shows_red_for_override.rs`
    (pi-1 M-5 — moved from c40 demo-negatives where
    c28 wasn't a dep). Install a trifecta plugin with
    `--i-know-what-im-doing`; run `rfl status`; assert
    the entry is rendered with the red ANSI marker.
    Lives here because the `rfl status` surface lands
    in this commit.
- **Size.** small.

### c29 — test(rafaello): one-hop trifecta refusal + transitive-not-chased integration tests

- **What.** Scope §Tr4 tests + §"Demo bar" negative 3.
  - `rafaello/tests/rfl_install_refuses_one_hop_outbound_via_other_plugin.rs`
    — install plugin A into a lock that already
    has plugin B (network-open) subscribing to A's
    published topic; assert install of A fails
    with `TrifectaRefused` (the one-hop direct
    check from security RFC §7.1.1).
  - `rafaello/tests/rfl_install_does_not_chase_transitive_outbound.rs`
    — install A into a lock with B and C where
    A→B→C and only C is network-open and B does
    NOT subscribe to A's publish; assert install
    of A **accepts** (`decisions.md` row 11 — the
    transitive non-feature; one-hop direct only).
    Audit log records `install_accepted`.
- **Why.** Scope §Tr4 + §"Demo bar" negative 3.
  Asserts a deliberate non-feature; pi will want
  this isolated.
- **Depends on.** c27.
- **Acceptance.** Both tests pass. `cargo test
  --workspace` green.
- **Size.** small.

---

## Phase K — `rafaello-mailcat` sink-declaring fixture

Scope §TP. One commit; the surface is small enough not
to split (manifest + openrpc.json + bin + tests).

### c30 — feat(rafaello-mailcat): manifest + `openrpc.json` + `grant_match` schema + bin implementation

- **What.** Scope §TP1 + §TP2 + §TP3 + §TP4.
  - **Manifest** at
    `rafaello/crates/rafaello-mailcat/rafaello.toml`
    and a copy under
    `rafaello/fixtures/m5a-locks/rafaello-mailcat/`:
    ```toml
    schema = 1
    name = "mailcat"
    version = "0.0.0"
    entry = "bin/rfl-mailcat"
    rafaello = ">=0.1, <0.2"
    load = "eager"

    [provides]
    tools = ["send-mail"]

    [provides.tool.send-mail]
    sinks = ["mail"]
    always_confirm = false
    grant_match = "schemas/send-mail-grant.json"
    ```
  - **`grant_match` schema** at
    `crates/rafaello-mailcat/schemas/send-mail-grant.json`:
    ```json
    {
      "type": "object",
      "properties": { "to": {"type": "string"} },
      "required": ["to"]
    }
    ```
  - **`openrpc.json`** sibling at the manifest's
    parent directory (pi-4 M-2). m1 already requires
    every plugin to ship one; m5a's
    `ToolSchemaCatalog::build` reads it. Declares one
    method matching `provides.tools`:
    ```json
    {
      "openrpc": "1.2.6",
      "info": { "title": "mailcat", "version": "0.0.0" },
      "methods": [
        {
          "name": "send-mail",
          "params": [
            { "name": "to",      "required": true,  "schema": { "type": "string" } },
            { "name": "subject", "required": false, "schema": { "type": "string" } },
            { "name": "body",    "required": false, "schema": { "type": "string" } }
          ],
          "result": { "name": "ok", "schema": { "type": "object" } }
        }
      ]
    }
    ```
  - **Bin implementation** in
    `src/bin/rfl_mailcat.rs`: subscribes to its own
    `plugin.<topic-id>.tool_request`; publishes
    `plugin.<topic-id>.tool_result`. Behaviour:
    appends the request payload to a file named
    `mailcat.log` under the per-plugin private state
    dir (auto-granted — `decisions.md` row 16/37).
    No actual SMTP. Returns
    `{ok: false, error: "missing 'to' field"}` if
    the request omits `to`.
  - **Package `bin/rfl-mailcat` shim file** (pi-1 B-4 /
    m4 c20 precedent). Live `manifest::validate_with_package`
    canonicalises the `entry` path and requires it to
    exist on disk. The shim is a POSIX shell stub
    committed at
    `rafaello/crates/rafaello-mailcat/bin/rfl-mailcat`
    (and the fixture copy under
    `rafaello/fixtures/m5a-locks/rafaello-mailcat/bin/rfl-mailcat`)
    with `chmod +x` and body `#!/bin/sh\nexec "$@"`.
    Never executed at runtime (the real bin path used
    by `rfl chat` points at the cargo-built bin under
    the workspace target dir); exists only to satisfy
    the validator. Same shim ships under both fixture
    variant directories from B9 above.
- **Why.** Scope §TP. The sink-declaring fixture used
  by every gate integration test and the demo-bar
  positive.
- **Depends on.** c05.
- **Acceptance.** Tests in
  `rafaello-mailcat/tests/`:
  - `mailcat_appends_to_log_on_tool_request.rs`
  - `mailcat_returns_error_on_missing_to_field.rs`
  - `mailcat_manifest_declares_mail_sink.rs` —
    `manifest::parse` succeeds; `provides.tool.send-mail.sinks
    == ["mail"]`.
  - `mailcat_manifest_validate_with_package_succeeds.rs`
    (pi-1 B-9) — assert `manifest::validate_with_package`
    against the openrpc-sibling-present + valid
    `grant_match` path. This is the only openrpc-related
    surface c30 owns; method-vs-tool consistency tests
    move fully to c31 (the row that introduces
    `ToolSchemaCatalog::build`). c30 ships the two
    fixture variants (canonical + typo'd-method) so c31
    can extend them.
  - **Fixture variants shipped here, consumed at c31:**
    `rafaello/fixtures/rafaello-mailcat-good/openrpc.json`
    (the canonical one above) and
    `rafaello/fixtures/rafaello-mailcat-method-typo/openrpc.json`
    (same shape but `methods[0].name = "send-male"` —
    typo). Plus matching manifests under each fixture
    directory. c30 commits both directory trees; no
    consistency assertion runs against them in c30.
- **Size.** small-to-medium. ~150 LoC bin + manifest
  files + ~150 LoC tests.

---

## Phase L — `rfl-openai` provider plugin

Scope §OP. Seven commits: catalog + service supervision
extension → wire client → bus adapter → manifest+lock
fixtures → stub bin → tools_list call → negative matrix.

### c31 — feat(rafaello-core::supervisor): `ToolSchemaCatalog` + `CorePluginService` + `PluginSupervisor` signature extension

- **What.** Scope §OP2 items 1-5.
  - New type
    `crates/rafaello-core/src/supervisor/tool_catalog.rs::ToolSchemaCatalog`
    with:
    ```rust
    pub struct ToolSchemaCatalog { schemas: Vec<ToolSchema> }
    pub struct ToolSchema {
        pub name: String,
        pub description: Option<String>,
        pub parameters_schema: serde_json::Value,
    }
    pub enum ToolCatalogError {
        ToolMissingOpenRpcMethod { canonical: CanonicalId, tool: String },
        OpenRpcParseError { canonical: CanonicalId, source: serde_json::Error },
        // ...
    }
    impl ToolSchemaCatalog {
        pub fn build(
            acl: &BrokerAcl,
            compiled: &BTreeMap<CanonicalId, CompiledPlugin>,
            package_dirs: &BTreeMap<CanonicalId, PathBuf>,
        ) -> Result<Self, ToolCatalogError> { ... }
        pub fn list(&self) -> &[ToolSchema] { ... }
    }
    ```
    `build` reads each plugin's `openrpc.json` from
    `package_dirs[canonical]`, parses it, and
    synthesises one `ToolSchema` per declared tool by
    (a) matching OpenRPC `methods[i].name` against
    `provides.tools` entries (m5a-owned consistency
    check — fail with `ToolMissingOpenRpcMethod` per
    pi-4 M-2 — live `validate_with_package` only
    checks sibling presence + entry resolution +
    `grant_match` path + exec-path syntax, NOT this
    consistency); (b) projecting `methods[i].params`
    into a JSON-Schema object `{type: "object",
    properties: { param.name: param.schema, ... },
    required: [<required>]}`. Surplus methods
    (in openrpc but not in `provides.tools`) are
    accepted silently. **Sinks / `grant_match` /
    `always_confirm` are NOT forwarded** (gate-side
    concerns).
  - New type
    `crates/rafaello-core/src/supervisor/core_service.rs::CorePluginService
    { catalog: Arc<ToolSchemaCatalog> }`. Implements
    the fittings server-side trait with exactly one
    method, `core.tools_list`, whose handler returns
    `{tools: catalog.list().to_vec()}` (cloned).
  - Extend `PluginSupervisor::new` (live
    `supervisor.rs:282-303`) signature from
    `(broker, config)` to
    `(broker, config, tool_catalog:
    Arc<ToolSchemaCatalog>)`. Store the catalog as a
    new field.
  - Extend `build_connection_service`
    (`supervisor.rs:813-826`) to compose the
    `CorePluginService` only for provider connections:
    ```rust
    let core = if self.is_provider(&canonical) {
        Some(CorePluginService { catalog: self.tool_catalog.clone() })
    } else { None };
    ```
    `is_provider(&canonical)` is
    `self.broker.plugin_acl(&canonical).and_then(|a|
    a.provider_id).is_some()` (pi-4 M-1 — live
    `Broker::plugin_acl` at `bus.rs:243`; `PluginAcl`
    carries `provider_id: Option<String>`).
  - Grow `SupervisorConnectionService` with an
    optional third field `core:
    Option<CorePluginService>`. Method-not-found
    fall-through is the natural fittings behaviour
    for non-providers (scope §OP2 item 5).
  - **All live `PluginSupervisor::new` call sites
    updated in this commit** (pi-1 B-5 — signature
    cutover discipline, m0 §4.1). Concretely:
    - `crates/rafaello/src/lib.rs::run_chat`: build
      the real catalog via
      `ToolSchemaCatalog::build(&acl, &compiled_plugins,
      &package_dirs)` (the function already has all
      three inputs as locals after lock compilation)
      and pass `Arc::new(catalog)` as the third arg
      to `PluginSupervisor::new`. This wires the
      catalog at `rfl chat` startup; the rest of the
      orchestration plumbing (gate, UserGrants, etc.)
      lands at c38.
    - All `rafaello-core/tests/supervisor_*` and any
      other `PluginSupervisor::new` test call sites:
      use a new `ToolSchemaCatalog::empty_for_tests()
      -> Arc<Self>` constructor (cfg-gated `any(test,
      feature = "test-fixture")`) that returns an
      empty-schemas catalog. The existing
      `ExtraServiceFactory` test seam is unchanged.
    - The remaining rfl-chat orchestration plumbing
      (gate, UserGrants, slash handler, AuditWriter
      wired into ReemitRouter) lands at c38 — not
      deferred call-site updates, but additive
      construction on top of this commit's catalog
      wiring.
  - **m4 fixture OpenRPC backfill** (pi-2 B-2). Live
    `rafaello/fixtures/rafaello-readfile/openrpc.json`
    currently declares `methods: []` while
    `rafaello/fixtures/rafaello-readfile/rafaello.toml`
    declares `provides.tools = ["read-file"]`. After
    `ToolSchemaCatalog::build` lands here with the
    method-vs-tool consistency check, the readfile
    fixture would fail catalog construction with
    `ToolMissingOpenRpcMethod { tool: "read-file"
    }`. **c31 backfills the readfile openrpc.json in
    the same commit:**
    ```json
    {
      "openrpc": "1.2.6",
      "info": { "title": "readfile", "version": "0.0.0" },
      "methods": [
        {
          "name": "read-file",
          "params": [
            { "name": "path", "required": true,
              "schema": { "type": "string" } }
          ],
          "result": { "name": "ok",
            "schema": { "type": "object" } }
        }
      ]
    }
    ```
    `rafaello-mockprovider`'s manifest declares
    `[provides] provider = "mock"` with no
    `tools = [...]` array, so its empty
    `methods: []` openrpc stays valid (no
    consistency mismatch).
- **Why.** Scope §OP2 (round-4 form). Closes
  pi-3 M-1's data-source ambiguity and pi-4 M-2's
  consistency-check ownership.
- **Depends on.** c01, c30 (pi-1 M-4 — c31 builds the
  catalog from c30's mailcat fixture variants and
  extends c30's openrpc-consistency tests).
- **Acceptance.** Tests in `rafaello-core/tests/`:
  - `tool_schema_catalog_build_from_openrpc_synthesises_parameters_schema.rs`
    — build against a fixture with one tool and one
    openrpc method; assert the synthesised
    `parameters_schema` shape.
  - `tool_schema_catalog_build_errors_when_openrpc_method_missing_for_tool.rs`
    — fixture with `provides.tools = ["send-mail"]`
    but openrpc declares only `"send-mail-typo"`;
    `build` errors with
    `ToolMissingOpenRpcMethod`.
  - `tool_schema_catalog_omits_sinks_grant_match_always_confirm.rs`
    — fixture with all three gate-side fields set;
    assert they are absent from the
    `parameters_schema`.
  - `core_plugin_service_responds_to_core_tools_list_for_provider.rs`
    — spawn a `SupervisorConnectionService` with
    `Some(CorePluginService)`; call
    `core.tools_list`; assert response carries the
    catalog.
  - `core_plugin_service_method_not_found_for_non_provider_plugin.rs`
    — `core: None`; assert `MethodNotFound`.
  - `supervisor_new_accepts_tool_catalog_arg.rs` —
    signature compile check.
  - `mailcat_openrpc_method_matches_provides_tools.rs`
    (pi-1 B-9 — owned by c31, not staged in c30). Build
    `ToolSchemaCatalog` from c30's
    `rafaello/fixtures/rafaello-mailcat-good/`; assert
    success and that the synthesised
    `parameters_schema` covers `to/subject/body`.
  - `mailcat_openrpc_missing_method_for_provides_tools_errors.rs`
    (pi-1 B-9 — owned by c31). Build against c30's
    `rafaello-mailcat-method-typo` variant; assert
    `ToolCatalogError::ToolMissingOpenRpcMethod`.
  - `readfile_fixture_catalog_builds_with_backfilled_openrpc.rs`
    (pi-2 B-2) — build `ToolSchemaCatalog` from
    `rafaello/fixtures/rafaello-readfile/` after this
    commit's openrpc backfill; assert success and
    that the synthesised `parameters_schema` for
    `read-file` carries the `path` param.
  - `mockprovider_fixture_catalog_builds_without_tools.rs`
    (pi-2 B-2) — build against
    `rafaello/fixtures/rafaello-mockprovider/`;
    assert `catalog.list()` is empty (no tools
    advertised) and no error.
  - `rfl_chat_existing_m4_readfile_demo_still_starts.rs`
    in `rafaello/tests/` (pi-2 B-2 regression) — load
    an m4-shaped readfile lock; assert
    `ToolSchemaCatalog::build` succeeds at run_chat
    startup; assert `PluginSupervisor::new` accepts
    the catalog and the existing m4
    `rfl_chat_demo_bar_read_file.rs` flow still
    drives a `read-file` tool call end-to-end (the
    full m4 demo bar continues passing under the
    catalog cutover).

### c32 — feat(rafaello-openai::wire): Chat Completions client + error mapping

- **What.** Scope §OP1 + §OP1a.
  - `crates/rafaello-openai/src/wire.rs` — request /
    response structs per scope §OP1's wire-shape
    table:
    ```rust
    pub struct ChatCompletionRequest { model, messages, tools, tool_choice }
    pub struct ChatCompletionResponse { id, choices, usage }
    pub struct Choice { index, message, finish_reason }
    pub struct Msg { role, content, tool_calls, tool_call_id }
    pub struct ToolCall { id, type: "function", function: ToolCallFn }
    pub struct ToolCallFn { name, arguments: String /* JSON-encoded */ }
    ```
  - HTTP: `POST <endpoint>/chat/completions`;
    `Authorization: Bearer <api-key>`;
    `stream: false`; 60s per-request timeout; no
    retries (m5a).
  - `crates/rafaello-openai/src/error.rs` —
    `pub fn map_to_assistant(err: &OpenaiError) ->
    String` produces the deterministic strings
    scope §OP1 / §OP1a list:
    - 4xx → `openai: client error <status>: <body excerpt>`;
    - 5xx → `openai: server error <status>`;
    - 401/403 → `openai: auth failed (<status>); check API key env var`;
    - connection / timeout → `openai: transport error: <reqwest::Error display>`;
    - malformed JSON → `openai: malformed response: <serde error>` + log full body to stderr;
    - empty `choices` → `(no response)`;
    - multiple choices → use `choices[0]`, stderr warning;
    - invalid tool args → `openai: invalid tool args from model: <serde error>`;
    - unknown tool → `openai: model proposed unknown tool '<name>'`.
  - `RFL_OPENAI_MODEL` is **required**; missing →
    `OpenaiConfigError::MissingModel` returned at
    plugin startup before any HTTP call (scope §OP6
    M-6 / pi-2 M-6 — no plugin-source default).
- **Why.** Scope §OP1. The wire client + error
  mapping is a self-contained module exercisable
  against a `httpmock`-style stub at unit-test
  level; the bus integration lands in c33.
- **Depends on.** c03.
- **Acceptance.** Tests in `rafaello-openai/tests/`:
  - `openai_http_401_emits_auth_failed_assistant_message.rs`
  - `openai_http_500_emits_server_error_assistant_message.rs`
  - `openai_malformed_response_body_emits_diagnostic.rs`
  - `openai_empty_choices_emits_no_response_assistant_message.rs`
  - `openai_multiple_choices_uses_first_logs_warning.rs`
  - `openai_invalid_tool_arguments_string_emits_error_assistant_only.rs`
  - `openai_unknown_tool_name_from_model_emits_error_assistant.rs`
  - `openai_missing_model_env_errors_before_request.rs`
    (pi-2 M-6 — assert `OpenaiConfigError::MissingModel`
    before the HTTP path runs).
  Tests use a small in-process `tokio` HTTP server
  (~80 LoC; the `rafaello-openai-stub` bin lands in
  c35, but unit tests use an in-process variant for
  speed).
- **Size.** medium. ~200 LoC wire + error + ~250
  LoC tests.

### c33 — feat(rafaello-openai): bus-side adapter (subscribe user_message/tool_result, publish tool_request/assistant_message)

- **What.** Scope §OP3 + §OP1 conversation history.
  - Subscribe to `core.session.user_message` and
    `core.session.tool_result` per the m4 fixture
    pattern.
  - Maintain a per-session in-memory `Vec<Msg>`:
    user messages → `role: "user"`; assistant
    messages → `role: "assistant"`; tool_results →
    `role: "tool"` with `tool_call_id` taken from
    `in_reply_to[0]`.
  - On user_message arrival: send a ChatCompletion
    request via the c32 wire client.
  - Response handling per §OP1 table:
    - `finish_reason == "stop" | "length"` → publish
      one `provider.openai.assistant_message`.
    - `finish_reason == "tool_calls"` → publish one
      `provider.openai.tool_request` per
      `tool_calls[i]` in array order, fresh
      `request_id`, all carrying the same
      `in_reply_to` (the user_message id).
    - Mixed content + tool_calls → emit
      `assistant_message` first, then tool_requests
      in array order.
  - All publishes use mandatory envelope
    `request_id` (fresh ULID) and `in_reply_to`
    populated per security RFC §7.2.6 rows 2-3
    (assistant_message cites union of observed
    user_message + tool_result; tool_request cites
    prior tool_result ids).
- **Why.** Scope §OP3. The provider's
  publish/subscribe shape mirrors `rafaello-mockprovider`'s
  m4 footprint.
- **Depends on.** c03, c32.
- **Acceptance.** Tests in `rafaello-openai/tests/`:
  - `openai_emits_assistant_message_for_user_message.rs`
    — against an in-process stub HTTP server.
  - `openai_emits_tool_request_when_model_returns_tool_call.rs`
  - `openai_in_reply_to_populated_for_assistant_message.rs`
  - `openai_in_reply_to_populated_for_tool_request.rs`
  - `openai_handles_tool_call_followed_by_assistant_message.rs`
    — multi-turn.
  - `openai_multiple_tool_calls_one_response_emits_each_with_shared_in_reply_to.rs`
  - `openai_mixed_content_and_tool_calls_emits_assistant_then_tool_requests.rs`
- **Size.** medium.

### c34 — feat(rafaello-openai): manifest + lock fixture with `allow_secrets` + `env.set` keys

- **What.** Scope §OP4 + §OP5.
  - Manifest at
    `rafaello/crates/rafaello-openai/rafaello.toml`
    and copy under
    `rafaello/fixtures/m5a-locks/rafaello-openai/`:
    `schema = 1`, `name = "openai"`, `version =
    "0.0.0"`, `entry = "bin/rfl-openai"`, `rafaello
    = ">=0.1, <0.2"`, `load = "eager"`.
    `[provides] provider = "openai"`.
    `[bus] subscribes = ["core.session.user_message",
    "core.session.tool_result"]`, `publishes =
    ["provider.openai.tool_request",
    "provider.openai.assistant_message"]`.
    `[capabilities.default.filesystem] read_dirs =
    [] write_dirs = []`.
    `[capabilities.default.network] mode = "proxy"
    allow_hosts = ["litellm.thepromisedlan.club"]`.
    `[capabilities.default.env] pass = []
    allow_secrets = ["LITELLM_API_KEY",
    "OPENAI_API_KEY", "ANTHROPIC_API_KEY"]`.
  - **Package `bin/rfl-openai` shim file** (pi-1 B-4 /
    m4 c20 precedent). POSIX shell stub at
    `rafaello/crates/rafaello-openai/bin/rfl-openai` and
    fixture copy under
    `rafaello/fixtures/m5a-locks/rafaello-openai/bin/rfl-openai`
    with `chmod +x` and body `#!/bin/sh\nexec "$@"`. Exists
    only to satisfy `manifest::validate_with_package`'s
    `entry`-resolution check; runtime spawn (c38) points
    at the cargo-built `target/.../rfl-openai`.
  - **`openrpc.json` sibling** (pi-1 B-4 + scope row 31
    requirement). The openai plugin is a *provider*,
    not a tool provider, so it declares **zero** methods —
    but m1's `validate_with_package` requires the sibling
    file's presence regardless of content. Ship at both
    `rafaello/crates/rafaello-openai/openrpc.json` and the
    fixture-tree copy:
    ```json
    {
      "openrpc": "1.2.6",
      "info": { "title": "openai", "version": "0.0.0" },
      "methods": []
    }
    ```
    `ToolSchemaCatalog::build` (c31) iterates `methods`
    and silently produces no entries for this plugin —
    correct, because the openai plugin owns no tools
    (its `provides.tools` is absent / empty).
  - **Complete m5a fixture lock — all four plugins**
    (pi-2 B-3 — round-2's two-plugin lock was
    insufficient; the five-tree orchestration in c38
    spawns openai + mailcat + readfile + mockprovider,
    all four must be installed) at
    `rafaello/fixtures/m5a-locks/rafaello.lock`. The
    single canonical fixture lock c39's demo-bar test
    consumes; contains entries for:
    - `builtin:openai@0.0.0` — active provider.
      Bindings and grant as below.
    - `local:mailcat@0.0.0` — active mail-sink tool.
      Bindings + grant + tool_meta projected from
      c30's mailcat manifest (sinks = ["mail"],
      always_confirm = false, grant_match path
      pointing at `schemas/send-mail-grant.json` —
      see M4 correction below).
    - `local:readfile@0.0.0` — non-sink tool, retained
      from m4. Reuse the existing m4 readfile lock
      entry shape unchanged.
    - `local:mockprovider@0.0.0` — installed-but-not-active
      alternative provider, retained from m4 for the
      bonus-negatives matrix. Reuse the m4 lock entry
      shape unchanged.
    The lock is hand-written; digests recorded from
    `rfl install --fixture` against each fixture tree.
    The openai entry is exactly as below:
    `[plugin."builtin:openai@0.0.0".bindings]
    provider = true; provider_id = "openai"`.
    `[plugin."builtin:openai@0.0.0".grant.bundles.default.network]
    mode = "proxy"; allow_hosts =
    ["127.0.0.1"]` (for CI stub; manual-validation
    lock overrides to the real LiteLLM host).
    `[plugin."builtin:openai@0.0.0".grant.bundles.default.env]
    pass = ["LITELLM_API_KEY"]; allow_secrets =
    ["LITELLM_API_KEY", "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY"]`.
    `[plugin."builtin:openai@0.0.0".grant.bundles.default.env.set]
    RFL_OPENAI_API_KEY_ENV = "LITELLM_API_KEY";
    RFL_OPENAI_ENDPOINT_URL =
    "https://litellm.thepromisedlan.club/v1";
    RFL_OPENAI_MODEL = "vllm/qwen3.6-27b"`.
    Mailcat's lock entry pins
    `bindings.tool_meta.send-mail.grant_match =
    "schemas/send-mail-grant.json"` (pi-2 M-4 — the
    grant_match value is the **JSON-Schema sibling
    path**, NOT openrpc.json; openrpc.json carries
    only the tool param schema for the model-facing
    catalog, while grant_match validates the user's
    `/grant` template).
    The lock also sets `session.provider_active =
    "builtin:openai@0.0.0"` (pi-3 B-1 — live field name
    per `rafaello-core/src/lock/session.rs:11`).
    **No `session.tool_owner` entries** (pi-4 B-1):
    `session.tool_owner` is conflict-resolution state
    populated only when two installed plugins claim the
    same tool name; live `validate::lock` rejects
    redundant entries with `ToolOwnerRedundant` when
    only one claimant exists. The m5a fixture has
    exactly one `send-mail` claimant (mailcat) and one
    `read-file` claimant (readfile), so the table stays
    empty.
- **Why.** Scope §OP4 + §OP5 + pi-1 B-5
  (env-pass-by-name); the lock-side TOML uses live
  `GrantEnv` shape.
- **Depends on.** c03, c06, c30, c31 (c30 ships the
  mailcat manifest + fixture tree; c31 backfills the
  readfile openrpc.json and lands the
  `ToolSchemaCatalog::build` consistency check that
  the combined lock's readfile + mockprovider entries
  rely on — pi-2 B-3).
- **Acceptance.** Tests in
  `rafaello-openai/tests/`:
  - `openai_manifest_compiles.rs` — `manifest::parse`
    + `manifest::validate_with_package` succeed.
  - `compile_openai_lock_with_rfl_openai_envset_keys_succeeds.rs`
    (pi-1 B-8 — scope §M1.1 explicit test). Build a
    lock with `RFL_OPENAI_ENDPOINT_URL` /
    `RFL_OPENAI_MODEL` / `RFL_OPENAI_API_KEY_ENV` in
    `env.set`; `compile_plugin` succeeds; live
    `scrubber::reject_reserved` accepts the names
    because they are NOT in `RESERVED_ENV_VARS`.
  - `openai_lock_with_litellm_api_key_pass_honoured_via_manifest_allow_secrets.rs`
    — `compile_plugin` retains the `LITELLM_API_KEY`
    pass entry (`EnvPlan.pass` includes it) because
    `allow_secrets` covers the name. Inspect the
    `CompiledPlugin.env_plan.pass` field (pi-2 N-4 —
    not validation output).
  - `openai_lock_with_unsanctioned_secret_env_var_stripped.rs`
    — a variant lock that adds `RANDOM_API_KEY` to
    `env.pass` (not in `allow_secrets`); the
    compiled plan drops the pass entry.
  - `openai_endpoint_url_taken_from_env_var.rs`
  - `openai_model_taken_from_env_var.rs`
  - `openai_api_key_resolved_via_indirection_env_var.rs`
  - `m5a_fixture_lock_validates_and_compiles.rs` —
    the combined lock from
    `rafaello/fixtures/m5a-locks/rafaello.lock` passes
    `validate::lock` and `compile_plugin` for **all
    four** entries (openai + mailcat + readfile +
    mockprovider — pi-2 B-3) before c39 consumes it.
    **Also calls `ToolSchemaCatalog::build(&acl,
    &compiled_plugins, &package_dirs)` against the
    combined lock** (pi-3 M-3) and asserts the
    resulting catalog's `list()` contains exactly
    two `ToolSchema` entries with `name = "send-mail"`
    (from mailcat) and `name = "read-file"` (from
    readfile). The openai + mockprovider providers
    contribute no entries (no `provides.tools`).
  - `m5a_fixture_lock_session_pins_provider_active.rs`
    (pi-3 B-1 + pi-4 B-1 — only `provider_active`
    asserted; `session.tool_owner` is intentionally
    empty per the no-conflict invariant) — assert
    `session.provider_active == "builtin:openai@0.0.0"`
    and `session.tool_owner.is_empty()`.
- **Size.** medium (TOML + combined fixture lock +
  ~7 small tests).

### c35 — feat(rafaello-openai-stub): deterministic `/v1/chat/completions` HTTP stub bin

- **What.** Scope §W2 + §A8. Fill the c04
  placeholder. `crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`:
  - Bind to `127.0.0.1:0`, read the assigned port,
    print it to stdout (so the test harness can
    parse it).
  - Serve POST `/v1/chat/completions` with a
    deterministic response read from a JSON file
    path given via `--response <path>` (or
    `RFL_OPENAI_STUB_RESPONSE` env var) — supports
    a `Vec<ChatCompletionResponse>` so multi-turn
    tests can iterate.
  - Read request body; assert wire shape via
    `serde_json::from_slice` (a malformed request
    crashes the stub with stderr — useful test
    signal).
  - 5s self-timeout (`RFL_FIXTURE_MAX_LIFETIME`
    pattern from m2 retro §5.4); exit cleanly on
    SIGTERM.
- **Why.** Scope §A8 + scope risk #3 — gives CI a
  network-free `rfl-openai` exercise path.
- **Depends on.** c04.
- **Acceptance.** Tests in
  `rafaello-openai-stub/tests/`:
  - `stub_responds_to_chat_completions_post.rs` —
    POST a body; receive the response from the file.
  - `stub_rejects_malformed_post_with_400.rs`.
  - `stub_self_timeout_exits_within_lifetime.rs`.
- **Size.** small. ~150 LoC stub + ~100 LoC tests.

### c36 — feat(rafaello-openai): `core.tools_list` call after handshake + cache + fatal exit on failure

- **What.** Scope §OP2 item 6 + §OP7.
  - After the fittings handshake completes (and
    before subscribing to `core.session.user_message`),
    the bin calls
    `peer.call("core.tools_list", json!({}))`. The
    response is cached on the plugin heap.
  - Failure (any error, including `MethodNotFound`)
    → exit non-zero with stderr `openai:
    core.tools_list failed: <...>`. The supervisor's
    existing `WatcherEvent::Exit` /
    `WatcherEvent::Crash` path reports it as a
    normal plugin-startup failure.
  - The cached schemas are forwarded as the
    `tools` field of every ChatCompletion request
    in c33's adapter (extend c33's
    `ChatCompletionRequest::tools` population).
- **Why.** Scope §OP2 item 6 + pi-3 N-4 rename.
- **Depends on.** c31, c33, c35.
- **Acceptance.** Tests in `rafaello-openai/tests/`:
  - `openai_calls_tools_list_after_handshake.rs` —
    against the in-tree `CorePluginService` (built
    in c31); assert the call happens before any
    `chat_completions` POST.
  - `openai_request_carries_tool_schemas.rs` —
    after the cache populates, the next request
    carries the schemas in `tools`.
  - `openai_exits_nonzero_when_core_tools_list_returns_method_not_found.rs`
    — spawn against a non-provider service (no
    `CorePluginService`); assert exit code
    non-zero.
  - `openai_tools_list_failure_exits_nonzero_and_supervisor_reports_crash.rs`
    (pi-3 N-4 / round-4 rename — round-3 used the
    removed `PostHandshakeFailure` framing) — wire
    via the supervisor's existing crash path;
    assert the `WatcherEvent::Exit | Crash`
    surfaces.
- **Size.** small.

---

## Phase M — `rfl chat` orchestration extension

Scope §CHAT + §CG6. Two commits; c38 bundles the
agent-loop pivot (old c25) per pi-1 B-3.

### c37 — feat(rafaello-tui): RFL_TUI_TEST_* env hooks for confirm-answer + grant-before-message

- **What.** Scope §CHAT3.
  - `RFL_TUI_TEST_CONFIRM_ANSWER` — `"allow"` /
    `"deny"` / `"always_allow_session"` /
    `"timeout"` / unset. When set, on the next
    `confirm_request` the TUI observes, auto-publish
    the answer after `RFL_TUI_TEST_CONFIRM_DELAY_MS`
    (default 0). `"timeout"` skips publishing
    entirely.
  - `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE` — JSON
    `{"tool": "send-mail", "args_subset": {...}}`.
    On startup, before the test message is sent,
    auto-publish a synthetic
    `frontend.tui.slash_command` carrying a
    `/grant` payload.
- **Why.** Scope §CHAT3. Lets integration tests
  drive the TUI deterministically (extends m4's
  `RFL_TUI_TEST_MESSAGE`).
- **Depends on.** c17, c25.
- **Acceptance.** Tests in `rafaello-tui/tests/`:
  - `tui_test_confirm_answer_publishes_allow_after_delay.rs`
  - `tui_test_grant_before_message_publishes_slash_grant_first.rs`
  - `tui_test_confirm_answer_timeout_does_not_publish.rs`
- **Size.** small.

### c38 — feat(rafaello): rfl chat orchestration extension + agent-loop pivot — UNSPLITTABLE CUTOVER

- **What.** Scope §CHAT1 + §CHAT2 + §CG6 (agent-loop
  pivot bundled per pi-1 B-3). **Unsplittable cutover**:
  the gate must drive dispatch *and* the agent loop must
  stop driving dispatch in the same commit, otherwise
  every `tool_request` reaches the plugin twice and the
  per-commit green-bar fails. m0 c08 / m4 c07 precedent.
  Two coordinated edits:
  - `crates/rafaello/src/lib.rs::run_chat` extended:
    - Construct an empty
      `Arc<RwLock<UserGrants>>`.
    - Construct an `Arc<AuditWriter>` from
      `session_controller.audit_writer()` (c08).
    - The `Arc<ToolSchemaCatalog>` is already wired in
      from c31's run_chat update; reuse it here.
    - Register the core-side slash handler (c18) as
      an internal subscriber on
      `frontend.tui.slash_command`.
    - Construct an `Arc<ConfirmState>`; pass it into
      both the `ConfirmationGate` constructor and the
      `ReemitRouter` via
      `.with_confirm_state_and_audit(state.clone(),
      audit.clone())` (the c14 builder).
    - Construct the `ConfirmationGate` wired to the
      broker, `UserGrants`, `AuditWriter`,
      `ConfirmState`, compiled-plugin map; spawn its
      task **before** the supervisor spawns any
      provider so the gate is ready to subscribe
      internally to `core.session.tool_request`.
  - `crates/rafaello-core/src/agent/mod.rs:143-217` —
    **remove the direct
    `broker.publish_for_tool_dispatch(...)` call** in
    `handle_tool_request`. The agent loop now only
    persists the `tool_call` entry and observes the
    canonical `core.session.tool_request`; the gate
    drives the `plugin.<topic-id>.tool_request`
    publish for every event (passthrough,
    grant-match, or post-confirm allow).
  - The orchestration tree becomes a five-tree:
    `rfl chat` → `rfl-tui` + `rfl-openai` +
    `rfl-mailcat` (+ `rfl-readfile` and
    `rfl-mockprovider` retained as installed-but-not-active
    alternatives in the fixture lock for the
    bonus negatives).
  - **Spawn order at run_chat** (pi-3 M-2 + pi-4 M-1).
    After the gate spawns and after the active
    provider is spawned (the entry whose canonical id
    matches `session.provider_active`), **iterate the
    compiled-plugin map and spawn every other plugin
    with `bindings.provider = true` whose canonical
    id differs from `session.provider_active`**
    (canonical-id comparison, NOT provider_id —
    pi-4 M-1 corrected the round-3 wording; canonical
    is the unique-per-install identifier while
    `provider_id` is the public protocol string used
    only in topic routing) through the same
    `PluginSupervisor`. These are
    *installed-but-not-active* provider entries
    (e.g. mockprovider in the m5a fixture lock).
    They spawn so the supervisor can manage their
    lifecycle and so `rfl provider use` (post-v1)
    has live children to switch between, but
    `ReemitRouter` stays subscribed only to
    `provider.<active.provider_id>.**` (resolved
    from the active canonical's `PluginAcl.provider_id`
    field — `provider.openai.**` in the fixture) —
    inactive providers can publish but their
    `provider.mock.**` events fall outside the
    re-emit scope and are dropped. Then iterate
    `bindings.tools` and spawn every tool plugin
    (mailcat, readfile). The five-tree orchestration
    matches scope §CHAT2: active provider + tools
    + inactive provider all reaped on shutdown.
  - **m4-test migration**: m4 tests that exercise tool
    dispatch through the agent loop must now construct
    a permissive gate or use a small
    `gate_or_synthetic_dispatch` test helper (added in
    this commit) that wraps the gate's passthrough
    arm. The migration is in-commit; no m4 test is
    left behind.
- **Why.** Scope §CHAT1 + §CHAT2 + §CG6 + the risks
  inventory §"Risks" #1 leak-mitigation calls + pi-1
  B-3 (cutover discipline).
- **Depends on.** c08, c13, c14, c15, c16, c18, c20,
  c21, c22, c23, c24, c31, c36 (pi-2 M-3 — c18 is the
  core-side slash handler registered as an internal
  subscriber here; c16 is c18's transitive dep for
  jsonschema template validation at `/grant` time).
  The agent-loop pivot (round-1 c25) is bundled here
  per pi-1 B-3 — no separate pivot commit.
- **Acceptance.** Tests in `rafaello/tests/`:
  - `rfl_chat_constructs_gate_before_provider_spawn.rs`
    — assert ordering via a test seam on
    `PluginSupervisor` that records "first spawn
    callback at <instant>" and the gate's task
    `record "subscribed at <instant>"`; assert
    `gate < first_spawn`.
  - `rfl_chat_tool_dispatch_goes_through_gate.rs` —
    five-tree orchestration; a `tool_request`
    observed in the agent loop produces NO
    `plugin.<id>.tool_request` until the gate's
    passthrough fires.
  - `rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly.rs`
    — extend m4's c25 smoke test to assert all
    five children reap on shutdown.
  - `rfl_chat_spawns_inactive_provider_but_reemit_ignores_it.rs`
    (pi-3 M-2) — load the m5a fixture lock (which
    has both `openai` active and `mockprovider`
    installed-but-not-active); assert the
    supervisor spawns both provider children; assert
    a `provider.mock.assistant_message` publish from
    the mockprovider does NOT produce a
    `core.session.assistant_message` re-emission
    (ReemitRouter stays scoped to
    `provider.openai.**`).
  - `core_tools_list_registered_before_provider_spawn.rs`
    (scope §"Risks" #6) — the supervisor's
    per-connection `CorePluginService` is composed
    at handshake time; assert the provider's
    first `core.tools_list` call succeeds.
  - `agent_loop_does_not_dispatch_tool_request_directly.rs`
    in `rafaello-core/tests/` (pi-1 B-3 — pivot
    bundled). With **no gate constructed**, observe
    a `core.session.tool_request` event; assert no
    `plugin.<id>.tool_request` is published (the
    handler only persists). Asserts the dispatch
    code path is gone from the agent loop.
  - `rfl_chat_no_double_dispatch_when_gate_constructed.rs`
    (pi-1 B-3) — five-tree harness; subscribe an
    internal observer to
    `plugin.<topic-id>.tool_request`; drive one
    `tool_request`; assert exactly one publish (not
    two).
  - All migrated m4 tests under the in-commit
    `gate_or_synthetic_dispatch` helper continue
    passing; `cargo test --workspace --features
    test-fixture` green on Linux + macOS CI.
- **Size.** **unsplittable cutover** (m0 c08 / m4 c07
  class). ~250 LoC orchestration changes + ~5 LoC
  agent-loop removal + ~150 LoC m4 test migrations +
  ~250 LoC new tests. Body cites pi-1 B-3 + scope §CG6.

---

## Phase N — Demo bar headline + bonus negatives + manual validation

Scope §"Demo bar" + §"Manual validation".

### c39 — test(rafaello): demo-bar headline `rfl_chat_demo_bar_send_mail.rs` (allow + deny arms)

- **What.** Scope §"Demo bar" §Positive.
  - New `rafaello/tests/rfl_chat_demo_bar_send_mail.rs`.
    Spawn `rfl chat` against the m5a fixture lock
    (`fixtures/m5a-locks/...`) with `rfl-openai`
    active + `rfl-mailcat` installed. CI points the
    openai plugin at the c35 `rfl-openai-stub`'s
    recorded response that proposes a `send-mail`
    tool call with `args.to = "alice@example.com"`.
    Drive the user_message via
    `RFL_TUI_TEST_MESSAGE="please email alice"`.
    Run the test **twice** with different
    `RFL_TUI_TEST_CONFIRM_ANSWER`:
    - **allow arm:** assert SQLite `entries`
      table contains, in `seq` order, `text` (user),
      `tool_call` (status `Allowed`), `tool_result`
      (ok), `text` (assistant); mailcat.log has one
      entry; audit log records `confirm_request`
      and `confirm_allowed`.
    - **deny arm:** assert `entries` contains
      `text` (user), `tool_call` (status `Denied`),
      `tool_result` (`{ok: false, error:
      "user_denied"}`, taint `[{source: "system",
      detail: "user_denied"}]`), `text` (assistant
      — the model's reply to the denial);
      mailcat.log empty; audit log records
      `confirm_denied`.
- **Why.** Scope §"Demo bar" §Positive. This is the
  headline test for the milestone.
- **Depends on.** c30, c34, c35, c37, c38 (pi-1 B-6 —
  c34 ships the complete fixture lock this test
  loads from `rafaello/fixtures/m5a-locks/rafaello.lock`).
- **Acceptance.** Both arms pass on Linux + macOS
  CI; `cargo test --workspace --features
  test-fixture` green.
- **Size.** medium. ~250 LoC test harness + fixture
  responses.

### c40 — test(rafaello): demo-bar negatives — timeout + always_allow_session restart + bonus negatives

- **What.** Scope §"Demo bar" §Negative 1, §Negative
  2, §"Bonus negatives implied by the security RFC".
  - `rafaello/tests/rfl_chat_demo_bar_send_mail_timeout.rs`
    — Negative 1. Same setup as c39 but
    `RFL_TUI_TEST_CONFIRM_ANSWER=timeout`; uses
    `tokio::time::pause` + advance past 60s. Assert
    synthetic `tool_result` shape (§CG4a, `taint =
    [{source: "system", detail: "confirm_timeout"}]`,
    `in_reply_to = [held_id]`); entries / mailcat
    state match deny arm; audit row
    `confirm_timeout`.
  - `rafaello/tests/rfl_chat_always_allow_session_clears_on_restart.rs`
    — Negative 2. First invocation with
    `RFL_TUI_TEST_CONFIRM_ANSWER=always_allow_session`;
    mailcat.log +1; audit records
    `confirm_allowed_with_session_grant` and
    `grant_added`. Second invocation in the same
    tempdir (same SQLite, same lock, fresh `rfl
    chat` process → fresh `UserGrants`); fresh TUI
    answers `deny` after 10ms via
    `RFL_TUI_TEST_CONFIRM_ANSWER=deny` +
    `RFL_TUI_TEST_CONFIRM_DELAY_MS=10` (pi-1 N-6).
    Assert: second run prompts again (fresh
    `confirm_request` audit entry); deny holds
    (mailcat.log unchanged from first run).
  - `rafaello/tests/rfl_chat_always_confirm_true_holds_non_sink_tool.rs`
    — bonus. A fixture tool with `sinks = []` and
    `always_confirm = true`. Assert the gate fires
    the prompt even though no sinks are declared.
  - (`rfl_install_status_shows_red_for_override.rs`
    moved to c28's acceptance per pi-1 M-5; the
    `rfl status` surface lands in c28, not here.)
  - `rafaello/tests/rfl_chat_grant_revoked_blocks_next_call_but_not_in_flight.rs`
    — pi-1 M-7 bonus. Grant `send-mail
    to=alice@…`; observe one allowed call; revoke;
    observe the next call prompts again. The
    in-flight call (mid-dispatch, not yet
    `tool_result`) is NOT retroactively
    un-allowed.
  - Negative 4 (verbatim flow blocked) is
    **explicitly skipped** — deferred to m5b per
    scope §"In scope" item §m5a → m5b boundary.
- **Why.** Scope §"Demo bar". m5a's contract.
- **Depends on.** c29, c30, c38, c39 (the
  status-red bonus moved to c28; no extra dep here).
- **Acceptance.** All four remaining test files pass on
  Linux + macOS CI.
- **Size.** medium-to-large. ~400 LoC across
  fixtures + harnesses.

### c41 — docs(rafaello-m5a): manual-validation.md skeleton

- **What.** Scope §"Manual validation". New file
  `rafaello/plans/milestones/m5a-sinks-confirmation/manual-validation.md`
  with sections matching scope §1-§6:
  1. Real-network demo (LiteLLM proxy + `send-mail`).
  2. Slash-command demo (`/grant`, `/grants list`,
     `/revoke` round-trip).
  3. Trifecta refusal demo (`rfl install` refuses;
     `--i-know-what-im-doing` accepts; `rfl
     status` shows red marker).
  4. macOS CI URL capture.
  5. TUI keyboard interaction walkthrough (every
     documented key).
  6. Audit-log inspection (dump `audit_events`).
  Each section has a step-list scaffold; concrete
  command lines + URLs land during Phase 3 manual
  runs (per m4 retrospective pattern).
- **Why.** Scope §"Manual validation".
  CI can't exercise the LiteLLM proxy
  (`LITELLM_API_KEY` not present); the headline
  test uses the stub; manual validation runs the
  same test shape against the real proxy.
- **Depends on.** c39.
- **Acceptance.** File exists with all six section
  headers + step scaffolds. `cargo test
  --workspace` still green. No code changes.
- **Size.** small. Docs-only.

---

## Cross-checks (round 5 self-audit)

> Numbers below verified against the actual
> `### cNN — ...` headings post-round-2 sed. Total: 41
> commits. The bundled rfl-chat-orchestration +
> agent-loop pivot is c38 (not c37 as the round-2
> cross-checks accidentally claimed).

- **Every §"In scope" bullet maps to ≥1 row.** W1-W4
  → c01-c05. Si1-Si3 → c09. Tr1-Tr4 → c27-c29 (install
  c27, status c28, transitive-not-chased tests c29).
  CT0-CT5 → c11-c14. CG1-CG8 → c13 + c20-c24 + c38
  (CG6 agent-loop pivot bundled into c38's
  unsplittable cutover per pi-1 B-3). OM1-OM3 → c10.
  UG1-UG5 → c15-c16. SL0-SL5 → c17-c19. TUI1-TUI5 →
  c25-c26. OP1-OP7 → c31-c36 (ToolSchemaCatalog c31,
  openai wire c32, bus adapter c33, manifest+lock
  c34, stub c35, tools_list c36). TP1-TP4 → c30
  (mailcat fixture). AL1-AL4 → c08 + folded into
  c14/c18/c20/c22/c23/c24/c27. M1.1 → c06 + c34
  (c34 owns
  `compile_openai_lock_with_rfl_openai_envset_keys_succeeds.rs`;
  c06 owns
  `scrubber_reject_reserved_unchanged_for_seven_core_names.rs`
  — pi-1 B-8). M1.2 → c07. CHAT1-CHAT3 → c37 + c38.
  I → c39-c40.
- **Every §"Demo bar" demo covered.** Positive →
  c39 (demo-bar headline allow/deny arms). Negative
  1 (timeout) → c40. Negative 2
  (always_allow_session restart) → c40. Negative 3
  (one-hop trifecta, no transitive) → c29. Negative
  4 (verbatim) → m5b. Bonus negatives → c40 (with
  the status-red bonus moved to c28 per pi-1 M-5).
- **No row exceeds size budget without
  unsplittable-cutover justification.** Three
  unsplittable cutovers: c06 (allow_secrets schema +
  scrubber signature), c10 (broker
  outstanding-dispatched map populator+consumer),
  c38 (rfl chat orchestration + agent-loop pivot —
  pi-1 B-3). All three bodies cite m0 c08 / m4 c07
  precedent.
- **Phase preambles match scope's internal-split
  sections.** Scope item 1 → Phase A. Item 2 → B.
  Item 3 → C+D (audit infra threaded early so later
  commits wire it inline). Item 4 → E. Item 6 → H.
  Item 7 → F. Item 8 → G. Item 9 → I. Item 10 → C
  (audit). Item 11 → J. Item 12 → K. Item 13 → L.
  Item 14 → M. Item 15 → N.
- **Topic-id / env-var / manifest / lock paths
  match scope verbatim.** `core.session.confirm_*`,
  `core.session.confirm_resolved` (pi-1 M-1 +
  pi-2 M-1 wire-contract table — new),
  `frontend.tui.confirm_answer`,
  `frontend.tui.slash_command`,
  `core.session.command_result`,
  `bindings.tool_meta.<n>.{sinks, grant_match,
  always_confirm}`,
  `grant.bundles.<bundle>.env.{pass, set, allow_secrets}`,
  `RFL_OPENAI_API_KEY_ENV`,
  `RFL_OPENAI_ENDPOINT_URL`, `RFL_OPENAI_MODEL`,
  `builtin:openai@0.0.0`. All spellings checked
  against scope.md round 6.
- **OP6 vs Tr1 wording nit (pi-6 N-1) folded.** c27
  step 10 pins the canonical stderr string
  `warning: unused allow_secrets entry '<name>' (no
  matching env.pass entry)`. §OP6 is not re-cited
  anywhere with an alternate wording.
- **Tests-with-code rule.** Every named test in
  scope sits in the commit row that introduces its
  surface. Exceptions: c30 (mailcat) ships fixture
  variants but tests only the m1 happy-path; the
  openrpc method-vs-tool consistency tests live in
  c31 (which owns `ToolSchemaCatalog::build`) —
  explicit per pi-1 B-9 (not a
  stub-against-wrong-behaviour ladder).
- **Signature-cutover discipline** (pi-1 B-2 / B-5).
  c14's `ReemitRouter` change uses a
  backward-compatible `with_confirm_state_and_audit`
  builder; no live reemit-test or run_chat call
  site breaks at c14. c31's `PluginSupervisor::new`
  signature change updates every live call site in
  the same commit (run_chat builds the catalog;
  tests use `ToolSchemaCatalog::empty_for_tests()`).
- **Fixture-validator discipline** (pi-1 B-4 / B-6).
  c30 and c34 both ship package `bin/` shim files
  so `manifest::validate_with_package`'s entry-path
  canonicalisation passes. c34 ships a minimal
  empty-methods `openrpc.json` for the openai
  provider, and the complete combined m5a fixture
  lock — **all four plugins** (openai + mailcat +
  readfile + mockprovider) per pi-2 B-3 — so c39
  consumes a real lock at the canonical path.
- **m4 readfile fixture OpenRPC backfill** (pi-2
  B-2). c31 also updates
  `rafaello/fixtures/rafaello-readfile/openrpc.json`
  to declare the `read-file` method matching
  `provides.tools = ["read-file"]`. Without this
  backfill, the round-3 `ToolSchemaCatalog::build`
  would reject every existing m4 readfile lock at
  rfl-chat startup. Mockprovider is unaffected (no
  `provides.tools`).
- **Short-circuit late-vs-duplicate audit kind**
  (pi-2 B-4). c24's stale-answer-after-short-circuit
  test audits `confirm_duplicate` (because
  `try_resolve` transitions to `ResolvedByAnswer`,
  which `prior_outcome` classifies as `Duplicate`),
  not `confirm_late`.

---

## Sizing summary

Round 3 sizing — each commit is in exactly one
category:

- **small** (≲50 LoC including tests): 20 commits —
  c01, c02, c03, c04, c05, c07, c08, c09, c11, c12,
  c16, c17, c19, c23, c28, c29, c35, c36, c37, c41.
- **small-to-medium**: 4 commits — c15, c21, c24, c30.
- **medium** (50-200 LoC): 13 commits — c10, c13,
  c14, c18, c20, c25, c26, c27, c31, c32, c33, c34,
  c39.
- **medium-to-large** (200-500 LoC): 2 commits —
  c22, c40.
- **large** (~300 LoC, body-justified): 1 commit —
  c06 (`allow_secrets` cutover).
- **unsplittable cutover** (LoC across multiple
  call sites; body cites m0 c08 / m4 c07): 1 row
  not already counted — c38. (c06 and c10 are both
  also unsplittable cutovers; counted in their
  size bucket above.)

Total: 20 + 4 + 13 + 2 + 1 + 1 = **41 commits**.
Scope §"Sizing & split recommendation" estimated
30-38; round-3 lands at 41 for these reasons:

- Audit log infrastructure (c08) is unbundled from
  gate/slash/install per the tests-with-code rule.
- The gate's five commits (c20-c24) match scope
  §"Internal split" item 6's "~4-5 commits" upper
  bound; the CG6 pivot is bundled into c38 (per
  pi-1 B-3) rather than a separate isolated commit.
- The slash-command flow splits across three
  commits (TUI parser c17 / core handler c18 / TUI
  renderer c19) honouring pi-1 B-1's bus-mediated
  rewrite.
- The `rfl-openai` provider is six commits
  (c31-c36) covering catalog cutover, wire client,
  bus adapter, manifest+lock fixture, stub bin,
  and tools_list call — at scope §"Internal split"
  item 13's "~5-6 commits" upper bound.

Pi-4 expects zero-blocker convergence on round 5;
ratification follows pending pi-5 verification.

---

*End of m5a commits.md round 5 draft.*
