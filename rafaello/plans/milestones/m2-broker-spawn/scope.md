# m2 — rafaello-core broker + locked plugin spawn — scope

> **Status:** round-7 draft, addressing pi-review-6 (3 blocking +
> 6 non-blocking — all tiny mechanical: tokio_command call site,
> ShutdownFailure shareable wait-error, broker_acl.plugins →
> broker.plugin_acl, plus stale-wording cleanups). Pi round-7
> review pending. Not yet owner-ratified.
> Trajectory: 555 → 14 → 8 → 8 → 6 → 3.

## Goal

Land the **first runtime** of rafaello: an in-process bus broker
plus a plugin supervisor that spawns subprocess plugins inside
lockin sandboxes, with the broker enforcing publish/subscribe
authority from m1's `BrokerAcl`. The broker authenticates
publishers from the connection identity (set at spawn time via
the inherited socketpair fd), not from message bodies. The
supervisor's only entry point is `spawn(plan: &CompiledPlugin,
paths: &SpawnPaths)`;
there is no debug bypass, no `spawn_unsandboxed`, no `RFL_INSECURE`.

m2 is the structural moment where m1's pure data-transformation
becomes a running system. Every later milestone (m3 TUI, m4
provider + agent loop, m5 confirmation/sinks) layers on the
broker + supervisor primitive m2 ships. The supervisor's only
spawn entry point is `PluginSupervisor::spawn(plan: &CompiledPlugin,
paths: &SpawnPaths)` (pi-3 §1) — a two-argument call where
`SpawnPaths` carries the project-root + private-state-dir the
caller computed from its own layout knowledge.

The deliverable is:
1. New modules in the existing `rafaello-core` crate (m1 owns
   the crate; m2 grows it): `bus`, `supervisor`, plus error
   surface additions.
2. A new `[[bin]]` target `rfl-bus-fixture` **inside the
   `rafaello-core` crate** (so `env!("CARGO_BIN_EXE_rfl-bus-fixture")`
   resolves from `rafaello-core`'s integration tests — pi-1 §5/§62/§63).
   The binary is test-only (`required-features = ["test-fixture"]`
   and the test runner enables the feature); not shipped in any
   release artifact.
3. Workspace dependencies wiring fittings + lockin + outpost +
   tokio + nix into `rafaello-core`.
4. Integration tests under `rafaello/crates/rafaello-core/tests/`
   exercising broker + supervisor end-to-end.

No new behaviour lands in the `rafaello`-bin (`rfl chat` is m3).

### Lock-correspondence claim, scoped honestly (pi-1 §22, §83, §84)

The "no hardcoded bypass" property holds at the **API surface
level**: `PluginSupervisor::spawn` accepts only `&CompiledPlugin
+ &SpawnPaths` and consults a `Broker` constructed from
`BrokerAcl`. There is no path that constructs a child process
from a raw entry path.

It does **not** hold against a caller who hand-mutates a
`CompiledPlugin` value (the struct fields are `pub` by m1's
choice). The supervisor performs `InvalidPlan` spot-checks for
the cases that would otherwise crash the lockin builder —
non-absolute paths, **ASCII control characters in paths**
(pi-4 §2 — lockin's builder asserts on these), reserved env
vars, topic-id ↔ canonical mismatch. It does not re-run
V3/digest/grant validation — that contract sits with
`compile::compile_plugin` and m1's `validate::lock`. The supervisor trusts a well-formed
`CompiledPlugin` the way m1's `compile_plugin` trusts a
prior-validated `Lock`. Production callers that obtain a
`CompiledPlugin` exclusively from `compile_plugin` get the full
guarantee; test code that hand-constructs values is responsible
for staying within the spot-check set.

The retrospective records this as a v1 contract; tightening to
an opaque/validated plan type is a v2 nice-to-have and is
explicitly listed in §"Acceptance summary" as a known scoped
deviation.

## Inputs

- `rafaello/plans/overview.md` §3 (process model), §4 (the bus
  — esp. §4.1 transport-vs-broker, §4.2–4.3 grammar +
  namespaces, §4.4 core re-emit, §4.5 envelopes, §4.6 reserved
  env vars), §5 (lifecycle, §5.5 private state), §6 (compiler
  outputs the supervisor consumes — m2 reads the plan, doesn't
  produce it), §15.6 (PeerHandle).
- `rafaello/plans/decisions.md` rows **3, 4, 5, 13, 17, 22, 32,
  37, 38**.
- `rafaello/plans/streams/a-security/rfc-security-model.md`
  §5 end-to-end (with the §5.7 v1-status banner), §6.4 / §6.5 /
  §6.8 / §6.9 (m2 closes), §7.2.6 `in_reply_to` enforcement
  table (m2 enforces a strict subset, see §B6), §7.4.1 v1-status
  banner (helpers deferred — m2 must not introduce surfaces).
- `rafaello/plans/streams/b-fittings/rfc-fittings-notifications.md`
  for the `PeerHandle` / `Server` / `Client` / `with_service`
  surface m0 landed.
- `rafaello/plans/glossary.md`.
- m1's existing `rafaello-core` surface: `compile::CompiledPlugin`
  (incl. `topic_id`, `entry_absolute`, `filesystem`, `network`,
  `env`, `limits`, `subscribe_patterns`, `publish_topics`,
  `auto_subscribes`, `tool_meta`, `provider_id`, `load`, `flags`),
  `broker_acl::{BrokerAcl, PluginAcl}` (incl. `tool_routes` —
  m2 stores it in the broker but does not yet route),
  `topic_id::derive`, `validate::topic::{validate_topic,
  validate_pattern, pattern_matches_topic}` (the matcher already
  exists and is re-exported — pi-1 §6 + §68), `lock::canonical_id::CanonicalId`,
  the typed errors in `error::*`.
- Lockin's public Rust API. Verified package coordinates
  (pi-1 §1; the round-2 wording about "m2 explicitly re-injects
  TMPDIR/TMP/TEMP after env_clear" was incorrect and is dropped
  per pi-3 §8 — current lockin does not expose the private-tmp
  path before spawn, so m2 cannot restore those vars; SP4
  step 12 documents the resulting deviation):
  - Crate `lockin/crates/sandbox`, **package name `lockin`**
    (not `lockin-sandbox`).
  - `lockin::Sandbox::builder() -> SandboxBuilder` with all
    builder methods named in m1 scope.
  - **Tokio path chosen (pi-4 §3).** m2 uses lockin's
    `--features tokio` API throughout the supervisor:
    `SandboxBuilder::tokio_command(self, program: &Path) ->
    anyhow::Result<lockin::tokio::SandboxedCommand>` consumes
    the builder. This avoids `spawn_blocking` for child wait
    and keeps the supervisor consistently async.
  - `SandboxBuilder::inherit_fd_as(self, fd: OwnedFd, child_fd:
    RawFd) -> Self` consumes the `OwnedFd` (pi-1 §24).
  - `lockin::tokio::SandboxedCommand::env`/`envs`/`env_remove`/
    `env_clear`/`current_dir` apply env + cwd; async
    `spawn() -> std::io::Result<lockin::tokio::SandboxedChild>`.
  - `lockin::tokio::SandboxedChild::{wait, try_wait, kill, id,
    as_child, as_child_mut, into_parts}`. `wait()` is async,
    returning `std::io::Result<ExitStatus>`. `id()` returns
    `Option<u32>` per `tokio::process::Child` (vs. sync
    `std::process::Child::id() -> u32`). `kill()` sends
    `SIGKILL`; graceful termination uses `nix` (pi-1 §28).
  - `SandboxedCommand::env_clear()` documented to remove
    `TMPDIR`/`TMP`/`TEMP` pointing at the sandbox private tmp
    (pi-1 §27). m2 cannot re-inject them: `SandboxedCommand`
    does not expose the private-tmp path. Plugins consequently
    see no TMPDIR/TMP/TEMP unless they fall under a granted
    write_dir; system /tmp is not in the grant. See SP4 step
    12 for the documented deviation.
- Outpost: `outpost::NetworkPolicy::from_allowed_hosts(...)`
  for parse-time validation (m1 already uses it),
  `outpost_proxy::start(policy).await -> std::io::Result<ProxyHandle>`
  (an async function — pi-2 non-blocking #1) with
  `ProxyHandle::listen_addr() -> SocketAddr`.
- fittings real coordinates verified (pi-1 §1, §3, §4, §92;
  pi-3 §4 corrected paths):
  - Crate paths: `fittings/crates/{wire, core, server, client,
    transport, spawn, macros}`. Package names match.
  - `fittings_core::message::JsonRpcId` is the canonical type
    re-exported by `fittings_wire`.
  - `fittings_core::error::FittingsError` is the canonical
    error.
  - `fittings_core::transport::Connector` is the connector
    trait (NOT `fittings_transport::Connector` — pi-3 §4).
  - `fittings_transport::stdio::StdioTransport::new(reader,
    writer, max_frame_bytes)` is the stdio transport's full
    path (the crate root only declares `pub mod stdio` — pi-3
    §4). Used over the split halves of a `tokio::net::UnixStream`.
  - `fittings_server::Server::new(service, transport)` builds
    a server; `Server::peer()` exposes the connection-scoped
    `PeerHandle`; `Server::serve(self) -> Future` runs the
    loop.
  - `fittings_client::Client::connect(connector)` connects;
    `Client::with_service(svc)` registers an inbound service;
    `Client::with_notification_handler(handler)` registers a
    notification handler; `Client::peer()` exposes outbound
    notify/call.

## In scope

Per-commit granularity is the driver's call when drafting
`commits.md`; this section names public API surface and the
test matrix.

### W — workspace dependencies

- **W1.** Add to `rafaello/Cargo.toml`'s `[workspace.dependencies]`
  (introduced in m1 c01). Verified coordinates per pi-1 §1, §2:
  - `tokio = { version = "1", features = ["rt-multi-thread",
    "macros", "io-util", "net", "sync", "time"] }`
  - `tracing = "0.1"`
  - `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`
    (dev-only — used by the `#[tracing_test::traced_test]`
    pattern in tests + by manual-validation; pi-1 §197–§199)
  - `async-trait = "0.1"` (every fittings `Service` impl needs
    it; pi-1 §2)
  - `fittings-core = { path = "../fittings/crates/core" }`
  - `fittings-server = { path = "../fittings/crates/server" }`
  - `fittings-client = { path = "../fittings/crates/client" }`
  - `fittings-transport = { path = "../fittings/crates/transport" }`
    (pi-1 §2 + §4 — needed for `StdioTransport`)
  - **No** `fittings-wire`: m2 uses only `JsonRpcId` and
    `FittingsError` from `fittings-core`'s re-exports (pi-1 §136).
  - `lockin = { path = "../lockin/crates/sandbox", features = ["tokio"] }`
    — package name confirmed as `lockin`; the `tokio` feature
    enables `lockin::tokio` async wait helpers used by SP5
    (pi-1 §1, §165, §166).
  - `outpost-proxy = { path = "../outpost/crates/outpost-proxy" }`
  - `nix = { version = "0.29", features = ["socket", "fs", "signal", "process"] }`
    — `signal` for `kill(SIGTERM)`; `process` for `Pid` /
    `WaitStatus` typing (pi-1 §2, §28). Verify version
    consistency against lockin's nix dep at `commits.md` time.
  - `parking_lot = "0.12"` — used by the broker's internal
    `Mutex<BrokerState>` (pi-2 §3). chosen because `parking_lot`
    locks do not poison, which simplifies the broker's
    error surface (pi-2 non-blocking #2).
  - `anyhow = "1"` — `SpawnError::Lockin.source` is
    `anyhow::Error` because lockin's `SandboxBuilder::command`
    returns `anyhow::Result` (pi-2 §3).
- **W1 (dev-deps).** Added to `[workspace.dependencies]` so any
  workspace member can pick them up:
  - `tempfile = "3"` (already in m1)
  - `serial_test = "3"` — env-mutating tests serialise via
    `#[serial(env)]` (pi-1 §60, §137, §332)
  - `tracing-test = "0.2"` — `#[traced_test]` for capturing
    broker `tracing::warn!` output in tests
- **W2.** `rafaello-core/Cargo.toml`:
  - `[dependencies]` adds the runtime W1 entries with
    `workspace = true`.
  - `[dev-dependencies]` adds `tempfile`, `serial_test`,
    `tracing-test`, `tracing-subscriber` with `workspace = true`.
  - `[features]` adds `test-fixture = []` (the gate for the
    fixture binary — see W3).
  - `[[bin]]` declares the fixture target inline: `name =
    "rfl-bus-fixture"`, `path = "src/bin/rfl_bus_fixture.rs"`,
    `required-features = ["test-fixture"]`.
- **W3.** Test-only fixture binary inside `rafaello-core`
  (pi-1 §5, §62, §63, §394–§403). Rationale:
  - Putting the fixture as a `[[bin]]` of `rafaello-core` is
    the **only** approach where `env!("CARGO_BIN_EXE_rfl-bus-fixture")`
    is reliably resolved by Cargo at integration-test compile
    time. A separate workspace-member crate is not built
    automatically when `cargo test -p rafaello-core` runs and
    Cargo does not export the env var across packages.
  - The `required-features = ["test-fixture"]` gate ensures
    the binary is **not** built by default `cargo build` in
    release contexts; `cargo test -p rafaello-core --features
    test-fixture` is the canonical test invocation.
  - The binary is small (~200 lines) and has no production
    runtime path. It is an artefact of the m2 acceptance
    harness, equivalent in spirit to the fittings examples
    used for m0 testing.
- **W4.** No other top-level workspace deps. `serde`,
  `serde_json`, `thiserror` already in `[workspace.dependencies]`
  from m1.

### B — bus broker (`rafaello_core::bus`)

The broker is the in-process publish/subscribe layer. It owns
no transport: each plugin connection is a `fittings`
`PeerHandle` registered with the broker via the supervisor.

> **§B1 RegisteredPlugin drop semantics (pi-4 non-blocking
> #2).** Dropping the broker-held `RegisteredPlugin` guard
> removes the broker's registration entry and drops the
> broker's clone of the plugin's `PeerHandle`; it does NOT
> guarantee that other clones of `PeerHandle` (held by the
> supervisor's `SpawnHandle`, by tests, etc.) close. The
> broker's only guarantee is "fan-out to this plugin stops".
> Connection close happens when the last `PeerHandle` clone
> AND the serve loop both drop.

- **B1.** New module `rafaello_core::bus`. Public surface:
  - `pub struct Broker` — cheap-clone handle wrapping
    `Arc<BrokerInner>`. Cloning `Broker` is the *only* sharing
    primitive; supervisor takes a `Broker` by value, not
    `Arc<Broker>` (pi-1 §87, §138). Internal state uses
    `parking_lot::Mutex<BrokerState>` over a `BTreeMap<CanonicalId,
    PluginConn>` for deterministic ordering (pi-1 §46, §522).
  - `Broker::new(acl: BrokerAcl) -> Result<Self, BrokerError>`
    (pi-2 §1). Construction runs the §B10 defence-in-depth
    pattern/topic revalidation. On failure: `InvalidTopic`,
    `InvalidPattern`, or any other validation-level
    `BrokerError`. **No event is emitted by construction
    itself** (any subscriber would not be registered yet);
    boot emission is the explicit `publish_boot` call below.
  - `Broker::register_plugin(&self, canonical: CanonicalId,
    peer: PeerHandle) -> Result<RegisteredPlugin, BrokerError>`.
    Returns an RAII guard. Errors `NotInAcl(canonical)` or
    `AlreadyRegistered(canonical)` (pi-1 §11, §139). The guard
    is `!Clone` (RAII single-owner) but **`Send + Sync`** — it
    holds only an `Arc<BrokerInner>` backref + the canonical
    id (pi-2 §9). On drop it removes the broker's registration
    entry and drops the broker's clone of the plugin's
    `PeerHandle`; per the §B1 preface (pi-4 non-blocking #2,
    pi-5 non-blocking) it does NOT guarantee that other
    `PeerHandle` clones close — fan-out simply stops.
  - `Broker::try_reserve_registration(&self, canonical: &CanonicalId)
    -> Result<(), BrokerError>` (pi-2 §12) — cheap precheck
    used by `PluginSupervisor::spawn` Phase A to reject
    `NotInAcl` / `AlreadyRegistered` BEFORE allocating
    socketpair / proxy / sandbox. Does NOT actually reserve
    a slot (no token returned); the actual reservation
    happens at `register_plugin` after spawn. There is a
    benign race window (another thread could register
    between this check and `register_plugin`); m2's only
    consumer is the supervisor whose registry is itself
    serialised, so the race is not exploitable.
  - `Broker::publish_core(&self, topic: &str, payload: Value)
    -> Result<(), BrokerError>`. Topic must start with `core.`
    (parsed structurally per §B3) and pass grammar revalidation.
    No publisher exclusion (core publishes to itself = no-op
    since core is not registered as a subscriber; pi-1 §128).
    For lifecycle/observability, m2 uses this to emit
    `core.lifecycle.publish_rejected` (see §B9). The
    `core.lifecycle.boot` event is emitted via
    `publish_boot` (next bullet), NOT automatically by
    `Broker::new` (pi-3 §2).
  - `Broker::publish_boot(&self) -> Result<(), BrokerError>`
    — sugar over `publish_core("core.lifecycle.boot",
    json!({"version": "<rafaello-core version>",
    "plugin_count": <ACL plugin count>}))`. Tests call this
    explicitly after registering observers (pi-5 non-blocking
    — without subscribers, the call is a no-op fan-out).
    `PluginSupervisor::new` does NOT call this automatically
    (an early call before any observer is registered would
    deliver to nobody — pi-5 non-blocking #3); m3's `rfl
    chat` is the natural site to call it once the TUI
    frontend is registered.
  - `Broker::handle_plugin_publish(&self, canonical: &CanonicalId,
    raw_params: &Value) -> Result<(), BrokerError>`. Takes
    `raw_params` (the raw JSON `params` of the inbound
    `bus.publish` notification) so payload-decode errors
    surface as `BrokerError::InvalidPayload` from the broker
    itself (pi-1 §12). Decoding to `PublishMsg` with
    `deny_unknown_fields` happens inside this function. Also
    requires the canonical to be currently **registered**
    (returns `NotRegistered` if not — pi-1 §140, §141, §526),
    so a stale service whose guard already dropped cannot
    publish.
  - `Broker::contains_plugin(&self, canonical: &CanonicalId) -> bool`
    — checks ACL membership (NOT live registration; pi-3
    non-blocking #3). Use `is_registered` for live-registry
    checks if needed (m2 doesn't expose one publicly — only
    `try_reserve_registration` returns the relevant signal).
  - `Broker::plugin_acl(&self, canonical: &CanonicalId) -> Option<PluginAcl>`
    — returns a clone of the per-plugin ACL entry for the
    supervisor's Phase A topic-id consistency + provider-id
    inspection (pi-3 §5: this is the canonical lookup path;
    test helpers MUST NOT reach for an `acl()` accessor). Returns
    `None` if `canonical` not in ACL.
  - `Broker::shutdown(&self)` — drains and unregisters every
    plugin; idempotent. Used by `PluginSupervisor::shutdown`.
- **B2.** `BrokerError` typed enum (lives in
  `rafaello_core::error`). Every variant that carries publisher
  identity uses `Publisher` (defined below) so the same variant
  represents both plugin and core publishers (pi-2 §2):
  ```rust
  pub enum Publisher {
      Core,
      Plugin(CanonicalId),
  }
  ```
  Variants:
  - `NotInAcl(CanonicalId)` — canonical absent from the static
    `BrokerAcl`.
  - `NotRegistered(CanonicalId)` — canonical present in ACL
    but no live registration.
  - `AlreadyRegistered(CanonicalId)`.
  - `UnknownNamespace { publisher: Publisher, topic: String }`
    — top-level segment is **not** one of the known set
    `{core, provider, plugin, frontend}` (pi-1 §7, §251,
    §471; pi-5 §3). Example: `evil.foo`, `random.thing`.
    Distinct from `PublishOnReservedNamespace` per pi-5 §3.
  - `PublishOnReservedNamespace { publisher: Publisher, topic: String }`
    — top-level segment IS one of `{core, provider, plugin,
    frontend}`, but the publisher is not authorised on that
    namespace. Plugin published `core.*`/`provider.*`/
    `frontend.*`/foreign `plugin.<other>.*`, OR core
    published a non-`core.*` known-namespace topic
    (pi-2 §2).
  - `PublishOutsideGrant { canonical: CanonicalId, topic: String }`
    — plugin published a `plugin.<own-topic-id>.*` topic not in
    its lock-granted `publish_topics` set. Plugin-only by
    construction (core has no grant; its only restriction is
    namespace).
  - `InvalidTopic { publisher: Publisher, topic: String, reason: String }`
    — grammar revalidation. `reason` is the `Display` of the
    underlying `ValidationError` (current m1 `ValidationError`
    derives only `Debug + Error`, not `Clone + PartialEq` — pi-2
    §4 — so storing a `String` keeps `BrokerError` cheap to
    derive without reaching into m1).
  - `InvalidPattern { reason: String }` — used by `Broker::new`
    when `BrokerAcl` carries a malformed subscribe pattern
    (pi-2 §2).
  - `InvalidPayload { publisher: Publisher, reason: String }`
    — params decode failure (`serde_json` error mapped to
    `String`) or body shape violation. `publisher` because
    rejection events need it.
  - `InvalidInReplyTo { canonical: CanonicalId, topic: String,
    reason: InReplyToReason }` — covers both missing and wrong
    arity (pi-1 §43). `InReplyToReason = Missing | EmptyArray |
    UnexpectedMultiple` enum. Plugin-only by class.
  - `Internal { detail: String }` — broker-internal failures
    (channel send error during shutdown, etc.). Not a security
    boundary. (`parking_lot` does not poison, so lock-poisoning
    is not a possible cause — pi-2 non-blocking #2.)
  Top-level `Error` enum gets `#[from]` arms for `BrokerError`
  and `SpawnError`. `BrokerError` derives only `Debug`
  (`thiserror`-derived). It is **not** `Clone` and **not**
  `PartialEq` (pi-2 §4). Tests use `matches!` on variants;
  shared substring assertions on the `reason: String` field
  cover the few cases that need finer comparison.
- **B3.** **Publish-authority enforcement (structural).**
  Per pi-1 §7, §8, §97, the broker parses topics
  structurally (split by `.`) instead of using `starts_with`.
  Pseudocode for `handle_plugin_publish`:
  1. Decode `raw_params` to `PublishMsg`. On error →
     `InvalidPayload`.
  2. Run `validate::topic::validate_topic(&msg.topic)`. On
     error → `InvalidTopic`. (Grammar before namespace; pi-1 §97.)
  3. Split `msg.topic` into segments. Look at `segments[0]`:
     - Not in `{core, provider, plugin, frontend}` →
       `UnknownNamespace`.
     - `core` / `provider` / `frontend` → `PublishOnReservedNamespace`
       (plugins never publish on these in m2).
     - `plugin`:
       - If `segments.len() < 3` (e.g. `plugin.<id>` two-segment
         only) → `PublishOnReservedNamespace` (pi-1 §8). Two-
         segment `plugin.<id>` is grammar-valid but semantically
         empty; treat as outside any plugin's authority.
       - If `segments[1] != publisher_acl.topic_id` →
         `PublishOnReservedNamespace` (cross-plugin masquerade).
       - Else: check `msg.topic` is exact-string-member of
         `publisher_acl.publish_topics`. If not →
         `PublishOutsideGrant`.
  4. Per-topic-class checks (§B6).
  5. Otherwise authorised; proceed to fan-out (§B7).
  
  **`auto_subscribes` is NOT publish authority** (pi-1 §9, §69,
  §248). It is a subscribe grant only. Publishing on a topic
  in `auto_subscribes` and not in `publish_topics` is
  `PublishOutsideGrant`.
- **B4.** **`bus.publish` notification params shape.** The
  on-wire type:
  ```rust
  #[derive(Debug, Clone, Deserialize)]
  #[serde(deny_unknown_fields)]
  pub struct PublishMsg {
      pub topic: String,
      pub payload: serde_json::Value,
      #[serde(default)]
      pub in_reply_to: Option<Vec<JsonRpcId>>,
      #[serde(default)]
      pub taint: Option<Vec<TaintEntry>>,
  }
  
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  #[serde(deny_unknown_fields)]  // pi-1 §44, §309
  pub struct TaintEntry {
      pub source: String,
      pub detail: Option<String>,
  }
  ```
  Required: `topic` (string), `payload` (any JSON value
  including `null` — pi-1 §312). `in_reply_to` and `taint` are
  optional; absent fields are `None`. **`request_id` is not
  carried in m2 events** (pi-1 §41, §95, §145): every fittings
  notification already has a connection-implicit message id
  the transport layer assigns; the overview §4.5 envelope
  `request_id` field is meaningful only for events that
  themselves are responses to something, which m4 introduces.
  m2's retrospective records this staging.
- **B5.** **Topic-grammar revalidation.** Run before any
  ACL check (pi-1 §97). For `publish_core`, topic must start
  with `core.` after grammar — and per pi-1 §128, `publish_core`
  performs the same structural namespace check as plugin
  publishes, just with `Some` instead of `None` for publisher
  identity.
- **B6.** **`in_reply_to` enforcement (m2 subset).** Per
  security RFC §7.2.6 and pi-1 §43, §307:
  - `plugin.<id>.tool_result` and `plugin.<id>.rpc_reply`:
    `in_reply_to` MUST be present, MUST be exactly one entry.
    Missing → `InvalidInReplyTo { reason: Missing }`. Empty
    array → `InvalidInReplyTo { reason: EmptyArray }`. ≥ 2
    entries → `InvalidInReplyTo { reason: UnexpectedMultiple }`.
    m2 does **not** validate that the referenced id exists
    (no correlation map yet — m4 adds it).
  - All other `plugin.<id>.*` topics: `in_reply_to` is
    optional. If present, m2 stores it verbatim and does NOT
    superset-check taint (m4 territory).
  - `provider.*`, `frontend.*`, `core.*` from plugins: rejected
    earlier in §B3, never reach this check.
- **B7.** **Fan-out.** After authorisation (§B3) and class
  checks (§B6), construct a `BusEvent` (§B8), serialise to
  `serde_json::Value` via `serde_json::to_value(&bus_event)`,
  then for each registered plugin **other than** the
  publisher, check the set `subscribe_patterns ∪ auto_subscribes`
  against the topic using `validate::topic::pattern_matches_topic`
  (the existing m1 helper — pi-1 §6). If matched, call
  `peer.notify("bus.event", value.clone())` on that plugin's
  `PeerHandle`. (pi-4 §4: `PeerHandle::notify` takes
  `serde_json::Value`, not a generic `Serialize` ref —
  serialise once outside the loop, clone per fan-out.) The publishing plugin is excluded from its own
  fan-out (pi-1 §245, §247). Fan-out uses a snapshot of the
  registry (clone the `PeerHandle` list under lock, release
  lock, then iterate; pi-1 §46, §1022) so notify cannot
  deadlock against `RegisteredPlugin::drop`.
  - **Result-routing protection (pi-1 §14).** Topics
    `plugin.<id>.tool_result` and `plugin.<id>.rpc_reply` are
    **NOT** delivered to other plugin subscribers in m2. Their
    publish authority + arity check still runs (so plugins
    that hand-craft these get the same broker-level diagnostics
    m4 will see), but the fan-out path is a no-op log
    (`tracing::debug!`). m4 wires the canonical re-emission
    (`core.session.tool_result`) and that path is the only
    thing other plugins observe. This preserves the security
    RFC §5.4.1 invariant that direct delivery of
    `plugin.<id>.tool_result` is forbidden, without m2 having
    to ship the validation+re-emission machinery.
  - `peer.notify` returning `Ok(())` does NOT mean delivered —
    the bounded notify sink may have dropped the frame
    silently per fittings semantics (pi-1 §47, §1030). Tests
    that need observation use round-trip patterns, not raw
    counts.
  - `peer.notify` returning `Err(_)` (transport closed) is
    logged at `tracing::warn!` and does NOT fail the publish.
    Best-effort delivery (pi-1 §47, §89).
  - `bus.event` is m2's chosen outbound notification method
    name (vs. the inbound `bus.publish`). The asymmetry is
    intentional: outbound carries publisher identity (§B8) and
    is a different schema; reusing `bus.publish` would
    conflate directions.
- **B8.** **`BusEvent` outbound type** (pi-1 §13, §527–§534):
  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct BusEvent {
      pub topic: String,
      pub payload: serde_json::Value,
      pub publisher: PublisherIdentity,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub in_reply_to: Option<Vec<JsonRpcId>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub taint: Option<Vec<TaintEntry>>,
  }
  
  #[derive(Debug, Clone, Serialize)]
  #[serde(tag = "kind", rename_all = "snake_case")]
  pub enum PublisherIdentity {
      Core,
      Plugin {
          canonical: String,  // CanonicalId rendered to wire string
          topic_id: String,
      },
      // Future m4 / m5 / m3:
      // Provider { canonical, provider_id },
      // Frontend { attach_id },
  }
  ```
  Tests asserting "publish authority was the fixture's
  canonical id" inspect `event.publisher` (pi-1 §13, §186).
- **B9.** **`core.lifecycle.publish_rejected` event.** Emitted
  by the broker after every rejection (`UnknownNamespace`,
  `PublishOnReservedNamespace`, `PublishOutsideGrant`,
  `InvalidTopic`, `InvalidInReplyTo`, `InvalidPayload` —
  pi-2 non-blocking #3). Schema:
  ```json
  {
    "canonical": "<canonical-id-or-null>",
    "topic": "<topic-or-null-if-decode-failed>",
    "code": "<machine-readable-error-code>",
    "message": "<human-readable-detail>"
  }
  ```
  - `code` is one of `unknown_namespace`,
    `publish_on_reserved_namespace`, `publish_outside_grant`,
    `invalid_topic`, `invalid_in_reply_to_missing`,
    `invalid_in_reply_to_empty`,
    `invalid_in_reply_to_multiple`, `invalid_payload` (pi-1
    §122, §178).
  - `topic: null` only for `invalid_payload` decode failures
    that happened before topic extraction (pi-1 §178, §179).
    For best-effort topic recovery on decode failure, the
    broker tries a permissive `Value`-level extraction first
    and includes the topic only if it parses as a string.
  - The rejection event is published via an internal
    `publish_core_internal` path (pi-1 §15) that bypasses the
    structural namespace re-check (the broker has already
    constructed it correctly); it still runs grammar
    revalidation and fan-out. Subscribers of `core.lifecycle.**`
    receive it. **No recursion possible**: rejection-event
    construction does not itself call rejection emission.
  - `core.lifecycle.boot` schema: `{ "version": "<rafaello-core
    version>", "plugin_count": <n> }`. Emitted only via the
    explicit `Broker::publish_boot()` call (pi-3 §2).
- **B10.** **`Broker::new` defence-in-depth pattern validation**
  (pi-1 §70, §99, §270). On construction, re-run
  `validate_topic` against every `publish_topics` entry and
  `validate_pattern` against every `subscribe_patterns` /
  `auto_subscribes` entry across the supplied `BrokerAcl`. On
  failure, return `Result<Self, BrokerError>` with `InvalidTopic`
  / `InvalidPattern`. This catches hand-constructed `BrokerAcl`
  values that bypassed `broker_acl::compile`.

### SP — plugin supervisor (`rafaello_core::supervisor`)

The supervisor owns child processes, lockin sandboxes, the
outpost proxy lifecycle (when proxy mode is configured), and
the socketpair plumbing that authenticates the bus connection.

- **SP1.** New module `rafaello_core::supervisor`. Public
  surface (pi-1 §17, §29, §100, §138, §368):
  ```rust
  pub struct PluginSupervisor { /* private */ }
  
  pub struct SupervisorConfig {
      pub shutdown_grace: Duration,        // default 200ms
      pub fittings_max_frame_bytes: usize, // default 1 MiB
  }
  impl Default for SupervisorConfig { ... }
  
  // Per-spawn paths the caller computes from its own layout
  // knowledge (pi-2 §13, pi-3 §1). Both fields must be
  // absolute (Phase A SP4.3 verifies). project_root is the
  // process's cwd; private_state_dir is the per-plugin write
  // grant the m1 compiler injected and the supervisor
  // create_dir_alls before spawn. private_state_dir SHOULD
  // be derivable from `project_root` + topic-id form (m1
  // convention), but the supervisor does NOT enforce that
  // relationship — the harness/m3 callers compute paths
  // their way; supervisor only requires absolute + writable.
  pub struct SpawnPaths {
      pub project_root: PathBuf,
      pub private_state_dir: PathBuf,
  }
  
  impl PluginSupervisor {
      // PluginSupervisor::new does NOT return Result and
      // does NOT auto-emit boot (pi-5 non-blocking #3).
      // Callers responsible for calling broker.publish_boot()
      // once observers are registered.
      pub fn new(broker: Broker, config: SupervisorConfig) -> Self;
      pub async fn spawn(&self, plan: &CompiledPlugin,
                         paths: &SpawnPaths)
          -> Result<SpawnHandle, SpawnError>;
      pub async fn shutdown(self) -> ShutdownReport;
  
      // Test-only escape hatch — see §SP2 below.
      #[cfg(any(test, feature = "test-fixture"))]
      pub fn with_extra_service(broker: Broker,
          config: SupervisorConfig,
          factory: ExtraServiceFactory) -> Self;
  }
  
  pub struct SpawnHandle { /* Arc-shared inside */ }
  impl SpawnHandle {
      pub fn canonical(&self) -> &CanonicalId;
      pub fn topic_id(&self) -> &str;
      // pi-4 §3: tokio::process::Child::id returns Option<u32>;
      // expose the Option directly. Returns None after wait
      // observes exit (consistent with tokio semantics).
      pub fn child_pid(&self) -> Option<u32>;
      pub fn peer(&self) -> &PeerHandle;
      // pi-4 §8 + pi-5 §1: cached status using
      // `tokio::sync::watch<Option<Arc<ReaperOutcome>>>`
      // initialised to None. The reaper task awaits
      // child.wait().await, then sends Some(Arc::new(outcome))
      // exactly once. A separate watcher task awaits the
      // reaper's JoinHandle; on JoinError it sends
      // Some(Arc::new(ReaperOutcome::ReaperPanicked)). Late
      // callers see the cached Arc immediately. Returning
      // Arc<ReaperOutcome> sidesteps the `io::Error: !Clone`
      // problem (pi-5 §1).
      pub async fn wait(&self) -> Arc<ReaperOutcome>;
      pub fn try_wait(&self) -> Option<Arc<ReaperOutcome>>;
  }
  
  pub enum ReaperOutcome {
      Exited(std::process::ExitStatus),
      WaitFailed(std::io::Error),    // never cloned
      ReaperPanicked,
  }
  
  pub struct ShutdownReport {
      pub clean: Vec<CanonicalId>,
      pub forced: Vec<CanonicalId>,         // SIGKILLed after grace
      pub failed: Vec<(CanonicalId, ShutdownFailure)>,
  }
  pub enum ShutdownFailure {
      SignalSendFailed(nix::errno::Errno),
      // Shareable projection of the reaper's wait error
      // (pi-6 §2): shutdown observes ReaperOutcome through
      // the same Arc<> cache as everyone else and cannot
      // own a fresh non-cloneable io::Error. Stores the
      // ErrorKind + message so the report is informative
      // and PartialEq-friendly without holding the original.
      WaitFailed { kind: std::io::ErrorKind, message: String },
      ReaperPanicked,
  }
  ```
  - `&self` (not `&mut self`) on `spawn` so multi-spawn tests
    just hold multiple `SpawnHandle`s without lifetime games
    (pi-1 §17). Internal state guarded by `parking_lot::Mutex`,
    plus a separate `parking_lot::Mutex<HashSet<CanonicalId>>
    in_flight` set for duplicate-spawn protection on a
    multi-thread runtime (pi-3 §7). Per pi-6 non-blocking #5:
    Phase A inserts canonical into `in_flight` as an RAII
    guard (drop = remove); the guard is held through Phase B;
    on successful `broker.register_plugin` the guard is
    dropped immediately (registration is now source of truth);
    every Phase A/B failure path also drops the guard so
    retries are unblocked.
  - `SpawnHandle` is cloneable (`Arc`-backed); the *underlying
    process* is killed when the **last** `SpawnHandle` clone
    plus the supervisor's internal handle both drop (pi-1 §54,
    §364, §510). Supervisor holds one clone in its registry;
    callers hold others. RAII intent: dropping the supervisor
    or removing from its registry triggers cleanup if nobody
    else holds a handle.
  - `wait` / `try_wait` give tests deterministic exit-status
    access (pi-1 §18). Implementation uses a per-spawn reaper
    task that owns the `lockin::tokio::SandboxedChild` and
    awaits `child.wait().await` directly (m2 uses lockin's
    `tokio` feature throughout — pi-4 §3).
    The reaper publishes an `Arc<ReaperOutcome>` to a
    `tokio::sync::watch` channel; `wait` resolves on the
    transition to `Some` (pi-5 §1).
  - `shutdown(self)` is the cooperative path: SIGTERM every
    live plugin; wait up to `config.shutdown_grace`; SIGKILL
    survivors. Returns a `ShutdownReport` instead of a single
    `Result` so per-plugin partial failures are visible
    (pi-1 §102, §371). Consumes `self` deliberately — once
    initiated, the supervisor is gone.
  - `Drop` for `PluginSupervisor` is best-effort
    **synchronous** SIGKILL only (pi-1 §28, §371; pi-6
    non-blocking #4): it cannot `await`, so it sends SIGKILL
    via `nix::sys::signal::kill` and relies on the
    already-running per-spawn reaper task (which owns the
    child) to perform the actual `wait()` and reap. The
    reaper task continues until the child exits even after
    supervisor Drop. Tests that want deterministic graceful
    shutdown call `shutdown(self).await` instead, which
    blocks on the reaper.
- **SP2.** **Test-only `ExtraServiceFactory` hook**
  (pi-1 §117, §169, §428, §515–§518). The per-connection
  fittings `Service` impl built by the supervisor handles
  `bus.publish` (notification, `bus.publish` only). Tests need
  to additionally serve `core.fixture.*` request methods so
  the fixture's `peer.call` round-trips have a counterparty.
  
  Production `PluginSupervisor::new` builds a service that
  returns `MethodNotFound` for anything other than
  `bus.publish`. The test-only `with_extra_service`
  constructor takes a factory:
  ```rust
  type ExtraServiceFactory =
      Arc<dyn Fn(CanonicalId) -> Box<dyn Service + Send + Sync>
          + Send + Sync>;
  ```
  The factory is invoked once per spawn; the resulting service
  is composed via a small router: `bus.publish` →
  broker; `core.fixture.*` → factory's service; everything
  else → `MethodNotFound`.
  
  Gated `#[cfg(any(test, feature = "test-fixture"))]` so it
  is unreachable from a release `rafaello-core`. m4 will need
  a real "compose extra services" mechanism for renderer
  invocations; m2 ships the test-only seed.
- **SP3.** `SpawnError` typed enum (pi-1 §19, §28, §85,
  §154, §475–§479):
  ```rust
  #[non_exhaustive]
  pub enum SpawnError {
      NotInAcl(CanonicalId),
      AlreadyRegistered(CanonicalId),
      InvalidPlan { canonical: CanonicalId, reason: InvalidPlanReason },
      EntryNotExecutable { canonical: CanonicalId, path: PathBuf },
      Lockin { canonical: CanonicalId, source: anyhow::Error },
      Spawn { canonical: CanonicalId, source: std::io::Error },
      ProxyStart { canonical: CanonicalId, source: std::io::Error },
      Socketpair { canonical: CanonicalId, source: nix::errno::Errno },
      FittingsBuild { canonical: CanonicalId, source: FittingsError },
      ReservedEnvInPlan { canonical: CanonicalId, var: String },
      // Post-spawn transport-setup I/O errors (pi-2 §8 +
      // pi-4 non-blocking #1): set_nonblocking,
      // UnixStream::from_std failures.
      TransportSetup { canonical: CanonicalId, source: std::io::Error },
      // Private-state directory create_dir_all failure
      // (pi-4 non-blocking #5 — clearer than reusing
      // TransportSetup).
      PrivateStateDirCreate { canonical: CanonicalId, path: PathBuf, source: std::io::Error },
  }
  
  pub enum InvalidPlanReason {
      NonAbsolutePath { kind: PathKind, path: PathBuf },
      ControlCharsInPath { kind: PathKind, path: PathBuf },  // pi-4 §2
      TopicIdMismatch { expected: String, got: String },
      // (no ReservedEnvVar — pi-4 non-blocking #4 dropped the
      //  redundant variant; SpawnError::ReservedEnvInPlan is
      //  the single error path for reserved env collisions.)
      NetworkAllowHostsInvalid { source: outpost::DomainPatternParseError },
      ProviderNotInM2 { provider_id: String },  // pi-2 §11
  }
  
  pub enum PathKind {
      ReadPath, ReadDir, WritePath, WriteDir,
      ExecPath, ExecDir, EntryAbsolute,
      // pi-4 §1: SpawnPaths fields get their own kinds.
      ProjectRoot, PrivateStateDir,
  }
  ```
  (`TransportSetup` is now in the enum block above per pi-4
  non-blocking #1 — it covers `set_nonblocking` /
  `UnixStream::from_std` failures during SP4 step 14, distinct
  from `Spawn` and from `FittingsBuild`.)
  - `lockin` returns `anyhow::Result` from `command(...)`;
    `Lockin.source` is `anyhow::Error` (pi-1 §19 #3, §155).
  - `fittings_core::error::FittingsError` is the canonical
    type (pi-1 §19 #4).
  - `Socketpair.source` is `nix::errno::Errno` for nix 0.29
    (pi-1 §154); verify at `commits.md` time.
  - `Spawn.source` and `ProxyStart.source` are
    `std::io::Error` (pi-1 §163, §483).
- **SP4.** **Spawn sequencing.** All cheap validation runs
  first; resource allocation only after the plan passes
  defence-in-depth checks (pi-1 §156, §157, §266, §489). The
  rewritten sequence:
  
  **Phase A — validate plan (synchronous, no resources):**
  1a. **Acquire supervisor `in_flight` reservation** (pi-5
     §4): atomically insert `plan.canonical` into the
     supervisor's `Mutex<HashSet<CanonicalId>>` `in_flight`
     set. If already present, return `SpawnError::AlreadyRegistered`
     immediately. The reservation is held as a local guard
     (RAII) — every Phase A / Phase B failure path drops it
     so a retry can succeed; success path moves the guard
     into the `SpawnHandle`'s shared state and only releases
     it after broker `register_plugin` succeeds. This closes
     the concurrent-spawn race that
     `try_reserve_registration` alone cannot catch.
  1b. `broker.try_reserve_registration(&plan.canonical)` →
     errors map to `SpawnError::NotInAcl` /
     `SpawnError::AlreadyRegistered` (pi-2 §12). This catches
     ACL membership and any cross-supervisor conflict the
     `in_flight` set cannot see (m2's only consumer is one
     supervisor, but the broker is conceptually shareable).
  2. `broker.plugin_acl(&plan.canonical)` → on `Some(acl)`,
     compare `acl.topic_id` against `plan.topic_id`; mismatch
     ⇒ `InvalidPlan { reason: TopicIdMismatch }` (pi-1 §260,
     §488; pi-6 §3 — use the public lookup, not the private
     `broker_acl.plugins` field). `None` is unreachable here
     since step 1b's `try_reserve_registration` already
     returned `NotInAcl`, but defensively map to
     `SpawnError::NotInAcl` for completeness.
  3. **Path validation** — for each path field in
     `plan.filesystem`, `plan.entry_absolute`,
     `paths.project_root`, and `paths.private_state_dir`
     (pi-4 §1):
     a. assert absolute (`path.is_absolute()`); non-absolute
        ⇒ `InvalidPlan { reason: NonAbsolutePath { kind } }`
        where `kind` covers each `PathKind` variant including
        the new `ProjectRoot` / `PrivateStateDir` (pi-1 §489).
     b. assert no ASCII control chars (lockin's builder
        methods `assert_no_control_chars` would otherwise
        panic in Phase B); a control char ⇒ `InvalidPlan {
        reason: ControlCharsInPath { kind, path } }`
        (pi-4 §2). Defined as `path.as_os_str().as_bytes()
        .iter().any(|b| *b < 0x20 || *b == 0x7f)`.
  4. Iterate `plan.env.set` keys + `plan.env.pass` entries:
     if any equals `RFL_BUS_FD` / `RFL_PLUGIN` / `RFL_HELPER_FD`
     / `RFL_PROJECT_ROOT` / `RFL_PRIVATE_STATE_DIR` /
     `RFL_TOPIC_ID` ⇒ `ReservedEnvInPlan { var }` (pi-1 §157,
     §266, §319, §320; pi-2 §6 added `RFL_TOPIC_ID`).
  5. If `plan.network` is `NetworkPlan::Proxy { allow_hosts }`,
     dry-run `outpost::NetworkPolicy::from_allowed_hosts(...)`.
     Failure ⇒ `InvalidPlan { reason: NetworkAllowHostsInvalid }`
     (pi-1 §287, §490).
  6. `metadata(&plan.entry_absolute).is_file() &&
     metadata.permissions().mode() & 0o111 != 0`. Otherwise
     ⇒ `EntryNotExecutable`. Symlinks are followed
     (`fs::metadata` not `fs::symlink_metadata`) per pi-1
     §113, §1653.
  7. **Provider refusal (pi-2 §11).** If
     `broker.plugin_acl(&plan.canonical)` returns a
     `PluginAcl` with `provider_id.is_some()` (the public
     field on m1's `PluginAcl`, fed from `bindings.provider
     = true` + `bindings.provider_id`), reject with
     `InvalidPlan { reason: ProviderNotInM2 { provider_id } }`.
     m2 has no provider authority wiring; m4 removes this
     refusal. The fixture lock NEVER sets `provider = true`.
     m2 retrospective notes the staged restriction.
  
  **Phase B — allocate child resources (async):**
  8. `nix::sys::socket::socketpair(AddressFamily::Unix,
     SockType::Stream, None, SockFlag::SOCK_CLOEXEC)` →
     `(core_fd, child_fd)`, both `OwnedFd`. `SOCK_CLOEXEC`
     prevents accidental leaks (pi-1 §31, §503; pi-4
     non-blocking #3 — Rust shape, not C-shape).
  9. If proxy mode: `outpost_proxy::start(policy).await` →
     `ProxyHandle`. Capture `proxy.listen_addr().port()`.
  10. Build `SandboxBuilder`. Methods consume `self` and
      return `Self`; reassign in loops (pi-1 §161, §162):
      ```rust
      let mut builder = lockin::Sandbox::builder();
      for p in &plan.filesystem.read_paths { builder = builder.read_path(p); }
      // ... write_path, write_dir, read_dir, exec_path, exec_dir
      builder = match &plan.network {
          NetworkPlan::Deny => builder.network_deny(),
          NetworkPlan::AllowAll => builder.network_allow_all(),
          NetworkPlan::Proxy { .. } => builder.network_proxy(proxy_port),
      };
      builder = builder.max_cpu_time(plan.limits.max_cpu_time)
                       .max_open_files(plan.limits.max_open_files)
                       .disable_core_dumps();  // pi-1 §381 — call always
      if let Some(n) = plan.limits.max_address_space {
          builder = builder.max_address_space(n);
      }
      if let Some(n) = plan.limits.max_processes {
          builder = builder.max_processes(n);
      }
      builder = builder.inherit_fd_as(child_fd, RFL_BUS_FD_NUMBER);
      // child_fd MOVED into builder (OwnedFd consumed). No further drop.
      ```
      The `child_fd` `OwnedFd` is **moved** into the builder
      and is no longer accessible by name (pi-1 §24, §375).
  11. **(deferred to caller — pi-2 §13).** The supervisor does
      NOT scan `plan.filesystem.write_dirs` to infer
      project-root / private-state-dir. That inference is
      ambiguous against plan mutation and against any
      future m1 layout change. Instead, supervisor takes an
      explicit `SpawnPaths` argument alongside the plan
      (added to the `spawn` signature; see SP1 update below).
      `SpawnPaths { project_root: PathBuf, private_state_dir:
      PathBuf }` is the contract: the caller (test harness or
      m3 `rfl chat`) provides paths it computed from its own
      layout knowledge, and the supervisor ensures
      `private_state_dir` exists (`fs::create_dir_all`) and
      injects `RFL_PROJECT_ROOT` / `RFL_PRIVATE_STATE_DIR` /
      `RFL_TOPIC_ID` (the latter from `plan.topic_id`) at
      step 12. If `create_dir_all` fails ⇒
      `SpawnError::PrivateStateDirCreate { path, source }`
      (pi-4 non-blocking #5 — dedicated variant rather than
      reusing `TransportSetup`).
      
      **SP1 signature update:** `pub async fn spawn(&self,
      plan: &CompiledPlugin, paths: &SpawnPaths) -> Result<...>`.
      `SpawnPaths` is `Clone + Debug`, lives in
      `rafaello_core::supervisor`. The harness derives
      `private_state_dir = project_root.join(".rafaello-plugin-data").join(&plan.topic_id)`
      explicitly.
  12. `builder.tokio_command(&plan.entry_absolute).map_err(|e|
      Lockin { canonical, source: e })?` →
      `lockin::tokio::SandboxedCommand` (pi-6 §1: must be
      `tokio_command`, NOT `command` — the latter returns the
      sync variant which would not typecheck against the
      async `child.wait().await` later in the sequence).
      Apply env to the command (pi-1 §266, §1928; pi-2 §5, §6):
      ```rust
      cmd.env_clear();                      // step (a)
      // NOTE: lockin's SandboxedCommand::env_clear documents
      // that it removes TMPDIR/TMP/TEMP pointing at the
      // sandbox-owned private tmp. Lockin does NOT currently
      // expose a public accessor for the private tmp path
      // before spawn (pi-2 §5), so m2 cannot re-inject those
      // vars. Plugins that need a temp dir create one under
      // their granted write_dirs or use system /tmp (which is
      // not in the sandbox grant, so it will be denied — a
      // behavioural change vs. lockin defaults). m2's
      // retrospective records this as a known deviation; a
      // future small lockin API addition (Sandbox::private_tmp
      // returning the path before spawn, or a builder-level
      // env preserver) would close it without scope expansion.
      for key in &plan.env.pass {           // step (b)
          if let Some(val) = std::env::var_os(key) {
              cmd.env(key, val);
          }
      }
      for (k, v) in &plan.env.set { cmd.env(k, v); }  // step (c)
      if let NetworkPlan::Proxy { .. } = plan.network {  // step (d)
          let proxy_url = format!("http://127.0.0.1:{}", proxy_port);
          for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY",
                      "http_proxy", "https_proxy", "all_proxy"] {
              cmd.env(key, &proxy_url);
          }
          for key in ["NO_PROXY", "no_proxy"] {
              cmd.env(key, "");  // empty ⇒ no host bypass
          }
      }
      cmd.env("RFL_BUS_FD", RFL_BUS_FD_NUMBER.to_string());  // step (e), reserved LAST
      cmd.env("RFL_PLUGIN", plan.canonical.to_string());
      cmd.env("RFL_PROJECT_ROOT", &paths.project_root);
      cmd.env("RFL_PRIVATE_STATE_DIR", &paths.private_state_dir);
      cmd.env("RFL_TOPIC_ID", &plan.topic_id);  // pi-2 §6
      cmd.current_dir(&paths.project_root);   // step (f), pi-1 §36, §882
      ```
      Reserved `RFL_*` vars go LAST so they unconditionally
      override any prior collision (pi-1 §266, §267). The
      Phase A SP4.4 check guarantees no collision in normal
      flow; this is belt-and-suspenders.
  13. `cmd.spawn().map_err(Spawn)?` → `SandboxedChild`.
  > **Post-spawn unwind contract (pi-5 §5).** Once
  > `cmd.spawn()` succeeds at step 13, every subsequent
  > failure path MUST kill **and reap** the child before
  > returning the error: `nix::sys::signal::kill(pid,
  > SIGKILL)`, then `child.wait().await` to collect the exit
  > status (so the kernel doesn't leave a zombie under tests
  > with parallel cargo workers). This applies to step-14
  > `TransportSetup`, step-15 service-build errors, and the
  > step-17 broker register failure. Parent-side
  > `core_fd` / `ProxyHandle` drops happen after the
  > child is reaped.
  
  14. **Build fittings transport.** Convert `core_fd` (still
      owned by parent) to `tokio::net::UnixStream`:
      ```rust
      let std_stream = std::os::unix::net::UnixStream::from(core_fd);
      std_stream.set_nonblocking(true)
          .map_err(|e| TransportSetup { canonical, source: e })?;
      let stream = tokio::net::UnixStream::from_std(std_stream)
          .map_err(|e| TransportSetup { canonical, source: e })?;
      let (reader, writer) = stream.into_split();
      let transport = fittings_transport::stdio::StdioTransport::new(
          reader, writer, config.fittings_max_frame_bytes);
      ```
      (pi-1 §4, §348, §349; pi-2 §8 added the explicit
      `TransportSetup` mapping.) Failures here trigger the
      step-13 unwind: SIGKILL the child via `nix`, drop
      `ProxyHandle` if any, drop `core_fd` (closes parent
      half).
  15. Build the per-connection `Service` impl. In production
      mode this is `BusPublishService { broker, canonical }`
      handling `bus.publish` (notification only; rejecting
      requests with `MethodNotFound`; pi-1 §49, §1051,
      §1054). In test mode with `with_extra_service`, compose
      via a router as in §SP2.
  16. `let server = Server::new(service, transport);`
      `let peer = server.peer();`
  17. `let registered = broker.register_plugin(canonical.clone(),
      peer.clone())?` — see pi-1 §88 on cloning. The Phase A
      `try_reserve_registration` precheck makes
      `AlreadyRegistered` here practically impossible
      (single-supervisor invariant), but the call still maps
      `BrokerError` to `SpawnError` for completeness. Any
      error here unwinds per the post-spawn contract above:
      SIGKILL the child, **`child.wait().await` to reap**
      (pi-6 non-blocking #6), drop proxy, abort serve loop.
      On success, the in-flight reservation guard from
      step 1a is **dropped immediately** — registration is
      now the source of truth (pi-6 non-blocking #5).
  18. Spawn the reaper task: `tokio::spawn(async move {
      child.wait().await })`; it owns the
      `lockin::tokio::SandboxedChild` and publishes the
      `Arc<ReaperOutcome>` to the
      `tokio::sync::watch` channel inside the
      `SpawnHandle`'s shared state. **A second watcher task
      awaits the reaper's `JoinHandle`** (pi-5 §1); on
      `JoinError` (panic) it publishes
      `Arc::new(ReaperOutcome::ReaperPanicked)` so callers
      observe the failure rather than waiting forever.
      Otherwise the watcher just confirms the reaper exited
      cleanly. The watcher's own `JoinHandle` is stored for
      shutdown (pi-6 non-blocking #1: stale "broadcast" line
      removed; the channel is `tokio::sync::watch` per
      §SP1).
  19. Spawn the serve loop: `tokio::spawn(server.serve())`.
      Store its `JoinHandle` in the shared state for
      shutdown (pi-1 §1903).
  20. Insert the new `SpawnHandle` into supervisor's
      registry. Return a clone.
  
  **Race contract (pi-1 §20, §339, §342, §343):** the broker
  registration (step 17) happens **before** the serve loop
  starts (step 19). Inbound `bus.publish` frames the child
  may have written between socket creation (step 8) and
  registration (step 17) sit in the kernel socket buffer
  unread; the serve loop at step 19 begins draining them
  only after registration is in place. So the broker is
  guaranteed to see the canonical as registered when the
  first publish is processed. The earlier round-1 sentence
  "before the child has had a chance to publish" is corrected
  to "before any inbound frame is processed". This is the
  invariant tests rely on.
- **SP5.** **Lifecycle.** Per-plugin teardown:
  - On final `SpawnHandle` drop OR explicit per-plugin removal
    OR supervisor shutdown:
    1. Drop the `RegisteredPlugin` guard (broker stops
       fan-out to this peer immediately).
    2. SIGTERM the child via `nix::sys::signal::kill(Pid::from_raw(
       pid as i32), Signal::SIGTERM)` — graceful path, called
       only by `shutdown` (pi-1 §28, §721). Wait up to
       `config.shutdown_grace` for the reaper task to observe
       exit. On timeout, SIGKILL via the same nix call (Drop's
       fallback path uses SIGKILL directly).
    3. Drop the proxy `ProxyHandle` (pi-1 §103, §104, §540).
       Proxy drop **after** child is dead so a misbehaving
       plugin can't observe the proxy disappear and panic
       differently. For SIGKILL'd children this race is
       irrelevant (kill is instant).
    4. Abort the serve-loop `JoinHandle` (drops the transport
       and closes the core-side fd) (pi-1 §374).
  - `Drop` on `PluginSupervisor` calls best-effort SIGKILL +
    abort serve-loops on every handle in the registry,
    synchronously. No grace period (Drop can't await).
  - **Process tree (pi-1 §105, §327).** lockin/syd kills the
    direct child only; descendants spawned via `exec_paths`
    are not tracked by m2. The scope explicitly does NOT
    claim closure of attack 6.9; the retrospective records
    this gap. Tests do not assert subprocess containment.
- **SP6.** **No hardcoded bypass (pi-1 §32, §57).** The only
  spawn entry points are `PluginSupervisor::spawn` and (in
  test/feature mode) `with_extra_service` which differs only
  in service composition. There is no `spawn_unsandboxed`,
  no `RFL_INSECURE` env override, no debug feature. The
  fixture binary spawned through the same path as any other
  plugin, including a copy of its binary into a temp
  plugin_dir + manifest + openrpc.json + computed digests
  (see §H below). pi review **must** continue to defend this
  invariant on every round.
- **SP7.** **`RFL_BUS_FD_NUMBER = 3`** is a `pub const` in
  the supervisor module (pi-1 §499) so the fixture and tests
  can reference it without magic numbers. fd 3 is the first
  fd above stdio; lockin's `inherit_fd_as` maps the inherited
  fd to this number deterministically (pi-1 §502, §1156).

### H — test harness (`rafaello-core/tests/common/m2_harness.rs`)

The test harness owns the non-trivial work of constructing
plausible lock entries pointing at the fixture binary, since
m1's compile contract requires entries inside the plugin_dir
and digests recomputed against the on-disk content.

- **H1.** Module path: `rafaello/crates/rafaello-core/tests/common/m2_harness.rs`,
  re-exported as `mod common; use common::m2_harness as h;` from
  every integration test file. Sibling helpers `m1_*` from m1
  remain untouched.
- **H2.** `FixtureLockBuilder` (pi-1 §394, §402, §405):
  - Constructs a `tempfile::TempDir` representing the project
    root.
  - For each fixture instance, creates a sub-directory
    `<project>/plugins/<canonical-name>/` (the plugin_dir)
    containing:
    - A copy of the `rfl-bus-fixture` binary at `bin/fixture`
      (preserving exec bit per pi-1 §398).
    - A minimal `rafaello.toml` declaring `name = <canonical-name>`,
      `version = ...`, `entry = "bin/fixture"`, plus the
      `[provides]`, `[capabilities.default]`, `[bus]`, `[load]`
      sections needed for the test scenario. Helper builders
      (`with_publish_topic`, `with_subscribe_pattern`,
      `with_env_set`, `with_read_dir`, etc.) construct the
      manifest fields.
    - A minimal valid `openrpc.json` sibling (m1 row 31; pi-1
      §403). m2 carries a single canonical "no-op" sibling
      shipped in `tests/common/empty_openrpc.json`; the
      manifest never names methods (m1 row 31 dropped `[rpc]`),
      so the same sibling works for every fixture.
  - Programmatically constructs the `Lock` value with proper
    `granted_capabilities`, `bindings`, recomputed `digest`
    fields via m1's actual public surface — `digest::content_digest`
    + `digest::manifest_digest` + `RecomputedDigests` (pi-3
    non-blocking #1; the round-2 wording "digest::recompute_*"
    was incorrect). Does NOT copy code.
  - Runs `validate::lock(&lock, &path_context)` to satisfy
    `compile_plugin`'s precondition (pi-1 §405, §406). Then
    runs `compile_plugin` for each entry to produce
    `CompiledPlugin` plans; runs `broker_acl::compile(&lock)`
    for the `BrokerAcl`.
  - Returns `(BrokerAcl, Vec<CompiledPlugin>, ProjectLayout)`.
- **H3.** `Spawn` helper:
  - Takes `BrokerAcl + Vec<CompiledPlugin> + Vec<ExtraService>`.
  - Constructs `Broker::new(acl)?`, then
    `PluginSupervisor::with_extra_service(broker.clone(), config,
    factory)`. The factory map keys on canonical so per-fixture
    extra services compose.
  - For each plan, computes `paths = SpawnPaths { project_root,
    private_state_dir: project_root.join(".rafaello-plugin-data")
    .join(&plan.topic_id) }`, then calls
    `supervisor.spawn(plan, &paths).await` (the supervisor's
    SP4 step 11 calls `fs::create_dir_all(private_state_dir)`
    which already creates intermediate `.rafaello-plugin-data`;
    the harness does not need to pre-create — pi-6
    non-blocking #3). Returns
    `(Broker, PluginSupervisor, Vec<SpawnHandle>)`.
- **H4.** `Observer` helper (pi-1 §74, §170, §387, §417;
  pi-4 §4 added the sync-handler clarification):
  - Spawns one fixture instance configured as
    `RFL_FIXTURE_MODE=observer` (which holds open until
    SIGTERM and forwards every `bus.event` notification it
    receives back to core via
    `peer.call("core.fixture.observed", event_value).await`).
    Because `Client::with_notification_handler` takes a
    **synchronous** closure (`Fn(String, Value)`), the
    fixture's notification handler clones its outbound
    `PeerHandle`, then spawns an async forwarding task via
    `tokio::spawn(async move { peer_clone.call(...).await })`
    so the synchronous handler returns immediately and
    forwarding runs on the runtime. The harness registers a
    `core.fixture.observed` extra service that pushes the
    event into a `tokio::sync::mpsc::UnboundedSender` whose
    receiver is returned to the test.
  - Tests await events on this receiver with a bounded
    timeout. Receivers are flushed on observer drop.
  - Subscribe pattern: the observer's lock grants explicit
    namespace patterns like `core.**`, `plugin.**`, etc. — NEVER
    bare `**` (pi-1 §33, §463). Helper `Observer::watch_all`
    builds the multi-namespace grant.
- **H5.** **Two-phase readiness handshake** (pi-1 §21,
  §333, §385, §386; **pi-4 §5 added the fixture-ready
  half**). The race exists in two places:
  1. **Fixture-ready (plugin → core).** After the fixture's
     `Client::connect`, `with_service`, and
     `with_notification_handler` are all installed, the
     fixture calls `peer.call("core.fixture.ready",
     serde_json::json!({"mode": "<mode>"}))` exactly once.
     The harness registers a `core.fixture.ready` extra
     service that records each fixture's readiness and
     unblocks the harness-side wait. Without this, core
     calls to `core.fixture.start` / `core.fixture.echo` /
     `core.fixture.dump_env` could land before the fixture's
     service registry is installed, returning
     `MethodNotFound`.
  2. **Publisher-start (core → plugin).** Once both the
     observer fixture AND the publishing fixture have
     completed step 1, the harness calls
     `spawn_handle.peer().call("core.fixture.start",
     serde_json::Value::Null).await` (pi-4 §4: `()` is not a
     valid `peer.call` param; use `Value::Null`) on the
     publishing fixture. The fixture's `start` handler is
     the trigger for any publish-on-start mode and returns
     immediately. The harness then awaits the expected
     events on the observer channel.
  The two-phase shape makes every test schedule-deterministic
  even under multi-thread tokio scheduling.

### F — fixture binary (`rafaello-core` `[[bin]] rfl-bus-fixture`)

Test-only subprocess binary inside `rafaello-core`. Behaviour
is fully controlled by env vars set by the harness via
`plan.env.set` (which the supervisor honours in SP4 step 12).

- **F1.** Single binary entry point at
  `crates/rafaello-core/src/bin/rfl_bus_fixture.rs`. Built
  only with `--features test-fixture`. Depends on
  `fittings-core`, `fittings-client`, `fittings-transport`,
  `tokio`, `serde_json`, `async-trait` from
  `rafaello-core`'s test/feature deps (pi-1 §114).
- **F2.** **Mode dispatch via `RFL_FIXTURE_MODE`** (replaces
  the round-1 grab-bag of independent env vars per pi-1 §412,
  §415–§418).
  
  **Universal fixture init (every mode does this in order)**
  per pi-4 §5:
  1. Parse `RFL_BUS_FD`, set up transport (§F3).
  2. Build `Client` via `OneShotConnector`.
  3. Install `with_service` and/or
     `with_notification_handler` per mode requirements.
  4. Call `peer.call("core.fixture.ready",
     json!({"mode": "<mode>"})).await` exactly once. The
     harness `core.fixture.ready` extra service unblocks
     each fixture's readiness wait. This MUST complete
     before the fixture either accepts inbound calls or
     proceeds with mode-specific behavior.
  
  **Publish-and-exit modes** (`publish_one`,
  `publish_bad_namespace`, `publish_bad_grammar`,
  `publish_outside_grant`, `publish_bad_in_reply_to_*`,
  `publish_with_taint`) all share this post-init shape per
  pi-4 §6 + pi-5 §2:
  - Wait for `core.fixture.start`.
  - Perform the configured publish via `client.notify(
    "bus.publish", params).await` (using `Client::notify`,
    NOT `client.peer().notify(...)` — pi-5 §2: the
    `Client`-level `notify`/`call` traverse the same
    `ClientCommand` FIFO queue, guaranteeing the publish
    is in line ahead of any subsequent call; `PeerHandle`
    notifications go through a separate
    `outbound_request_rx` queue and `tokio::select!` may
    process a follow-up call before the notification).
  - Call `client.call("core.fixture.after_publish",
    Value::Null).await` — this is the **flush ack**, also
    via `Client::call` for FIFO ordering. The harness
    `core.fixture.after_publish` extra service records the
    call and returns. Because this call sits behind the
    publish in the same FIFO, returning from the call
    proves the publish was at least dequeued and written
    in order.
  - Exit 0.
  
  Mode-specific bodies:
  - `publish_one` — publish one `bus.publish` notification
    with `topic` from `RFL_FIXTURE_TOPIC` and `payload`
    from `RFL_FIXTURE_PAYLOAD_JSON` (parsed).
  - `publish_with_taint` — publish a `bus.publish`
    notification whose top-level params have `topic`,
    `payload`, AND `taint` from `RFL_FIXTURE_TAINT_JSON`
    (parsed; an array of `TaintEntry`-shaped JSON). pi-4 §7
    added this mode because `taint` lives at the top-level
    of `PublishMsg`, not inside `payload`.
  - `publish_full_params` (escape hatch) — publish a
    `bus.publish` notification whose params are the verbatim
    JSON value from `RFL_FIXTURE_FULL_PARAMS_JSON`. Used by
    tests asserting decode-side rejection of arbitrary
    malformed shapes (e.g. extra unknown fields, missing
    required fields).
  - `publish_bad_namespace` — publish `core.session.user_message`
    (or `RFL_FIXTURE_TOPIC` if set); broker rejects, fixture
    continues to flush ack and exits.
  - `publish_bad_grammar` — publish topic
    `plugin.<RFL_TOPIC_ID>.UPPERCASE`.
  - `publish_outside_grant` — publish
    `plugin.<RFL_TOPIC_ID>.ungranted` (the test's lock grant
    omits this exact topic).
  - `publish_bad_in_reply_to_missing` — publish
    `plugin.<RFL_TOPIC_ID>.tool_result` with no `in_reply_to`.
  - `publish_bad_in_reply_to_empty` — same topic with
    `in_reply_to: []`.
  - `publish_bad_in_reply_to_multiple` — same topic with
    `in_reply_to: ["a", "b"]`.
  - `respond_peer_call` — register a fittings `Service` impl
    handling:
    - `core.fixture.start` → empty response.
    - `core.fixture.echo` → echoes params.
    - `core.fixture.dump_env` → returns
      `{ "env": { "<key>": "<value>", ... } }` over **only the
      keys allow-listed via `RFL_FIXTURE_ENV_KEYS`** (comma-
      separated). Includes `RFL_BUS_FD`, `RFL_PLUGIN`, etc.
      when listed.
    - `core.fixture.write_private_state` → writes
      `{ marker: <random> }` to `<RFL_PRIVATE_STATE_DIR>/marker`,
      returns `{ "wrote": "<absolute-path>" }`.
    - `core.fixture.report_open_result` → attempts
      `std::fs::read(path)` for `RFL_FIXTURE_OPEN_PATH`,
      returns `{ "ok": true }` on success or `{ "ok": false,
      "errno": <int> }` on failure (pi-1 §37, §895).
    - `core.fixture.try_write_path` → attempts
      `std::fs::write(path, b"x")` for
      `RFL_FIXTURE_WRITE_PATH`, returns errno on failure
      (used by lockin write-denial test).
    - Sleeps until SIGTERM. **Holds open by default**
      (pi-1 §415, §417 — service modes always hold).
  - `call_core_then_exit` — wait for `start`, call
    `peer.call("core.fixture.ping", { "n": 42 })` once; exit
    0 on response, exit 2 on error.
  - `observer` — register `core.fixture.observed`-pushing
    notification handler (`Client::with_notification_handler`,
    pi-1 §170, §172, §2015): on every inbound `bus.event`
    notification, call `peer.call("core.fixture.observed",
    event_payload)` to forward upstream. Wait for `start`
    (no-op other than ack), then sleep until SIGTERM.
- **F3.** **Wire setup.** Parse `RFL_BUS_FD` as `RawFd`
  (negative ⇒ exit 3, pi-1 §500). Convert exactly once via
  `OwnedFd::from_raw_fd(fd)` inside an `unsafe` block (pi-1
  §59, §1166). Wrap as `std::os::unix::net::UnixStream::from(owned)`,
  `set_nonblocking(true)`, then `tokio::net::UnixStream::from_std(...)`.
  Split into `(reader, writer)`; build
  `fittings_transport::stdio::StdioTransport::new(reader, writer,
  1 << 20)`. **Connector wrapper** (pi-2 §7): the fixture
  defines a tiny `OneShotConnector { transport:
  Mutex<Option<StdioTransport>> }` implementing
  `fittings_core::transport::Connector` (pi-3 §4); `Connector::connect` takes
  the transport via `Option::take` (panics on second call —
  acceptable for a one-shot bus connection). The shape mirrors
  the `OneShotConnector` already used in fittings' own
  client tests (`fittings/crates/client/src/lib.rs:623`).
  Then `Client::connect(OneShotConnector::new(transport)).await`.
  Register service via `with_service` for inbound requests in
  modes that need it; register notification handler via
  `Client::with_notification_handler` for `observer` mode
  (`fittings/crates/client/src/lib.rs:166`). Use
  `Client::peer()` for outbound.
- **F4.** Logging: `eprintln!` only for hard failures (parse,
  fd setup). No `tracing` setup (pi-1 §198 — keep fixture
  output clean).
- **F5.** **Reserved `RFL_TOPIC_ID` and `RFL_PRIVATE_STATE_DIR`
  env vars** (pi-1 §35, §283, §285) join `RFL_BUS_FD` and
  `RFL_PLUGIN` in the reserved set:
  - `RFL_TOPIC_ID` — the hashed topic-id form derived from
    canonical at compile time. Fixture uses it to construct
    its own `plugin.<id>.*` topics without re-implementing
    base32 hashing.
  - `RFL_PRIVATE_STATE_DIR` — absolute path to the per-plugin
    private state dir. Fixture writes there in the
    `write_private_state` mode.
  - `RFL_PROJECT_ROOT` — the project root (parent of
    `.rafaello-plugin-data`). Useful for tests that read
    relative paths.
  All three added to the SP4.4 reserved-env-in-plan rejection
  set so plugin authors can't accidentally collide.

### I — integration test suite

Tests live under `rafaello/crates/rafaello-core/tests/`. Most
are `#[tokio::test(flavor = "multi_thread")]`; the pure
pattern-matcher tests are plain `#[test]` (pi-1 §431). Tests
needing parent env mutation use
`#[serial_test::serial(env)]`. Tests needing process/fd
inspection use `#[serial_test::serial(proc)]`. Each test uses
`tempfile::tempdir()` for transient state (m1 pattern).

All spawn-bearing tests are
`#[cfg(target_os = "linux")]` (pi-1 §220, §221). The pure
unit tests run on every platform.

#### Positive integration tests

| Test file | Exercises |
|-----------|-----------|
| `bus_pattern_matches.rs` | Re-exports m1's `pattern_matches_topic` cases; adds zero-trailing negatives (`core.session.**` does NOT match `core.session`; `plugin.id_x.**` does NOT match `plugin.id_x`) per pi-1 §91. Pure unit test. |
| `broker_publish_boot_event.rs` | `Broker::new(acl)?` returns `Ok`; then register the observer; then call `broker.publish_boot()` explicitly. Observer receives `core.lifecycle.boot` with payload `{ "version": <rafaello-core version string>, "plugin_count": <ACL plugin count> }`. The event has `publisher == PublisherIdentity::Core`. (pi-2 §1 + pi-6 non-blocking #2: boot is emitted only via the explicit `publish_boot()`; `PluginSupervisor::new` does NOT auto-emit it.) |
| `broker_register_unregister.rs` | Register canonical present in ACL → ok; drop guard → `contains_plugin` still true (canonical stays in ACL) but `handle_plugin_publish` returns `NotRegistered` (pi-1 §11, §139, §185). |
| `broker_publish_core_happy_path.rs` | `publish_core("core.lifecycle.boot", payload)` with one registered observer subscribed to `core.lifecycle.**` → observer's `peer` receives one `bus.event` whose `publisher = PublisherIdentity::Core`. |
| `broker_publish_core_invalid_topic_rejected.rs` | `publish_core("plugin.x.y", ...)` → `BrokerError::PublishOnReservedNamespace` (core publishing on a non-core namespace). `publish_core("core.Bad", ...)` → `InvalidTopic` (pi-1 §71, §1278). `publish_core("evil.foo", ...)` → `UnknownNamespace`. |
| `bus_event_schema_round_trip.rs` | Build a `BusEvent` with each `PublisherIdentity` variant; serialise with `serde_json`; deserialise back into a permissive test struct; assert all fields preserved. Verifies the wire shape m4 will need. |
| `supervisor_spawn_fixture_happy_path.rs` | Headline test. Two fixtures: A in `publish_one` mode publishing `plugin.<A>.hello` with payload `{"msg":"hi"}`; B in `observer` mode subscribed to `plugin.**`. Harness handshake (§H5). Observer receives the event; assertion: `event.topic == "plugin.<A>.hello"`, `event.payload == {"msg":"hi"}`, `event.publisher == Plugin { canonical: A, topic_id: <A's id> }`. |
| `supervisor_peer_call_core_to_plugin.rs` | A in `respond_peer_call` mode. Test calls `spawn_a.peer().call("core.fixture.echo", json!({"x":1})).await` → response `{"x":1}`. |
| `supervisor_peer_call_plugin_to_core.rs` | A in `call_core_then_exit` mode. Harness extra service registers `core.fixture.ping` (echoes params + 1). Wait for fixture exit; `matches!(*spawn.wait().await, ReaperOutcome::Exited(s) if s.code() == Some(0))` (pi-5 §1 — `wait` returns `Arc<ReaperOutcome>`). |
| `supervisor_bus_publish_round_trip_two_plugins.rs` | A in `publish_one`, B with subscribe grant on `plugin.<A>.greet`. B in observer-like mode forwards via `core.fixture.observed` extra service. After handshake, B receives A's event. Asserts cross-plugin fan-out via lock grants. |
| `supervisor_lifecycle_drop_kills_child.rs` | Spawn A in `respond_peer_call` (holds open). Drop the `SpawnHandle` and the supervisor's internal copy by calling `shutdown(self).await`. After completion, `shutdown_report.clean.contains(A)`. The reaper task observed the exit; the test does NOT use `kill -0` (pi-1 §367, §731). |
| `supervisor_proxy_starts_and_env_injected.rs` | A in `respond_peer_call` with `NetworkPlan::Proxy { allow_hosts: ["example.com"] }`. Harness calls `core.fixture.dump_env` requesting `RFL_FIXTURE_ENV_KEYS=HTTP_PROXY,HTTPS_PROXY,ALL_PROXY,NO_PROXY,http_proxy,https_proxy,all_proxy,no_proxy,RFL_BUS_FD` (pi-5 §6 — both cases listed). Assertions: uppercase + lowercase `*_PROXY` are all `http://127.0.0.1:<port>`; uppercase + lowercase `NO_PROXY` are empty; `RFL_BUS_FD = "3"`. The proxy port is non-zero and `<= u16::MAX`. (pi-1 §26, §107, §108, §325, §496, §542.) |
| `supervisor_env_pass_set_applied.rs` | `#[serial(env)]`. Plan with `env.pass = ["FAKE_PUBLIC_ENV"]` and `env.set = {"FOO": "bar"}` (using a non-secret-pattern key per pi-1 §409). Test sets `FAKE_PUBLIC_ENV=abc` in parent env, dumps from fixture, asserts both keys present plus reserved `RFL_*`. |
| `supervisor_env_set_overrides_pass.rs` | `#[serial(env)]`. Plan with `env.pass = ["FOO_VAR"]` and `env.set = {"FOO_VAR": "set-wins"}`. Parent has `FOO_VAR=pass-loses`. Dump: `FOO_VAR == "set-wins"` (pi-1 §321). |
| `supervisor_env_clear_strips_unrelated.rs` | `#[serial(env)]`. Parent has `RANDOM_PARENT_VAR=secret` (not in pass/set). Dump asserts `RANDOM_PARENT_VAR` is absent (pi-1 §323). |
| `supervisor_private_state_dir_writable.rs` | A in `respond_peer_call`. Harness calls `core.fixture.write_private_state`; fixture writes to `<RFL_PRIVATE_STATE_DIR>/marker`; test verifies file exists and the directory path includes the topic-id form per row 37 (pi-1 §35, §286, §326). The supervisor created the dir at SP4 step 11 before spawn. |
| `supervisor_taint_round_trip.rs` | A in `publish_with_taint` mode (pi-4 §7) with `RFL_FIXTURE_TAINT_JSON=[{"source":"test","detail":"x"}]`. Observer receives event; `event.taint == Some(vec![TaintEntry{...}])` byte-equal (pi-1 §308, §356). |

#### Negative integration tests

| Test file | Asserts |
|-----------|---------|
| `broker_publish_unknown_namespace_rejected.rs` | `handle_plugin_publish` with topic `"evil.foo"` from a registered plugin → `UnknownNamespace`. Also test `"random.thing.bar"` (pi-1 §7, §305). |
| `broker_publish_short_plugin_topic_rejected.rs` | Topics `"plugin.<own-id>"` and `"plugin.<other-id>"` (two-segment) → `PublishOnReservedNamespace` (pi-1 §8, §306). |
| `broker_publish_core_namespace_rejected.rs` | Plugin publishes `"core.session.user_message"` → `PublishOnReservedNamespace`. Asserts `core.lifecycle.publish_rejected` event fires with `code: "publish_on_reserved_namespace"`. |
| `broker_publish_provider_namespace_rejected.rs` | Plugin publishes `"provider.openai.tool_request"` → `PublishOnReservedNamespace` (m2 staged: even provider-bound plugins are rejected because m2's spawn refuses to register provider plugins per SP4 #7; pi-1 §10). |
| `broker_publish_frontend_namespace_rejected.rs` | Plugin publishes `"frontend.tui.confirm_answer"` → `PublishOnReservedNamespace`. |
| `broker_publish_other_plugin_namespace_rejected.rs` | Plugin A publishes `"plugin.<B-topic-id>.tool_result"` → `PublishOnReservedNamespace`. |
| `broker_publish_outside_grant_rejected.rs` | Plugin A publishes `"plugin.<A-topic-id>.ungranted"` (A's grant lists only `"plugin.<A>.granted"`) → `PublishOutsideGrant`. |
| `broker_publish_invalid_topic_grammar_rejected.rs` | Three sub-cases (each a separate `#[test]` in one file): `"plugin.<id>.UPPERCASE"`, `"plugin.<id>.has spaces"`, `""` → `InvalidTopic` with the respective `ValidationError`. |
| `broker_publish_extra_field_rejected.rs` | Service receives `bus.publish` notification whose `params` JSON has an extra unknown key → `InvalidPayload`. Test goes through the per-connection Service so decoding actually happens (pi-1 §314). |
| `broker_tool_result_in_reply_to_missing_rejected.rs` | Plugin publishes `plugin.<id>.tool_result` with no `in_reply_to` → `InvalidInReplyTo { reason: Missing }`. |
| `broker_tool_result_in_reply_to_empty_rejected.rs` | Same with `in_reply_to: []` → `InvalidInReplyTo { reason: EmptyArray }`. |
| `broker_tool_result_in_reply_to_multiple_rejected.rs` | Same with two ids → `InvalidInReplyTo { reason: UnexpectedMultiple }`. |
| `broker_rpc_reply_in_reply_to_missing_rejected.rs` | Same shape for `plugin.<id>.rpc_reply`. |
| `broker_publishing_plugin_excluded_from_own_fanout.rs` | Plugin A's grant subscribes to `plugin.<A-topic-id>.**` AND publishes `plugin.<A-topic-id>.foo`. The publishing plugin does NOT receive the event back; observer plugin B subscribed to the same pattern DOES (pi-1 §245, §247). |
| `broker_unsubscribed_plugin_does_not_receive.rs` | A publishes `plugin.<A>.foo`; B subscribed only to `core.**` does NOT receive it (pi-1 §298, §299). |
| `broker_tool_result_not_fanned_out_to_other_plugins.rs` | A publishes `plugin.<A>.tool_result` with valid `in_reply_to`; B subscribed to `plugin.**` does NOT receive it (the result-routing protection of §B7; pi-1 §14). Asserts the `tracing::debug!` message via `traced_test`. |
| `broker_register_canonical_not_in_acl_rejected.rs` | `register_plugin(canonical_not_in_acl, peer)` → `BrokerError::NotInAcl` (pi-1 §302). |
| `broker_register_duplicate_rejected.rs` | Register A; register A again → `AlreadyRegistered` (pi-1 §301). |
| `broker_invalid_acl_rejected_at_construction.rs` | Hand-build a `BrokerAcl` containing an invalid pattern (e.g. `"**"`); `Broker::new(acl)` returns `Err(InvalidPattern)` (pi-1 §70, §270). |
| `supervisor_spawn_canonical_not_in_acl_refused.rs` | Hand-mutate a `CompiledPlugin.canonical` to a value not in the broker's ACL. `spawn` → `SpawnError::NotInAcl`. No socketpair, no proxy started (verified by counting `outpost_proxy` start calls via dependency-injection in test mode — see §H6 below). |
| `supervisor_spawn_topic_id_mismatch_refused.rs` | Hand-mutate `CompiledPlugin.topic_id` to a wrong value. → `SpawnError::InvalidPlan { reason: TopicIdMismatch }`. |
| `supervisor_spawn_relative_path_refused.rs` | Hand-mutate `CompiledPlugin.entry_absolute` to a relative path. → `SpawnError::InvalidPlan { reason: NonAbsolutePath { kind: EntryAbsolute, .. } }` (pi-1 §489). |
| `supervisor_spawn_relative_spawn_path_refused.rs` | Pass `SpawnPaths { project_root: PathBuf::from("relative/path"), .. }` to `spawn` → `SpawnError::InvalidPlan { reason: NonAbsolutePath { kind: ProjectRoot, .. } }` (pi-5 non-blocking — covers SpawnPaths Phase A). Sub-case for `private_state_dir` non-absolute. |
| `supervisor_spawn_control_chars_in_path_refused.rs` | Hand-mutate `CompiledPlugin.entry_absolute` to a path containing `'\n'` → `SpawnError::InvalidPlan { reason: ControlCharsInPath { .. } }` (pi-4 §2; pi-5 non-blocking — covers the lockin-panic-prevention spot-check). |
| `supervisor_spawn_entry_not_executable_refused.rs` | Plan points to a regular file with no exec bit (created by harness via `chmod 0644`). → `SpawnError::EntryNotExecutable`. Phase A sequencing means no proxy started yet (pi-1 §156). |
| `supervisor_spawn_reserved_env_in_set_refused.rs` | Plan with `env.set = {"RFL_BUS_FD": "99"}`. → `SpawnError::ReservedEnvInPlan { var: "RFL_BUS_FD" }`. |
| `supervisor_spawn_reserved_env_in_pass_refused.rs` | Plan with `env.pass = ["RFL_PLUGIN"]`. → `SpawnError::ReservedEnvInPlan { var: "RFL_PLUGIN" }`. |
| `supervisor_spawn_reserved_env_helper_refused.rs` | Plan with `env.set = {"RFL_HELPER_FD": "..."}`. → `SpawnError::ReservedEnvInPlan { var: "RFL_HELPER_FD" }` (defence in depth even though m1 doesn't yet reject this — pi-1 §23, §270). |
| `supervisor_spawn_provider_lock_refused.rs` | Lock has `bindings.provider = true`. `spawn` → `SpawnError::InvalidPlan { reason: ProviderNotInM2 }` (pi-1 §10, §132). |
| `supervisor_spawn_duplicate_canonical_refused.rs` | Spawn A successfully; attempt to spawn A again → `SpawnError::AlreadyRegistered`. The Phase A precheck (`try_reserve_registration` + supervisor `in_flight` set) ensures NO socketpair / proxy / child is allocated for the second call. Verified via `harness::TestHooks::*_count()` deltas being zero across the failing call (pi-3 §7). |
| `supervisor_lockin_denies_outside_grant_read.rs` | A in `respond_peer_call` with grant `read_dirs = [<project-root>]` only. `RFL_FIXTURE_OPEN_PATH=/etc/passwd`. Test calls `core.fixture.report_open_result`; assertion: `ok == false`, `errno` indicates kernel denial (typically `EPERM` per syd's policy — exact errno asserted with `matches!` not `==` since lockin's specific errno mapping may differ across syd versions). |
| `supervisor_lockin_denies_outside_grant_write.rs` | Same shape; A's `write_dirs = [<private-state>]` only. `RFL_FIXTURE_WRITE_PATH=<project-root>/forbidden`. Calls `core.fixture.try_write_path`; asserts denial; verifies `<project-root>/forbidden` does not exist after. |

#### Manual validation in `manual-validation.md`

Path: `rafaello/plans/milestones/m2-broker-spawn/manual-validation.md`
(pi-1 §194). Captured items:

- `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture`
  green inside the devshell on Linux. (Tests gated on
  `target_os = "linux"` skip outside.)
- `RUST_LOG=rafaello_core=debug nix develop --impure --command
  cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core
  --features test-fixture --test supervisor_spawn_fixture_happy_path
  -- --nocapture` (pi-2 §14: `--test <name>` is a Cargo arg and
  must precede `--`, not follow it). Shows the broker
  registration trace + fan-out. Test uses `tracing-subscriber`
  init via `tracing_test::traced_test` or explicit setup
  (pi-1 §197).
- `find rafaello/crates/rafaello-core/src -name '*.rs' | sort`
  showing the new modules `bus.rs`, `bus/publish_msg.rs`,
  `supervisor.rs`, `supervisor/lifecycle.rs`,
  `bin/rfl_bus_fixture.rs`, plus the additions to `error.rs`
  / `lib.rs`.
- `ls /proc/<fixture-pid>/fd/` snapshot during a
  `respond_peer_call` fixture run. Documented invariants
  (NOT exact-count assertions per pi-1 §32, §553):
  - fd 3 exists and is `socket:[...]`.
  - The parent (`cargo test` process) does not have a
    duplicate of the same socket inode in its `/proc/self/fd`
    snapshot (i.e. the child-side half of the socketpair
    is not leaked back into the parent).
  - The Tokio-runtime fds present (eventfd, epoll) are
    expected; not enumerated.
- `nix develop --impure --command cargo doc --manifest-path
  rafaello/Cargo.toml -p rafaello-core --no-deps` warning-free
  (pi-1 §196, §223).
- `nix develop --impure --command cargo build --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture
  --bin rfl-bus-fixture` green, demonstrating the fixture
  binary builds (pi-1 §224).

### H6 — test-only supervisor hooks

Pi-1 §107, §191, §552 flagged the proxy/spawn-call-count
assertions as unobservable through the public API. m2 adds a
`#[cfg(any(test, feature = "test-fixture"))]` `TestHooks`
struct **owned per-supervisor** (NOT a global atomic — pi-3
non-blocking #2):

```rust
pub struct TestHooks { /* AtomicUsize fields, owned by one supervisor */ }
impl TestHooks {
    pub fn outpost_starts(&self) -> usize;
    pub fn socketpair_creates(&self) -> usize;
    pub fn child_spawns(&self) -> usize;
}
impl PluginSupervisor {
    #[cfg(any(test, feature = "test-fixture"))]
    pub fn test_hooks(&self) -> Arc<TestHooks>;
}
```

Counters increment from inside that supervisor's spawn path
only; tests on different supervisors do not interfere even
under parallel execution. Negative tests assert counter
deltas across a failing call. Production builds compile out
the counters entirely.

### E — error-surface additions

m2 grows `rafaello_core::error`:

- `pub enum BrokerError { ... }` (variants enumerated in §B2),
  `#[non_exhaustive]`, `thiserror`-derived, `Debug` only — NOT
  `Clone`, NOT `PartialEq` (pi-3 §3 + B2; storing
  `ValidationError`'s `Display` as a string keeps derives
  cheap and avoids reaching into m1).
- `pub enum SpawnError { ... }` (variants enumerated in §SP3),
  `#[non_exhaustive]`, `thiserror`-derived. Source-error
  variants are NOT `Clone`/`PartialEq` (pi-1 §126); tests
  use `matches!` on the variant.
- `pub enum InvalidPlanReason { ... }` companion enum.
- Both error types re-exported from `lib.rs`. Top-level
  `Error` enum gains `#[from]` arms.

## Out of scope

The following are explicitly NOT in m2 and are not allowed to
sneak in via "while I'm here" implementation drift. Items are
mapped to the milestone or RFC that owns them.

- **`rfl chat` / any CLI surface.** m3.
- **Frontends.** No registration path, no UDS attach socket,
  no `frontend.hello`. m3.
- **Provider plugins.** `provider.<provider-id>.*` namespace
  is reserved by name; m2's spawn explicitly refuses lock
  entries with `bindings.provider = true` (`SpawnError::InvalidPlan
  { reason: ProviderNotInM2 }`). m4 wires the provider role
  into broker publish authority.
- **Tool dispatch.** No `core.session.tool_request` →
  `plugin.<id>.tool_request` routing. The `auto_subscribes`
  field from m1's `BrokerAcl` is honoured for fan-out; no tool
  is registered, no provider proposes one.
- **Sink confirmation, `user_grants`, taint synthesis, taint
  superset enforcement.** m4 + m5.
- **Core re-emission of `provider.*` / `plugin.<id>.tool_result`
  → `core.session.*`.** m4. The §B7 result-routing
  protection (no fan-out for `plugin.<id>.tool_result` /
  `plugin.<id>.rpc_reply`) is the m2 placeholder that
  preserves the security RFC §5.4.1 invariant until m4 wires
  re-emission.
- **`request_id` field on bus events.** m4 introduces it
  alongside tool dispatch (pi-1 §41, §145). m2 retrospective
  records the staging.
- **Session persistence.** m3.
- **Renderer model.** m3.
- **`bus.subscribe` runtime request.** Subscribes are derived
  entirely from the lock at registration time. m2's bus
  service rejects `bus.subscribe` requests with
  `MethodNotFound` for now; pi-1 §241 — a dedicated rejection
  test is not added (the generic `MethodNotFound` path is
  fittings-shipped behavior).
- **Helper plugins.** `helper_for` / `RFL_HELPER_FD` deferred
  (`decisions.md` row 26). m1 already rejects `helper_for` at
  parse time. m2 must not add a spawn path. SP4.4's
  `RFL_HELPER_FD` reserved-name check is *defence in depth*.
- **Lazy-load orchestrator.** m3.
- **macOS spawn-bearing tests.** Linux-only, gated `#[cfg(target_os
  = "linux")]`. m6 polish revisits.
- **Cross-plugin RPC routing (`plugin.<a>.rpc_call.<b>`).**
  Not in v1. m2 enforces `rpc_reply` arity but does not route
  `rpc_call`; cross-plugin `rpc_call` topics go through
  generic event fan-out (which the lock would have to grant).
- **Process-tree containment.** SIGTERM / SIGKILL kills the
  direct child only. Subprocess containment is a security RFC
  §6.9 residual risk; m2 does not claim closure (pi-1 §57,
  §105).
- **Lockin builder knobs not in `CompiledPlugin`.** `allow_kvm`,
  `allow_interactive_tty`, `allow_non_pie_exec`,
  `raw_seatbelt_rule` are not surfaced. m2 ships the
  conservative default; a v1 plugin that needs these triggers
  a m1 plan-field addition.
- **`PluginSupervisor::shutdown` for individual plugins.**
  m2 supports drop-the-handle teardown and supervisor-wide
  `shutdown`. Per-plugin cooperative stop is m3 territory
  (when a CLI consumer needs `rfl plugin stop`).
- **Real CONNECT through the outpost proxy.** m2 starts the
  proxy and injects `HTTP_PROXY`/etc. but no test issues a
  real CONNECT. m4 (with the bundled `rfl-openai` provider)
  exercises the full network path. m2's claim is "proxy is
  wired and the env reaches the child"; m2 does NOT claim
  "proxy enforces hostnames" (pi-1 §3, §232, §543).

## Risks

1. **Fixture binary built only via `--features test-fixture`
   gate.** If `cargo test -p rafaello-core` (without the
   feature) is run, integration tests will fail to compile
   (`env!("CARGO_BIN_EXE_rfl-bus-fixture")` errors).
   Mitigations (pi-2 non-blocking #4):
   - The per-commit agent-prompt template (drafted in
     `commits.md`'s introduction + reproduced in
     `driver-notes.md`) MUST include `--features test-fixture`
     in every cargo invocation it cites. The driver enforces
     this at orchestration time.
   - Optionally make `default = ["test-fixture"]` so naked
     `cargo test -p rafaello-core` works. The downside: a
     downstream `cargo build -p rafaello-core` will compile
     the fixture binary by default. Decision deferred to
     `commits.md`; the safer pick is to keep the feature
     opt-in and rely on prompt enforcement.
2. **`PeerHandle` outside-handler usage in production.** m0
   added the API and tested it; m2 is the first non-test
   in-tree consumer. Subtle correlator-id bugs may surface
   under the bidirectional load m2 introduces. Mitigation:
   the `supervisor_peer_call_*` tests directly cover both
   directions; the `traced_test` infra captures any internal
   warnings.
3. **lockin requires syd on Linux.** Tests fail outside the
   devshell. Mitigation: every `cargo test` invocation in
   `manual-validation.md` is `nix develop --impure`-prefixed.
   CI runs in the devshell. The acceptance summary is scoped
   to "green inside `nix develop --impure`" (pi-1 §61, §234).
4. **Test isolation: parent env mutation.** Cargo runs tests
   in threads. `supervisor_env_*` tests use `#[serial(env)]`
   to serialise (pi-1 §60, §332). Other tests that allocate
   fds/processes do not need this gate.
5. **Reaper task vs `wait` race.** The reaper task owns the
   `lockin::tokio::SandboxedChild`; `SpawnHandle::wait`
   resolves on a `tokio::sync::watch` whose value transitions
   from `None` to `Some(Arc<ReaperOutcome>)`. Late `wait`
   callers see the cached `Arc` immediately (pi-5 §1).
   Verified by `supervisor_peer_call_plugin_to_core.rs`.
6. **Private-state directory creation race.** Supervisor
   creates `<project>/.rafaello-plugin-data/<topic-id>/`
   before spawn (SP4 step 11). If the parent dir doesn't
   exist (project root somewhere else), `create_dir_all`
   creates intermediate dirs. The harness controls the
   project root via `tempdir`, so this is straightforward;
   real-world `rfl chat` (m3) will own this same path
   already.
7. **Tracing subscriber init in tests.** `tracing-test`
   provides per-test setup; without it, `tracing::warn!`
   output is invisible. Tests asserting log content (via
   `logs_assert`) include the `#[traced_test]` attribute.
8. **`outpost-proxy` direct dep brings in `tokio`.** Already
   in W1; no new top-level dep.
9. **`SpawnHandle::Drop` and `PluginSupervisor::Drop` may
   double-signal.** Mitigation: the kill operation is
   idempotent (an already-dead pid returns ESRCH which is
   ignored), and the broker registration drop is single-shot
   via the `Option` shape on the guard.
10. **The "ProviderNotInM2" refusal cascades into m4's lock
    layout.** m4 will need to remove this refusal when it
    introduces provider authority. This is an intentional
    staged restriction that m2's retrospective documents.

## Internal split (driver guidance for `commits.md`)

Suggested grouping; `commits.md` picks final granularity. Pi
review-2 may reshape these.

1. **Workspace deps + crate skeleton + fixture bin scaffold +
   reserved-env additions** (W1, W2, W3, F1 empty `main`):
   ~2-3 commits.
2. **Reserved env var additions to m1 scrubber** (extending
   m1's `RESERVED_ENV_VARS` to include `RFL_HELPER_FD`,
   `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`, `RFL_PRIVATE_STATE_DIR`
   — pi-1 §23). Tiny m1 patch landed in m2's branch with a
   migration note in the commit body; m2 retrospective
   records the small back-reach. ~1 commit.
3. **Bus types + error enum** (B1 types only, B2, B4, B8, E):
   no behaviour yet, just the public type surface; build-only
   tests like m1's `error_surface_compiles.rs`. ~2 commits.
4. **Broker registration + lifecycle** (B1 register/unregister,
   B10 ACL validation, `broker_register_*` tests). ~2 commits.
5. **Broker publish path** (B3, B5, B6, B7, B9 lifecycle
   rejection event; the entire `broker_publish_*` matrix).
   Largest single group. ~4-5 commits, possibly split by
   error class (namespace rejection / grammar rejection /
   in_reply_to / fan-out).
6. **Supervisor plan validation (Phase A)** (SP1 type, SP3
   error enum, SP4 phase A; the `supervisor_spawn_*_refused`
   tests for in-ACL / topic-id-mismatch / non-absolute /
   not-executable / reserved-env / provider-refused cases).
   ~3 commits.
7. **Supervisor resource allocation (Phase B)** (SP4 phase B
   except service composition, SP7 const, the
   `supervisor_spawn_fixture_happy_path` and
   `supervisor_peer_call_*` tests). ~3-4 commits.
8. **Supervisor lifecycle + drop + shutdown** (SP5,
   `supervisor_lifecycle_drop_kills_child`,
   `supervisor_spawn_duplicate_canonical_refused`). ~2 commits.
9. **Supervisor proxy startup + env injection**
   (`supervisor_proxy_starts_and_env_injected`). ~1 commit.
10. **Supervisor lockin denial proofs**
    (`supervisor_lockin_denies_*`). ~1 commit.
11. **Cross-plugin scenarios + manual validation**
    (`supervisor_bus_publish_round_trip_two_plugins`,
    private-state, taint round-trip, manual-validation.md).
    ~2 commits.

Realistic total: **~22-30 commits, sequential**. Comparable
to m1's 37-row plan despite smaller surface area, because
m2's commits each integrate broker + supervisor + fittings +
lockin + outpost in one slice and per-commit greenness
pressure tends to grow them. The driver should NOT pre-emptively
split m2 into m2a/m2b unless something blocking surfaces
during pi review.

## Acceptance summary

m2 is done when:

- Every named test in the *Positive* and *Negative* matrices
  above is implemented and passes. Tests may split or merge
  during `commits.md` drafting as long as the named
  behaviours are all covered.
- `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture`
  green on Linux inside the devshell (pi-1 §61, §234).
- `nix develop --impure --command cargo build --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture
  --bin rfl-bus-fixture` green.
- `nix develop --impure --command cargo doc --manifest-path
  rafaello/Cargo.toml -p rafaello-core --no-deps` warning-free.
- `manual-validation.md` (path:
  `rafaello/plans/milestones/m2-broker-spawn/manual-validation.md`)
  records the items in the *Manual validation* section above.
- `retrospective.md` (path:
  `rafaello/plans/milestones/m2-broker-spawn/retrospective.md`)
  written, with anticipated drift items addressed:
  - **Provider-rejection staging.** m2's
    `SpawnError::InvalidPlan { reason: ProviderNotInM2 }`
    refuses lock entries with `bindings.provider = true`.
    m4 must remove this refusal and route `provider.<id>.*`
    publish authority. Decisions row addition or m4 scope
    cross-reference (pi-1 §10, §132, §437).
  - **`request_id` omission in `BusEvent`.** Overview §4.5
    enumerates `request_id` as part of the bus envelope; m2
    omits it because m4's tool-dispatch flow is the only
    consumer. Update overview §4.5 with a "v1 staging:
    request_id field is m4" note (pi-1 §41, §145, §438).
  - **`core.lifecycle.publish_rejected` schema.** m2
    introduces this event; the security RFC has no entry for
    it. Add a §"core.lifecycle.* events" section to the
    security RFC body OR a banner referring to m2's scope
    §B9 (Stream A owns payload schemas per `decisions.md`
    row 23) (pi-1 §15, §122, §178, §437).
  - **`core.lifecycle.boot` schema.** Same.
  - **`bus.event` outbound method name + `BusEvent` shape.**
    m2 fixes the outbound notification method as `bus.event`
    and the schema as in §B8. Stream A retrospective drift
    item.
  - **`PublisherIdentity` enum.** Same — Stream A schema
    addition.
  - **m1 reserved-env-var list extended (`RFL_HELPER_FD`,
    `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`, `RFL_PRIVATE_STATE_DIR`).**
    Single small m1 patch landed in m2 (commit group 2);
    decisions row addition or note in retrospective (pi-1
    §23, §270).
  - **Lock-correspondence claim is API-level, not
    forge-proof** (pi-1 §22, §83). m2 retrospective records
    that `CompiledPlugin`'s public fields permit hand-mutation
    and the supervisor spot-checks (Phase A) the cases that
    would otherwise crash the lockin builder. Tightening to
    an opaque/validated plan type is a v2 nice-to-have.
  - **Result-routing protection.** m2 adds a m4-anticipating
    behaviour (no fan-out of `plugin.<id>.tool_result` /
    `rpc_reply`). The retrospective points m4 at the §B7
    handover; m4 replaces the `tracing::debug!` no-op with
    the canonical re-emission path.
  - **m1 manifest validation may permit unknown top-level
    namespaces in `publishes` grants** (pi-1 §254, §255,
    §409). m2's broker rejects them at runtime; m1 should
    arguably reject them at parse/V3 time. m2 retrospective
    files the gap as a m3-or-m4 follow-up patch to m1, NOT
    a m2 in-scope item.
- No follow-up Stream RFC drift is owed by m2 BEYOND the
  items above. m2 does not modify Stream A or B RFC bodies
  in this branch (m1's banner-based reconciliation is the
  precedent; further patches accumulate as retrospective
  deltas).
