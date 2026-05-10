# m3 — sessions, local-spawned TUI, built-in rendering — scope

> **Status:** round-1 draft. Awaiting pi adversarial review.
> Inheritances from m2 retrospective (§5.1 unwind-window
> coverage, §4.4 fixture process leak, §2.8 m1
> publishes-grant unknown-namespace gap) explicitly addressed
> below — each of the three is in scope unless pi argues
> otherwise.

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

- **W1.** Add to `rafaello/Cargo.toml`'s `[workspace.dependencies]`:
  - `ratatui = "0.29"` (TUI rendering)
  - `crossterm = "0.28"` (terminal control; ratatui's default
    backend)
  - `rusqlite = { version = "0.32", features = ["bundled"] }`
    (bundles SQLite — no system-sqlite dependency; lockin /
    devshell remain free of system libsqlite3)
  - `tui-input = "0.10"` (line editor widget for the prompt
    box) — pi may argue against; the alternative is a hand-
    rolled minimal editor, ~80 LoC, no dep
  - `unicode-width = "0.2"` (terminal column counts;
    transitive in ratatui but used directly by m3's renderer
    text wrapping)
  - `ulid = "1"` (entry ids — overview §11 / Stream E §3
    specify ULID)
  - `chrono = { version = "0.4", default-features = false,
    features = ["serde", "clock"] }` (entry timestamps;
    `created_at` per Stream E §3)
  - `time = ...` is the alternative — m3 picks `chrono`
    because Stream E spells `created_at` as `date-time`
    string and `chrono`'s serde integration is the obvious
    fit.
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
- **F4.** `FrontendHandle::Drop` SIGKILLs the child if alive
  (mirrors m2 c26). Cooperative shutdown via
  `FrontendSupervisor::shutdown(self) -> ShutdownReport` —
  SIGTERM + 2 s grace + SIGKILL, identical to m2 c25.
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
- **B2.** New `Broker::register_frontend(&self, attach_id:
  AttachId, peer: PeerHandle) -> Result<RegisteredFrontend,
  BrokerError>`. Symmetric to `register_plugin`. Errors
  `NotInAcl` / `AlreadyRegistered`. RAII guard
  `RegisteredFrontend` mirrors `RegisteredPlugin`.
- **B3.** New `Broker::try_reserve_frontend_registration`
  precheck (m2 §B1 pattern).
- **B4.** Promote `PublisherIdentity::Frontend { attach_id:
  String }` from commented-future to live. Bus event
  serialisation gains the new variant — `kind: "frontend"`
  per the existing `#[serde(tag = "kind", rename_all =
  "snake_case")]` convention.
- **B5.** Publish authority for frontends (m3 enforcement,
  symmetric to m2 §B3 plugin path):
  - `frontend.<attach-id>.*` only — same exact-match-against-
    `publish_topics` rule as plugins.
  - Top-level segments other than `frontend.<own-attach-id>`
    → `PublishOnReservedNamespace { publisher: Publisher::
    Frontend(attach_id), topic }`.
  - `Publisher` enum gains a `Frontend(AttachId)` variant.
    `BrokerError`'s existing `publisher`-bearing variants
    accept the new arm without API breakage.
  - `auto_subscribes` is NOT publish authority for frontends
    either (m2 §B3's rule applies).
- **B6.** Fan-out (m2 §B7): a frontend subscriber receives
  `core.session.**` events the same way plugins do. Result-
  routing protection (m2 §B7's `plugin.<id>.tool_result` /
  `rpc_reply` no-fan-out) is unchanged — m4 territory. m3
  does not introduce any frontend-targeted result-routing
  carve-out.
- **B7.** `BrokerAcl` defence-in-depth pattern revalidation
  (m2 §B10) extends to the new `frontends` map. New tests:
  `broker_construct_with_invalid_frontend_pattern_rejected.rs`,
  `broker_register_frontend_unknown_attach_id_rejected.rs`,
  `broker_register_frontend_duplicate_rejected.rs`.

### S — session store (`rafaello_core::session`)

Sessions persist conversation entries to SQLite. m3 ships
the storage layer; the entry stream is generated only by the
in-test fixture-entry harness (no provider yet).

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
    m4+).
  - `pub fn SessionStore::append_entry(&self, entry:
    &Entry) -> Result<(), SessionError>` — INSERT into
    `entries`.
  - `pub fn SessionStore::load_entries(&self) ->
    Result<Vec<Entry>, SessionError>` — SELECT in `seq`
    order. Used at startup to replay history into the
    TUI.
  - `pub fn SessionStore::session_id(&self) -> &str` —
    ULID assigned at first open; persisted in a single-
    row `session_meta` table.
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
  - `nix::fcntl::flock(fd, Flock::LockExclusiveNonblock)`.
  - On `EWOULDBLOCK`, read the holder's pid from the file
    contents (the holder writes `std::process::id()` on
    successful lock) and return `SessionError::Locked {
    holder_pid }`.
  - The lock is released on `Drop` (close fd → kernel
    releases). No explicit release path.
  Cross-platform: Linux + macOS both support
  `LockExclusiveNonblock`. flock is per-fd, not per-pid —
  fork inheritance is not an issue because m3 never forks
  with the lock held.
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
      pub seq: Option<u64>,
      pub tags: Vec<String>,
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
  driver. `pipeline.render(entry, caps) -> RenderNode`
  algorithm:
  1. Look up `entry.kind` in the registry.
  2. If not present → emit `Unknown { kind, payload,
     fallback }` per Stream E §6 (the server-side
     downgrade for unknown kinds, matching the demo bar
     "in-test harness publishes one entry per built-in
     kind plus one unknown kind; TUI renders all of them
     with the unknown kind falling back").
  3. If present, **catch panics** with `std::panic::
     catch_unwind(AssertUnwindSafe(|| renderer.render(...)))`
     and on panic emit `Callout { kind: Warn, child:
     KeyValue { pairs: [("renderer", entry.kind),
     ("error", "panicked"), ("fallback", fallback.text)] }
     }`. The panic is logged at `tracing::error!` with
     entry id + kind for diagnosability.
  4. If the renderer returns `Err(_)` (non-panic), same
     fallback as the panic path but with `("error",
     err.to_string())`.
  5. After the renderer returns successfully, walk the
     tree and downgrade any nodes the frontend
     `Capabilities` reports it cannot handle to `Unknown
     { kind: "<node-name>", payload: <serialised-node>,
     fallback }`. (`fallback` here is the entry's
     top-level `fallback`, or a default Block if none.)
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
     `RFL_PROJECT_ROOT` (required; abs path), the
     fittings reserved env vars.
  2. Adopt the inherited fd as a tokio `UnixStream`,
     wrap with `fittings_transport::stdio::StdioTransport`-
     equivalent (m2 c19 already split on
     `tokio::net::UnixStream`).
  3. Build a `fittings_client::Client` with a
     `BusEventHandler` (notification handler) that turns
     `bus.event` notifications into channel sends to the
     UI thread.
  4. Initialise terminal (crossterm raw mode +
     alternate-screen optional — m3 picks alternate-
     screen for the v1 cut to keep scrollback handling
     simple). On exit, restore raw mode (Drop guard) so
     a panic in the TUI does not leave the user's
     terminal corrupted.
  5. Run the UI loop: redraw on each entry-event arrival,
     handle keyboard input (q to quit; arrow keys to
     scroll; no other commands in m3 — the prompt box is
     wired but submitting is a no-op since m3 has no
     `user_message` publish path).
  6. On exit, close the bus connection, restore the
     terminal, exit 0.
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
- **C2.** `rfl chat` flow:
  1. Resolve project root (cwd by default; `--project-root`
     override).
  2. Open `SessionStore` → acquire flock → on
     `SessionError::Locked { holder_pid }` print a
     friendly error citing the holder pid and exit non-
     zero (matches the demo-bar negative).
  3. Build the `BrokerAcl` for m3: zero plugins, one
     frontend `tui` with `subscribe_patterns =
     ["core.session.**", "core.lifecycle.**"]`.
  4. `Broker::new(acl)` → `FrontendSupervisor::new`.
  5. Replay session history: for every entry in
     `store.load_entries()`, render through `RenderPipeline`
     and `broker.publish_core("core.session.entry.replay",
     {entry, tree})`. (Replay is a separate event class
     so the TUI knows to skip the "fresh entry" animation;
     pi may push back on a separate topic vs. a metadata
     flag — round-1 takes the separate-topic position
     because it makes the wire-level shape unambiguous.)
  6. Spawn the TUI: `frontend_supervisor.spawn(&compiled,
     &paths).await?`.
  7. Run the in-test fixture-entry harness (only when
     `RFL_HARNESS_FIXTURES` is set — production
     `rfl chat` does NOT publish anything; m4 replaces
     this with the agent loop). The harness publishes
     one entry per built-in kind + one unknown kind, all
     with `stream_state: "final"`.
  8. Wait on the TUI handle. On TUI exit, shutdown the
     broker, drop the supervisor, close the store.
- **C3.** Error handling: every error path prints a
  human-readable message on stderr and returns a non-
  zero exit. Pi may push back on the exit code matrix
  (`SessionError::Locked` = 1, plan validation = 2, etc.)
  — round-1 leaves it as "1 for any error".

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
  the publishes-grant validation: every entry's first
  segment must be one of `{plugin, frontend}`. (`core.*`
  and `provider.*` are publish-authority of core / providers
  only; a plugin manifest declaring publish on those is a
  category error.) For `plugin.<topic-id>.*`, the existing
  topic-id-matches-this-plugin check applies; for
  `frontend.<attach-id>.*`, m1 manifests do not declare
  these (frontends are not plugins) — m3 rejects.
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

#### Positive matrix

- `frontend_register_with_broker.rs` — spawn the TUI via
  `FrontendSupervisor::spawn` against a real broker; the
  frontend ends up in the broker's frontend registry; its
  `attach_id` is `tui`.
- `frontend_subscribes_to_core_session_events.rs` —
  publish a `core.session.entry.finalized` event from
  core; the TUI's `BusEventHandler` receives it (verify
  via a `respond_peer_call`-style fixture in the TUI bin
  itself; m3 may add a `RFL_TUI_TEST_MODE` env var to
  short-circuit the real terminal initialisation — pi
  may push back, round-1 takes this position).
- `session_store_round_trip.rs` — open a `SessionStore`
  in a tempdir, append three entries (one of each `text`
  / `code_block` / `tool_call`), close, reopen, load —
  see the three back in order.
- `session_store_replay_after_restart.rs` — open store,
  append entries, drop, reopen with `rfl chat`-style
  flow, verify replay events fire.
- `renderer_pipeline_built_in_kinds.rs` — for each of the
  eight built-in kinds, render a sample entry through the
  full pipeline; assert the resulting tree matches a
  hand-written expected JSON (round-trip-stable).
- `renderer_pipeline_unknown_kind_falls_back.rs` —
  publish an entry with `kind = "myorg:custom"` and an
  author `fallback`; pipeline returns `Unknown { kind,
  payload, fallback }`; downgrade path applies.
- `renderer_pipeline_unknown_kind_no_fallback_uses_default.rs`
  — same but `fallback = None`; pipeline returns
  `Callout { kind: Warn, child: KeyValue }` per Stream
  E §6 second bullet.
- `renderer_pipeline_panic_returns_callout.rs` — register
  a test renderer that panics; pipeline catches the
  panic and returns a `Callout { kind: Warn }` with the
  entry's fallback text; the panic is logged at
  `tracing::error!`. (Uses `tracing-test`.)
- `renderer_capabilities_downgrade.rs` — render an entry
  whose tree contains an `Image` node; render with
  `Capabilities` reporting `image: ["none"]` and
  `nodes` excluding `Image`; the `Image` node downgrades
  to `Unknown { fallback }`.
- `rfl_chat_demo_bar.rs` — end-to-end: spawn `rfl chat`
  with `RFL_HARNESS_FIXTURES=1` against a tempdir
  project root; the TUI runs in headless test mode;
  one entry per built-in kind + one unknown kind get
  rendered; SQLite contains nine `entries` rows after
  shutdown. (This is the headline test; lands at the
  end of the milestone.)
- `supervisor_spawn_unwinds_after_register.rs` — see
  §H6.3.
- `supervisor_spawn_post_register_reaps_child.rs`
  (Linux-only) — see §H6.3.
- `supervisor_spawn_unwinds_after_socketpair.rs` — see
  §H6.3.

#### Negative matrix

- `frontend_publish_on_core_namespace_rejected.rs` — TUI
  attempts `frontend.tui.confirm_answer` (currently NOT
  in m3's `publish_topics` because m3 has no
  confirmation flow); broker rejects with
  `PublishOutsideGrant`. Same test class for `core.foo`,
  `plugin.foo`, `provider.foo` from the frontend →
  `PublishOnReservedNamespace`.
- `frontend_register_unknown_attach_id_rejected.rs` —
  ACL has only `tui`; `register_frontend("ide", ...)`
  fails with `BrokerError::NotInAcl`.
- `frontend_register_duplicate_rejected.rs`.
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
  in pid A; open same path in pid B (or in the same
  process via a different fd); B gets `SessionError::
  Locked { holder_pid: A }`. Cross-platform Linux +
  macOS.
- `session_store_schema_mismatch_errors.rs` — open store
  whose `session_meta.schema_version = "0"` (manually
  pre-seeded); errors with `SessionError::SchemaMismatch
  { found: "0", expected: "1" }`.
- `renderer_pipeline_panic_does_not_propagate.rs` (covered
  by the positive `panic_returns_callout`, listed under
  negatives because it asserts "the panic does not crash
  the pipeline").
- `manifest_publishes_unknown_namespace_rejected.rs` —
  see §M1.2.

### H — test harness (`rafaello-core/tests/common/m3_harness.rs`)

A new module under the existing `tests/common/` (m2's
`m2_harness.rs` set the precedent). m3's harness adds:

- `FixtureEntryBuilder` — fluent builder for a synthetic
  `Entry` with each built-in kind. Reused by every
  positive renderer test.
- `TestSessionStore::open_in_tempdir() -> (SessionStore,
  TempDir)` — common setup for store tests.
- `tui_test_mode_command(...)` — wraps
  `Command::new(env!("CARGO_BIN_EXE_rfl-tui")).env(
  "RFL_TUI_TEST_MODE", "1").env("RFL_BUS_FD", ...)` for
  the headless TUI mode.

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
8. **Replay event class.** Round-1 uses
   `core.session.entry.replay` as a separate topic
   class. Pi may argue for a metadata flag on
   `entry.finalized` instead. The trade-off:
   separate-topic is wire-explicit (a TUI built for m4+
   provider events doesn't have to special-case the
   finalized stream); a metadata flag keeps the topic
   set smaller. Round-1 picks separate-topic because
   the wire-shape stability matters more than topic
   parsimony in v1.
9. **m1 publishes-grant patch back-reach.** Touching
   `validate::manifest` in m3 is a small back-reach to
   m1 (m2 c04 set the precedent for back-reaching). The
   tightening is purely additive (manifests that
   previously accepted unknown namespaces would have
   failed at runtime anyway), so existing m1 tests
   continue to pass without modification. Mitigation:
   a one-line note in the commit body marks it as the
   m3-owned m1 patch and points at m2 retro §2.8.
10. **Demo-bar headline test (`rfl_chat_demo_bar.rs`)
    spawning a real subprocess from inside cargo test.**
    The pattern is a stretch of m2's
    `supervisor_spawn_fixture_happy_path` precedent —
    the rafaello bin spawned from a test, which then
    spawns rfl-tui as a subprocess. Two-level subprocess
    chains can leak processes if any layer panics.
    Mitigation: the L1 fixture self-timeout pattern
    extends to `rfl-tui`'s `RFL_TUI_TEST_MODE` (exit
    after 30 s default).

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
  rafaello/Cargo.toml --workspace --bins` green (verifies
  `rfl`, `rfl-tui`, and the unchanged `rfl-bus-fixture`
  with `--features test-fixture`).
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
  - **Replay topic class.** If round-1's
    `core.session.entry.replay` decision sticks, a new
    decisions row pins it. If pi argues us to a metadata
    flag instead, the row records that.
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
