# m2-broker-spawn — commits

> **Status:** round-2 draft, addressing pi commits round-1 (7
> blocking + many per-commit findings + 5 structural
> reorderings). Pi commits round-2 review pending. Not yet
> owner-ratified. Phase 3 per-commit agent work begins on
> `rafaello-v0.1` after `commits.md` ratifies.

Ordered commit list for m2, derived from `scope.md` (round 11,
ratified 2026-05-09). Each commit is one logical idea **and
leaves the workspace green** — pre-commit hooks (rustfmt +
clippy + cargo test) gate every commit; intermediate non-green
states are not allowed. Commits land sequentially on per-commit
branches `agents/m2/c<NN>` rebased onto `rafaello-v0.1`, no
merge commits, no force pushes. Tests land with the code that
exercises them per `~/.claude/CLAUDE.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes:
  `rafaello-core` (the m1 crate m2 grows), `rfl-bus-fixture`
  (the test-only bin target inside `rafaello-core`),
  `rafaello` (workspace `Cargo.toml`), `rafaello-m2` (docs).
- "Acceptance" lists new tests + the pre-commit invariants the
  commit must keep green.
- "Depends on" cites the *lowest* commit numbers whose code or
  types this commit references. A commit only lands after every
  declared dependency has landed on `rafaello-v0.1`.
- Test files live under `rafaello/crates/rafaello-core/tests/`
  unless otherwise noted. Per-commit agents pre-flight
  `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture`
  until green before invoking pre-commit hooks.
- **Every per-commit agent prompt MUST include `--features
  test-fixture` in cited cargo invocations** (scope §"Risks" #1
  + §W3): without the feature, integration tests do not
  compile because `env!("CARGO_BIN_EXE_rfl-bus-fixture")`
  errors.
- Per the m1 lesson §4.2, the milestone driver inlines the
  full row text + every acceptance bullet verbatim into each
  per-commit prompt; agents do NOT re-read `commits.md`.
- **Driver-owned actions, NOT per-commit agent actions:**
  pushing branches to origin, capturing CI URLs, writing
  `retrospective.md` (Phase 4, not a per-commit task).

## m2a / m2b checkpoint

No internal split is planned. The driver re-evaluates after
**c14** (broker complete + types) and after **c21** (supervisor
end-to-end happy-path landed, real fixture spawn working);
if a split becomes obviously beneficial, the driver opens an
m2a / m2b owner-ratification request.

## Canonical test names

Wherever scope.md and commits.md both name a test, this
commits.md is canonical (pi-1 finding aligns with scope §"Demo
bar"). The headline test is **`supervisor_spawn_fixture_happy_path.rs`**
(matches scope.md's name; lands in c21).

---

## Group 0 — Foundation: workspace deps + crate wiring + fixture scaffold

### c01 — chore(rafaello): add m2 deps to `[workspace.dependencies]`

- **What.** Extend `rafaello/Cargo.toml`'s `[workspace.dependencies]`
  table (m1 c01 introduced) with the m2 runtime deps per scope
  §W1: `tokio` (features `rt-multi-thread`, `macros`,
  `io-util`, `net`, `sync`, `time`), `tracing`,
  `tracing-subscriber` (features `env-filter`), `async-trait`,
  `fittings-core` (path `../fittings/crates/core`),
  `fittings-server` (path `../fittings/crates/server`),
  `fittings-client` (path `../fittings/crates/client`),
  `fittings-transport` (path `../fittings/crates/transport`),
  `lockin` (path `../lockin/crates/sandbox`, features
  `["tokio"]`), `outpost-proxy` (path
  `../outpost/crates/outpost-proxy`),
  `nix` (version `0.29`, features `["socket", "fs", "signal",
  "process"]`), `parking_lot`, `anyhow`, `serial_test`,
  `tracing-test`. **No** `fittings-wire` (m2 only needs
  `JsonRpcId` / `FittingsError` from `fittings-core`'s
  re-exports — scope §W1).
- **Why.** scope §W1 (incl. coordinates verified per pi-1 §1
  + pi-3 §4: lockin's package name is `lockin`, fittings paths
  are `crates/{core,server,client,transport}`).
- **Depends on.** baseline.
- **Acceptance.** `nix develop --impure --command cargo metadata
  --manifest-path rafaello/Cargo.toml --format-version 1` succeeds
  and shows the new entries resolved; `nix develop --impure
  --command cargo build --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green (no rafaello-core consumer of the new
  deps yet — table-only addition); `nix develop --impure
  --command cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` warning-free.

### c02 — chore(rafaello-core): wire m2 deps + add `test-fixture` feature gate

- **What.** Update `rafaello/crates/rafaello-core/Cargo.toml`
  per scope §W2:
  - `[dependencies]` adds runtime W1 entries with `workspace =
    true`: `tokio`, `tracing`, `async-trait`, `fittings-core`,
    `fittings-server`, `fittings-client`, `fittings-transport`,
    `lockin`, `outpost-proxy`, `nix`, `parking_lot`, `anyhow`.
  - `[dev-dependencies]` adds `tempfile`, `serial_test`,
    `tracing-test`, `tracing-subscriber` with `workspace =
    true`.
  - `[features]` adds `test-fixture = []` (the gate for the
    fixture binary — see c03).
- **Why.** scope §W2.
- **Depends on.** c01.
- **Acceptance.** `nix develop --impure --command cargo build
  --manifest-path rafaello/Cargo.toml -p rafaello-core` green.
  `nix develop --impure --command cargo build --manifest-path
  rafaello/Cargo.toml -p rafaello-core --features test-fixture`
  green. `nix develop --impure --command cargo test
  --manifest-path rafaello/Cargo.toml -p rafaello-core` green
  (m1 tests still pass; no new tests yet).

### c03 — feat(rafaello-core): add `rfl-bus-fixture` `[[bin]]` scaffold

- **What.** Add `[[bin]]` to `rafaello/crates/rafaello-core/Cargo.toml`:
  `name = "rfl-bus-fixture"`, `path = "src/bin/rfl_bus_fixture.rs"`,
  `required-features = ["test-fixture"]`. Create the bin file
  with a minimal `main` that parses `RFL_FIXTURE_MODE` env (if
  unset or unknown — `eprintln!("rfl-bus-fixture: unknown mode
  '{}'", mode); std::process::exit(64);`), and otherwise exits 0
  (`scaffold_only` is the only valid mode in c03; real modes
  land in c20 + c22). This makes the unknown-mode failure path
  explicit and testable from c03 onward (pi-1 c21 finding —
  unknown mode behavior must be specified now).
- **Why.** scope §W3, §F1. Establishes the binary target so
  `env!("CARGO_BIN_EXE_rfl-bus-fixture")` resolves from
  rafaello-core's integration tests.
- **Depends on.** c02.
- **Acceptance.** `nix develop --impure --command cargo build
  --manifest-path rafaello/Cargo.toml -p rafaello-core
  --features test-fixture --bin rfl-bus-fixture` green.
  Without the feature, `cargo build -p rafaello-core` does NOT
  build the bin. Two new tests:
  - `tests/fixture_binary_resolves.rs` (gated `#[cfg(feature
    = "test-fixture")]`) — uses
    `env!("CARGO_BIN_EXE_rfl-bus-fixture")`,
    `Command::new(path).env("RFL_FIXTURE_MODE",
    "scaffold_only").status()`, asserts exit 0.
  - `tests/fixture_binary_unknown_mode_exits_64.rs` — same
    setup but `RFL_FIXTURE_MODE=bogus`; assert exit code 64.

---

## Group 1 — Reserved env list extension (small back-reach to m1)

### c04 — feat(rafaello-core): extend reserved env list with m2 names

- **What.** Update `rafaello/crates/rafaello-core/src/scrubber.rs`
  `RESERVED_ENV_VARS` to include `RFL_HELPER_FD`,
  `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`, `RFL_PRIVATE_STATE_DIR`
  alongside the existing `RFL_BUS_FD` / `RFL_PLUGIN`. m1's
  scrubber already rejects reserved names in `env.set` /
  `env.pass` at compile time; m2 grows the reserved set so
  hand-mutated plans cannot collide with m2-reserved injections
  (defence in depth — scope §SP6 + pi-1 §23, §270).
- **Why.** scope §F5 + §SP4.4.
- **Depends on.** c01.
- **Acceptance.** New `tests/env_scrubber_reserved_m2_names.rs`
  table-driven over all four new names, asserting each name in
  `env.set` triggers the existing `EnvKeyReserved` (or m1's
  variant — verify against m1 source) when present. The four
  rows are NOT optional driver choices (pi-1 c04 finding —
  per-commit prompts must be deterministic). Existing 269 m1
  tests still pass.

---

## Group 2 — Bus error surface + wire types

### c05 — feat(rafaello-core): add `BrokerError` + `SpawnError` + companion enums

- **What.** New `BrokerError`, `SpawnError`, `InvalidPlanReason`,
  `Publisher`, `InReplyToReason`, `PathKind`, `ReaperOutcome`,
  `ShutdownFailure` enums in `rafaello/crates/rafaello-core/src/error.rs`
  per scope §B2 + §SP3. All `thiserror`-derived,
  `#[non_exhaustive]`. `BrokerError` is `Debug` only (NOT
  `Clone`, NOT `PartialEq`); `SpawnError` source-error variants
  store the real types (`anyhow::Error`, `std::io::Error`,
  `nix::errno::Errno`, `FittingsError`,
  `outpost::DomainPatternParseError`). Top-level
  `rafaello_core::Error` enum gets `#[from]` arms for
  `BrokerError` and `SpawnError`. Re-exports added to `lib.rs`.
- **Why.** scope §B2, §SP3, §E.
- **Depends on.** c02.
- **Acceptance.** `tests/m2_error_surface_compiles.rs` — build-only
  assertion that each new variant is reachable through
  `rafaello_core::Error`. To avoid brittle source-construction,
  use `Default::default()` + `Box::leak` patterns where source
  errors aren't easily constructible: e.g. `let _: SpawnError =
  SpawnError::SandboxBuild { canonical: ..., source:
  anyhow::anyhow!("test") };`. The c05 agent picks workable
  source constructors per variant and inlines them in the
  commit body.

### c06 — feat(rafaello-core::bus): add wire types — `PublishMsg`, `BusEvent`, `TaintEntry`, `PublisherIdentity`

- **What.** New module `rafaello_core::bus` with public types
  per scope §B1 / §B4 / §B8:
  - `pub struct PublishMsg { topic, payload, in_reply_to,
    taint }` — **`#[derive(Debug, Clone, Deserialize)]` only**
    (decode-side wire type per scope §B4) +
    `#[serde(deny_unknown_fields)]`. Optional fields use
    `#[serde(default)]`.
  - `pub struct TaintEntry { source, detail }` —
    **`#[derive(Debug, Clone, Serialize, Deserialize, PartialEq,
    Eq)]`** (round-trips through both PublishMsg and BusEvent
    so it's the one type that carries both halves) +
    `#[serde(deny_unknown_fields)]`.
  - `pub struct BusEvent { topic, payload, publisher,
    in_reply_to, taint }` — **`#[derive(Debug, Clone,
    Serialize)]` only** (encode-side wire type per scope §B8) +
    `#[serde(skip_serializing_if = "Option::is_none")]` on the
    optional fields.
  - `pub enum PublisherIdentity { Core, Plugin { canonical:
    String, topic_id: String } }` — **`#[derive(Debug, Clone,
    Serialize)]`** + `#[serde(tag = "kind", rename_all =
    "snake_case")]`.
  - `JsonRpcId` from `fittings_core::message::JsonRpcId`
    re-exported from `bus`.
  - `bus` re-exports from `lib.rs`. No `Broker` type yet —
    that's c07.
- **Why.** scope §B1, §B4, §B8. Wire types are intentionally
  one-way per scope: `PublishMsg` is what plugins send IN
  (decode-only), `BusEvent` is what core sends OUT
  (encode-only). Pi-1 B1 finding — round-trip tests would
  contradict the one-way derives.
- **Depends on.** c05.
- **Acceptance.** `tests/bus_wire_types_schema.rs` (NOT
  `bus_types_round_trip.rs` — pi-1 B1):
  - **PublishMsg decode tests:** decode
    `serde_json::json!({"topic": "plugin.id_x.foo", "payload":
    {"k":1}, "in_reply_to": ["a"], "taint":
    [{"source":"web","detail":"x"}]})` to `PublishMsg`; assert
    each field present + correct values. Test `payload: null`
    decodes successfully. Test `deny_unknown_fields` fires by
    decoding `{"topic":"a.b","payload":null,"unknown":1}` and
    asserting the `serde_json::Error` mentions `unknown`. Same
    for unknown field inside a `TaintEntry`.
  - **BusEvent encode tests:** construct one `BusEvent` per
    `PublisherIdentity` variant; serialise via `serde_json::to_value`;
    assert the resulting `Value` matches expected JSON shape
    (tag-based discriminator visible, optional fields omitted
    when `None`). For deserialise-side coverage of `BusEvent`,
    define a test-only permissive struct
    `BusEventReceived { topic, payload, publisher: Value, ... }`
    with `Deserialize` and decode into that.
  - `nix develop --impure --command cargo test --manifest-path
    rafaello/Cargo.toml -p rafaello-core --features test-fixture`
    green.

---

## Group 3 — Broker registration + lifecycle

### c07 — feat(rafaello-core::bus): `Broker::new` + ACL revalidation + `register_plugin` + `RegisteredPlugin` RAII guard

- **What.** Implement `Broker` per scope §B1 + §B10:
  - `pub struct Broker(Arc<BrokerInner>)` cheap-clone handle.
    Internal state `parking_lot::Mutex<BrokerState>` with
    `BTreeMap<CanonicalId, PluginConn>` registry.
  - `Broker::new(acl: BrokerAcl) -> Result<Self, BrokerError>`
    runs §B10 defence-in-depth pattern/topic revalidation.
  - `Broker::register_plugin(canonical, peer) ->
    Result<RegisteredPlugin, BrokerError>` with `NotInAcl` /
    `AlreadyRegistered`. `RegisteredPlugin` is `!Clone`,
    `Send + Sync` RAII single-owner guard.
  - `Broker::try_reserve_registration(&self, canonical) ->
    Result<(), BrokerError>` cheap precheck.
  - `Broker::contains_plugin(&self, canonical) -> bool` — ACL
    membership only.
  - `Broker::plugin_acl(&self, canonical) -> Option<PluginAcl>`.
  - `Broker::shutdown(&self)` — drains all registrations
    idempotently.
  - **Concrete test `PeerHandle` strategy** (pi-1 c07
    finding — must not be agent-discretion). Add
    `tests/common/peer_test_kit.rs`: a thin helper that
    constructs `PeerHandle` via `fittings_core::context::PeerHandle::new(
    notify_tx, dropped_counter, cancellation_token)` (verify
    the exact constructor by reading
    `fittings/crates/core/src/context.rs`); returns a
    `(PeerHandle, mpsc::Receiver<OutboundNotification>)` pair
    so tests can observe what would have been notified. The
    receiver type matches whatever fittings exposes for the
    notification channel; the c07 agent pins the exact type
    in the helper.
  - No publish/fan-out path yet — that's c09–c12.
- **Why.** scope §B1, §B10.
- **Depends on.** c06.
- **Acceptance.** Five new tests using `peer_test_kit`:
  - `tests/broker_register_unregister.rs`,
    `tests/broker_register_canonical_not_in_acl_rejected.rs`,
    `tests/broker_register_duplicate_rejected.rs`,
    `tests/broker_invalid_acl_rejected_at_construction.rs` —
    behaviours per scope §B1 / §B10.
  - **`tests/bus_pattern_matches.rs`** (pi-1 B4 — was missing).
    Pure unit test (`#[test]`, no tokio) re-exporting m1's
    `validate::topic::pattern_matches_topic` cases plus the
    zero-trailing negatives (`core.session.**` does NOT match
    `core.session`; `plugin.id_x.**` does NOT match
    `plugin.id_x`) per scope §B7 + pi-1 §91. This test belongs
    to the bus group because m2 is the first consumer of the
    matcher, but it tests m1 code — fine, it's a behaviour
    spec.

### c08 — feat(rafaello-core::bus): `publish_boot` happy path

- **What.** Implement `Broker::publish_boot(&self) ->
  Result<(), BrokerError>` per scope §B1: emits
  `core.lifecycle.boot` with payload `{"version":
  env!("CARGO_PKG_VERSION"), "plugin_count":
  acl.plugins.len()}` via a thin
  `fan_out_one_event(BusEvent)` private helper that iterates
  registered plugins under snapshot lock and calls
  `peer.notify("bus.event", value)`. NO ACL checks on the
  helper since core is the publisher.
- **Why.** scope §B1, §B7 (initial fan-out shape), §B8.
- **Depends on.** c07.
- **Acceptance.** `tests/broker_publish_boot_event.rs` —
  register an observer via `peer_test_kit`; call
  `broker.publish_boot()`; observer's receiver yields one
  `bus.event` notification with the expected schema +
  `publisher == PublisherIdentity::Core`.

---

## Group 4 — Broker publish path: ACL + grammar + fan-out

### c09 — feat(rafaello-core::bus): `handle_plugin_publish` — params decode + grammar revalidation + live-registration check

- **What.** Implement
  `Broker::handle_plugin_publish(&self, canonical, raw_params)
  -> Result<(), BrokerError>` per scope §B3 steps 1–2 + §B5:
  - Live-registration check: if canonical not currently
    registered → `NotRegistered`.
  - Decode `raw_params` to `PublishMsg`; on serde failure →
    `InvalidPayload { publisher: Plugin(canonical), reason:
    e.to_string() }`.
  - `validate::topic::validate_topic(&msg.topic)`; on failure
    → `InvalidTopic { publisher: Plugin(canonical), topic,
    reason: ve.to_string() }`.
  - No namespace parsing yet (c10), no ACL checks (c11), no
    fan-out (c12). Function returns `Ok(())` after grammar
    passes.
- **Why.** scope §B3 (1–2), §B5, §B1 `NotRegistered`.
- **Depends on.** c08.
- **Acceptance.** Three new tests:
  - `tests/broker_publish_extra_field_rejected.rs` —
    `bus.publish` params with extra unknown key →
    `InvalidPayload` (`reason` contains `unknown`).
  - `tests/broker_publish_invalid_topic_grammar_rejected.rs`
    — three `#[test]` cases: `"plugin.<id>.UPPERCASE"`,
    `"plugin.<id>.has spaces"`, `""` → `InvalidTopic` each
    with a meaningful `reason` string.
  - `tests/broker_handle_publish_after_unregister_returns_not_registered.rs`
    — register A via `peer_test_kit`; drop guard; call
    `handle_plugin_publish(A, valid_params)` →
    `NotRegistered(A)`.

### c10 — feat(rafaello-core::bus): structural namespace parsing — `UnknownNamespace` + `PublishOnReservedNamespace`

- **What.** Add the structural namespace step per scope §B3
  step 3 (split topic by `.`, look at `segments[0]`):
  - Not in `{core, provider, plugin, frontend}` →
    `UnknownNamespace`.
  - `core` / `provider` / `frontend` →
    `PublishOnReservedNamespace`.
  - `plugin`:
    - `segments.len() < 3` (e.g. `plugin.<id>` two-segment) →
      `PublishOnReservedNamespace`.
    - `segments[1] != publisher_acl.topic_id` →
      `PublishOnReservedNamespace` (cross-plugin masquerade).
  Does NOT yet enforce `PublishOutsideGrant` for own-namespace
  topics (c11).
- **Why.** scope §B3 step 3, pi-5 §3.
- **Depends on.** c09.
- **Acceptance.** Six new tests, one per rejection class:
  - `tests/broker_publish_unknown_namespace_rejected.rs` —
    plugin publishes `"evil.foo"`, sub-case `"random.thing.bar"`
    → `UnknownNamespace`.
  - `tests/broker_publish_short_plugin_topic_rejected.rs` —
    `"plugin.<own-id>"` and `"plugin.<other-id>"` (two-segment)
    → `PublishOnReservedNamespace`.
  - `tests/broker_publish_core_namespace_rejected.rs`,
    `tests/broker_publish_provider_namespace_rejected.rs`,
    `tests/broker_publish_frontend_namespace_rejected.rs`,
    `tests/broker_publish_other_plugin_namespace_rejected.rs`
    — each rejects with `PublishOnReservedNamespace`.

### c11 — feat(rafaello-core::bus): outside-grant rejection + `in_reply_to` arity enforcement

- **What.** Add per scope §B3 (own-namespace grant) + §B6
  (in_reply_to):
  - `plugin.<own-topic-id>.*` topic not in
    `publisher_acl.publish_topics` exact-string set →
    `PublishOutsideGrant`. (`auto_subscribes` is NOT publish
    authority.)
  - For topics ending in `.tool_result` or `.rpc_reply`:
    require `in_reply_to: Some(vec![one_id])`. Missing →
    `InvalidInReplyTo { reason: Missing }`. Empty array →
    `InvalidInReplyTo { reason: EmptyArray }`. Two or more →
    `InvalidInReplyTo { reason: UnexpectedMultiple }`.
- **Why.** scope §B3, §B6, pi-1 §43.
- **Depends on.** c10.
- **Acceptance.** Five new tests as in round 1 (unchanged):
  `broker_publish_outside_grant_rejected.rs`,
  `broker_tool_result_in_reply_to_{missing,empty,multiple}_rejected.rs`,
  `broker_rpc_reply_in_reply_to_missing_rejected.rs`.

### c12 — feat(rafaello-core::bus): fan-out + publisher exclusion + result-routing protection + unsubscribed drop

- **What.** Wire up the §B7 fan-out path inside
  `handle_plugin_publish`: after authorisation + class checks,
  construct `BusEvent` (with `publisher: PublisherIdentity::Plugin
  { canonical: canonical.to_string(), topic_id:
  publisher_acl.topic_id.clone() }`), serialise via
  `serde_json::to_value` ONCE, snapshot the registry under
  the broker mutex (clone the per-plugin `(canonical, peer,
  subscribe_patterns ∪ auto_subscribes)` list), release the
  mutex, then iterate: for each registered plugin **other
  than the publisher**, check the topic against the patterns
  via `validate::topic::pattern_matches_topic`; if matched
  call `peer.notify("bus.event", value.clone())`. Fan-out
  errors logged via `tracing::warn!` but do NOT fail the
  publish call (best-effort delivery).
  
  **Result-routing protection** (scope §B7): topics ending in
  `.tool_result` or `.rpc_reply` skip the per-subscriber
  delivery loop entirely; a `tracing::debug!` records the
  suppression.
- **Why.** scope §B7, §B8, pi-1 §14, pi-1 §247.
- **Depends on.** c11.
- **Acceptance.** Four new tests (pi-1 B4 — added
  unsubscribed_drop):
  - `tests/broker_publish_round_trip.rs` — A publishes
    `"plugin.<A>.greet"`; observer B subscribed to
    `"plugin.**"` receives one `bus.event` matching topic +
    payload + publisher.
  - `tests/broker_publishing_plugin_excluded_from_own_fanout.rs`
    — A subscribes to `"plugin.<A>.**"` AND publishes;
    observer B receives event; A does NOT.
  - `tests/broker_unsubscribed_plugin_does_not_receive.rs`
    (pi-1 B4) — A publishes `"plugin.<A>.foo"`; B subscribed
    only to `"core.**"` does NOT receive it.
  - `tests/broker_tool_result_not_fanned_out_to_other_plugins.rs`
    — A publishes `"plugin.<A>.tool_result"` with valid
    `in_reply_to`; B subscribed to `"plugin.**"` does NOT
    receive it. Asserts the `tracing::debug!` message via
    `#[tracing_test::traced_test]`.

### c13 — feat(rafaello-core::bus): `publish_core` + `core.lifecycle.publish_rejected`

- **What.** Two pieces:
  - `Broker::publish_core(&self, topic, payload) ->
    Result<(), BrokerError>` per scope §B1: revalidate
    grammar FIRST (matching the §B5 "grammar before namespace"
    rule); structurally parse the topic; reject
    `UnknownNamespace` / `PublishOnReservedNamespace` (using
    the `Publisher::Core` variant); build `BusEvent {
    publisher: PublisherIdentity::Core, ... }`; fan out via
    the same helper from c12 (no publisher-exclusion since
    core is not registered).
  - **`core.lifecycle.publish_rejected` event emission** per
    scope §B9: every rejection path in `handle_plugin_publish`
    (UnknownNamespace, PublishOnReservedNamespace,
    PublishOutsideGrant, InvalidTopic, InvalidInReplyTo,
    InvalidPayload) emits one `core.lifecycle.publish_rejected`
    event via an internal `publish_core_internal` path that
    bypasses the structural namespace re-check (the broker has
    already constructed the topic correctly) but still runs
    grammar + fan-out. Schema per §B9. `topic = null` only
    for `InvalidPayload` decode failures that happened before
    topic extraction; the broker tries permissive `Value`-level
    extraction first. Reject-event construction does NOT call
    rejection emission (no recursion).
- **Why.** scope §B1, §B9, pi-1 §15, pi-1 c13 finding (grammar
  ordering).
- **Depends on.** c12.
- **Acceptance.** Three new tests:
  - `tests/broker_publish_core_happy_path.rs` —
    `publish_core("core.lifecycle.test", payload)` with
    observer subscribed to `"core.**"` receives one
    `bus.event` with `publisher == Core`.
  - `tests/broker_publish_core_invalid_topic_rejected.rs` —
    `publish_core("plugin.x.y", ...)` →
    `PublishOnReservedNamespace { publisher: Core }`.
    `publish_core("core.Bad", ...)` → `InvalidTopic`.
    `publish_core("evil.foo", ...)` → `UnknownNamespace`.
  - `tests/broker_publish_rejected_event_fired_on_each_rejection_class.rs`
    — table-driven over all six rejection classes
    (`UnknownNamespace`, `PublishOnReservedNamespace`,
    `PublishOutsideGrant`, `InvalidTopic`, `InvalidInReplyTo`,
    `InvalidPayload`); for each, assert a
    `core.lifecycle.publish_rejected` event fires with the
    expected `code` field. Observer subscribed to
    `"core.lifecycle.**"`.

---

## Group 5 — Supervisor scaffolding (resource ownership defined upfront)

### c14 — feat(rafaello-core::supervisor): scaffolding with explicit managed-vs-observation split

- **What.** New module `rafaello_core::supervisor` per scope
  §SP1 + §SP2, with the resource-ownership shape **defined
  here so later commits do not redesign it** (pi-1 B2):
  ```rust
  pub struct PluginSupervisor {
      broker: Broker,
      config: SupervisorConfig,
      in_flight: Arc<parking_lot::Mutex<HashSet<CanonicalId>>>,
      // Supervisor-owned managed state, NOT shared with
      // external SpawnHandle clones.
      managed: parking_lot::Mutex<BTreeMap<CanonicalId, ManagedSpawn>>,
  }
  
  // Internal struct — supervisor owns these resources; on
  // shutdown/drop they take() out of the Mutex<Option<_>>.
  struct ManagedSpawn {
      observation: Arc<SpawnObservation>,
      registered: Option<RegisteredPlugin>,
      proxy: Option<ProxyHandle>,
      serve_join: Option<tokio::task::JoinHandle<()>>,
      reaper_join: Option<tokio::task::JoinHandle<()>>,
      watcher_join: Option<tokio::task::JoinHandle<()>>,
  }
  
  // Handle-observable state — Arc-shared with external clones.
  // External clones can observe wait outcome via the watch
  // channel even after supervisor takes managed resources.
  struct SpawnObservation {
      canonical: CanonicalId,
      topic_id: String,
      cached_pid: Option<u32>,    // cached at spawn time, before reaper takes child
      peer: PeerHandle,            // clone of the peer
      outcome: tokio::sync::watch::Receiver<Option<Arc<ReaperOutcome>>>,
  }
  
  pub struct SpawnHandle(Arc<SpawnObservation>);
  
  pub struct SupervisorConfig {
      pub shutdown_grace: Duration,        // default 200ms
      pub fittings_max_frame_bytes: usize, // default 1 << 20
  }
  
  pub struct SpawnPaths {
      pub project_root: PathBuf,
      pub private_state_dir: PathBuf,
  }
  
  impl SpawnHandle {
      pub fn canonical(&self) -> &CanonicalId;
      pub fn topic_id(&self) -> &str;
      pub fn child_pid(&self) -> Option<u32>;  // returns cached_pid; None after wait observes
      pub fn peer(&self) -> &PeerHandle;
      pub async fn wait(&self) -> Arc<ReaperOutcome>;
      pub fn try_wait(&self) -> Option<Arc<ReaperOutcome>>;
  }
  ```
  - `PluginSupervisor::new(broker, config) -> Self` synchronous,
    no boot auto-emit.
  - `pub async fn spawn(&self, plan, paths) -> Result<SpawnHandle,
    SpawnError>` — c14 stub returns
    `SpawnError::SandboxBuild { canonical, source: anyhow::anyhow!("supervisor
    not yet implemented — c15+") }`. **Note for the c14 agent:**
    do NOT add a public method that panics. The c15 commit
    DELETES this stub when it implements Phase A.
  - `pub async fn shutdown(self) -> ShutdownReport` — empty
    report; real teardown is c25.
  - `Drop for PluginSupervisor` — no-op for c14 (real Drop
    logic is c26).
  - `#[cfg(any(test, feature = "test-fixture"))]
    PluginSupervisor::with_extra_service` constructor — same
    as `new` for c14; service plumbing lands in c19.
  - `#[cfg(any(test, feature = "test-fixture"))]` `TestHooks`
    struct: per-supervisor `AtomicUsize` counters
    `outpost_starts`, `socketpair_creates`, `child_spawns`. All
    wired (return real values), but stay 0 until later commits
    increment them.
  - `pub const RFL_BUS_FD_NUMBER: i32 = 3;` exported.
- **Why.** scope §SP1, §SP2, §SP7, pi-1 B2 (split shape so
  external `SpawnHandle` clones cannot keep
  `RegisteredPlugin`/`ProxyHandle` alive after supervisor
  shutdown).
- **Depends on.** c13.
- **Acceptance.** `tests/supervisor_types_compile.rs` —
  build-only assertion that every public type in §SP1 is
  reachable. `tests/supervisor_spawn_unimplemented_returns_sandbox_build.rs`
  — `spawn(plan, paths).await` returns
  `Err(SpawnError::SandboxBuild { .. })` (deleted in c15).
  `TestHooks` counters all return 0.

---

## Group 6 — Supervisor Phase A (cheap validation, no resources)

### c15 — feat(rafaello-core::supervisor): Phase A — `in_flight` reservation + topic-id consistency + path validation

- **What.** Implement scope §SP4 Phase A steps 1a, 1b, 2, 3
  inside `PluginSupervisor::spawn`. Replace the c14 stub body
  (delete `tests/supervisor_spawn_unimplemented_returns_sandbox_build.rs`).
  Remaining steps 4–7 still return `SandboxBuild { source:
  "Phase A step <N> not yet implemented" }`.
  - **Step 1a** — RAII `InFlightGuard` acquires
    `in_flight.lock().insert(plan.canonical.clone())`; if
    already present → `SpawnError::AlreadyRegistered`.
  - **Step 1b** — `broker.try_reserve_registration(&plan.canonical)`
    maps to `NotInAcl` / `AlreadyRegistered`.
  - **Step 2** — `broker.plugin_acl(&plan.canonical)` →
    compare ACL `topic_id` against `plan.topic_id`; mismatch
    → `InvalidPlan { reason: TopicIdMismatch }`. `None`
    defensively maps to `NotInAcl`.
  - **Step 3** — for each path field:
    - Assert `path.is_absolute()`; non-absolute →
      `InvalidPlan { reason: NonAbsolutePath { kind, path } }`.
    - Assert no ASCII control chars; failure →
      `InvalidPlan { reason: ControlCharsInPath { kind,
      path } }`.
  
  **Pi-1 B5: real duplicate-spawn test deferred.** c15 does
  NOT add a duplicate-canonical test; the synthetic in-flight
  precondition was contradictory. The real
  `supervisor_spawn_duplicate_canonical_refused.rs` test lands
  in c24 (after harness + real spawn work).
- **Why.** scope §SP4 Phase A 1a–3, pi-1 B5 (defer duplicate
  test).
- **Depends on.** c14.
- **Acceptance.** Five new tests + one deletion:
  - DELETE
    `tests/supervisor_spawn_unimplemented_returns_sandbox_build.rs`.
  - `tests/supervisor_spawn_canonical_not_in_acl_refused.rs`
    — `SpawnError::NotInAcl`. `TestHooks` counters all == 0.
  - `tests/supervisor_spawn_topic_id_mismatch_refused.rs` —
    hand-mutate `topic_id` → `TopicIdMismatch`. Counters == 0.
  - `tests/supervisor_spawn_relative_path_refused.rs` —
    relative `entry_absolute` → `NonAbsolutePath { kind:
    EntryAbsolute }`. Counters == 0.
  - `tests/supervisor_spawn_relative_spawn_path_refused.rs`
    — `SpawnPaths { project_root: relative }` →
    `NonAbsolutePath { kind: ProjectRoot }`. Sub-case for
    `private_state_dir`. Counters == 0.
  - `tests/supervisor_spawn_control_chars_in_path_refused.rs`
    — newline in `entry_absolute` → `ControlCharsInPath`.
    Counters == 0.
  
  Each test uses a synthetic `CompiledPlugin` constructor
  (the c15 agent inlines the helper inside the test file —
  the real harness lands in c23). The synthetic plan only
  needs to satisfy Phase A; no real binary needed yet.

### c16 — feat(rafaello-core::supervisor): Phase A — reserved env + outpost dry-run + entry-executable + provider refusal

- **What.** Implement scope §SP4 Phase A steps 4, 5, 6, 7:
  - **Step 4** — reserved env check (`RFL_BUS_FD`, `RFL_PLUGIN`,
    `RFL_HELPER_FD`, `RFL_PROJECT_ROOT`,
    `RFL_PRIVATE_STATE_DIR`, `RFL_TOPIC_ID`) →
    `ReservedEnvInPlan { var }`.
  - **Step 5** — if `NetworkPlan::Proxy`, dry-run
    `outpost::NetworkPolicy::from_allowed_hosts`; failure →
    `InvalidPlan { reason: NetworkAllowHostsInvalid {
    source } }`.
  - **Step 6** — entry-executable check.
  - **Step 7** — provider refusal via
    `broker.plugin_acl(...).provider_id.is_some()`.
- **Why.** scope §SP4 4–7, pi-1 B4 (add invalid_allow_hosts
  test).
- **Depends on.** c15.
- **Acceptance.** **Six** new tests (pi-1 c16 finding —
  was incorrectly listed as "four"; was missing
  invalid_allow_hosts test):
  - `tests/supervisor_spawn_reserved_env_in_set_refused.rs`
    — table-driven over all six reserved names (pi-1 c16 —
    "table-test all reserved env names, not just three"):
    each name in `env.set` →
    `ReservedEnvInPlan { var: <name> }`.
  - `tests/supervisor_spawn_reserved_env_in_pass_refused.rs`
    — same table for `env.pass`.
  - `tests/supervisor_spawn_invalid_proxy_allow_hosts_refused.rs`
    (pi-1 B4 — was missing) — `NetworkPlan::Proxy {
    allow_hosts: vec!["not a valid host pattern!"] }` →
    `InvalidPlan { reason: NetworkAllowHostsInvalid {
    source: ... } }`. Counters == 0.
  - `tests/supervisor_spawn_entry_not_executable_refused.rs`
    — file with no exec bit → `EntryNotExecutable`.
    Counters == 0.
  - `tests/supervisor_spawn_provider_lock_refused.rs` —
    `bindings.provider = true` → `InvalidPlan { reason:
    ProviderNotInM2 { provider_id: "openai" } }`.
    Counters == 0.

---

## Group 7 — Supervisor Phase B (resource allocation, async spawn)

### c17 — feat(rafaello-core::supervisor): Phase B steps 8–12 — socketpair + proxy + lockin builder + tokio_command + private state

- **What.** Implement scope §SP4 Phase B steps 8–12:
  - **Step 8** — `nix::sys::socketpair(AddressFamily::Unix,
    SockType::Stream, None, SockFlag::SOCK_CLOEXEC)`;
    `SpawnError::Socketpair` on failure.
    `TestHooks::socketpair_creates` increments here.
  - **Step 9** — if `NetworkPlan::Proxy`,
    `outpost_proxy::start(policy).await`. On failure
    `SpawnError::ProxyStart`. Capture `proxy.listen_addr().port()`.
    `TestHooks::outpost_starts` increments.
  - **Step 10** — build `lockin::Sandbox::builder()`, apply
    filesystem + network + limits + `inherit_fd_as`
    (consumes `child_fd`).
  - **Step 11** — `paths.private_state_dir`
    `fs::create_dir_all(...)?` → on failure
    `SpawnError::PrivateStateDirCreate { path, source }`.
  - **Step 12** — `builder.tokio_command(&plan.entry_absolute)?`
    → on failure `SandboxBuild { source }`. Returns
    `lockin::tokio::SandboxedCommand`.
  - **Step 13 stub** — c17 still cannot spawn; after building
    the command, drop it + drop `ProxyHandle` + drop
    `core_fd` (parent half) and return
    `SandboxBuild { source: anyhow::anyhow!("Phase B step 13+
    not yet implemented") }`.
- **Why.** scope §SP4 8–12, §SP7, pi-1 §502, pi-4 §3.
- **Depends on.** c16.
- **Acceptance.** Three new integration tests using the c03
  fixture binary path (`env!("CARGO_BIN_EXE_rfl-bus-fixture")`)
  as a real executable entry — but spawn doesn't actually
  invoke it yet; the c17 agent inlines a synthetic-plan
  helper extension that points entry at the fixture binary
  while keeping Phase A's ACL machinery intact:
  - `tests/supervisor_spawn_unwinds_after_socketpair.rs` —
    `socketpair_creates == 1` after spawn returns the "step
    13+" error; parent's open-fd count returns to baseline
    (sample `/proc/self/fd` entries — Linux-only via
    `#[cfg(target_os = "linux")]` per scope §"Platform
    gating").
  - `tests/supervisor_spawn_starts_proxy_for_proxy_plan.rs`
    — `NetworkPlan::Proxy { allow_hosts: ["example.com"] }`,
    spawn → `outpost_starts == 1`. Proxy port no longer bound
    after spawn returns (drop happened in unwind).
  - `tests/supervisor_spawn_skips_proxy_for_deny_plan.rs` —
    `NetworkPlan::Deny`, spawn → `outpost_starts == 0`.

### c18 — feat(rafaello-core::supervisor): Phase B step 12 env application — env_clear + pass + set + proxy_env + reserved RFL_*

- **What.** After `tokio_command`, apply env per scope §SP4
  step 12 (a)–(f): `env_clear`, then iterate `plan.env.pass`
  + `plan.env.set`, then proxy env if applicable, then
  reserved `RFL_*` last (override semantics), then
  `current_dir(&paths.project_root)`.
  
  Document the no-tmpdir-restoration deviation per scope
  §SP4. **No env tests in c18** (env behavior is
  unobservable without a fixture's dump_env service —
  c20+); behavioural verification is c27. c18 lands ONLY
  the env-application code path + a build-only compile
  check.
- **Why.** scope §SP4 12, pi-1 c18 finding (env tests
  scheduled in dedicated c27, not vague forward-references).
- **Depends on.** c17.
- **Acceptance.** No new behavioural tests. `tests/supervisor_env_application_compiles.rs`
  asserts the env-application function builds (visible
  through a private `#[cfg(test)]` re-export the c18 agent
  adds, OR compiles indirectly via the unwind tests). The
  c17 tests still pass.

### c19 — feat(rafaello-core::supervisor): Phase B steps 13–17 — spawn + transport + Server + bus.publish service + broker register

- **What.** Implement scope §SP4 Phase B steps 13–17:
  - **Step 13** — `cmd.spawn()` → `lockin::tokio::SandboxedChild`;
    on failure `SpawnError::Spawn`. `TestHooks::child_spawns`
    increments. After this point every error must SIGKILL +
    `child.wait().await` to reap (per the post-spawn contract).
    **Cache pid before moving child**: `let cached_pid =
    child.id();` (from `tokio::process::Child::id()` returning
    `Option<u32>`); store in `SpawnObservation.cached_pid`
    when the handle is constructed.
  - **Step 14** — convert `core_fd` to
    `tokio::net::UnixStream::from_std(...)` with
    `set_nonblocking`. On failure
    `SpawnError::TransportSetup { source }`; unwind: SIGKILL
    + reap.
  - **Step 15** — split into `(reader, writer)`; build
    `fittings_transport::stdio::StdioTransport::new(reader,
    writer, config.fittings_max_frame_bytes)`.
  - **Step 16** — build the per-connection `Service` impl
    using **real fittings types** (pi-1 c19): the impl
    lives in a `BusPublishService { broker: Broker,
    canonical: CanonicalId }` struct implementing
    `fittings_core::service::Service` (the actual trait
    name — verify against
    `fittings/crates/core/src/service.rs`). The trait method
    receives `(req: fittings_core::message::Request, ctx:
    fittings_core::context::ServiceContext)` and returns
    `Result<fittings_core::message::Response, FittingsError>`.
    For the `bus.publish` notification path
    (`req.id == None`), call `broker.handle_plugin_publish(
    &canonical, &req.params)` — any `BrokerError` is already
    handled by §B9 lifecycle emission, the service returns
    `Ok(Response { jsonrpc: ..., id: JsonRpcId::Null, result:
    Value::Null, error: None })` regardless (notifications
    have no response, so the `Response` is swallowed by the
    server — pi-1 c19 finding). For unknown methods, return
    `Err(FittingsError::method_not_found("..."))`. Test mode
    (`with_extra_service`): compose with the factory's service
    via a small router (`bus.publish` → broker; everything
    else → factory or `MethodNotFound`).
  - **Step 17** — `Server::new(service, transport)`; capture
    `peer = server.peer()`; call `broker.register_plugin(
    plan.canonical.clone(), peer.clone())` mapping
    `BrokerError::NotInAcl(c)` → `SpawnError::NotInAcl(c)` and
    `BrokerError::AlreadyRegistered(c)` →
    `SpawnError::AlreadyRegistered(c)` (other variants
    unreachable per scope §B1). On success: drop the
    `InFlightGuard` from c15 step 1a immediately (broker
    registration is now source of truth).
  - Steps 18–20 still stubbed; the function returns the
    "Phase B step 18+ not yet implemented" error AFTER
    SIGKILL + reap of the child + drop of `RegisteredPlugin`
    + drop of `ProxyHandle` + drop of `core_fd`.
- **Why.** scope §SP4 13–17, §SP2, pi-1 c19 (real fittings
  types + Response { id: Null }).
- **Depends on.** c18.
- **Acceptance.** Two cleanup tests:
  - `tests/supervisor_spawn_unwinds_after_register.rs` —
    spawn returns the "step 18+" error; `TestHooks` counters
    increment by exactly 1 each (socketpair, proxy if
    applicable, child spawn); after spawn returns, the
    broker assertions are: `broker.contains_plugin(canonical)
    == true` (still in ACL) AND
    `broker.try_reserve_registration(&canonical) == Ok(())`
    (no live registration; pi-1 c19 — fixed contradictory
    rollback assertion); `in_flight` set no longer contains
    canonical.
  - `tests/supervisor_spawn_post_register_reaps_child.rs`
    (Linux-only via `#[cfg(target_os = "linux")]`) —
    `cached_pid` is `Some(_)` immediately after spawn
    returns; within ≤ 200ms the OS no longer has that pid
    alive (poll `/proc/<pid>/status`). Confirms the
    post-spawn reap fired during unwind.

---

## Group 8 — Fixture binary minimal (so c21 reaper test uses real fixture)

### c20 — feat(rfl-bus-fixture): minimal init — RFL_BUS_FD transport + readiness handshake + `respond_peer_call` mode

- **What** (pi-1 B3 structural reorder — minimal fixture
  before Phase B 18-20). Replace c03's empty `main` with the
  universal fixture init per scope §F2 + §F3, but **only the
  `respond_peer_call` mode + the universal `scaffold_only`
  exit-0 sentinel mode**:
  - Parse `RFL_BUS_FD`; on missing/invalid → `eprintln!` +
    exit 3.
  - `OwnedFd::from_raw_fd(fd)` once in `unsafe`; wrap as
    `tokio::net::UnixStream::from_std(...)`.
  - Build `OneShotConnector` per scope §F3 (mirrors
    `fittings/crates/client/src/lib.rs:623`); construct
    `StdioTransport::new(reader, writer, 1 << 20)`; call
    `Client::connect(OneShotConnector::new(transport)).await`.
  - **`RFL_FIXTURE_MODE` dispatch** for c20:
    - `scaffold_only` → exit 0 immediately (pre-Client setup
      path; for c03 backward-compat).
    - `respond_peer_call` — install service + notification
      handler → call `client.call("core.fixture.ready",
      json!({"mode":"respond_peer_call"})).await` → register
      service handling `core.fixture.start` (empty ack),
      `core.fixture.echo` (echo params); sleep until SIGTERM.
    - Unknown mode → `eprintln!` + exit 64 (matches c03
      contract).
  - No tracing init (per scope §F4).
- **Why.** scope §F1, §F2 (subset), §F3, §F4, §F5 (subset),
  pi-1 B3 (minimal fixture before c21 reaper test).
- **Depends on.** c03, c19 (supervisor must be functional
  enough to spawn the fixture — c19 gives us spawn through
  step 17 but not through 20, so c20 spawns are still
  unwound; integration tests in c20 use direct
  `Command::new(env!("CARGO_BIN_EXE_rfl-bus-fixture"))` with
  manual env to verify the fixture's standalone behaviour
  WITHOUT going through the supervisor).
- **Acceptance.** Two new tests that exercise the fixture
  WITHOUT the supervisor (the supervisor cannot complete a
  spawn until c21):
  - `tests/fixture_responds_to_ready_then_holds_open.rs` —
    create a socketpair via `nix`, fork via
    `Command::new(env!(...)).env("RFL_BUS_FD", ...)`
    `.env("RFL_FIXTURE_MODE", "respond_peer_call")` with the
    child fd inherited via the standard Unix pre-exec hook;
    parent side runs a fittings `Server` over the parent fd;
    receive `core.fixture.ready`; send
    `core.fixture.start`; send `core.fixture.echo` with
    `{"x":1}`; receive `{"x":1}` response; SIGTERM the
    child; verify exit. Linux-only via `#[cfg(target_os =
    "linux")]` (uses fork + fd inheritance).
  - `tests/fixture_unknown_mode_exits_64.rs` — already exists
    from c03; verify still passes after c20's mode dispatch
    extension.

---

## Group 9 — Supervisor Phase B finalization (reaper + serve loop + handle return)

### c21 — feat(rafaello-core::supervisor): Phase B steps 18–20 — reaper + watcher + serve loop + headline test

- **What.** Implement scope §SP4 Phase B steps 18–20:
  - **Step 18** — spawn the reaper task: `tokio::spawn(async
    move { let outcome = match child.wait().await { Ok(s) =>
    ReaperOutcome::Exited(s), Err(e) =>
    ReaperOutcome::WaitFailed(e) }; let _ =
    watch_tx.send(Some(Arc::new(outcome))); })`.
  - **Spawn the watcher task** per scope §SP4 step 18 + pi-5
    §1: awaits `reaper_handle`; on `JoinError` publishes
    `ReaperOutcome::ReaperPanicked`.
  - **Step 19** — `tokio::spawn(server.serve())`; store
    `JoinHandle` in `ManagedSpawn.serve_join`.
  - **Step 20** — wrap `SpawnObservation` in `Arc`; build
    `ManagedSpawn { observation: Arc::clone(&obs), registered:
    Some(reg), proxy: maybe_proxy, serve_join, reaper_join,
    watcher_join }`; insert into `supervisor.managed`; return
    `Ok(SpawnHandle(Arc::clone(&obs)))`. The
    `InFlightGuard` from c15 step 1a was already dropped at
    c19 step 17.
  - **`SpawnHandle::wait()` / `try_wait()`** wired to the
    watch channel: `wait` awaits the watch transition; late
    callers see the cached `Arc` immediately.
  - **`SpawnHandle::child_pid()`** returns
    `cached_pid` until `wait` observes `Some`, then `None`
    (pi-1 c20 finding — cached at step 13 in c19, reaper owns
    the child after that).
- **Why.** scope §SP4 18–20, §SP1, pi-5 §1, pi-1 c20 (cached
  pid).
- **Depends on.** c19, c20 (real fixture for headline test).
- **Acceptance.** **The headline test lands here.** Three new
  tests, all using the c20 minimal fixture via direct
  `Command`-style spawn replaced by `PluginSupervisor::spawn`:
  - `tests/supervisor_spawn_fixture_happy_path.rs` (canonical
    name per scope §"Demo bar") — spawn the c20 fixture in
    `respond_peer_call` mode via `PluginSupervisor::spawn`.
    Build the `CompiledPlugin` synthetically (real harness is
    c23). After spawn returns `Ok(handle)`:
    - `handle.canonical()` matches the plan.
    - `handle.try_wait()` returns `None` initially.
    - Send `nix::sys::signal::kill(pid, SIGTERM)`; the
      fixture exits.
    - `handle.wait().await` resolves to
      `Arc<ReaperOutcome::Exited(_)>` within bounded wait.
  - `tests/supervisor_peer_call_core_to_plugin.rs` (pi-1 B4
    — canonical scope test name) — fixture in
    `respond_peer_call`; harness calls
    `handle.peer().call("core.fixture.echo", json!({"x":1})).await`
    → `Ok({"x":1})`.
  - `tests/supervisor_spawn_handle_clone_observes_same_outcome.rs`
    — clone the `SpawnHandle`; both `wait().await` calls
    return `Arc::ptr_eq` outcomes.

---

## Group 10 — Remaining fixture modes + harness

### c22 — feat(rfl-bus-fixture): remaining modes — publish-and-exit + observer + call_core_then_exit + extra peer-call methods

- **What.** Add the remaining `RFL_FIXTURE_MODE` dispatch
  arms per scope §F2:
  - **Publish-and-exit modes** (`publish_one`,
    `publish_with_taint`, `publish_full_params`,
    `publish_bad_namespace`, `publish_bad_grammar`,
    `publish_outside_grant`,
    `publish_bad_in_reply_to_{missing,empty,multiple}`):
    - Wait for `core.fixture.start`.
    - Build `bus.publish` params per the mode (`topic` from
      `RFL_FIXTURE_TOPIC`, `payload` from
      `RFL_FIXTURE_PAYLOAD_JSON`, optionally `taint` from
      `RFL_FIXTURE_TAINT_JSON`, or full verbatim params from
      `RFL_FIXTURE_FULL_PARAMS_JSON`).
    - `client.notify("bus.publish", params).await` (NOT
      `client.peer().notify(...)` — pi-5 §2: shares the
      `ClientCommand` FIFO with `Client::call`).
    - `client.call("core.fixture.after_publish",
      Value::Null).await` — flush ack.
    - Exit 0.
  - **`call_core_then_exit` mode** — call
    `client.peer().call("core.fixture.ping", json!({"n":42})).await`;
    exit 0 on Ok response with payload non-empty (the
    fixture does NOT assert specific response shape — pi-1
    c22 finding — exit-0 just means "the call completed
    successfully"; the harness chooses the response shape);
    exit 2 on Err.
  - **`observer` mode** — `with_notification_handler` per
    scope §H4: synchronous closure clones an outbound
    `PeerHandle` and `tokio::spawn`s the forwarding
    `peer.call("core.fixture.observed", event_value).await`.
    After universal ready + `core.fixture.start` ack, sleep
    until SIGTERM.
  - **Extra `respond_peer_call` methods** (extends c20):
    `core.fixture.dump_env` (allow-listed via
    `RFL_FIXTURE_ENV_KEYS`), `core.fixture.write_private_state`
    (writes `<RFL_PRIVATE_STATE_DIR>/marker`),
    `core.fixture.report_open_result`
    (`RFL_FIXTURE_OPEN_PATH`),
    `core.fixture.try_write_path` (`RFL_FIXTURE_WRITE_PATH`).
- **Why.** scope §F2 (full mode set), §H4, pi-4 §5–§7,
  pi-5 §2.
- **Depends on.** c21.
- **Acceptance.** Three new tests directly exercising the
  fixture via supervisor spawn (uses the c21 supervisor
  surface):
  - `tests/fixture_publish_one_emits_event.rs` — two
    fixtures: A in `publish_one`, B in `observer`. Spawn
    both via supervisor; harness ready/start handshake (both
    fixtures); observer's harness-side `core.fixture.observed`
    extra service receives one `bus.event` matching A's
    publish.
  - `tests/fixture_call_core_then_exit_completes.rs` —
    fixture in `call_core_then_exit`; harness extra service
    registers `core.fixture.ping` returning `{"echo": "ok"}`;
    fixture exits 0 within bounded wait.
  - `tests/fixture_dump_env_returns_allow_listed_keys.rs` —
    fixture in `respond_peer_call`; call
    `core.fixture.dump_env` with
    `RFL_FIXTURE_ENV_KEYS=RFL_BUS_FD,RFL_PLUGIN`; response
    contains both keys with expected values
    (`RFL_BUS_FD == "3"`, `RFL_PLUGIN == plan.canonical.to_string()`).

### c23 — test(rafaello-core): m2 harness — `FixtureLockBuilder` + `Spawn` + `Observer` + `ReadinessGate`

- **What.** New file
  `rafaello/crates/rafaello-core/tests/common/m2_harness.rs`
  per scope §H1–§H5. **Uses real m1 API names** (pi-1 B6):
  - `FixtureLockBuilder` — creates `tempfile::TempDir`
    project root; materialises
    `<project>/plugins/<name>/` plugin_dir containing copy
    of `env!("CARGO_BIN_EXE_rfl-bus-fixture")` at
    `bin/fixture` (preserve exec bit), minimal
    `rafaello.toml`, and a canonical "no-op" `openrpc.json`
    sibling shipped at `tests/common/empty_openrpc.json`.
  - **Constructs the `Lock` value with the actual m1 API**:
    ```
    PluginEntry {
        entry,             // PathBuf for the entry within the package
        digest,            // m1 ContentDigest computed via digest::content_digest
        manifest_digest,   // m1 ManifestDigest computed via digest::manifest_digest
        granted_at,        // chrono::DateTime<Utc>
        grant: Grant { ... },        // NOT granted_capabilities
        bindings: Bindings { ... },
        flags: PluginFlags { ... },
    }
    ```
    The c23 agent reads `rafaello/crates/rafaello-core/src/lock/mod.rs`
    + `digest/mod.rs` to confirm exact field names + variant
    names before writing.
  - Builds `LockValidationContext { project_root, home,
    plugin_dirs, cache_root, state_root }` per the m1 type
    in `validate/mod.rs`. Calls `validate::lock(&lock,
    &lock_validation_context)`. Then `compile::compile_plugin(
    &lock, canonical, &PathContext { ... },
    &RecomputedDigests { ... })` for each plugin and
    `broker_acl::compile(&lock)`.
  - `Spawn` helper takes `BrokerAcl`, `Vec<CompiledPlugin>`,
    and a per-canonical extra-service factory map. Returns
    `(Broker, PluginSupervisor, Vec<SpawnHandle>)`.
  - `Observer` helper spawns `RFL_FIXTURE_MODE=observer` +
    registers `core.fixture.observed` extra service draining
    into `tokio::sync::mpsc::UnboundedReceiver`.
    `Observer::watch_all` builds the multi-namespace grant
    (`core.**`, `plugin.**`, `provider.**`, `frontend.**`).
  - `ReadinessGate` helper wraps the harness side of the
    `core.fixture.ready` extra service.
  - `ExtraServiceFactory` type alias matches scope §SP2
    exactly: `Arc<dyn Fn(CanonicalId) -> Box<dyn Service +
    Send + Sync> + Send + Sync>` (pi-1 c23 — type alignment).
  - The harness IS test code; does not leak into
    `rafaello-core` public API.
- **Why.** scope §H1–§H5, pi-1 B6 (real m1 API names),
  pi-3 non-blocking #1.
- **Depends on.** c22.
- **Acceptance.** The c20–c22 fixture tests are refactored
  to use the harness (their inline plumbing collapses). New
  test `tests/harness_lock_builder_round_trip.rs` —
  `FixtureLockBuilder` builds a one-plugin lock; assert
  `validate::lock` returns `Ok(...)`; assert `compile_plugin`
  returns `Ok(CompiledPlugin)` with expected fields.

### c24 — test(rafaello-core): real duplicate-spawn refusal (pi-1 B5)

- **What** (pi-1 B5 — defer real duplicate test until real
  spawn works). No new code; integration test only:
  `tests/supervisor_spawn_duplicate_canonical_refused.rs`
  using the c23 harness:
  - `Spawn` plugin A successfully (real spawn through c21).
  - Build a second `CompiledPlugin` for the same canonical
    via `FixtureLockBuilder` (or reuse the first plan).
  - Call `supervisor.spawn(plan, paths).await` on the same
    canonical → `SpawnError::AlreadyRegistered`.
  - `TestHooks` counter deltas across the failing call are
    **all zero** (no socketpair, no proxy, no child spawn);
    the Phase A in_flight + `try_reserve_registration` path
    catches the duplicate before resource allocation.
- **Why.** scope §"Demo bar" negatives, pi-1 B5.
- **Depends on.** c23.
- **Acceptance.** As above.

---

## Group 11 — Lifecycle: shutdown + Drop

### c25 — feat(rafaello-core::supervisor): cooperative `shutdown(self)` — SIGTERM + grace + SIGKILL + reaper wait + `ShutdownReport`

- **What.** Implement `PluginSupervisor::shutdown(self).await
  -> ShutdownReport` per scope §SP1 + §SP5:
  - For each `(canonical, mut managed)` in
    `self.managed.lock().drain()`:
    - `managed.registered.take().drop()` — broker fan-out
      stops immediately.
    - `nix::sys::signal::kill(Pid::from_raw(pid as i32),
      Signal::SIGTERM)`. On `Errno::ESRCH` treat as already
      exited.
    - **Wait up to `config.shutdown_grace`** for the watch
      channel to transition to `Some(...)`.
    - On grace-timeout: SIGKILL via the same nix call, then
      **continue waiting on the watch** until it transitions
      (pi-1 B7 — must reap before returning the per-plugin
      result so `ShutdownReport` reflects actual state).
    - `managed.proxy.take().drop()` — after child is dead.
    - `managed.serve_join.take().abort()`.
    - `managed.reaper_join.take()` and
      `managed.watcher_join.take()` are intentionally NOT
      aborted — they own the wait observation that the report
      depends on, and finish naturally once the child is
      reaped.
    - Build the per-plugin entry: `Exited(_)` →
      `report.clean.push(canonical)` (graceful) or
      `report.forced.push(canonical)` (SIGKILL fallback);
      `WaitFailed(io_err)` →
      `report.failed.push((canonical, ShutdownFailure::WaitFailed
      { kind: io_err.kind(), message: io_err.to_string() }))`;
      `ReaperPanicked` →
      `report.failed.push((canonical, ShutdownFailure::ReaperPanicked))`.
- **Why.** scope §SP1, §SP5, pi-2 §13, pi-6 §2, pi-1 B7
  (forced-kill reaper wait).
- **Depends on.** c24.
- **Acceptance.** Two new tests:
  - `tests/supervisor_shutdown_clean.rs` — fixture in
    `respond_peer_call` (handles SIGTERM via tokio's default
    handler — exits); call `shutdown().await`;
    `report.clean` contains the canonical;
    `report.forced` empty; bounded wait.
  - `tests/supervisor_shutdown_forced.rs` — add the
    `RFL_FIXTURE_TRAP_SIGTERM=1` env knob to the fixture
    universal init (the c25 agent extends c20's init: when
    the env var is set, install
    `tokio::signal::ctrl_c`-style trap that ignores SIGTERM;
    inline this fixture extension in the commit body); call
    `shutdown` with `shutdown_grace = Duration::from_millis(50)`;
    `report.forced` contains the canonical; SIGKILL fired
    after grace; bounded wait < `shutdown_grace * 4`. The
    test asserts `report.forced` AND that the child PID is
    actually reaped (no zombie — pi-1 B7).

### c26 — feat(rafaello-core::supervisor): `Drop` for `PluginSupervisor` — best-effort SIGKILL + reaper handoff

- **What.** Implement `Drop` for `PluginSupervisor` per scope
  §SP5 + pi-6 non-blocking #4:
  - Synchronous best-effort: for each managed plugin in
    `self.managed.get_mut()`,
    `nix::sys::signal::kill(Pid::from_raw(pid), SIGKILL)`
    (ignoring ESRCH); abort serve_join; drop `RegisteredPlugin`
    + `ProxyHandle` (the supervisor-owned resources from
    `ManagedSpawn`).
  - The reaper task continues until the OS marks the child
    dead and publishes the outcome to its watch — outstanding
    `SpawnHandle` clones can still observe it. The
    supervisor does not block waiting (Drop cannot await).
  - **External `SpawnHandle` clones do NOT keep
    `RegisteredPlugin` / `ProxyHandle` alive** (pi-1 B2 —
    enforced by the c14 managed-vs-observation split: clones
    only hold `Arc<SpawnObservation>`).
- **Why.** scope §SP5, pi-1 §28, pi-6 non-blocking #4,
  pi-1 B2 (clone-isolation).
- **Depends on.** c25.
- **Acceptance.** `tests/supervisor_drop_kills_managed_children.rs`
  — spawn two fixtures in `respond_peer_call`; hold a clone
  of one `SpawnHandle`; drop the supervisor; bounded wait —
  both fixtures exit (via SIGKILL); the held `SpawnHandle::wait().await`
  resolves to `Arc<ReaperOutcome::Exited(_)>` with
  signal-indicating-SIGKILL semantics. Critically: the broker
  no longer has either canonical registered (verified via
  `broker.try_reserve_registration` returning `Ok(())`).

---

## Group 12 — Env behaviour tests

### c27 — test(rafaello-core): supervisor env behaviour — pass + set + override + clear (pi-1 B4)

- **What** (pi-1 B4 — three named scope tests were missing
  schedule). No new code. Three integration tests using the
  c23 harness's `core.fixture.dump_env` plumbing. All three
  use `#[serial_test::serial(env)]` per scope §"Risks" #4.
- **Why.** scope §"Demo bar" positives, pi-1 B4.
- **Depends on.** c26.
- **Acceptance.** Three tests:
  - `tests/supervisor_env_pass_set_applied.rs` — plan with
    `env.pass = ["FAKE_PUBLIC_ENV"]` (NOT a `*_SECRET`/
    `*_TOKEN`/`*_KEY` pattern — m1's scrubber would strip
    those) and `env.set = {"FOO": "bar"}`. Test sets
    `FAKE_PUBLIC_ENV=abc` in parent env. Dump asserts both
    keys present plus reserved `RFL_*` (RFL_BUS_FD,
    RFL_PLUGIN, RFL_TOPIC_ID, RFL_PROJECT_ROOT,
    RFL_PRIVATE_STATE_DIR).
  - `tests/supervisor_env_set_overrides_pass.rs` — plan with
    `env.pass = ["FOO_VAR"]` and `env.set = {"FOO_VAR":
    "set-wins"}`. Parent has `FOO_VAR=pass-loses`. Dump:
    `FOO_VAR == "set-wins"`.
  - `tests/supervisor_env_clear_strips_unrelated.rs` —
    parent has `RANDOM_PARENT_VAR=secret` (not in
    pass/set). Dump asserts `RANDOM_PARENT_VAR` is absent.

---

## Group 13 — Proxy + lockin denial proofs + cross-plugin

### c28 — test(rafaello-core): proxy startup + env injection end-to-end

- **What.** No new code. Integration test for the proxy +
  env paths together via the c23 harness:
  `tests/supervisor_proxy_starts_and_env_injected.rs` —
  fixture A in `respond_peer_call` mode with
  `NetworkPlan::Proxy { allow_hosts: ["example.com"] }`;
  harness calls `core.fixture.dump_env` with
  `RFL_FIXTURE_ENV_KEYS=HTTP_PROXY,HTTPS_PROXY,ALL_PROXY,NO_PROXY,http_proxy,https_proxy,all_proxy,no_proxy,RFL_BUS_FD`.
  Assertions: every uppercase + lowercase `*_PROXY` is
  `http://127.0.0.1:<port>`; both `NO_PROXY` and `no_proxy`
  are `""`; `RFL_BUS_FD == "3"`. `TestHooks::outpost_starts
  == 1`.
- **Why.** scope §"Demo bar" positive, pi-5 §6 (lowercase
  proxy env keys).
- **Depends on.** c27.
- **Acceptance.** As above.

### c29 — test(rafaello-core): lockin denial proofs (read + write outside grant)

- **What.** No new code. Two integration tests:
  - `tests/supervisor_lockin_denies_outside_grant_read.rs`
    — fixture A with `read_dirs = [<project-root>]` only;
    `RFL_FIXTURE_OPEN_PATH=/etc/hosts`. Harness calls
    `core.fixture.report_open_result`; asserts `ok == false`,
    `errno` matches `EPERM` / `EACCES` / `ENOENT` (`matches!`
    not `==`).
  - `tests/supervisor_lockin_denies_outside_grant_write.rs`
    — A's `write_dirs = [<private-state>]` only;
    `RFL_FIXTURE_WRITE_PATH=<project-root>/forbidden`.
    Harness calls `core.fixture.try_write_path`; asserts
    denial; verifies `<project-root>/forbidden` does not
    exist after.
- **Why.** scope §"Demo bar" negatives, scope §"Platform
  gating" (errno via `matches!`).
- **Depends on.** c28.
- **Acceptance.** As above.

### c30 — test(rafaello-core): cross-plugin round-trip + private-state + taint round-trip

- **What.** Three integration tests covering remaining
  positive demo-bar items:
  - `tests/supervisor_bus_publish_round_trip_two_plugins.rs`
    — A in `publish_one` publishing `"plugin.<A>.greet"`;
    B with subscribe grant on `"plugin.<A>.greet"` —
    observer-style B forwards via `core.fixture.observed`.
    After two-phase readiness handshake + `core.fixture.start`
    to A, B receives one event matching topic + payload +
    publisher.
  - `tests/supervisor_private_state_dir_writable.rs` —
    A in `respond_peer_call`; harness calls
    `core.fixture.write_private_state`; fixture writes to
    `<RFL_PRIVATE_STATE_DIR>/marker`; test verifies file
    exists and the path matches
    `project_root.join(".rafaello-plugin-data").join(plan.topic_id).join("marker")`.
  - `tests/supervisor_taint_round_trip.rs` — A in
    `publish_with_taint` mode with
    `RFL_FIXTURE_TAINT_JSON=[{"source":"test","detail":"x"}]`;
    observer receives event; `event.taint` round-trips
    byte-equal.
  - **`supervisor_peer_call_plugin_to_core.rs`** (pi-1 B4 —
    canonical scope test name) — A in `call_core_then_exit`
    mode; harness extra service registers `core.fixture.ping`
    returning `{"echo":"ok"}`; fixture exits 0 within bounded
    wait. (Could split into a separate commit if c30 grows
    too large; the c30 agent picks at implementation time and
    inlines the choice in the commit body.)
- **Why.** scope §"Demo bar" positives, pi-1 B4.
- **Depends on.** c29.
- **Acceptance.** As above.

---

## Group 14 — Manual validation (Phase 4 retrospective is driver-owned, not a per-commit task)

### c31 — docs(rafaello-m2): write `manual-validation.md` capturing m2 demo bar evidence

- **What.** Create
  `rafaello/plans/milestones/m2-broker-spawn/manual-validation.md`
  per scope §"Manual validation":
  - Capture of `nix develop --impure --command cargo test
    --manifest-path rafaello/Cargo.toml -p rafaello-core
    --features test-fixture` output (last 20 lines: `<n>
    tests passed; 0 failed`).
  - Capture of `RUST_LOG=rafaello_core=debug nix develop
    --impure --command cargo test --manifest-path
    rafaello/Cargo.toml -p rafaello-core --features
    test-fixture --test supervisor_spawn_fixture_happy_path
    -- --nocapture` showing broker registration trace +
    fan-out traces.
  - `find rafaello/crates/rafaello-core/src -name '*.rs' |
    sort` listing the new modules (`bus.rs` family +
    `supervisor.rs` family + `bin/rfl_bus_fixture.rs` —
    actual layout per c06/c07/c12/c14/c20).
  - `ls /proc/<fixture-pid>/fd/` snapshot during a
    `respond_peer_call` fixture run with the documented
    invariants (fd 3 is `socket:[...]`; parent does not have
    a duplicate of the same socket inode).
  - `nix develop --impure --command cargo doc
    --manifest-path rafaello/Cargo.toml -p rafaello-core
    --no-deps` warning-free.
  - `nix develop --impure --command cargo build
    --manifest-path rafaello/Cargo.toml -p rafaello-core
    --features test-fixture --bin rfl-bus-fixture` green.
  - **macOS CI capture placeholder** — the c31 agent inserts
    a `## macOS CI` section with text "captured by milestone
    driver post-c31; CI URL: <pending driver action>".
    **The driver pushes `rafaello-v0.1` to origin AFTER c31
    lands** (pi-1 B4 + pi-1 c29 finding — branch push is a
    driver-owned action, NOT a per-commit agent task) and
    fills in the CI URL + summary in a follow-up
    docs-only commit on `rafaello-v0.1` (NOT a `commits.md`
    item; the driver-notes.md captures this step).
- **Why.** scope §"Manual validation", §"Acceptance summary",
  pi-1 c29 (driver owns push/CI capture, not per-commit
  agent).
- **Depends on.** c30.
- **Acceptance.** `manual-validation.md` exists, contains
  every section above. Driver verifies before merging that
  all captures are recent.

### Phase 4 — `retrospective.md` is driver-owned, NOT a per-commit task

(pi-1 B4 — retrospective.md is required by scope acceptance
but per the m1 pattern is created in Phase 4 by the
milestone driver, NOT a per-commit agent task. m1 retrospective
took 4 pi rounds; m2 budget for at least 2 per `plans/README.md`
"Patterns from prior milestones".)

---

## What changed from prior drafts

### Round-2 changes addressing pi commits round-1

- **B1 (c06 serde direction).** c06 acceptance rewritten to
  match the one-way derives: `PublishMsg` decode-only,
  `BusEvent` encode-only (round-trip via test-only
  permissive struct). Test renamed
  `bus_wire_types_schema.rs`.
- **B2 (supervisor resource ownership).** c14 now defines
  `ManagedSpawn` (supervisor-owned: `RegisteredPlugin`,
  `ProxyHandle`, `JoinHandle`s in `Mutex<Option<_>>`) split
  from `SpawnObservation` (handle-shared: canonical,
  topic_id, cached_pid, peer clone, watch receiver). External
  `SpawnHandle` clones cannot keep managed resources alive.
  c20/c25/c26 wire correctly to the split.
- **B3 (c20 fixture dependency).** Structurally reordered:
  the minimal fixture (`respond_peer_call` mode + universal
  init) is now **c20**, BEFORE the supervisor reaper/serve-loop
  finalization (now **c21**). c21 headline test uses the
  real fixture; no `#[ignore]` paths.
- **B4 (missing scope tests).** Added: `bus_pattern_matches.rs`
  (c07), `broker_unsubscribed_plugin_does_not_receive.rs`
  (c12), `supervisor_invalid_proxy_allow_hosts_refused.rs`
  (c16), three env tests in new c27,
  `supervisor_peer_call_core_to_plugin.rs` (c21),
  `supervisor_peer_call_plugin_to_core.rs` (c30),
  real `supervisor_spawn_duplicate_canonical_refused.rs`
  (c24). Retrospective note added — `retrospective.md` is
  driver-owned Phase 4, not per-commit.
- **B5 (c15 duplicate test).** Removed; real test deferred
  to c24 after harness + real spawn work.
- **B6 (c23 harness API names).** c23 rewritten to use real
  m1 API: `PluginEntry { entry, digest, manifest_digest,
  granted_at, grant: Grant, bindings, flags }`;
  `LockValidationContext { project_root, home, plugin_dirs,
  cache_root, state_root }`; `validate::lock(&lock,
  &lock_validation_context)`; `compile::compile_plugin(...)`;
  `broker_acl::compile(...)`. `ExtraServiceFactory` type alias
  matches scope §SP2 exactly.
- **B7 (c25 forced-kill reap).** c25 algorithm now **continues
  waiting on the watch after SIGKILL** until reaper observes
  exit; `ShutdownReport` reflects actual state.
- **Per-commit fixes.** c04 acceptance now table-driven
  (deterministic). c07 specifies the test peer-kit shape.
  c12 adds unsubscribed_drop. c13 reorders grammar before
  namespace for `publish_core`. c16 acceptance lists six
  tests (was incorrectly "four"). c19 contradiction
  removed; real fittings type names; `Response { id:
  JsonRpcId::Null, ... }` for notification path. c20
  `cached_pid` semantics specified. c21 fixture mode
  unknown-mode behavior pinned. c22 `call_core_then_exit`
  response shape clarified as harness-supplied. c23
  `ExtraServiceFactory` shape pinned. c25 fixture extension
  inlined. c26 enforces clone-isolation. c29 push assignment
  reassigned to driver.
- **Total commits: 31** (was 29 in round 1; +c20 minimal
  fixture, +c24 real duplicate test, +c27 env tests,
  -synthetic c15 dup test, -original c30 grouping).

### Notes for round-2 pi review

If a finding is structural-reorganisation (move work between
commits), say so — the m1 retrospective §4.1 calls these out
as the kind of phase-boundary discoveries that justify
rounds. If a finding is wording polish, mark it non-blocking.
