# m2 — rafaello-core broker + locked plugin spawn — scope

> **Status:** round-1 draft. Pi review pending. Not yet
> owner-ratified. The `commits.md` and Phase 3 work do not start
> until this document ratifies.

## Goal

Land the **first runtime** of rafaello: a bus broker plus a
plugin supervisor that spawns subprocess plugins inside lockin
sandboxes, with the broker enforcing publish/subscribe authority
from m1's `BrokerAcl`. The broker authenticates publishers from
the connection identity (set at spawn time), not from message
bodies. The supervisor refuses to spawn anything that does not
correspond to a lock entry — there is no hardcoded bypass path.

m2 is the structural moment where m1's pure data-transformation
becomes a running system. Every later milestone (m3 TUI, m4
provider + agent loop, m5 confirmation/sinks) layers on the
broker + supervisor primitive m2 ships.

The deliverable is a small set of new modules in the existing
`rafaello-core` crate (m1 owns the crate; m2 grows it), one new
in-tree fixture-plugin binary used by the integration tests, and
the workspace dependencies that wire fittings + lockin + outpost
into `rafaello-core`. Exercised primarily by `cargo test
--manifest-path rafaello/Cargo.toml -p rafaello-core` (unit +
integration). No new behaviour lands in the `rafaello`-bin
(`rfl chat` is m3).

## Inputs

- `rafaello/plans/overview.md` §3 (process model), §4 (the bus
  — esp. §4.1 transport-vs-broker, §4.2–4.3 grammar +
  namespaces, §4.4 core re-emit, §4.5 envelopes, §4.6 reserved
  env vars), §5 (lifecycle, §5.5 private state), §6 (compiler
  outputs the supervisor consumes — m2 reads the plan, doesn't
  produce it), §15.6 (PeerHandle).
- `rafaello/plans/decisions.md` rows **3, 4, 5, 13, 17, 22, 32,
  37, 38** (load-bearing for m2). Row 3 (broker = core,
  transport = fittings) is the layering rule m2 must not
  collapse. Row 4 fixes the four namespaces. Row 5 fixes the
  topic-id derivation m2 uses literally on every publish ACL
  check. Row 13 fixes bus authentication = inherited
  socketpair fd. Row 22 promotes `PeerHandle` (notify + call
  in both directions) to v1. Row 32 says lockin is configured
  via the Rust API at spawn time, with m1's `CompiledPlugin`
  as the structured source. Row 37 fixes the topic-id form of
  the per-plugin private-state path.
- `rafaello/plans/streams/a-security/rfc-security-model.md`
  §5 end-to-end (bus ACL, transport, confirmation reservations,
  frontend principal reservation), §7.2.6 (`in_reply_to`
  enforcement table — the m2 enforcement set is a strict
  subset, see §B6 below), §6 (attack scenarios — m2 closes
  6.4, 6.5, 6.8, partial 6.9; m2 does NOT yet close 6.1, 6.2,
  6.3, 6.10 since those depend on m4/m5/m1 surfaces already
  landed). Read the §5.7 + §7.4.1 v1-status banners — frontend
  external attach and helper plugins are deferred and m2 must
  not introduce surfaces for them.
- `rafaello/plans/streams/b-fittings/rfc-fittings-notifications.md`
  for `PeerHandle` shape; m0 already landed it. m2 is the first
  in-tree consumer of `Server::peer()` / `Client::peer()` /
  `Client::with_service()` outside fittings' own tests.
- `rafaello/plans/glossary.md`.
- m1's existing `rafaello-core` surface — `compile::CompiledPlugin`,
  `broker_acl::{BrokerAcl, PluginAcl}`, `topic_id::derive`,
  `validate::topic::{validate_topic, validate_pattern}`,
  `lock::canonical_id::CanonicalId`, the typed errors
  enumerated in `error::*`. m2 calls into these; it does not
  modify them.
- Lockin's public Rust API for the v1 sandbox backend
  (`lockin/crates/sandbox/src/lib.rs`):
  - `Sandbox::builder() -> SandboxBuilder` with
    `read_path`/`read_dir`/`write_path`/`write_dir`/
    `exec_path`/`exec_dir`/`network`/`network_deny`/
    `network_allow_all`/`network_proxy(loopback_port: u16)`/
    `inherit_fd`/`inherit_fd_as`/`max_cpu_time`/
    `max_open_files`/`max_address_space`/`max_processes`/
    `disable_core_dumps`/`allow_kvm`/
    `allow_interactive_tty`/`allow_non_pie_exec`/
    `raw_seatbelt_rule`.
  - `SandboxBuilder::command(self, program: &Path) ->
    Result<SandboxedCommand>` consumes the builder.
  - `SandboxedCommand::env`/`envs`/`env_remove`/`env_clear`
    apply env to the prepared command (after builder is
    consumed). `spawn() -> Result<SandboxedChild>`,
    `SandboxedChild::{wait,try_wait,kill,id,as_child,
    as_child_mut,into_parts}`.
- `outpost::NetworkPolicy` (already used by m1 for parse-time
  dry-run) plus `outpost_proxy::start(policy) -> ProxyHandle`
  / `ProxyHandle::listen_addr()` for runtime proxy startup.

## In scope

The work decomposes into module groups in `rafaello-core` plus
one new fixture binary. Per-commit granularity is the driver's
call when drafting `commits.md`; this section names the public
API surface and the negatives the test matrix asserts.

### W — workspace dependencies

- **W1.** Add to `rafaello/Cargo.toml`'s `[workspace.dependencies]`
  (introduced in m1 c01):
  - `tokio = { version = "1", features = ["rt-multi-thread",
    "macros", "io-util", "net", "sync", "time", "process"] }`
  - `tracing = "0.1"`
  - `fittings-core = { path = "../fittings/crates/fittings-core" }`
  - `fittings-server = { path = "../fittings/crates/fittings-server" }`
  - `fittings-client = { path = "../fittings/crates/fittings-client" }`
  - `fittings-wire = { path = "../fittings/crates/fittings-wire" }`
  - `lockin-sandbox = { path = "../lockin/crates/sandbox", package = "lockin-sandbox" }`
    (verify the actual crate name; the m1 scope doc names the
    re-export root as `lockin::Sandbox::builder()` but the
    workspace may publish the leaf crate name)
  - `outpost-proxy = { path = "../outpost/crates/outpost-proxy" }`
  - `nix = { version = "0.27", features = ["socket", "fs"] }`
    (for `socketpair(2)` — there is no stable `std::os::unix`
    socketpair helper; lockin uses `nix` for the same reason)
  - dev-deps: `tempfile = { workspace = true }` already in m1;
    add `assert_cmd = "2"` for the fixture-plugin binary
    integration tests.
- **W2.** `rafaello-core/Cargo.toml` `[dependencies]` adds the
  m2 deps from W1 (`workspace = true`). `[dev-dependencies]`
  adds the fixture-plugin path-dep (W3). m1's existing deps
  stay.
- **W3.** New in-tree binary crate
  `rafaello/crates/fixtures/rfl-bus-fixture/` (added to
  `[workspace.members]`). Tiny self-contained subprocess
  plugin: speaks the m2 bus protocol over `RFL_BUS_FD`, used
  exclusively by m2 integration tests. NOT installed by `rfl`
  end users; the `Cargo.toml` carries `publish = false` and a
  README pointer back to m2 scope. Built unconditionally by
  the workspace so per-commit `cargo test` can call it. The
  test harness resolves the binary path via
  `env!("CARGO_BIN_EXE_rfl-bus-fixture")` (cargo's standard
  bin-test integration; no manual build steps).
- **W4.** No new top-level workspace deps beyond W1. `serde`,
  `serde_json`, `thiserror` already in workspace.dependencies
  from m1 c01.

### B — bus broker (`rafaello_core::bus`)

The broker is the in-process publish/subscribe layer. It owns
no transport: each plugin connection is a `fittings`
`PeerHandle` registered with the broker via the supervisor.
The broker's job is publish-authority enforcement, subscribe
ACL, fan-out, and topic-grammar revalidation.

- **B1.** New module `rafaello_core::bus`. Public surface:
  - `pub struct Broker` — owns the live registry of plugin
    connections + the `BrokerAcl` it was constructed from.
    Cheap-to-clone `Arc<BrokerInner>` shape so multiple
    fittings `Service` impls can hold a clone.
  - `Broker::new(acl: BrokerAcl) -> Self`.
  - `Broker::register_plugin(canonical: CanonicalId, peer:
    PeerHandle) -> Result<RegisteredPlugin, BrokerError>` —
    called by the supervisor immediately after spawn (before
    the child has had a chance to publish anything; the spawn
    sequencing is §SP3). Returns a `RegisteredPlugin` RAII
    guard whose `Drop` unregisters the plugin from the
    broker. Errors:
    - `BrokerError::NotInAcl(canonical)` — canonical not
      present in `BrokerAcl::plugins`.
    - `BrokerError::AlreadyRegistered(canonical)` —
      duplicate registration.
  - `Broker::publish_core(topic: &str, payload: Value) ->
    Result<(), BrokerError>` — core-side publish; `topic`
    must start with `"core."` and pass topic-grammar
    revalidation (§B5). Used by m2 only in tests + a single
    `BootSummary` event; m4/m5 grow the call sites.
  - `Broker::handle_plugin_publish(canonical: &CanonicalId,
    msg: PublishMsg) -> Result<(), BrokerError>` — called
    from the per-plugin fittings Service impl on inbound
    `bus.publish` notifications. Validates publisher-vs-topic
    authority (§B3), grammar (§B5), payload shape (§B4),
    `in_reply_to` (§B6), then fans out (§B7).
  - `pub struct PublishMsg { topic: String, payload: Value,
    in_reply_to: Option<Vec<JsonRpcId>>, taint:
    Option<Vec<TaintEntry>> }` — the on-wire shape for
    `bus.publish` notification params; serde-derived;
    deny_unknown_fields.
  - `pub struct TaintEntry { source: String, detail: Option<String> }`
    — m2 round-trips the field; m4 enforces the populated-by-core
    rule. m2 stores plugin-supplied taint verbatim (no
    synthesis, no superset check; both are m4 work).
- **B2.** `BrokerError` typed enum (lives in
  `rafaello_core::error`, mirror of the m1 pattern):
  - `NotInAcl(CanonicalId)`
  - `AlreadyRegistered(CanonicalId)`
  - `PublishOnReservedNamespace { canonical: CanonicalId,
    topic: String }` — plugin tried to publish `core.*`,
    `provider.*`, `frontend.*`, or another plugin's
    `plugin.<other-topic-id>.*`.
  - `PublishOutsideGrant { canonical: CanonicalId, topic:
    String }` — plugin published a `plugin.<own-topic-id>.*`
    topic that wasn't in its lock-granted `publishes` set.
  - `InvalidTopic { topic: String, reason: String }` —
    grammar revalidation (§B5).
  - `InvalidPayload { reason: String }` — params decode
    failure or `bus.publish` body shape violation (§B4).
  - `MissingInReplyTo { canonical: CanonicalId, topic:
    String }` — plugin published a class that requires
    `in_reply_to` without one (§B6 m2 subset).
  - `Internal { detail: String }` — broker-internal failures
    (channel send error, etc.); not a security boundary.
- **B3.** **Publish-authority enforcement** (the core
  invariant). For each `handle_plugin_publish`:
  1. Compute `expected_self = format!("plugin.{}.",
     plugin_acl.topic_id)`.
  2. If `topic` starts with `core.`, `provider.`, or
     `frontend.` → `PublishOnReservedNamespace`.
  3. If `topic` starts with `plugin.<X>.` where `<X> !=
     plugin_acl.topic_id` → `PublishOnReservedNamespace`
     (cross-plugin masquerade attempt).
  4. If `topic` starts with `plugin.<own-topic-id>.` and is
     not in `plugin_acl.publish_topics ∪
     plugin_acl.auto_subscribes` → `PublishOutsideGrant`.
     (Note: `auto_subscribes` is the implicit
     `plugin.<topic-id>.tool_request` self-subscribe; a
     plugin publishing on its own subscribed-to topic is a
     no-op nonsense case that m2 lets through to the
     subscribe layer, which will then drop it because the
     plugin is not its own subscriber. **Decision held over
     for pi**: should publish-on-own-auto-subscribe be a
     validation error rather than a delivery no-op? The
     conservative choice is to error; m2 takes it.)
  5. Otherwise authorised; proceed.
- **B4.** **`bus.publish` payload shape**. The m2 wire shape
  for the `bus.publish` notification's `params` is exactly
  `PublishMsg` (B1). Decode with
  `#[serde(deny_unknown_fields)]` so a plugin sending extra
  keys gets `InvalidPayload`. Required fields: `topic` (string),
  `payload` (any JSON value, including `null`). Optional:
  `in_reply_to` (array of `JsonRpcId` per fittings-wire's
  type — string | number | null), `taint` (array of
  `TaintEntry`).
- **B5.** **Topic-grammar revalidation.** Every publish
  (plugin or core) re-runs `validate::topic::validate_topic`
  against the publish topic. The lock-time grant's
  `publishes` list was already validated at compile, but a
  plugin can construct any string at runtime; the broker is
  the runtime-authority gate. Symmetric for the (currently
  absent) runtime subscribe path. Failure →
  `InvalidTopic`.
- **B6.** **`in_reply_to` enforcement (m2 subset).** Per
  security RFC §7.2.6, the v1 enforcement table is large.
  m2 ships the **two enforceable-in-isolation entries** and
  defers the rest to m4 (which introduces the provider +
  tool dispatch surface they reference):
  - `plugin.<id>.tool_result` → `in_reply_to` required,
    exactly one entry. m2 enforces presence + arity; m2
    does NOT yet enforce that the referenced id matches a
    real prior `tool_request` (no tool dispatch yet → no
    correlation map → that check is m4's).
  - `plugin.<id>.rpc_reply` → `in_reply_to` required,
    exactly one entry. Same scope: m2 enforces presence +
    arity; m2 does NOT yet enforce that the referenced id
    matches a real prior `rpc_call` (no plugin-to-plugin
    RPC routing in m2).
  Other entries from §7.2.6 (`provider.<id>.tool_request`,
  `provider.<id>.assistant_message`,
  `frontend.<id>.confirm_answer`,
  `frontend.<id>.user_message`, `plugin.<id>.progress`)
  do not apply to m2 because the publishers don't exist
  yet (no provider, no frontend, no tool dispatch). The
  optional `plugin.<id>.*` superset rule is m4 territory;
  m2 stores `in_reply_to` verbatim without superset
  checks.
- **B7.** **Fan-out.** For each registered plugin, check the
  set `subscribe_patterns ∪ auto_subscribes` against the
  delivered topic using a fresh pattern-matcher
  (`validate::pattern::matches(pattern, topic)` — m2 adds
  this small helper since m1 only validated patterns syntactically).
  If matched, call `peer.notify("bus.event", BusEvent {
  topic, payload, in_reply_to, taint })` on that plugin's
  `PeerHandle`. The publishing plugin is **excluded from its
  own fan-out** (no echo): even if its grant subscribes to a
  pattern that matches its own publish, the broker does not
  loop the event back. Fan-out errors are logged via
  `tracing::warn!` with the target canonical id but do NOT
  fail the publish call (best-effort delivery; per `notifications-rfc`
  §3b a slow consumer drops to the sink, not the publisher).
  - **`bus.event` is m2's chosen outbound notification
    method.** Plugins distinguish events from any other
    incoming notifications by method name. `bus.publish`
    is plugin→core; `bus.event` is core→plugin. Both are
    fittings notifications (no response).
- **B8.** **Pattern matcher.** `rafaello_core::validate::pattern`
  gains `pub fn matches(pattern: &str, topic: &str) -> bool`.
  Implements the security-RFC §5.1 rules:
  - `*` matches exactly one segment.
  - `**` (final pseg only) matches one or more trailing
    segments.
  - Other psegs match by exact string equality.
  Inputs are assumed pre-validated (caller must have run
  `validate_pattern` / `validate_topic`). Tested as a unit
  with the §5.1 example matrix.

### SP — plugin supervisor (`rafaello_core::supervisor`)

The supervisor owns child processes, lockin sandboxes, the
outpost proxy lifecycle (when proxy mode is configured), and
the socketpair plumbing that authenticates the bus connection.

- **SP1.** New module `rafaello_core::supervisor`. Public
  surface:
  - `pub struct PluginSupervisor`. Holds an `Arc<Broker>`
    and an internal `Vec<SpawnedPlugin>` registry.
  - `PluginSupervisor::new(broker: Arc<Broker>) -> Self`.
  - `PluginSupervisor::spawn(&mut self, plan: &CompiledPlugin)
    -> Result<&SpawnedPlugin, SpawnError>` — async (called
    from inside a tokio runtime). Sequencing in §SP3.
  - `PluginSupervisor::shutdown(self) -> Result<(),
    SpawnError>` — sends SIGTERM to every spawned child,
    waits up to a configurable grace period (default 5 s),
    then SIGKILLs survivors. Drops the `Broker`'s registered
    handles. m2 does NOT yet implement the per-plugin
    `rfl plugin stop` CLI; this is a library function.
  - `pub struct SpawnedPlugin { canonical: CanonicalId,
    topic_id: String, child_pid: u32, peer: PeerHandle, ... }`
    — opaque-ish view; the public fields are the ones tests
    need (`canonical`, `topic_id`, `child_pid`, `peer`).
- **SP2.** `SpawnError` typed enum:
  - `NotInAcl(CanonicalId)` — broker rejected registration.
  - `Lockin { canonical, source: lockin_sandbox::Error }` —
    `SandboxBuilder::command(...)` failed (e.g. missing syd
    on Linux).
  - `Spawn { canonical, source: std::io::Error }` —
    `SandboxedChild::spawn()` failed.
  - `ProxyStart { canonical, source: std::io::Error }` —
    `outpost_proxy::start` failed (port bind failure, etc.).
  - `Socketpair { source: nix::Error }` —
    `socketpair(AF_UNIX, SOCK_STREAM, 0)` failed.
  - `FittingsBuild { canonical, source: fittings_core::Error }`
    — failed to build the per-connection `Server` /
    `Client` from the inherited fd.
  - `EntryNotExecutable { canonical, path: PathBuf }` —
    pre-spawn sanity check on `plan.entry_absolute`.
- **SP3.** **Spawn sequencing.** This is the load-bearing
  sequence. Order matters because the broker must accept the
  fd before the child has a chance to publish:
  1. Verify `plan.canonical` is in `broker.acl.plugins`;
     else `SpawnError::NotInAcl`. (Defence in depth — same
     check the broker repeats at registration. Up-front
     check produces a clean error before any resources are
     allocated.)
  2. `socketpair(AF_UNIX, SOCK_STREAM, 0)` → `(core_fd,
     child_fd)`. Both `OwnedFd`. CLOEXEC defaults are
     irrelevant here because lockin's `inherit_fd` mechanism
     deliberately survives the CLOEXEC sweep.
  3. If `plan.network` is `NetworkPlan::Proxy { allow_hosts }`,
     synthesise an `outpost::NetworkPolicy::from_allowed_hosts`
     and call `outpost_proxy::start(policy).await` to obtain
     a `ProxyHandle`. Capture `proxy.listen_addr().port()`.
  4. Build `SandboxBuilder` from `plan.filesystem` /
     `plan.network` / `plan.limits`:
     - For each `read_paths` / `read_dirs` /
       `write_paths` / `write_dirs` / `exec_paths` /
       `exec_dirs` from `FilesystemPlan`, call the
       matching `SandboxBuilder` method. Order does not
       matter; the resulting policy is order-independent.
     - `NetworkPlan::Deny` → `network_deny()`.
     - `NetworkPlan::AllowAll` → `network_allow_all()`.
       (Used only by `--i-know-what-im-doing` overrides;
       m2 carries it because the plan supports it.)
     - `NetworkPlan::Proxy { allow_hosts: _ }` →
       `network_proxy(loopback_port)` from step 3.
     - `LimitsPlan` → `max_cpu_time`, `max_open_files`,
       `max_address_space` (only if `Some`), `max_processes`
       (only if `Some`). `disable_core_dumps()` is set
       unconditionally on lockin Linux (defensive default).
       The other lockin builder knobs (`allow_kvm`,
       `allow_interactive_tty`, `allow_non_pie_exec`,
       `raw_seatbelt_rule`) are NOT exposed by `CompiledPlugin`
       and the supervisor does not call them. This is
       intentional: m2 ships the conservative default only;
       knobs return when a v1 plugin needs them, with a
       matching plan field added.
  5. `inherit_fd_as(child_fd, RFL_BUS_FD_NUMBER)` —
     **`RFL_BUS_FD_NUMBER` is fixed = `3`** in v1 (the first
     fd above stdio). The plugin then learns the number from
     the env var (§SP6). Choosing a fixed number simplifies
     the lockin policy (no need to communicate the dynamic
     fd number into the sandbox) and matches the convention
     systemd / fittings-spawn use.
  6. `SandboxBuilder::command(&plan.entry_absolute)` →
     `SandboxedCommand`. If `plan.entry_absolute` is not an
     executable file, return `SpawnError::EntryNotExecutable`
     before consuming the builder. (Spot check; the kernel
     would also reject at exec time, but a typed error
     before the spawn is friendlier and easier to test.)
  7. **Apply env to the prepared command** (after the
     builder is consumed):
     - `env_clear()` first — start from nothing.
     - Inject the reserved vars: `RFL_BUS_FD = "3"`,
       `RFL_PLUGIN = canonical.to_string()`. Per
       security-RFC §5.5.1, these are NEVER user-supplied
       (m1's env scrubber already strips them from `pass`).
     - For each `plan.env.pass` key, look it up in the
       parent process env (`std::env::var_os`), and forward
       only if present. Missing keys are silently skipped
       (m2 carries m1's behavior; presence is a deployment
       concern, not a spawn-time error).
     - For each `(k, v)` in `plan.env.set`, `env(k, v)`.
     - Order: `pass` before `set`, so `set` overrides on
       collision (matches m1 compile behavior).
  8. `command.spawn()` → `SandboxedChild`. The child has
     `child_fd` open at fd 3 and the env above; nothing
     else.
  9. **Drop `child_fd` on the parent side.** It was passed
     by `inherit_fd_as` (which takes ownership of the fd
     internally — verify against lockin's `OwnedFd` API).
     The parent retains `core_fd` only.
  10. Build a fittings transport from `core_fd` (an
      `AsyncFd<UnixStream>` wrapper) and construct a
      `Server` + `Client` over it (per §15.6 each peer
      runs both halves).
  11. Build the `Service` impl that handles inbound
      `bus.publish` notifications by calling
      `broker.handle_plugin_publish(&canonical, msg)`. The
      Service belongs to the per-connection fittings server.
  12. Call `broker.register_plugin(canonical, peer)` with
      the `Server::peer()` handle for outbound `bus.event`
      delivery. Hold the returned `RegisteredPlugin` guard
      inside `SpawnedPlugin` so dropping a SpawnedPlugin
      tears the broker registration down.
  13. Spawn the fittings server's serve loop on the tokio
      runtime; store its `JoinHandle` inside `SpawnedPlugin`
      for shutdown.
  14. Return `&SpawnedPlugin`.
  Steps 1–9 are synchronous. Steps 3 + 10–13 use tokio.
  Failure at any step before broker registration tears down
  resources allocated by earlier steps (proxy handle, child
  fd, child process via SIGKILL on `SandboxedChild::kill`).
  This is the unwinding contract the negative-test matrix
  exercises.
- **SP4.** **No hardcoded bypass.** `PluginSupervisor::spawn`
  has exactly one entry point. There is no `spawn_unsandboxed`,
  no debug flag, no `RFL_INSECURE` env var. The fixture
  plugin is spawned through the same path as any other
  plugin — its lock entry is constructed by the test fixture
  helper at the start of each integration test. This is the
  invariant pi review must guard.
- **SP5.** **Lifecycle ownership.** `Drop` for `SpawnedPlugin`
  sends SIGTERM to the child (best-effort), drops the proxy
  handle (which shuts the proxy down), drops the
  `RegisteredPlugin` guard (which unregisters from the
  broker), and aborts the serve-loop join handle. The
  bookkeeping is RAII: tests can construct a `PluginSupervisor`,
  spawn a fixture, drop the supervisor at end-of-test, and
  not leak processes or proxies. `PluginSupervisor::shutdown`
  is the cooperative-shutdown variant that waits for SIGTERM
  to take effect before falling back to SIGKILL.
- **SP6.** **Reserved env vars.** Per security-RFC §5.5.1, m2
  injects `RFL_BUS_FD` and `RFL_PLUGIN`. m1's compile-time
  env scrubber already rejects these in `env.set` /
  `env.pass`. m2 adds a defence-in-depth assertion at spawn
  time: if `plan.env.set` or `plan.env.pass` contains
  `RFL_BUS_FD` / `RFL_PLUGIN` / `RFL_HELPER_FD` (the helper
  var is reserved per §5.5.1 even though helpers are deferred
  per row 26), `SpawnError::Internal { detail: "compile
  output contained reserved env var X — V3 should have
  rejected" }` returns. This is the m1-spot-check pattern
  (compile.rs already does similar `ValidationNotRun`
  asserts).

### F — fixture plugin (`rfl-bus-fixture` binary)

Tiny standalone subprocess plugin used exclusively by the m2
integration tests. It exercises the bus protocol from the
plugin side, in the conditions a real plugin would face. It
does NOT pretend to be a useful plugin; m4 ships the first
real one (`read-file`).

- **F1.** Crate `rafaello/crates/fixtures/rfl-bus-fixture/`,
  `publish = false`. Single binary entry point (`src/main.rs`).
  Workspace member; built unconditionally so test binaries
  can locate it via `env!("CARGO_BIN_EXE_rfl-bus-fixture")`.
- **F2.** Behavior controlled by env vars the test sets via
  `plan.env.set` (the fixture's lock entry). Each var enables
  one assertion path:
  - `RFL_FIXTURE_PUBLISH=plugin.<topic-id>.hello` — on
    startup, send one `bus.publish` notification with that
    topic and payload `{"msg": "hello"}`.
  - `RFL_FIXTURE_PUBLISH_BAD_NAMESPACE=core.session.user_message`
    — on startup, attempt one `bus.publish` on the named
    forbidden topic; do not exit on error (the broker
    silently rejects notifications since they have no
    response; the assertion is on the broker side, not
    here).
  - `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.echo` — register
    a fittings `Service` impl that handles a `core.fixture.echo`
    request by echoing the params back. Used by the
    core→plugin `peer.call` round-trip test.
  - `RFL_FIXTURE_CALL_CORE=core.fixture.ping` — on startup,
    issue one `peer.call("core.fixture.ping", {"n": 42})`
    and exit 0 on a successful response, exit 2 on error.
    Used by the plugin→core `peer.call` round-trip test.
  - `RFL_FIXTURE_OPEN_FILE=/etc/passwd` — on startup,
    attempt `std::fs::read(path)`. Used by the
    "lockin denies file open outside grant" negative.
    The fixture does not assert; the test reads the
    fixture's exit code (or watches stderr) and asserts
    the lockin sandbox kept the file unreachable.
  - `RFL_FIXTURE_PUBLISH_BAD_GRAMMAR=plugin.id_xxx.UPPERCASE`
    — invalid topic grammar; broker rejects.
  - `RFL_FIXTURE_PUBLISH_BAD_REPLY_TO=plugin.<topic-id>.tool_result`
    — publishes a `tool_result` topic with no `in_reply_to`;
    broker rejects with `MissingInReplyTo`.
  - `RFL_FIXTURE_HOLD_OPEN=1` — after performing the above
    behaviors, sleep until SIGTERM. Used by the lifecycle
    test that asserts `Drop` of the supervisor kills the
    child.
  - **`RFL_FIXTURE_HOLD_OPEN` defaults to 0**; without it
    the fixture exits 0 once the configured behaviours
    complete. This keeps tests fast.
- **F3.** Wire format: the fixture uses fittings-core to
  build a `Client` (for outbound `bus.publish` and
  `peer.call`) and a `Server` (for inbound
  `core.fixture.*` requests). The bus fd is read from
  `RFL_BUS_FD` (`u32`, parsed from env), wrapped in
  `tokio::net::UnixStream::from_std(...)` after
  `set_nonblocking(true)`. Both halves run on the same
  fd via fittings' `PeerHandle`.
- **F4.** No `tracing` setup, no logging output beyond
  `eprintln!` for hard failures. The fixture is reaped
  immediately by tests; verbose logging would clutter
  test output.

### I — integration test suite (`rafaello-core/tests/`)

Every test lives under `rafaello/crates/rafaello-core/tests/`
unless otherwise noted. Each test is one `#[tokio::test(flavor
= "multi_thread")]` (the rt-multi-thread flavor matches the
real `rfl chat` runtime; tests must be deterministic across
schedules).

Test fixtures (lock + project layout) are constructed
programmatically with `tempfile::tempdir()` per the m1 pattern
— no on-disk fixtures directory.

A small shared helper module
`rafaello/crates/rafaello-core/tests/common/m2_harness.rs`
(carved next to m1's `common/`) provides:
- `harness::FixtureLockBuilder` — builds a one-plugin or
  multi-plugin lock entry pointing at the
  `rfl-bus-fixture` binary path; wraps `compile_plugin` and
  `broker_acl::compile`.
- `harness::Spawn` — spawns a `PluginSupervisor` with the
  configured fixture(s), returns handles + a one-shot
  receiver for inbound `bus.event` deliveries from a
  test-side subscriber plugin (the harness adds a second
  fixture instance that subscribes to `**` for observation
  purposes).

The harness IS test code; it does not leak into
`rafaello-core`'s public API.

### Demo bar — see "Demo bar" section below.

### E — error-surface additions

m2 grows `rafaello_core::error`:
- `pub enum BrokerError { ... }` (variants enumerated in §B2).
- `pub enum SpawnError { ... }` (variants enumerated in §SP2).
- Both re-exported from `lib.rs`. Both `#[non_exhaustive]`,
  `thiserror`-derived. The top-level `Error` enum gains
  `#[from]` arms.

## Out of scope

The following are explicitly NOT in m2 and are not allowed to
sneak in via "while I'm here" implementation drift:

- **`rfl chat` / any CLI surface.** m3 ships the binary glue
  that constructs a `PluginSupervisor` from `rafaello.lock`.
  m2's only entry points are library functions called from
  tests.
- **Frontends.** `frontend.<attach-id>.*` namespace is
  reserved (`decisions.md` row 27); m2 has no frontend
  registration path, no UDS attach socket, no
  `frontend.hello`. The TUI is m3.
- **Provider plugins.** `provider.<provider-id>.*` namespace
  is reserved by name; m2's broker rejects publishes on
  it (a plugin's `bindings.provider = true` does NOT grant
  `provider.*` publish authority — m2 ignores `provider`
  bindings entirely; m4 wires the provider role into
  publish authority). The fixture plugin is NOT a provider.
- **Tool dispatch.** No `core.session.tool_request` →
  `plugin.<id>.tool_request` routing. The
  `auto_subscribes` field from m1's `BrokerAcl` is honoured
  for fan-out (so the routing path will work in m4 without
  m2 retro-fits) but no tool is registered, no provider
  proposes one.
- **Sink confirmation, `user_grants`, taint synthesis.**
  m4 (taint envelope + read-only tool) and m5 (sinks +
  confirmation + `user_grants`) own these.
- **Core re-emission of `provider.*` → `core.session.*`.**
  No provider, no re-emission.
- **Session persistence.** No SQLite, no entry log. m3
  territory.
- **Renderer model.** No render tree, no `Entry`. m3.
- **`bus.subscribe` runtime request.** Subscribes are derived
  entirely from the lock at registration time. The
  overview-§4.1 sketch of a runtime `bus.subscribe` request
  is reserved for future need; m2 does not need it (no
  dynamic subscription change in v1's design — the lock IS
  the ACL).
- **Helper plugins.** `helper_for` / `RFL_HELPER_FD` deferred
  to v2 (`decisions.md` row 26); m1 already rejects them at
  parse time. m2 must not add a spawn path for them. SP6's
  `RFL_HELPER_FD` reserved-name check is *defence in depth*
  — the spawn path itself does not consult the var.
- **Eager / boot / event / command / kind lazy-load
  triggers.** m2's supervisor exposes the synchronous
  `spawn(plan)` primitive; the lazy-load orchestrator that
  reads `LoadPolicy` and decides when to spawn lives in m3
  (when there's a session loop to drive it). m2 spawns from
  test code only.
- **macOS support.** The lockin sandbox compiles on macOS,
  but m2's integration tests rely on Linux specifics (syd
  for lockin, `/etc/passwd` as a denied-read target). The
  test suite runs Linux-only; macOS CI is skipped for m2
  with a `#[cfg_attr(target_os = "macos", ignore)]` on each
  spawn-bearing integration test. m6 polish revisits.
- **Cross-plugin RPC routing (`plugin.<a>.rpc_call.<b>` etc.).**
  Not in v1's design; m2 doesn't need it. The broker's
  publisher-vs-namespace rule already prevents the
  cross-plugin masquerade case (B3 #3).
- **`core.lifecycle.*` rejection events.** Per security-RFC
  §5.4.1 broker rejection should publish
  `core.lifecycle.tool_result_rejected` etc. m2 emits a
  single event class — `core.lifecycle.publish_rejected`
  with `{ canonical, topic, reason }` — when the broker
  rejects a plugin publish. The full lifecycle vocabulary
  (per-event-class rejection topics) is m4 territory once
  there are more event classes.
- **`PeerHandle::call` correlation across plugin shutdown.**
  m2's lifecycle drops the `PeerHandle` on plugin exit; any
  in-flight `peer.call` resolves with
  `FittingsError::Transport` per fittings' contract. m2
  does not ship a higher-level "the plugin died, retry the
  call" abstraction.

## Demo bar

Tests live under `rafaello/crates/rafaello-core/tests/`. Every
test is `#[tokio::test(flavor = "multi_thread")]` and uses
`tempfile` for transient state. Tests are Linux-only as noted
above.

### Positive integration tests

| Test file | Exercises |
|-----------|-----------|
| `bus_pattern_matches.rs` | Unit suite for `validate::pattern::matches` against the §5.1 example matrix (`*`, `**`, exact match, multi-segment `**`). |
| `broker_register_plugin.rs` | `Broker::register_plugin` happy path: a canonical present in the ACL registers, returns the RAII guard; dropping the guard removes the registration; `Broker::handle_plugin_publish` after drop returns `NotInAcl`. |
| `broker_publish_core.rs` | `Broker::publish_core("core.lifecycle.boot", ...)` with one registered plugin whose grant subscribes to `core.lifecycle.**` delivers the event via `peer.notify` on the plugin's `PeerHandle`. |
| `supervisor_spawn_fixture.rs` | The **headline test**: `PluginSupervisor::spawn(plan)` for the `rfl-bus-fixture` plan with `RFL_FIXTURE_PUBLISH=plugin.<topic-id>.hello` set. The fixture publishes one event; the harness's observer fixture (subscribed to `**`) receives it. Asserts `topic`, `payload`, and that the publish authority was the fixture's canonical id. |
| `supervisor_peer_call_core_to_plugin.rs` | Fixture has `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.echo`. Test calls `spawned.peer.call("core.fixture.echo", json!({"x":1})).await` and asserts the response is `{"x":1}`. |
| `supervisor_peer_call_plugin_to_core.rs` | Fixture has `RFL_FIXTURE_CALL_CORE=core.fixture.ping`. The supervisor's per-connection fittings `Server` registers a `Service` that responds to `core.fixture.ping` by echoing `+1`. The test waits for the fixture to exit 0 (success). |
| `supervisor_bus_publish_round_trip.rs` | Two-fixture topology: fixture A publishes `plugin.<A-topic-id>.greet` once; fixture B subscribed to `plugin.<A-topic-id>.greet` (granted in B's lock) receives the event and acks via a side-channel `peer.call` to a test-only `Service`. Exercises broker fan-out across multiple plugins. |
| `supervisor_lifecycle_drop_kills_child.rs` | Spawn fixture with `RFL_FIXTURE_HOLD_OPEN=1`. Drop the `PluginSupervisor`. After a bounded wait (≤ grace period), the child PID is no longer alive (`kill -0` returns ESRCH). |
| `supervisor_proxy_starts_for_proxy_plan.rs` | Construct a `CompiledPlugin` with `NetworkPlan::Proxy { allow_hosts: ["example.com"] }`. Supervisor.spawn → `outpost_proxy::start` is called once; the proxy listens on a loopback port; the lockin policy gets `network_proxy(port)`. The fixture does not actually issue a CONNECT (no real network in tests); the assertion is on the proxy startup + port plumbing. |
| `supervisor_env_pass_set_applied.rs` | Plan with `env.pass = ["FAKE_API_KEY"]` and `env.set = {"FOO": "bar"}`. Test sets `FAKE_API_KEY=abc` in the parent process before spawn (uses `temp_env::with_var` or equivalent). Fixture echoes its own env via `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.dump_env`; test calls and asserts both keys present with expected values, plus `RFL_BUS_FD=3` and `RFL_PLUGIN=<canonical>`. |
| `supervisor_private_state_dir_writable.rs` | Plan compiled from a lock entry with the m1-injected private-state grant. Fixture has `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.write_private_state`; on call, it writes a known byte to `${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/marker`. Test verifies the file lands on disk and the path uses the **topic-id** form (not the canonical id) per `decisions.md` row 37. |

### Negative integration tests

| Test file | Asserts |
|-----------|---------|
| `broker_publish_core_namespace_rejected.rs` | Plugin publishes `core.session.user_message` → broker returns `BrokerError::PublishOnReservedNamespace`; a `core.lifecycle.publish_rejected` event fires; no fan-out. |
| `broker_publish_provider_namespace_rejected.rs` | Plugin publishes `provider.openai.tool_request` → same `PublishOnReservedNamespace` (m2 reserves `provider.*` for m4 provider-plugin authority). |
| `broker_publish_frontend_namespace_rejected.rs` | Plugin publishes `frontend.tui.confirm_answer` → same. |
| `broker_publish_other_plugin_namespace_rejected.rs` | Plugin A publishes `plugin.<B-topic-id>.tool_result` → `PublishOnReservedNamespace`. The cross-plugin masquerade case. |
| `broker_publish_outside_grant_rejected.rs` | Plugin A publishes `plugin.<A-topic-id>.greet` when its lock-granted `publishes` list is `["plugin.<A-topic-id>.progress"]` → `PublishOutsideGrant`. |
| `broker_publish_invalid_topic_grammar_rejected.rs` | Plugin publishes `plugin.<topic-id>.UPPERCASE` (invalid pseg per §5.1) → `InvalidTopic`. Includes a sub-case for `plugin.<topic-id>.has spaces` and one for an empty topic. |
| `broker_publish_extra_field_rejected.rs` | Plugin sends `bus.publish` with an extra unknown field in `params` → `InvalidPayload` from `deny_unknown_fields`. |
| `broker_tool_result_missing_in_reply_to_rejected.rs` | Plugin publishes `plugin.<topic-id>.tool_result` without `in_reply_to` → `MissingInReplyTo`. |
| `broker_rpc_reply_missing_in_reply_to_rejected.rs` | Same for `plugin.<topic-id>.rpc_reply`. |
| `broker_publishing_plugin_excluded_from_own_fanout.rs` | Plugin A's grant subscribes to `plugin.<A-topic-id>.**` (a defensive self-subscribe configured in the lock) AND publishes `plugin.<A-topic-id>.foo`. The publishing plugin does NOT receive the event back; only other subscribers do. |
| `supervisor_spawn_canonical_not_in_acl_refused.rs` | Construct a `CompiledPlugin` whose canonical id is not present in the `BrokerAcl` (e.g. by compiling against a lock that only contains a different plugin, then mutating the canonical). `supervisor.spawn(plan)` → `SpawnError::NotInAcl`. No process is spawned (verifiable via process-table observation; or by the absence of the proxy port if the plan's network is `Proxy`). |
| `supervisor_spawn_entry_not_executable.rs` | `plan.entry_absolute` points at a regular file with no exec bit → `SpawnError::EntryNotExecutable` BEFORE `SandboxBuilder::command` is consumed. |
| `supervisor_lockin_denies_outside_grant_read.rs` | Spawn fixture with grant `read_dirs = ["${PROJECT_ROOT}"]` only (no `/etc`). Fixture has `RFL_FIXTURE_OPEN_FILE=/etc/passwd` and `RFL_FIXTURE_RESPOND_PEER_CALL=core.fixture.report_open_result`. Test calls `report_open_result`; fixture replies with the `errno` from the failed open. Assertion: open failed (typically `EPERM` or `ENOENT` depending on lockin's syd policy). The kernel sandbox is the enforcer; this is the integration assertion that the supervisor wired lockin correctly. |
| `supervisor_lockin_denies_outside_grant_write.rs` | Same shape with `write_dirs = ["${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>"]` and the fixture attempting to write to `${PROJECT_ROOT}/forbidden`. Write fails with the lockin error; the file does not exist after the fixture exits. |
| `supervisor_reserved_env_in_set_refused.rs` | Construct a `CompiledPlugin` with `env.set = {"RFL_BUS_FD": "99"}` (bypass m1's V3 by hand-constructing the plan). `supervisor.spawn` → `SpawnError::Internal { detail: "compile output contained reserved env var ..." }`. Defence-in-depth assertion that the spawn path checks even if V3 was skipped. |
| `supervisor_drop_during_spawn_unwinds.rs` | Force a failure at SP3 step 8 (e.g. `entry_absolute` points at `/usr/bin/false` so the child exits immediately, or use a chmod-busy spawn-failure path). Assertion: any allocated proxy handle, any open `core_fd`, any partially-registered broker entry are all torn down before `spawn()` returns the error. (Verifiable via fd count under `/proc/self/fd` before and after.) |

### Manual validation in `manual-validation.md`

The driver runs and captures, in addition to `cargo test
--manifest-path rafaello/Cargo.toml -p rafaello-core` green
on Linux:

- One end-to-end manual run of the fixture spawning under
  `cargo test -p rafaello-core --test supervisor_spawn_fixture
  -- --nocapture`, with `RUST_LOG=rafaello_core=debug`, to
  show the broker registration trace + the fixture's
  `bus.publish` round-trip in human-readable form.
- A `find rafaello/crates/rafaello-core/src -name '*.rs' |
  sort` capture demonstrating the m2 module additions
  (`bus.rs`, `supervisor.rs`, `validate/pattern.rs` extension)
  versus m1's surface.
- A `ls /proc/<pid>/fd/` snapshot of a running fixture
  showing exactly fds 0/1/2 (stdio inherited from cargo
  test) plus fd 3 (the bus socket). Demonstrates the
  socketpair + inherit_fd_as plumbing matches §SP3.
- `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` warning-free.
- `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core` green inside the
  devshell. (Per `plans/README.md` `nix develop` invocations
  always need `--impure`.)

## Risks

1. **Lockin's macOS implementation is untested by m2.** All
   spawn-bearing tests are Linux-only; macOS coverage waits
   for m6. Risk: a Linux-only assumption sneaks into the
   supervisor (e.g. `/proc/self/fd` introspection in tests
   leaks into non-test code). Mitigation: keep `/proc`
   peeking strictly inside `tests/` files; the supervisor
   itself uses only portable lockin/nix primitives.
2. **Fittings `PeerHandle` outside-handler usage in
   production code.** m0 added `Server::peer()` /
   `Client::peer()` for outside-handler use; m0's tests
   exercised it. m2 is the first non-test consumer. Risk:
   subtle correlator-id collisions or wakeup-loss bugs that
   m0's smaller tests didn't surface. Mitigation: the m2
   integration tests explicitly cover bidirectional
   `peer.call` (`supervisor_peer_call_*`) and the
   "publishing plugin excluded from own fan-out" topology
   exercises notification fan-out under registry mutation.
3. **Outpost proxy startup time vs. spawn synchronicity.**
   `outpost_proxy::start` is async; the fixture starts
   immediately after spawn. Risk: a fixture that issues a
   CONNECT before the proxy is listening hangs or races.
   Mitigation: m2 tests do not exercise real CONNECT calls
   (the proxy is a startup smoke test only); m4 proves
   real network plumbing under the bundled provider.
4. **Test isolation: parallel cargo tests sharing parent
   env vars.** `supervisor_env_pass_set_applied.rs` mutates
   the parent process env to test `env.pass`. Cargo runs
   tests in threads, not subprocesses. Risk: env-mutation
   test races a sibling. Mitigation: gate env-mutating
   tests on a `serial_test` mutex or run them in a
   dedicated `#[serial]` group; alternatively spawn a
   helper subprocess per test. `commits.md` picks the
   strategy.
5. **lockin requires syd on Linux.** The spawn path needs
   `syd` discoverable via `LOCKIN_SYD_PATH` or `PATH`. The
   devshell already provides it (lockin's tests rely on
   the same); m2 tests will fail outside the devshell.
   Mitigation: the manual-validation entry uses `nix
   develop --impure`; CI runs in the devshell. m2's
   `cargo test` invocation outside the devshell is not
   supported.
6. **`socketpair` ownership semantics across `inherit_fd_as`.**
   lockin's `inherit_fd_as` takes `OwnedFd`. After the
   builder is consumed and the child is spawned, the
   parent retains `core_fd` only. Risk: accidental
   double-close, or fd leaked across the spawn boundary,
   or the child inherits the parent half. Mitigation: the
   `supervisor_spawn_fixture.rs` test asserts `/proc/<pid>/fd`
   on the child shows exactly `0,1,2,3`; the supervisor
   side asserts `core_fd` is still readable post-spawn.
7. **CompiledPlugin mutation between compile and spawn.**
   Nothing prevents test code from constructing a
   `CompiledPlugin` literal with values that V3 would
   reject. SP6 catches the reserved-env case; SP3.1
   catches the not-in-ACL case; the rest is best-effort
   (the supervisor trusts that the plan came from
   `compile_plugin` after a successful `validate::lock`).
   Risk: production code that bypasses validate. Mitigation:
   the public API takes `&CompiledPlugin`; the only
   constructor is via `compile::compile_plugin`; the
   `pub` fields exist for read access only and the
   compiler doesn't expose a mutation API.

## Internal split (driver guidance for `commits.md`)

Suggested grouping; `commits.md` picks final granularity. Pi
review-2/-3 may reshape these; m1's reshuffles came from
phase-boundary discoveries (parse-vs-validate boundary moves)
that m2 can pre-empt by scoping the boundaries here.

1. **Workspace deps + crate skeleton** (W1, W2, W3): one
   commit for `[workspace.dependencies]` additions + the
   fixture binary scaffold (empty `main` returning 0). ~2
   commits.
2. **Pattern matcher** (B8): pure function with the §5.1
   example-matrix unit tests. Foundational dep for B7. ~1
   commit.
3. **Broker types + registration** (B1, B2): `Broker::new`,
   `register_plugin`, `RegisteredPlugin` RAII guard, the
   broker error enum. No publish path yet. Tests:
   `broker_register_plugin.rs`. ~2 commits.
4. **Broker publish path** (B3, B4, B5, B6, B7): `handle_plugin_publish`
   end-to-end with all the rejection rules + fan-out. Plus
   `publish_core`. Tests: every `broker_publish_*` and
   `broker_publishing_plugin_excluded_from_own_fanout.rs`
   plus `broker_publish_core.rs`. The lifecycle event
   `core.lifecycle.publish_rejected` lands here. ~3-5
   commits depending on how the matrix splits.
5. **Supervisor scaffolding + sequencing** (SP1, SP2, SP3
   steps 1, 6, 7, 8, 9): supervisor type, error enum,
   non-network spawn path (no proxy startup yet). Fixture
   binary grows to wire up RFL_BUS_FD reading + minimal
   fittings client/server. Tests:
   `supervisor_spawn_fixture.rs`,
   `supervisor_peer_call_core_to_plugin.rs`,
   `supervisor_peer_call_plugin_to_core.rs`,
   `supervisor_env_pass_set_applied.rs`,
   `supervisor_spawn_entry_not_executable.rs`,
   `supervisor_spawn_canonical_not_in_acl_refused.rs`,
   `supervisor_reserved_env_in_set_refused.rs`. ~4-6 commits.
6. **Supervisor lifecycle + drop** (SP5, SP6, the
   `supervisor_drop_during_spawn_unwinds.rs` and
   `supervisor_lifecycle_drop_kills_child.rs` tests): RAII
   teardown rules. Tests as named. ~2 commits.
7. **Supervisor + lockin denial proofs** (SP3 step 4
   filesystem mapping): integrate
   `supervisor_lockin_denies_outside_grant_read.rs` /
   `supervisor_lockin_denies_outside_grant_write.rs` /
   `supervisor_private_state_dir_writable.rs`. ~2 commits.
8. **Supervisor + outpost proxy startup** (SP3 step 3):
   `supervisor_proxy_starts_for_proxy_plan.rs`. ~1 commit.
9. **Two-plugin round-trip + manual validation** (the
   `supervisor_bus_publish_round_trip.rs` test +
   `manual-validation.md`). ~1 commit.

Realistic total: **~18–25 commits, sequential**. Smaller than
m1 (37 rows) by surface area but larger than m0's m0a-only
slice. The driver should NOT pre-emptively split m2 into
m2a/m2b unless something blocking surfaces during pi review;
the broker + supervisor are tightly coupled and a split would
make the demo bar split too.

## Acceptance summary

m2 is done when:

- Every named test in the *Positive integration tests* and
  *Negative integration tests* matrices above is implemented
  and passes. Tests may split or merge during `commits.md`
  drafting as long as the named behaviours are all covered.
- `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green on Linux.
- `cargo build -p rfl-bus-fixture` green (built by the
  workspace; cargo bin-test integration locates it).
- `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` warning-free.
- `manual-validation.md` records the items in the *Manual
  validation* section above.
- `retrospective.md` is written, with any drift surfaced
  during implementation landing in `overview.md` /
  `decisions.md` / stream RFCs as deltas (per
  `plans/README.md`'s authoring conventions). Anticipated
  drift items already known at scoping time:
  - The reserved-namespace list `provider.*` / `frontend.*`
    rejection in m2 (B3 #2) treats both as "not-yours" for
    every plugin; `decisions.md` doesn't yet pin the
    "provider role grants `provider.*` publish authority"
    rule, only that the bound provider plugin gets it. m4
    is the right milestone to pin this; m2's retrospective
    flags the open-question.
  - The `core.lifecycle.publish_rejected` event class is
    introduced by m2 but not enumerated in the security RFC.
    The retrospective should propose adding a single
    `core.lifecycle.*` schema entry to the v1 security
    surface (per `decisions.md` row 23, payload schemas
    are Stream A's responsibility).
  - `bus.event` as the outbound notification method name
    (vs. the inbound `bus.publish`) is m2's call; if pi
    or the m3 driver finds the asymmetry awkward, the
    retrospective records the decision-or-rename.
- No follow-up Stream RFC drift is owed by m2 beyond the
  items above; m2 does not modify Stream A or B RFC bodies
  (those would be m1-style retrospective patches and m1
  already landed its banner-based reconciliation).
