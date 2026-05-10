# m3 — sessions, local-spawned TUI, built-in rendering — scope

> **Status:** round-2 draft. Round 1 returned 10 blocking + 3
> cleanup findings (`pi-review-1.md`); round 2 addresses every
> one. Most consequential changes from round 1: replay no
> longer uses a separate `core.session.entry.replay` topic
> (collapsed into `entry.finalized`) and is published only
> after the TUI subscribes; the fixture-entry harness now
> goes through a new `SessionController::finalize_entry`
> (append → render → publish) so the demo bar's nine SQLite
> rows actually land; `O_CLOEXEC` on the session lock fd;
> `FrontendSupervisor` gains an explicit reaper task; TUI
> integration tests live in the `rafaello-tui` crate (Cargo's
> `CARGO_BIN_EXE_*` rule); deterministic test exit via
> `core.lifecycle.test_done`; broker error variants made
> publisher-symmetric. Pi convergence pending.

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
7. Integration tests under `rafaello/crates/rafaello-core/tests/`
   and `rafaello/crates/rafaello-tui/tests/` exercising the
   demo bar.

No agent loop, no provider, no tool dispatch in m3. The TUI
runs against an **in-test fixture-entry harness** that injects
static entries directly through the broker's
`publish_core` path on the `core.session.entry.finalized`
topic; `m4` replaces this with the real provider path.

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
    - `ulid = "1"` (entry ids — overview §11 / Stream E
      §3 specify ULID).
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
  - `[dependencies]` adds `rafaello-core`, `rafaello-tui`
    (path-deps), `tokio`, `tracing`, `tracing-subscriber`,
    `clap = { version = "4", features = ["derive"] }`,
    `anyhow` with `workspace = true`. (`clap` is m3's first
    introduction; pi may push back if `rfl chat` is the
    only subcommand and a hand-rolled arg parse is enough —
    round-1 keeps `clap` because m4–m6 will add more
    subcommands.)
- **W5.** No `default = ["test-fixture"]` flip on
  `rafaello-core`; m2's opt-in feature gate stays. m3's
  fixture binary (`rfl-bus-fixture`) is unchanged.

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

- **F1.** New module `rafaello_core::frontend`. Public
  surface:
  - `pub struct FrontendSupervisor` — owns the broker handle
    and the spawned frontend's lifecycle. `new(broker:
    Broker, config: FrontendConfig) -> Self`.
  - `pub struct CompiledFrontend` — the spawn-time plan.
    Fields: `attach_id: String` (kebab-case, validated
    against `^[a-z][a-z0-9-]{0,31}$`), `entry_absolute:
    PathBuf`, `argv: Vec<OsString>`, `env: EnvPlan` (m1's
    type, unchanged), `subscribe_patterns:
    BTreeSet<String>`, `auto_subscribes: BTreeSet<String>`,
    `publish_topics: BTreeSet<String>`. No filesystem /
    network plan because frontends are not sandboxed.
  - `pub struct FrontendPaths` — `project_root: PathBuf`
    (passed to the child as `RFL_PROJECT_ROOT`).
  - `pub fn FrontendSupervisor::spawn(&self, plan:
    &CompiledFrontend, paths: &FrontendPaths) ->
    Result<FrontendHandle, FrontendSpawnError>` — async.
  - `pub struct FrontendHandle` — analogous to m2's
    `SpawnHandle`. Carries `attach_id`, child pid, peer
    handle, drop-time SIGKILL.
- **F2.** `FrontendSpawnError` typed enum (lives in
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
    - Validate `attach_id` against the regex.
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
  - **Phase B** (resource allocation, async):
    - Create socketpair (`SOCK_STREAM | SOCK_CLOEXEC`,
      m2's pattern + macOS fcntl fallback per m2 §5.7).
    - Build `tokio::process::Command` (NOT
      `lockin::SandboxBuilder::tokio_command`; m3 frontend
      spawns are unsandboxed).
    - Apply env: `env_clear` then re-inject `RFL_BUS_FD`,
      `RFL_PROJECT_ROOT`, `RFL_PRIVATE_STATE_DIR` (the
      frontend's per-plugin state-equivalent dir under
      `${PROJECT_ROOT}/.rafaello-frontend-data/<attach-id>/`
      — see §S6 below), then `env.pass` then `env.set`
      (set wins, m2 c18 pattern).
    - Inherit fd `RFL_BUS_FD` via `pre_exec` clearing the
      child socketpair end's `FD_CLOEXEC` (the unsandboxed
      analogue of `SandboxBuilder::inherit_fd_as`; nix
      `fcntl(F_SETFD, 0)`).
    - Spawn the child.
    - Build a `fittings_server::Server` over the parent
      socketpair end with `BusPublishService` (m2 c19,
      reused — frontend publishes look identical on the
      wire).
    - Register the frontend with the broker
      (`Broker::register_frontend(attach_id, peer)` — see
      §B1).
    - Return `FrontendHandle`.
- **F4.** Lifecycle (pi-1 #4: round 1 was missing the
  reaper, which would have leaked zombies in tests and
  long-running parents — m2 SP5 already paid this
  tuition):
  - At spawn time, `FrontendSupervisor::spawn` hands the
    `tokio::process::Child` to a per-frontend **reaper
    task** spawned with `tokio::spawn`. The reaper owns
    the `Child` and `await`s `child.wait()`. On exit it
    pushes the `ExitStatus` (or pre-`wait` outcome) into
    a `tokio::sync::watch` channel.
  - `FrontendHandle::wait(&self) -> Arc<ReaperOutcome>`
    resolves on that watch (mirroring m2 §SP4 step 18).
    Late `wait` callers see the cached `Arc` immediately.
  - `FrontendHandle::Drop` sends a best-effort SIGKILL
    (idempotent on a dead pid; logged at `warn!` if the
    syscall fails) and **does not block**; the reaper
    completes the `wait()` asynchronously.
  - Cooperative shutdown: `FrontendSupervisor::shutdown(self)
    -> ShutdownReport` sends SIGTERM, awaits the reaper
    watch with a 2 s timeout, escalates to SIGKILL,
    awaits again with a 1 s timeout. Identical to m2 c25
    (SIGTERM + grace + SIGKILL + reaper wait); shape is
    the same so a future merge of `PluginSupervisor` and
    `FrontendSupervisor` would be mechanical.
  - **New positive test** `frontend_handle_wait_resolves_on_child_exit.rs`:
    spawn a `rfl-tui` child in `RFL_TUI_TEST_MODE`,
    publish a `core.lifecycle.test_done` event (§T2
    deterministic exit), `handle.wait().await` returns a
    `ReaperOutcome::Exited(0)`.
  - **New negative test** `frontend_handle_drop_does_not_leak_zombie.rs`:
    spawn a child, drop the handle without
    cooperatively shutting down, allow the reaper task
    a 500 ms grace, then assert no child is in zombie
    state (`/proc/<pid>/status` returns `ENOENT`).
    Linux-only.
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
  symmetric to m2 §B3 plugin path):
  - `frontend.<attach-id>.*` only — same exact-match-against-
    `publish_topics` rule as plugins.
  - Top-level segments other than `frontend.<own-attach-id>`
    → `PublishOnReservedNamespace { publisher: Publisher::
    Frontend(attach_id), topic }`.
  - `auto_subscribes` is NOT publish authority for frontends
    either (m2 §B3's rule applies).
- **B5.** Fan-out (m2 §B7): a frontend subscriber receives
  `core.session.**` events the same way plugins do. Result-
  routing protection (m2 §B7's `plugin.<id>.tool_result` /
  `rpc_reply` no-fan-out) is unchanged — m4 territory. m3
  does not introduce any frontend-targeted result-routing
  carve-out.
- **B6.** `BrokerAcl` defence-in-depth pattern revalidation
  (m2 §B10) extends to the new `frontends` map. New tests:
  `broker_construct_with_invalid_frontend_pattern_rejected.rs`,
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
    m3 has one core process per project per row 34).
  - `pub fn SessionStore::open(state_dir: &Path) ->
    Result<Self, SessionError>` — opens / creates
    `${state_dir}/session.sqlite`. Runs `PRAGMA
    journal_mode = WAL` and `PRAGMA synchronous = NORMAL`.
    Creates tables on first call (no migration framework
    in v1; future schema bumps add a migration step in
    m4+). Acquires the project flock per §S5.
  - `pub fn SessionStore::append_entry(&self, entry:
    &Entry) -> Result<u64, SessionError>` — INSERT into
    `entries`; returns the assigned `seq`.
  - `pub fn SessionStore::load_entries(&self) ->
    Result<Vec<Entry>, SessionError>` — SELECT in `seq`
    order. Used at startup for replay (§C2).
  - `pub fn SessionStore::session_id(&self) -> &str` —
    ULID assigned at first open; persisted in a single-
    row `session_meta` table.
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
  serde_json::Error }`, `Locked { holder_pid: u32 }` (see
  §S5 below), `SchemaMismatch { found, expected }`.
  `#[non_exhaustive]`, `thiserror`-derived. Re-exported
  from `lib.rs`.
- **S4.** Project state directory layout — `${PROJECT_ROOT}/
  .rafaello/state/`:
  - `session.sqlite` — the entry store.
  - `session.sqlite-wal`, `session.sqlite-shm` — WAL.
  - `session.lock` — flock'd file (see §S5).
  - The state dir is created if missing
    (`fs::create_dir_all`).
- **S5.** Concurrent-access fence. m3's demo bar requires
  "second `rfl chat` against the same project errors
  instead of fighting for SQLite". m3 takes a flock on
  `${state_dir}/session.lock` at `SessionStore::open`
  time:
  - Open the lockfile with **`O_CLOEXEC`** so the fd is
    not inherited by the spawned `rfl-tui` child (pi-1
    #3: `rfl chat` opens the store before spawning the
    TUI, so without `O_CLOEXEC` the child would inherit
    the lock fd and keep the lock alive after parent
    failure with a stale holder pid). Concretely:
    `OpenOptions::new().read(true).write(true).create(true)
    .custom_flags(libc::O_CLOEXEC).open(...)`.
  - `nix::fcntl::flock(fd, Flock::LockExclusiveNonblock)`.
  - On `EWOULDBLOCK`, read the holder's pid from the file
    contents (the holder writes `std::process::id()` on
    successful lock) and return `SessionError::Locked {
    holder_pid }`.
  - The lock is released on `Drop` (close fd → kernel
    releases). No explicit release path.
  - **New negative test** `session_store_lock_fd_not_inherited_by_child.rs`:
    open store, spawn a probe child via `tokio::process::
    Command` (no special inheritance), check
    `/proc/<pid>/fd` (Linux) / `lsof -p <pid>` (macOS;
    gated `#[cfg(target_os = "macos")]`) and assert the
    lock fd is NOT in the child's fd table.
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
- **R2.** `pub struct RendererRegistry` — `BTreeMap<String,
  Arc<dyn Renderer>>` keyed by `kind` string. Constructed
  via `RendererRegistry::with_builtins()` which registers
  the eight built-in kinds at compile time. Plugin
  renderers are NOT registered (decision row 29, deferred
  to v2).
- **R3.** `pub struct RenderPipeline` — the entry-to-tree
  driver. Pi-1 #9 surfaced that round 1 conflated three
  distinct fallback paths; round 2 separates them per
  Stream E §6 + §6 last paragraph:
  - **Path A — unknown entry kind / renderer-unavailable.**
    The `entry.kind` is not present in the registry. Stream
    E §6 specifies:
    1. If `entry.fallback.text` (or `.markdown`) is set →
       emit `Block { children: [ Text { text:
       fallback.text, emphasis: None } ] }` (markdown is
       converted to a render-tree if the markdown renderer
       can run; the markdown path is itself a built-in
       kind in m3, so `fallback.markdown` re-enters Path A
       with `kind = "text", payload = { text:
       fallback.markdown, markdown: true }` rather than
       reusing the markdown renderer in-place — pi may
       prefer the simpler "always emit `Text`" form;
       round 2 keeps the markdown path because Stream E
       §6 specifies it).
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
  ScrollbackClass`, `nodes: BTreeSet<String>`. m3's TUI
  reports `nodes = full set`, so the downgrade path is
  only exercised by tests that synthesise reduced
  capabilities. Capabilities are passed to `RenderPipeline`
  by the caller; m3 has no `frontend.hello` handshake
  (deferred per row 27 + overview §10.1 banner) — the TUI
  capabilities are baked in as compile-time constants on
  the core side, indexed by attach id.
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
     `RFL_TUI_TEST_MODE` (optional; if `=1`, see step 5),
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
  4. **Headless test mode** (when `RFL_TUI_TEST_MODE = 1`):
     skip terminal init entirely; collect every received
     `bus.event` into an in-memory log; exit cleanly on
     the first `core.lifecycle.test_done` event with
     `exit_code = 0`. No keyboard handling, no crossterm
     calls. The log is reachable via stderr (one line
     per received entry, JSON-encoded) so test harnesses
     can parse-and-assert without involving a fake
     terminal. **Deterministic exit** (pi-1 #7) — the
     test harness publishes `core.lifecycle.test_done`
     after the last fixture entry; the 60 s self-timeout
     (§L1 — extended to `rfl-tui`) is a defensive
     backstop only, NOT the test's exit signal.
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
  1. Resolve project root (cwd by default; `--project-root`
     override).
  2. Resolve `rfl-tui` binary path (§C3 below).
  3. Open `SessionStore` → acquire flock → on
     `SessionError::Locked { holder_pid }` print a
     friendly error citing the holder pid and exit non-
     zero (matches the demo-bar negative).
  4. Build the `BrokerAcl` for m3: zero plugins, one
     frontend `tui` with `subscribe_patterns =
     ["core.session.**", "core.lifecycle.**"]`,
     `auto_subscribes = []`, `publish_topics = []`.
  5. `Broker::new(acl)` → `FrontendSupervisor::new` →
     construct the `RendererRegistry::with_builtins()`
     and the `RenderPipeline`. Build the
     `SessionController` bundling store + pipeline +
     broker (§S1).
  6. **Spawn the TUI first**:
     `frontend_supervisor.spawn(&compiled,
     &paths).await?`. This registers the frontend with
     the broker and the broker fan-out is now live for
     `frontend.tui`.
  7. **Wait for TUI subscription readiness.** The
     fittings server publishes `bus.event`
     notifications best-effort with a bounded
     drop-on-full sink (m2 §B7 + Stream B); without an
     explicit ready handshake, the parent could publish
     before the TUI's `BusEventHandler` is registered.
     m3 reuses m2's readiness pattern: the TUI sends a
     `core.lifecycle.frontend_ready` notification on its
     bus connection once its handler is wired (this is
     just a `peer.notify` from inside the rfl-tui bin's
     startup); `rfl chat` waits on a fittings extra-
     service for that notification before proceeding.
     (Round-1 omitted readiness; pi-1 #1 surfaced the
     gap for replay specifically, but it applies to
     every publish from `rfl chat`.) Bounded wait:
     5 s; on timeout, `rfl chat` errors out — the TUI
     is broken or hung.
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
  10. Wait on `frontend_handle.wait().await`. On TUI
      exit, shutdown the broker
      (`frontend_supervisor.shutdown().await`), drop
      the controller, close the store (releases flock).
- **C3.** `rfl-tui` binary path resolution (pi-1 #6 —
  round 1 said "compile-time constants" without
  specifying the resolution order). Lookup order:
  1. **`RFL_TUI_PATH` env override** (highest priority).
     Used by tests and by anyone running `rfl chat` from
     a non-installed cargo target dir.
  2. **Sibling of `current_exe()`**. `current_exe()
     .parent().join("rfl-tui")` — the canonical
     installed-binary location (homebrew, nix wrapper,
     `cargo install`).
  3. **`CARGO_BIN_EXE_rfl-tui` (compile-time)**. m3 may
     bake this in via `option_env!` for cargo-driven
     workflows (`cargo run -- chat` from inside the
     `rafaello` package), but it is NOT a workspace-
     wide guarantee — pi-1 #5 — so it is the lowest
     priority. The resolver does not panic if it is
     unset.
  Errors out with a typed `RflChatError::TuiPathUnresolved`
  message naming all three lookups it tried. Test
  coverage: a positive `rfl_chat_resolves_tui_via_env_override.rs`
  test sets `RFL_TUI_PATH` to a stub binary and asserts
  startup; a negative test unsets all three and asserts
  the typed error.
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
      pub fn inject_pre_register_fault(&self);
      pub fn inject_post_register_fault(&self);
      pub fn pre_register_fault_consumed(&self) -> bool;
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
- **H6.2.** Inject points (one-line annotations in the
  spawn body):
  - **Pre-register**: after socketpair / proxy /
    `tokio_command` allocation, immediately before
    `broker.register_plugin(...)`.
  - **Post-register**: after `register_plugin`, before
    the function returns `Ok(handle)` (i.e. between
    register and the `SpawnHandle` construction +
    transport `Server::serve` install).
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
    — arms pre-register fault; spawn returns
    `SpawnError::SandboxBuild`; Linux fd-count returns to
    the pre-spawn baseline (read `/proc/self/fd`); proxy
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

- **M1.1.** In `rafaello_core::validate::manifest`, extend
  the publishes-grant validation. Pi-1 #11: round-1
  wording was self-contradictory ("first segment must be
  one of `{plugin, frontend}`" then "frontend rejected"
  then test expects `frontend.foo` rejected). The
  correct rule, scoped to plugin manifests:
  - **For a plugin manifest**, every `publishes` entry's
    first segment must be exactly `plugin` (the plugin's
    own `plugin.<topic-id>.*` namespace) — and the
    existing topic-id-matches-this-plugin check applies.
  - Top-level segments `core` / `provider` / `frontend`
    are publish-authority of core / provider plugins /
    frontends, and a plugin manifest declaring publish
    on those is a category error. Reject with a typed
    `ManifestError::PublishNamespaceForbidden { topic,
    namespace }`.
  - Top-level segments not in `{core, provider, plugin,
    frontend}` (`evil.foo`, `random.thing`) are not
    valid namespaces at all. Reject with
    `ManifestError::PublishNamespaceUnknown { topic,
    namespace }`.
- **M1.2.** New test `tests/manifest_publishes_unknown_namespace_rejected.rs`
  in `rafaello-core` covering the four cases:
  `core.foo`, `provider.foo`, `frontend.foo`, `evil.foo`
  → all reject with a typed error variant on
  `ManifestError`.
- **M1.3.** Existing m1 tests must continue to pass; the
  tightening is additive (manifests that previously
  accepted unknown namespaces would have failed at runtime
  anyway).

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
- `supervisor_spawn_unwinds_after_register.rs` — see
  §H6.3.
- `supervisor_spawn_post_register_reaps_child.rs`
  (Linux-only) — see §H6.3.
- `supervisor_spawn_unwinds_after_socketpair.rs` — see
  §H6.3.
- `frontend_handle_wait_resolves_on_child_exit.rs` — see
  §F4 (uses the `RFL_TUI_TEST_MODE` headless mode but
  spawns `rfl-tui` from rafaello-core via a path
  obtained from `RFL_TUI_PATH` provided by the test-
  harness build script — pi may push back on the
  build-script path; alternative is to move this test
  to rafaello-tui crate. Round 2 keeps it under
  rafaello-core because the assertion is on
  `FrontendHandle::wait`, which lives in rafaello-core,
  and the `rfl-tui` invocation is incidental.)
- `frontend_handle_drop_does_not_leak_zombie.rs`
  (Linux-only) — see §F4.
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
- `tui_paint_panic_isolation.rs` — feed the TUI a
  render-tree that the painter panics on (a synthetic
  `Unknown` variant the test code wires); assert the
  TUI does NOT exit; assert it continues processing
  subsequent entries; assert clean exit on `test_done`.
  (This exercises §T5 — paint panic isolation.)

`rafaello/tests/`:

- `rfl_chat_demo_bar.rs` — **headline test, lands at
  the end of the milestone.** Spawn `rfl chat` against
  a tempdir project root with
  `RFL_HARNESS_FIXTURES=1` and `RFL_TUI_TEST_MODE=1`;
  let the parent + TUI run; assert the SQLite store
  contains nine `entries` rows after shutdown (eight
  built-in kinds + one unknown kind); assert each row's
  `kind`, `seq`, and `payload` match the harness
  inputs. The `rfl-tui` path is provided to the spawned
  `rfl chat` via `RFL_TUI_PATH` set by the test, which
  itself reads the workspace target dir from the
  `CARGO_TARGET_DIR` env (with a fallback to
  `target/debug/rfl-tui` from the workspace root, m2
  c30's pattern).
- `rfl_chat_resolves_tui_via_env_override.rs` — see
  §C3 positive.
- `rfl_chat_locked_session_errors_with_holder_pid.rs` —
  hold the project flock in pid A (an in-test
  `SessionStore::open`); spawn `rfl chat` in pid B
  against the same project root; B exits non-zero with
  stderr citing pid A.
- `session_store_lock_fd_not_inherited_by_child.rs` —
  see §S5.

#### Negative matrix

`rafaello-core/tests/`:

- `frontend_publish_on_reserved_namespace_rejected.rs`
  — synthesise a frontend publish on `core.foo`,
  `plugin.foo`, `provider.foo`; broker rejects with
  `PublishOnReservedNamespace { publisher:
  Publisher::Frontend("tui"), topic }`.
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
  holder_pid: A }`. Cross-platform Linux + macOS.
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
- `in_memory_broker_with_tui_acl()` — constructs a
  `Broker` with a single `tui` frontend ACL and the
  `core.session.**` / `core.lifecycle.**` subscribe set.
- `record_subscriber()` — registers an in-process plugin-
  shaped subscriber that records every event into a
  `Vec<BusEvent>`. Used by §I controller tests.

`tui_harness.rs` (in rafaello-tui):

- `tui_test_mode_command()` — wraps
  `Command::new(env!("CARGO_BIN_EXE_rfl-tui"))
  .env("RFL_TUI_TEST_MODE", "1")
  .env("RFL_BUS_FD", ...)` for the headless TUI mode.
- `parent_socket_pair()` — creates a socketpair and
  returns the parent end as a `tokio::net::UnixStream`,
  child fd as `OwnedFd` to inherit.

Manual validation: §I integration tests are the contract;
m3's `manual-validation.md` records:

1. `cargo test --workspace` green on Linux + macOS.
2. A real interactive `rfl chat` session against the in-
   test fixture-entry harness (i.e. with
   `RFL_HARNESS_FIXTURES=1`), screen-recorded; verify
   eight built-in kinds render readably; verify
   unknown-kind falls back to the author-supplied
   fallback text; verify Ctrl+C / `q` quit cleanly
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
- **L2.** Tests don't override the default; the 60 s ceiling
  is generous (every m2/m3 happy path test completes in
  under 5 s) but keeps a panicked / abandoned worktree
  from leaking fixture processes for hours (m2 c20 saw a
  >1 h orphan).
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
    pub fn inject_pre_register_fault(&self);
    pub fn inject_post_register_fault(&self);
    pub fn pre_register_fault_consumed(&self) -> bool;
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
- **macOS-only TUI smoke gate.** m3 dev runs on Linux; the
  test suite is platform-agnostic by default and macOS is
  verified post-hoc via origin CI per m2's §5.7
  precedent. Per-test macOS gates are added only as CI
  proves them needed.
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
   waits on `core.lifecycle.frontend_ready` before
   replaying / publishing any entry; without it, fan-
   out to a not-yet-subscribed frontend is silently
   dropped (Stream B notification sink is bounded,
   drop-on-full). The 5 s bounded wait is a defensive
   ceiling — real readiness should fire within hundreds
   of ms. The notification flows on the TUI's bus
   connection, not the bus itself, so no broker ACL is
   needed for it.
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
5. **Broker frontend extension** (B1-B7): ~3 commits.
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
  green on Linux inside the devshell. macOS CI (triggered
  by pushing the m3 branch to origin) is captured in
  `manual-validation.md` before milestone close; failures
  discovered there get a per-test macOS gate and a
  retrospective follow-up rather than blocking
  ratification.
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
