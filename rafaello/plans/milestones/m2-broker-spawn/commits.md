# m2-broker-spawn — commits

> **Status:** round-1 draft. Pi review pending. Not yet
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

## m2a / m2b checkpoint

No internal split is planned: m2's broker + supervisor are
tightly coupled and a split would split the demo bar too. The
driver re-evaluates after **c14** (broker complete + types) and
after **c20** (supervisor end-to-end happy-path landed); if a
split becomes obviously beneficial, the driver opens an
m2a / m2b owner-ratification request rather than carrying it
silently.

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
  --manifest-path rafaello/Cargo.toml --format-version 1 |
  jq '.workspace_members'` lists no new members; the new entries
  are visible in `cargo metadata --format-version 1 |
  jq '.metadata // empty'` resolution; `nix develop --impure
  --command cargo build --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green (no rafaello-core consumer of the new
  deps yet — table-only addition). `nix develop --impure
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
  green (feature compiles; bin target lands in c03).
  `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core` green (m1 tests still
  pass; no new tests yet).

### c03 — feat(rafaello-core): add `rfl-bus-fixture` `[[bin]]` scaffold

- **What.** Add `[[bin]]` to `rafaello/crates/rafaello-core/Cargo.toml`:
  `name = "rfl-bus-fixture"`, `path = "src/bin/rfl_bus_fixture.rs"`,
  `required-features = ["test-fixture"]`. Create the bin file
  with a minimal `main` that prints a banner and exits 0:
  `eprintln!("rfl-bus-fixture v{} ready", env!("CARGO_PKG_VERSION"));
  std::process::exit(0);`. No fittings/transport/service code
  yet — that lands in c21.
- **Why.** scope §W3, §F1. Establishes the binary target so
  `env!("CARGO_BIN_EXE_rfl-bus-fixture")` resolves from
  rafaello-core's integration tests.
- **Depends on.** c02.
- **Acceptance.** `nix develop --impure --command cargo build
  --manifest-path rafaello/Cargo.toml -p rafaello-core
  --features test-fixture --bin rfl-bus-fixture` green.
  Without the feature, `cargo build -p rafaello-core` does NOT
  build the bin (verifies the gate works). New trivial test
  `tests/fixture_binary_resolves.rs` (gated `#[cfg(feature =
  "test-fixture")]` — uses `env!("CARGO_BIN_EXE_rfl-bus-fixture")`,
  invokes `Command::new(path).status()`, asserts exit 0). This
  proves the cargo bin-test integration works for subsequent
  m2 commits.

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
- **Why.** scope §F5 (fixture reserved env list) + §SP4.4
  (Phase A reserved-env check) require these names to be
  rejected by m1's existing scrubber so the V3 path catches
  collisions before they reach the supervisor.
- **Depends on.** c01.
- **Acceptance.** Update `tests/env_scrubber_strips_known_secrets.rs`
  (or add a sibling `env_scrubber_reserved_m2_names.rs` —
  driver picks at implementation time but inline the choice
  in the per-commit prompt) to assert each of the four new
  names triggers `EnvKeyReserved` (or m1's existing variant
  name — verify) when present in `env.set`. Existing 269 m1
  tests still pass under `nix develop --impure --command cargo
  test --manifest-path rafaello/Cargo.toml -p rafaello-core`.

---

## Group 2 — Bus error surface + wire types

### c05 — feat(rafaello-core): add `BrokerError` + `SpawnError` + companion enums

- **What.** New `BrokerError`, `SpawnError`, `InvalidPlanReason`,
  `Publisher`, `InReplyToReason`, `PathKind`, `ReaperOutcome`,
  `ShutdownFailure` enums in `rafaello/crates/rafaello-core/src/error.rs`
  per scope §B2 + §SP3. All `thiserror`-derived,
  `#[non_exhaustive]`. `BrokerError` is `Debug` only (NOT `Clone`,
  NOT `PartialEq` — see scope §B2 + pi-3 §3); `SpawnError`
  source-error variants store the real types (`anyhow::Error`,
  `std::io::Error`, `nix::errno::Errno`, `FittingsError`).
  Variant names per scope §B2 / §SP3 (no aliases). Top-level
  `rafaello_core::Error` enum gets `#[from]` arms for
  `BrokerError` and `SpawnError`. Re-exports added to `lib.rs`.
- **Why.** scope §B2, §SP3, §E. m1's pattern (m1 c02): land the
  error surface early so subsequent commits reference variant
  names without forward refs.
- **Depends on.** c02.
- **Acceptance.** `tests/m2_error_surface_compiles.rs` —
  build-only assertion that each new variant is reachable
  through `rafaello_core::Error` and the variant names match
  scope §B2 / §SP3 (one `let _: BrokerError = ...` per variant,
  one for each `SpawnError` variant, one for `ReaperOutcome::*`,
  one for `ShutdownFailure::*`). Runtime behavior: zero —
  variants are constructed by later commits. `nix develop
  --impure --command cargo test --manifest-path rafaello/Cargo.toml
  -p rafaello-core --features test-fixture` green.

### c06 — feat(rafaello-core::bus): add wire types — `PublishMsg`, `BusEvent`, `TaintEntry`, `PublisherIdentity`

- **What.** New module `rafaello_core::bus` with public types
  per scope §B1 / §B4 / §B8:
  - `pub struct PublishMsg { topic, payload, in_reply_to,
    taint }` with `#[derive(Debug, Clone, Deserialize)]` +
    `#[serde(deny_unknown_fields)]`. Optional fields use
    `#[serde(default)]`.
  - `pub struct TaintEntry { source, detail }` with
    `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq,
    Eq)]` + `#[serde(deny_unknown_fields)]`.
  - `pub struct BusEvent { topic, payload, publisher,
    in_reply_to, taint }` with `#[derive(Debug, Clone,
    Serialize)]` + `#[serde(skip_serializing_if = "Option::is_none")]`
    on the optional fields.
  - `pub enum PublisherIdentity { Core, Plugin { canonical:
    String, topic_id: String } }` with `#[derive(Debug, Clone,
    Serialize)]` + `#[serde(tag = "kind", rename_all =
    "snake_case")]`.
  - `JsonRpcId` from `fittings_core::message::JsonRpcId`
    re-exported from `bus` for convenience.
  - `bus` re-exports from `lib.rs`. No `Broker` type yet —
    that's c07.
- **Why.** scope §B1, §B4, §B8. Wire types only; broker logic
  comes later.
- **Depends on.** c05.
- **Acceptance.** `tests/bus_types_round_trip.rs` — constructs
  one `PublishMsg` with all fields populated (incl.
  `in_reply_to` array, `taint` array), serialises via
  `serde_json::to_value`, deserialises back, asserts byte-equal.
  Same for `BusEvent` with each `PublisherIdentity` variant.
  Asserts `deny_unknown_fields` fires by attempting to decode
  `{"topic":"a.b","payload":null,"unknown":1}` and asserting
  the decode error mentions `unknown`. `nix develop --impure
  --command cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core --features test-fixture` green.

---

## Group 3 — Broker registration + lifecycle

### c07 — feat(rafaello-core::bus): `Broker::new` + ACL revalidation + `register_plugin` + `RegisteredPlugin` RAII guard

- **What.** Implement `Broker` per scope §B1 + §B10:
  - `pub struct Broker(Arc<BrokerInner>)` cheap-clone handle.
    Internal state `parking_lot::Mutex<BrokerState>` with
    `BTreeMap<CanonicalId, PluginConn>` registry (deterministic
    ordering — scope §B1).
  - `Broker::new(acl: BrokerAcl) -> Result<Self, BrokerError>`
    runs §B10 defence-in-depth pattern/topic revalidation
    (`validate::topic::validate_topic` for every
    `publish_topics` entry; `validate::topic::validate_pattern`
    for every `subscribe_patterns` / `auto_subscribes` entry
    across the supplied ACL). Returns `InvalidTopic` /
    `InvalidPattern` on failure.
  - `Broker::register_plugin(canonical, peer) ->
    Result<RegisteredPlugin, BrokerError>` with
    `NotInAcl` / `AlreadyRegistered`. `RegisteredPlugin` is
    `!Clone`, `Send + Sync` (RAII single-owner guard holding
    `Arc<BrokerInner>` backref + canonical). On drop: removes
    the registry entry and drops the broker's `PeerHandle`
    clone (does NOT close other clones — scope §B1 preface,
    pi-5 non-blocking).
  - `Broker::try_reserve_registration(&self, canonical) ->
    Result<(), BrokerError>` cheap precheck for the supervisor
    (no actual reservation; checks ACL membership + live
    registration).
  - `Broker::contains_plugin(&self, canonical) -> bool` — ACL
    membership only (NOT live; scope §B1 + pi-3 non-blocking
    #3).
  - `Broker::plugin_acl(&self, canonical) -> Option<PluginAcl>`
    — clone of per-plugin ACL entry (scope §B1, pi-6 §3).
  - `Broker::shutdown(&self)` — drains all registrations
    idempotently.
  - No publish/fan-out path yet — that's c09–c12.
- **Why.** scope §B1, §B10.
- **Depends on.** c06.
- **Acceptance.** Three new integration tests:
  - `tests/broker_register_unregister.rs` — happy path
    register → drop guard → `contains_plugin` still true,
    re-register works.
  - `tests/broker_register_canonical_not_in_acl_rejected.rs` —
    `register_plugin(unknown, peer)` → `NotInAcl`.
  - `tests/broker_register_duplicate_rejected.rs` — second
    `register_plugin(A, peer)` → `AlreadyRegistered`.
  - `tests/broker_invalid_acl_rejected_at_construction.rs` —
    `Broker::new(acl)` with hand-built ACL containing pattern
    `"**"` (invalid grammar) → `InvalidPattern`. Sub-case for
    invalid topic (`"plugin.id.UPPER"`) → `InvalidTopic`.
  - All four use the test pattern of constructing fake
    `PeerHandle`s. **Note for the c07 agent:** broker
    integration tests need observable `PeerHandle`s; this
    commit picks the strategy at implementation time
    (scope §"H6 hooks" mentions the harness; c07 may need a
    minimal in-memory `PeerHandle` constructor or a
    `tests/common/peer_test_kit.rs` helper). Inline the
    chosen approach in the commit body.
  Pre-existing 269 m1 tests + the m2 c03/c04/c05/c06 tests
  still green.

### c08 — feat(rafaello-core::bus): `publish_boot` happy path

- **What.** Implement `Broker::publish_boot(&self) ->
  Result<(), BrokerError>` per scope §B1 (sugar over an
  internal `publish_core`-like path that emits
  `core.lifecycle.boot` with payload `{"version":
  env!("CARGO_PKG_VERSION"), "plugin_count":
  acl.plugins.len()}`). c08 ships ONLY the boot path because
  the full `publish_core` + `handle_plugin_publish` machinery
  lands across c09–c13; `publish_boot` only needs the fan-out
  primitive (a thin `fan_out_one_event(BusEvent)` private
  helper that iterates registered plugins under snapshot lock
  and calls `peer.notify("bus.event", value)`). NO ACL checks
  on the helper since core is the publisher and the topic is
  hardcoded; the helper is reused by c09–c13's full path.
- **Why.** scope §B1 (`publish_boot`), §B7 fan-out (initial
  shape), §B8 `BusEvent` consumer.
- **Depends on.** c07.
- **Acceptance.** `tests/broker_publish_boot_event.rs` —
  `Broker::new(acl)?` returns Ok; register an observer
  (using the same test peer-kit from c07); call
  `broker.publish_boot()`; observer receives one `bus.event`
  notification with topic `"core.lifecycle.boot"`, payload
  `{"version": ..., "plugin_count": <N>}`, `publisher ==
  PublisherIdentity::Core`. Pre-existing tests green.

---

## Group 4 — Broker publish path: ACL + grammar + fan-out

### c09 — feat(rafaello-core::bus): `handle_plugin_publish` — params decode + grammar revalidation

- **What.** Implement
  `Broker::handle_plugin_publish(&self, canonical: &CanonicalId,
  raw_params: &Value) -> Result<(), BrokerError>` per scope §B3
  steps 1–2 + §B5: decode `raw_params` to `PublishMsg` (returns
  `InvalidPayload`); run `validate::topic::validate_topic` on
  `msg.topic` (returns `InvalidTopic`); also enforces live
  registration (`NotRegistered` if canonical not registered).
  No namespace parsing yet (that's c10), no
  `in_reply_to`/grant checks (c11), no fan-out (c12). The
  function returns `Ok(())` on payload + grammar passing,
  short-circuiting before later checks.
- **Why.** scope §B3 (steps 1–2), §B5, §B1 `NotRegistered`.
- **Depends on.** c08.
- **Acceptance.** Three new tests:
  - `tests/broker_publish_extra_field_rejected.rs` —
    `bus.publish` params with an extra unknown key →
    `InvalidPayload` (decode error captured in `reason`).
  - `tests/broker_publish_invalid_topic_grammar_rejected.rs`
    — three `#[test]` cases: `"plugin.<id>.UPPERCASE"`,
    `"plugin.<id>.has spaces"`, `""` → `InvalidTopic` with
    the appropriate `ValidationError` `Display`.
  - `tests/broker_handle_publish_after_unregister_returns_not_registered.rs`
    — register A, drop guard, call `handle_plugin_publish(A,
    valid_params)` → `NotRegistered(A)`.

### c10 — feat(rafaello-core::bus): structural namespace parsing — `UnknownNamespace` + `PublishOnReservedNamespace`

- **What.** Add the structural namespace step per scope §B3
  steps 3 (only — split topic by `.`, look at `segments[0]`):
  - Not in `{core, provider, plugin, frontend}` →
    `UnknownNamespace`.
  - `core` / `provider` / `frontend` →
    `PublishOnReservedNamespace`.
  - `plugin`:
    - `segments.len() < 3` → `PublishOnReservedNamespace`
      (two-segment plugin topic).
    - `segments[1] != publisher_acl.topic_id` →
      `PublishOnReservedNamespace` (cross-plugin masquerade).
  Does NOT yet enforce `PublishOutsideGrant` for own-namespace
  topics (that's c11). Both `UnknownNamespace` and
  `PublishOnReservedNamespace` use the `Publisher::Plugin`
  variant.
- **Why.** scope §B3 step 3, pi-5 §3 (Unknown vs Reserved
  distinction).
- **Depends on.** c09.
- **Acceptance.** Five new tests, one per rejection class:
  - `tests/broker_publish_unknown_namespace_rejected.rs` —
    plugin publishes `"evil.foo"` → `UnknownNamespace`. Sub-case
    `"random.thing.bar"`.
  - `tests/broker_publish_short_plugin_topic_rejected.rs` —
    `"plugin.<own-id>"` and `"plugin.<other-id>"` (two-segment)
    → `PublishOnReservedNamespace`.
  - `tests/broker_publish_core_namespace_rejected.rs` —
    `"core.session.user_message"` → `PublishOnReservedNamespace`.
  - `tests/broker_publish_provider_namespace_rejected.rs` —
    `"provider.openai.tool_request"` →
    `PublishOnReservedNamespace`.
  - `tests/broker_publish_frontend_namespace_rejected.rs` —
    `"frontend.tui.confirm_answer"` →
    `PublishOnReservedNamespace`.
  - `tests/broker_publish_other_plugin_namespace_rejected.rs`
    — A publishes `"plugin.<B-topic-id>.tool_result"` →
    `PublishOnReservedNamespace`.

### c11 — feat(rafaello-core::bus): outside-grant rejection + `in_reply_to` arity enforcement

- **What.** Add the remaining §B3 step 3 plugin sub-step
  + §B6:
  - `plugin.<own-topic-id>.*` topic not in
    `publisher_acl.publish_topics` exact-string set →
    `PublishOutsideGrant`. (`auto_subscribes` is NOT publish
    authority — scope §B3 + pi-1 §9, §69, §248.)
  - For topics ending in `.tool_result` or `.rpc_reply` (per
    §B6): require `in_reply_to: Some(vec![one_id])`. Missing
    → `InvalidInReplyTo { reason: Missing }`. Empty array →
    `InvalidInReplyTo { reason: EmptyArray }`. Two or more
    entries → `InvalidInReplyTo { reason: UnexpectedMultiple }`.
- **Why.** scope §B3 step 3 (own-namespace grant), §B6
  (`in_reply_to` enforcement subset).
- **Depends on.** c10.
- **Acceptance.** Five new tests:
  - `tests/broker_publish_outside_grant_rejected.rs` — A
    publishes `"plugin.<A-topic-id>.ungranted"` (lock grant
    contains only `"plugin.<A>.granted"`) →
    `PublishOutsideGrant`.
  - `tests/broker_tool_result_in_reply_to_missing_rejected.rs`
    — publish `plugin.<A>.tool_result` with no
    `in_reply_to` → `InvalidInReplyTo { reason: Missing }`.
  - `tests/broker_tool_result_in_reply_to_empty_rejected.rs`
    — same with `in_reply_to: []` → `InvalidInReplyTo {
    reason: EmptyArray }`.
  - `tests/broker_tool_result_in_reply_to_multiple_rejected.rs`
    — same with two ids → `InvalidInReplyTo { reason:
    UnexpectedMultiple }`.
  - `tests/broker_rpc_reply_in_reply_to_missing_rejected.rs`
    — same shape for `plugin.<A>.rpc_reply`.

### c12 — feat(rafaello-core::bus): fan-out + publisher exclusion + result-routing protection

- **What.** Wire up the §B7 fan-out path inside
  `handle_plugin_publish`: after authorisation + class checks
  pass, construct a `BusEvent` (with
  `publisher: PublisherIdentity::Plugin { canonical, topic_id
  }`), serialise via `serde_json::to_value` ONCE, snapshot the
  registry under the broker mutex (clone the per-plugin
  `(canonical, peer, subscribe_patterns ∪ auto_subscribes)`
  list), release the mutex, then iterate: for each registered
  plugin **other than the publisher**, check the topic against
  the patterns via `validate::topic::pattern_matches_topic`; if
  matched call `peer.notify("bus.event", value.clone())`.
  Fan-out errors logged via `tracing::warn!` with the canonical
  but do NOT fail the publish call (best-effort delivery).
  **Result-routing protection** (scope §B7): topics ending in
  `.tool_result` or `.rpc_reply` skip the per-subscriber
  delivery loop entirely (the auth + arity checks above still
  ran); a `tracing::debug!` records the suppressed delivery so
  m4's canonical re-emission can verify the suppression in its
  own tests.
- **Why.** scope §B7, §B8 (`BusEvent` consumer), pi-1 §14
  (result-routing protection), pi-1 §247 (no echo).
- **Depends on.** c11.
- **Acceptance.** Three new tests:
  - `tests/broker_publish_round_trip.rs` — A publishes
    `"plugin.<A>.greet"` (granted); observer B subscribed to
    `"plugin.**"` receives one `bus.event` with `topic ==
    "plugin.<A>.greet"`, `publisher == Plugin {
    canonical: A, topic_id: <A's id> }`. Asserts the payload
    + taint + in_reply_to fields round-trip.
  - `tests/broker_publishing_plugin_excluded_from_own_fanout.rs`
    — A's grant subscribes to `"plugin.<A>.**"` AND publishes
    `"plugin.<A>.foo"`; observer B receives the event; A does
    NOT receive it (assertion via the harness's per-peer
    receiver).
  - `tests/broker_tool_result_not_fanned_out_to_other_plugins.rs`
    — A publishes `"plugin.<A>.tool_result"` with valid
    `in_reply_to`; B subscribed to `"plugin.**"` does NOT
    receive it. Asserts the `tracing::debug!` message via
    `#[tracing_test::traced_test]` — the suppression trace is
    visible.

### c13 — feat(rafaello-core::bus): `publish_core` + `core.lifecycle.publish_rejected`

- **What.** Two pieces:
  - `Broker::publish_core(&self, topic: &str, payload: Value)
    -> Result<(), BrokerError>` per scope §B1: structurally
    parse the topic; reject `UnknownNamespace` /
    `PublishOnReservedNamespace` (using the `Publisher::Core`
    variant for both errors per scope §B2); revalidate
    grammar; build `BusEvent { publisher:
    PublisherIdentity::Core, ... }`; fan out via the same
    helper from c12 (no publisher-exclusion since core is not
    a registered plugin per scope §B1).
  - **`core.lifecycle.publish_rejected` event emission** per
    scope §B9: every rejection path in `handle_plugin_publish`
    (UnknownNamespace, PublishOnReservedNamespace,
    PublishOutsideGrant, InvalidTopic, InvalidInReplyTo,
    InvalidPayload) emits one `core.lifecycle.publish_rejected`
    event via an internal `publish_core_internal` path that
    bypasses the structural namespace re-check (the broker
    has already constructed the topic correctly) but still
    runs grammar + fan-out. Schema per §B9: `{ canonical:
    Option<String>, topic: Option<String>, code: String,
    message: String }`. `topic = null` only for `InvalidPayload`
    decode failures that happened before topic extraction —
    the broker tries a permissive `Value`-level extraction
    first. Reject-event construction itself does NOT call
    rejection emission (no recursion).
- **Why.** scope §B1 (`publish_core`), §B2 (`Publisher` enum
  reuse), §B9 (lifecycle rejection event), pi-1 §15.
- **Depends on.** c12.
- **Acceptance.** Three new tests:
  - `tests/broker_publish_core_happy_path.rs` —
    `publish_core("core.lifecycle.test", payload)` with one
    observer subscribed to `"core.**"` — observer receives
    one `bus.event` with `publisher == Core`.
  - `tests/broker_publish_core_invalid_topic_rejected.rs` —
    `publish_core("plugin.x.y", ...)` →
    `PublishOnReservedNamespace { publisher: Core, ... }`.
    `publish_core("core.Bad", ...)` → `InvalidTopic`.
    `publish_core("evil.foo", ...)` → `UnknownNamespace`.
  - `tests/broker_publish_rejected_event_fired_on_each_rejection_class.rs`
    — table-driven: trigger each rejection class from
    c10/c11/c09 (`UnknownNamespace`,
    `PublishOnReservedNamespace`,
    `PublishOutsideGrant`, `InvalidTopic`,
    `InvalidInReplyTo`, `InvalidPayload`); for each, assert
    a `core.lifecycle.publish_rejected` event fires with the
    expected `code` field. Observer subscribed to
    `"core.lifecycle.**"`.

---

## Group 5 — Supervisor Phase A (cheap validation, no resources)

### c14 — feat(rafaello-core::supervisor): scaffolding — `PluginSupervisor` + `SpawnHandle` + `SpawnPaths` + `SupervisorConfig`

- **What.** New module `rafaello_core::supervisor` per scope
  §SP1 + §SP2:
  - `pub struct PluginSupervisor { broker: Broker, config:
    SupervisorConfig, in_flight: Arc<Mutex<HashSet<CanonicalId>>>,
    spawned: Mutex<BTreeMap<CanonicalId, Arc<SpawnedState>>> }`.
  - `pub struct SupervisorConfig { shutdown_grace: Duration
    /* default 200ms */, fittings_max_frame_bytes: usize
    /* default 1 << 20 */ }` + `Default` impl.
  - `pub struct SpawnPaths { project_root: PathBuf,
    private_state_dir: PathBuf }` + `Clone + Debug`.
  - `pub struct SpawnHandle(Arc<SpawnedState>)` cheap-clone
    handle with `canonical()`, `topic_id()`, `child_pid() ->
    Option<u32>`, `peer() -> &PeerHandle` (panics until c19
    wires the peer; the c14 agent picks a placeholder per the
    "build-only types" pattern), `wait() -> Arc<ReaperOutcome>`,
    `try_wait() -> Option<Arc<ReaperOutcome>>`. Wait/try_wait
    return `Arc::new(ReaperOutcome::ReaperPanicked)` until
    c20 wires the real reaper task.
  - `PluginSupervisor::new(broker, config) -> Self`
    (synchronous; does NOT auto-emit boot per scope §B1 +
    pi-5 non-blocking #3).
  - `pub async fn spawn(&self, plan, paths) -> Result<SpawnHandle,
    SpawnError>` — stub returning `unimplemented!()` is
    forbidden (would fail tests); instead return
    `Err(SpawnError::SandboxBuild { canonical: plan.canonical.clone(),
    source: anyhow::anyhow!("supervisor spawn not yet
    implemented — c15+") })`. This compiles; integration
    tests that need a working spawn do not land until c15+.
  - `pub async fn shutdown(self) -> ShutdownReport` — returns
    an empty `ShutdownReport { clean: vec![], forced: vec![],
    failed: vec![] }` (no spawned plugins yet).
  - `Drop` for `PluginSupervisor` is a no-op for now (no
    children to kill); real Drop logic lands in c25.
  - `#[cfg(any(test, feature = "test-fixture"))]
    PluginSupervisor::with_extra_service` constructor stub
    (returns the same as `new` — extra service plumbing is
    c19).
  - `#[cfg(any(test, feature = "test-fixture"))]` `TestHooks`
    struct + `PluginSupervisor::test_hooks()` — counters all
    return 0; real wiring is c14+ via individual SP4 steps.
- **Why.** scope §SP1, §SP2. Establishes the public surface
  so subsequent c15/c16/c17 commits hang Phase A / Phase B
  steps off concrete fields without forward-ref churn.
- **Depends on.** c13.
- **Acceptance.** `tests/supervisor_types_compile.rs` —
  build-only assertion that every public type in §SP1 is
  reachable through `rafaello_core::supervisor`; constructs
  one of each via the stub paths. `tests/supervisor_spawn_unimplemented_returns_sandbox_build.rs`
  — `spawn(plan, paths).await` returns
  `Err(SpawnError::SandboxBuild { .. })` for any plan (the
  c14 placeholder body). This test is **deleted in c15** when
  Phase A starts producing real refusals; the c14 agent
  inlines that future-deletion intent in the commit body so
  c15 doesn't accidentally leave it.

### c15 — feat(rafaello-core::supervisor): Phase A — `in_flight` reservation + topic-id consistency + path validation

- **What.** Implement scope §SP4 Phase A steps 1a, 1b, 2, 3
  inside `PluginSupervisor::spawn`. Replace the c14 stub
  body with these checks (steps 4–7 still return
  `SandboxBuild { source: "Phase A step <N> not yet
  implemented" }` so the function continues to fail safely on
  unimplemented steps). The wiring:
  - **Step 1a** — RAII `InFlightGuard` acquires
    `in_flight.lock().insert(plan.canonical.clone())`; if
    already present → `SpawnError::AlreadyRegistered`.
    Guard's drop removes from the set.
  - **Step 1b** — `broker.try_reserve_registration(&plan.canonical)`
    maps to `SpawnError::NotInAcl` /
    `SpawnError::AlreadyRegistered`.
  - **Step 2** — `broker.plugin_acl(&plan.canonical)` →
    compare ACL `topic_id` against `plan.topic_id`; mismatch
    → `SpawnError::InvalidPlan { reason: TopicIdMismatch {
    expected, got } }`. `None` (defensively) →
    `SpawnError::NotInAcl`.
  - **Step 3** — for each path in `plan.filesystem.{read_paths,
    read_dirs, write_paths, write_dirs, exec_paths, exec_dirs}`
    + `plan.entry_absolute` + `paths.project_root` +
    `paths.private_state_dir`:
    - Assert `path.is_absolute()`; non-absolute →
      `SpawnError::InvalidPlan { reason: NonAbsolutePath {
      kind, path } }` (with the appropriate `PathKind`
      including the new `ProjectRoot` / `PrivateStateDir`
      variants).
    - Assert no ASCII control chars (`as_bytes().iter()
      .all(|b| *b >= 0x20 && *b != 0x7f)`); failure →
      `SpawnError::InvalidPlan { reason: ControlCharsInPath {
      kind, path } }`.
- **Why.** scope §SP4 Phase A steps 1a–3, pi-2 §12 (precheck),
  pi-4 §1, pi-4 §2, pi-5 §4 (in-flight set), pi-6 §3 (use
  `broker.plugin_acl`).
- **Depends on.** c14.
- **Acceptance.** Five new tests, plus deletion of the c14
  `supervisor_spawn_unimplemented_returns_sandbox_build.rs`:
  - `tests/supervisor_spawn_canonical_not_in_acl_refused.rs`
    — hand-build a `CompiledPlugin` with a canonical not in
    the broker's ACL → `SpawnError::NotInAcl`. `TestHooks`
    counters all == 0 (no socketpair/proxy/spawn).
  - `tests/supervisor_spawn_topic_id_mismatch_refused.rs` —
    hand-mutate `CompiledPlugin.topic_id` to a wrong value
    → `SpawnError::InvalidPlan { reason: TopicIdMismatch {
    .. } }`. Counters == 0.
  - `tests/supervisor_spawn_relative_path_refused.rs` —
    hand-mutate `entry_absolute` to a relative path →
    `InvalidPlan { reason: NonAbsolutePath { kind:
    EntryAbsolute, .. } }`. Counters == 0.
  - `tests/supervisor_spawn_relative_spawn_path_refused.rs`
    — `SpawnPaths { project_root: PathBuf::from("rel/path"),
    .. }` → `InvalidPlan { reason: NonAbsolutePath { kind:
    ProjectRoot, .. } }`. Sub-case for `private_state_dir`
    relative.
  - `tests/supervisor_spawn_control_chars_in_path_refused.rs`
    — `entry_absolute` containing `'\n'` → `InvalidPlan {
    reason: ControlCharsInPath { .. } }`.
  - `tests/supervisor_spawn_duplicate_canonical_refused.rs`
    — spawn A successfully (using a fake plan that passes
    Phase A — needs minimal lock/ACL setup that the c15 agent
    constructs via direct `Broker::new` + manual canonical
    registration, since the harness depends on later commits);
    attempt to spawn A again → `SpawnError::AlreadyRegistered`.
    Counters == 0 across the failing call.
  Note: c15 still cannot complete a successful spawn (Phase B
  is c17+), so the duplicate test uses a synthetic
  "in_flight already contains canonical" precondition by
  manually inserting before calling `spawn`.

### c16 — feat(rafaello-core::supervisor): Phase A — reserved env + outpost dry-run + entry-executable + provider refusal

- **What.** Implement scope §SP4 Phase A steps 4, 5, 6, 7
  (the remaining cheap-validation steps):
  - **Step 4** — iterate `plan.env.set` keys + `plan.env.pass`
    entries; if any equals `RFL_BUS_FD` / `RFL_PLUGIN` /
    `RFL_HELPER_FD` / `RFL_PROJECT_ROOT` /
    `RFL_PRIVATE_STATE_DIR` / `RFL_TOPIC_ID` →
    `SpawnError::ReservedEnvInPlan { var }`.
  - **Step 5** — if `plan.network` is `NetworkPlan::Proxy {
    allow_hosts }`, dry-run
    `outpost::NetworkPolicy::from_allowed_hosts(allow_hosts)`;
    on parse failure → `SpawnError::InvalidPlan { reason:
    NetworkAllowHostsInvalid { source } }`.
  - **Step 6** — `std::fs::metadata(&plan.entry_absolute)` (NOT
    `symlink_metadata` — follow symlinks per pi-1 §113);
    `is_file() && permissions().mode() & 0o111 != 0`. On
    failure → `SpawnError::EntryNotExecutable { path }`.
  - **Step 7** — provider refusal:
    `broker.plugin_acl(&plan.canonical)` returning a
    `PluginAcl` with `provider_id.is_some()` →
    `SpawnError::InvalidPlan { reason: ProviderNotInM2 {
    provider_id } }`.
- **Why.** scope §SP4 steps 4–7, pi-2 §11 (provider refusal),
  pi-4 non-blocking #4 (single ReservedEnvInPlan path).
- **Depends on.** c15.
- **Acceptance.** Four new tests:
  - `tests/supervisor_spawn_reserved_env_in_set_refused.rs`
    — plan with `env.set = {"RFL_BUS_FD": "99"}` →
    `SpawnError::ReservedEnvInPlan { var: "RFL_BUS_FD" }`.
  - `tests/supervisor_spawn_reserved_env_in_pass_refused.rs`
    — plan with `env.pass = ["RFL_PLUGIN"]` →
    `SpawnError::ReservedEnvInPlan { var: "RFL_PLUGIN" }`.
  - `tests/supervisor_spawn_reserved_env_helper_refused.rs`
    — plan with `env.set = {"RFL_HELPER_FD": "..."}` →
    `SpawnError::ReservedEnvInPlan { var: "RFL_HELPER_FD" }`
    (defence in depth — m1 c04 already rejects via the
    scrubber, this proves the supervisor catches it too).
  - `tests/supervisor_spawn_entry_not_executable_refused.rs`
    — plan points to a regular file with no exec bit (created
    by the test via `chmod 0644`) → `SpawnError::EntryNotExecutable`.
    Counters == 0.
  - `tests/supervisor_spawn_provider_lock_refused.rs` — lock
    has `bindings.provider = true`, `bindings.provider_id =
    "openai"` → `SpawnError::InvalidPlan { reason:
    ProviderNotInM2 { provider_id: "openai" } }`. Counters
    == 0.
  Each test continues to use the synthetic-plan pattern from
  c15 (no harness yet).

---

## Group 6 — Supervisor Phase B (resource allocation, async spawn)

### c17 — feat(rafaello-core::supervisor): Phase B steps 8–12 — socketpair + proxy + lockin builder + tokio_command + private state

- **What.** Implement scope §SP4 Phase B steps 8–12:
  - **Step 8** — `nix::sys::socketpair(AddressFamily::Unix,
    SockType::Stream, None, SockFlag::SOCK_CLOEXEC)?` →
    `(core_fd, child_fd)` both `OwnedFd`. On failure →
    `SpawnError::Socketpair { source }`. `TestHooks::socketpair_creates`
    increments here.
  - **Step 9** — if `NetworkPlan::Proxy { allow_hosts }`:
    synthesise `outpost::NetworkPolicy::from_allowed_hosts(...)`
    and call `outpost_proxy::start(policy).await` → on failure
    `SpawnError::ProxyStart { source }`. Capture
    `proxy.listen_addr().port()`.
    `TestHooks::outpost_starts` increments here.
  - **Step 10** — build `lockin::Sandbox::builder()`, apply
    `plan.filesystem.*` paths via the consume-and-return
    builder methods (loop with reassignment), apply
    `NetworkPlan` (`Deny`/`AllowAll`/`Proxy(port)`), apply
    `LimitsPlan` (always `max_cpu_time`/`max_open_files`/
    `disable_core_dumps`; conditionally `max_address_space`
    /`max_processes`), then `inherit_fd_as(child_fd,
    RFL_BUS_FD_NUMBER)` (consumes `child_fd`).
  - **Step 11** — `paths.private_state_dir`
    `fs::create_dir_all(...)?` → on failure
    `SpawnError::PrivateStateDirCreate { path, source }`.
  - **Step 12** — `builder.tokio_command(&plan.entry_absolute)?`
    → on failure `SpawnError::SandboxBuild { source }`.
    Returns `lockin::tokio::SandboxedCommand` (NOT sync
    `command(...)`). c17 stops at the command construction;
    env application + spawn + fittings transport land in
    c18.
  - **Step 13 stub** — c17 still cannot spawn; the function
    returns `SpawnError::SandboxBuild { source:
    anyhow::anyhow!("Phase B step 13+ not yet
    implemented") }` AFTER cleaning up the prepared
    command + proxy + fds (call the unwind path the c17 agent
    inlines: drop builder, drop ProxyHandle if any, drop
    core_fd; child_fd was already consumed by step 10).
  - `pub const RFL_BUS_FD_NUMBER: i32 = 3;` exported from the
    module.
- **Why.** scope §SP4 steps 8–12, §SP7 (RFL_BUS_FD_NUMBER
  const), pi-1 §156 (entry check before resource alloc — c16
  already moved it to Phase A), pi-1 §502 (fd 3 convention),
  pi-4 §3 (tokio_command).
- **Depends on.** c16.
- **Acceptance.** Three new integration-shape tests (no real
  spawn yet — they assert the unwind cleanup):
  - `tests/supervisor_spawn_unwinds_after_socketpair.rs` —
    use `TestHooks::socketpair_creates` to verify the
    socketpair was created, then assert the parent's open-fd
    count returns to baseline after `spawn` returns the
    "step 13+ not implemented" error (smoke-checks that
    `core_fd` was dropped by the unwind).
  - `tests/supervisor_spawn_starts_proxy_for_proxy_plan.rs`
    — plan with `NetworkPlan::Proxy { allow_hosts:
    ["example.com"] }`, spawn → `outpost_starts` counter
    increments by 1 then proxy is dropped (port no longer
    bound after spawn returns).
  - `tests/supervisor_spawn_skips_proxy_for_deny_plan.rs` —
    plan with `NetworkPlan::Deny`, spawn → `outpost_starts`
    counter is 0.
  c16's synthetic-plan helper is extended for these tests to
  cover network plans + a real `entry_absolute` pointing at
  the c03 fixture binary path (`env!("CARGO_BIN_EXE_rfl-bus-fixture")`
  — exists, executable, but spawn doesn't actually invoke it
  yet).

### c18 — feat(rafaello-core::supervisor): Phase B step 12 env application — env_clear + pass + set + proxy_env + reserved RFL_*

- **What.** After `tokio_command` returns the
  `SandboxedCommand` (c17), apply env per scope §SP4 step 12
  (a)–(f):
  - `cmd.env_clear()`. Document the no-tmpdir-restoration
    deviation per scope §SP4 (lockin doesn't expose private_tmp
    pre-spawn; not an ABI guarantee).
  - For each `key in &plan.env.pass`: if
    `std::env::var_os(key).is_some()`, `cmd.env(key, val)`.
  - For each `(k, v) in &plan.env.set`: `cmd.env(k, v)`.
  - If `NetworkPlan::Proxy`: inject 8 proxy env vars
    (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` + lowercase, all
    set to `format!("http://127.0.0.1:{}", proxy_port)`;
    `NO_PROXY` + `no_proxy` set to empty string).
  - `cmd.env("RFL_BUS_FD", RFL_BUS_FD_NUMBER.to_string())`.
  - `cmd.env("RFL_PLUGIN", plan.canonical.to_string())`.
  - `cmd.env("RFL_PROJECT_ROOT", &paths.project_root)`.
  - `cmd.env("RFL_PRIVATE_STATE_DIR", &paths.private_state_dir)`.
  - `cmd.env("RFL_TOPIC_ID", &plan.topic_id)`.
  - `cmd.current_dir(&paths.project_root)`.
  c18 still cannot complete the spawn; after env application
  it drops the prepared command and returns the "Phase B step
  13+ not yet implemented" error. The unwind contract from
  c17 still applies.
- **Why.** scope §SP4 step 12, pi-2 §6 (RFL_TOPIC_ID), pi-4
  §5 (lockin private-tmp deviation), pi-5 §6 (lowercase
  proxy keys).
- **Depends on.** c17.
- **Acceptance.** No new tests in this commit (env application
  is unobservable until the fixture spawns + the dump_env
  service exists). The c17 tests still pass. A trivial unit
  test `tests/supervisor_env_application_compiles.rs` (or
  inlined under `#[cfg(test)]` in the supervisor module)
  asserts the env-application function builds; behaviour is
  exercised end-to-end by c26 (`supervisor_proxy_starts_and_env_injected.rs`)
  + c19 (`supervisor_env_pass_set_applied.rs` once the
  fixture is wired in c19+).

### c19 — feat(rafaello-core::supervisor): Phase B steps 13–17 — spawn + fittings transport + Server + bus.publish service + broker register

- **What.** Implement scope §SP4 Phase B steps 13–17:
  - **Step 13** — `cmd.spawn()` → `lockin::tokio::SandboxedChild`
    on failure `SpawnError::Spawn { source }`.
    `TestHooks::child_spawns` increments. After this point
    every error must SIGKILL + reap (per the post-spawn
    contract — pi-5 §5).
  - **Step 14** — convert `core_fd` to
    `tokio::net::UnixStream::from_std(...)` (unwind on
    `set_nonblocking` / `from_std` failure with
    `SpawnError::TransportSetup { source }`, SIGKILL the
    child + reap before returning).
  - **Step 15** — split into `(reader, writer)`; build
    `fittings_transport::stdio::StdioTransport::new(reader,
    writer, config.fittings_max_frame_bytes)`.
  - **Step 16** — build the per-connection `Service`:
    `BusPublishService { broker: Broker, canonical:
    CanonicalId }` implementing `fittings_core::Service`
    with one `bus.publish` notification handler that calls
    `broker.handle_plugin_publish(&canonical, &raw_params)`,
    converting any `BrokerError` into a logged event (the
    rejection is already published via §B9; the service
    returns `Ok(...)` so the transport stays alive — pi-1
    §1051). Other methods return `MethodNotFound` per
    fittings default. Test mode (`with_extra_service`):
    compose with the factory's service via a router.
  - **Step 17** — `Server::new(service, transport)`; capture
    `peer = server.peer()`; call
    `broker.register_plugin(plan.canonical.clone(),
    peer.clone())` mapping `BrokerError::NotInAcl(c)` →
    `SpawnError::NotInAcl(c)` and
    `BrokerError::AlreadyRegistered(c)` →
    `SpawnError::AlreadyRegistered(c)` (other variants
    unreachable per scope §B1). On success: drop the
    `InFlightGuard` from c15 step 1a immediately.
  - Steps 18–20 still stubbed; the function returns the
    "Phase B step 18+ not yet implemented" error AFTER
    SIGKILL + reap of the child + drop of `RegisteredPlugin`
    + drop of `ProxyHandle`.
- **Why.** scope §SP4 steps 13–17, §SP2 (extra service),
  pi-3 §4 (fittings paths), pi-5 §5 (reap on unwind), pi-6
  §3 (plugin_acl as canonical lookup), pi-7 non-blocking #2
  (broker error mapping).
- **Depends on.** c18.
- **Acceptance.** No new headline tests (still no working
  spawn). Two cleanup tests:
  - `tests/supervisor_spawn_unwinds_after_register.rs` —
    `TestHooks` counters increment by exactly 1 each
    (socketpair, proxy if applicable, child spawn); after
    spawn returns the error, the broker no longer has the
    canonical registered (verified via
    `broker.contains_plugin(...)` returning true but
    `try_reserve_registration` returning `NotInAcl` —
    actually `Ok(())` since the registration was rolled
    back); the in_flight set no longer contains the
    canonical.
  - `tests/supervisor_spawn_post_register_reaps_child.rs` —
    fixture exit observable via the OS (poll
    `/proc/<pid>/status` or use a side-channel); after
    spawn returns the "Phase B step 18+" error, the child
    is no longer alive within a bounded wait (≤ 200ms).
    Linux-only via `#[cfg(target_os = "linux")]` (per the
    platform-gating §"Platform gating" rule — this one
    needs `/proc`; macOS CI catches it).

### c20 — feat(rafaello-core::supervisor): Phase B steps 18–20 — reaper + watcher + serve loop + handle return

- **What.** Implement scope §SP4 Phase B steps 18–20:
  - **Step 18** — spawn the reaper task: `tokio::spawn(async
    move { let outcome = match child.wait().await { Ok(s) =>
    ReaperOutcome::Exited(s), Err(e) =>
    ReaperOutcome::WaitFailed(e) }; let _ =
    watch_tx.send(Some(Arc::new(outcome))); })`. The reaper
    owns the `lockin::tokio::SandboxedChild` for its
    lifetime. `watch_tx` writes into a
    `tokio::sync::watch::Sender<Option<Arc<ReaperOutcome>>>`
    inside the `SpawnedState`.
  - **Spawn the watcher task** (per scope §SP4 step 18 + pi-5
    §1): `tokio::spawn(async move { if let Err(_join_err) =
    reaper_handle.await { let _ =
    watch_tx_clone.send(Some(Arc::new(ReaperOutcome::ReaperPanicked))); } })`.
    Both join handles stored in `SpawnedState` for shutdown
    abort.
  - **Step 19** — `tokio::spawn(server.serve())`; store its
    `JoinHandle` in `SpawnedState` for shutdown.
  - **Step 20** — wrap `SpawnedState` in `Arc`, insert into
    `supervisor.spawned`, return
    `Ok(SpawnHandle(Arc::clone(&state)))`. Drop the
    `InFlightGuard` was already done at c19 step 17; verify
    here.
  - **`SpawnHandle::wait()` / `try_wait()`** wired to the
    watch channel: `wait` awaits the watch transition from
    `None` to `Some` and returns the `Arc<ReaperOutcome>`;
    `try_wait` returns `Some(Arc<...>)` if the watch already
    has a value, else `None`. Late callers see the cached
    `Arc` immediately.
  - **`SpawnHandle::child_pid()`** returns `Option<u32>` from
    the stored `tokio::process::Child::id()` (which the
    `lockin::tokio::SandboxedChild` exposes via `as_child()`
    or `into_parts()` — the c20 agent picks the exact API
    path).
- **Why.** scope §SP4 steps 18–20, §SP1 (`SpawnHandle::wait`
  returns `Arc<ReaperOutcome>`), pi-5 §1 (watcher catches
  reaper panic), pi-6 §2 (cached cloneable outcome).
- **Depends on.** c19.
- **Acceptance.** **The headline test lands here.** Two new
  tests:
  - `tests/supervisor_spawn_fixture_round_trip.rs` — spawn
    the c03 fixture in `respond_peer_call` mode (this test
    requires c21+'s fixture work; the c20 agent stubs the
    fixture call by using a no-op fixture mode that just
    sleeps until SIGKILL). Verify `spawn(plan, paths).await`
    returns `Ok(SpawnHandle)`; `handle.canonical()` matches
    the plan; `handle.try_wait()` returns `None` initially;
    after sending SIGKILL via `nix`, `handle.wait().await`
    resolves to `Arc<ReaperOutcome::Exited(s)>` with
    `s.signal() == Some(9)` or similar Linux-specific check
    (macOS-CI catches semantic differences).
    
    **Note for the c20 agent:** if the fixture-mode work
    isn't yet ready (c21 is later), use a minimal sleeper
    binary the c20 commit creates as a transient fixture
    OR mark this test `#[ignore]` and the c21 commit
    un-ignores it. Inline the chosen approach.
  - `tests/supervisor_spawn_handle_clone_observes_same_outcome.rs`
    — clone the `SpawnHandle`; both `wait().await` calls
    return the same `Arc<ReaperOutcome>` (pointer equality
    via `Arc::ptr_eq` or value equality on the variant).

---

## Group 7 — Fixture binary + test harness

### c21 — feat(rfl-bus-fixture): universal init + readiness handshake + `OneShotConnector` + transport

- **What.** Replace c03's empty `main` with the universal
  fixture init per scope §F2 + §F3:
  - Parse `RFL_BUS_FD` from env as `RawFd`; if missing or
    invalid → `eprintln!` + exit 3.
  - Convert `OwnedFd::from_raw_fd(fd)` (in `unsafe` block,
    exactly once); wrap as
    `std::os::unix::net::UnixStream::from(...)`,
    `set_nonblocking(true)`, then
    `tokio::net::UnixStream::from_std(...)`.
  - Define a tiny `OneShotConnector { transport:
    Mutex<Option<StdioTransport>> }` implementing
    `fittings_core::transport::Connector` (mirrors fittings'
    own client tests at
    `fittings/crates/client/src/lib.rs:623`); `Connector::connect`
    takes the transport via `Option::take` (panics on second
    call).
  - Build `StdioTransport::new(reader, writer, 1 << 20)`,
    then `Client::connect(OneShotConnector::new(transport))
    .await?`.
  - **`RFL_FIXTURE_MODE` dispatch** — read env, dispatch to
    one of: `publish_one`, `publish_with_taint`,
    `publish_full_params`, `publish_bad_namespace`,
    `publish_bad_grammar`, `publish_outside_grant`,
    `publish_bad_in_reply_to_missing`,
    `publish_bad_in_reply_to_empty`,
    `publish_bad_in_reply_to_multiple`, `respond_peer_call`,
    `call_core_then_exit`, `observer`. c21 implements ONLY
    the universal init + the `respond_peer_call` mode (the
    minimal mode needed for c20's headline test); other
    modes land in c22.
  - **Universal readiness signal** — after `Client::connect`
    + `with_service` + `with_notification_handler` are all
    installed, call `client.call("core.fixture.ready",
    json!({"mode": "<mode>"})).await` exactly once.
  - **`respond_peer_call` mode** — register a
    `fittings_core::Service` impl handling
    `core.fixture.start` → empty response (just an ack),
    `core.fixture.echo` → echoes params. Sleep until SIGTERM
    (holds open by default for service modes per scope §F2).
  - No tracing init (per scope §F4 — keep fixture output
    clean).
- **Why.** scope §F1, §F2, §F3, §F4, §F5 (RFL_BUS_FD parsing
  per §F3), pi-2 §7 (OneShotConnector pattern), pi-3 §4
  (fittings paths), pi-4 §5 (readiness handshake), pi-4 §6
  (fixture flush — flush ack lands in c22 with publish
  modes).
- **Depends on.** c03, c20 (for the harness side that
  spawns the fixture).
- **Acceptance.** Two new tests (these consume the supervisor
  + the harness — but the harness is not yet in c23, so c21
  uses a thin inline harness inside the test files):
  - `tests/fixture_respond_peer_call_echo.rs` — spawn the
    fixture in `respond_peer_call` mode via
    `PluginSupervisor::spawn`; harness extra service
    registers `core.fixture.ready` (records the signal)
    and `core.fixture.start` (no-op). After ready signal +
    start ack, harness calls
    `spawn_handle.peer().call("core.fixture.echo",
    json!({"x":1})).await` → response `{"x":1}`.
  - `tests/fixture_ready_signal_arrives.rs` — spawn the
    fixture; assert `core.fixture.ready` is received by
    the harness extra service before any other fixture
    behaviour.

### c22 — feat(rfl-bus-fixture): publish-and-exit modes + `call_core_then_exit` + `observer` mode

- **What.** Add the remaining `RFL_FIXTURE_MODE` dispatch
  arms per scope §F2:
  - **Publish-and-exit modes** (`publish_one`,
    `publish_with_taint`, `publish_full_params`,
    `publish_bad_namespace`, `publish_bad_grammar`,
    `publish_outside_grant`,
    `publish_bad_in_reply_to_{missing,empty,multiple}`):
    - Wait for `core.fixture.start`.
    - Build the `bus.publish` params per the mode (`topic`
      from `RFL_FIXTURE_TOPIC`, `payload` from
      `RFL_FIXTURE_PAYLOAD_JSON`, optionally `taint` from
      `RFL_FIXTURE_TAINT_JSON`, or full verbatim params
      from `RFL_FIXTURE_FULL_PARAMS_JSON`).
    - `client.notify("bus.publish", params).await` (NOT
      `client.peer().notify(...)` — pi-5 §2: `Client::notify`
      shares the `ClientCommand` FIFO with `Client::call`).
    - `client.call("core.fixture.after_publish",
      Value::Null).await` — flush ack via the same FIFO.
    - Exit 0.
  - **`call_core_then_exit` mode** — wait for
    `core.fixture.start`; call
    `client.peer().call("core.fixture.ping", json!({"n":
    42})).await`; exit 0 on response, exit 2 on error.
  - **`observer` mode** — register
    `with_notification_handler` per scope §H4 (synchronous
    closure that clones an outbound `PeerHandle` and
    `tokio::spawn`s the forwarding `peer.call(
    "core.fixture.observed", event_value).await`). After
    the universal ready signal + `core.fixture.start` ack,
    sleep until SIGTERM (holds open).
  - Other supporting `respond_peer_call` methods to add
    (extends c21's set):
    - `core.fixture.dump_env` → returns `{"env":
      {"<key>": "<value>", ...}}` over `RFL_FIXTURE_ENV_KEYS`
      (comma-separated) keys.
    - `core.fixture.write_private_state` → writes
      `{"marker": <random>}` to
      `<RFL_PRIVATE_STATE_DIR>/marker`, returns `{"wrote":
      <abs-path>}`.
    - `core.fixture.report_open_result` → attempts
      `std::fs::read(RFL_FIXTURE_OPEN_PATH)`; returns `{"ok":
      true}` on success or `{"ok": false, "errno": <int>}`
      on failure.
    - `core.fixture.try_write_path` → attempts
      `std::fs::write(RFL_FIXTURE_WRITE_PATH, b"x")`; returns
      success/errno result.
- **Why.** scope §F2 (full mode set), §H4 (observer
  notification handler), pi-4 §5 + §6 (readiness + flush),
  pi-4 §7 (taint + full-params modes), pi-5 §2 (Client FIFO).
- **Depends on.** c21.
- **Acceptance.** Three new fixture-internal tests (under
  `#[cfg(feature = "test-fixture")]`):
  - `tests/fixture_publish_one_emits_event.rs` — spawn
    fixture in `publish_one` mode + observer mode (two
    fixtures); after the harness's
    publisher-start handshake, observer's harness-side
    extra service receives one `bus.event` matching the
    publisher's `RFL_FIXTURE_TOPIC`/`PAYLOAD_JSON`.
  - `tests/fixture_call_core_then_exit_completes.rs` —
    spawn fixture in `call_core_then_exit`; harness extra
    service registers `core.fixture.ping` echoing `{"n":
    43}`; fixture exits 0 within bounded wait.
  - `tests/fixture_dump_env_returns_allow_listed_keys.rs`
    — spawn fixture in `respond_peer_call`; harness calls
    `core.fixture.dump_env` with
    `RFL_FIXTURE_ENV_KEYS=RFL_BUS_FD,RFL_PLUGIN`; response
    contains both keys with expected values
    (`RFL_BUS_FD == "3"`, `RFL_PLUGIN ==
    plan.canonical.to_string()`).

### c23 — test(rafaello-core): m2 harness — `FixtureLockBuilder` + `Spawn` + `Observer` helpers

- **What.** New file
  `rafaello/crates/rafaello-core/tests/common/m2_harness.rs`
  per scope §H1–§H5. Re-exported as `mod common; use
  common::m2_harness as h;` from every supervisor integration
  test file.
  - `FixtureLockBuilder` — creates a `tempfile::TempDir`
    project root, materialises a `<project>/plugins/<name>/`
    plugin_dir containing a copy of the
    `env!("CARGO_BIN_EXE_rfl-bus-fixture")` binary at
    `bin/fixture` (preserving exec bit), a minimal
    `rafaello.toml`, and a canonical "no-op" `openrpc.json`
    sibling shipped at
    `tests/common/empty_openrpc.json`. Programmatically
    constructs the `Lock` value with proper `granted_capabilities`,
    `bindings`, and `digest` fields via m1's actual API:
    `digest::content_digest`, `digest::manifest_digest`,
    `RecomputedDigests`. Runs `validate::lock(&lock,
    &path_context)` then `compile::compile_plugin` for each
    entry + `broker_acl::compile(&lock)`.
  - `Spawn` helper — takes `BrokerAcl`,
    `Vec<CompiledPlugin>`, and a per-canonical
    `Vec<ExtraService>` map; constructs `Broker::new(acl)?`,
    then `PluginSupervisor::with_extra_service(broker.clone(),
    config, factory)`; for each plan, computes `paths =
    SpawnPaths { project_root, private_state_dir:
    project_root.join(".rafaello-plugin-data").join(&plan.topic_id)
    }`, calls `supervisor.spawn(plan, &paths).await`, returns
    `(Broker, PluginSupervisor, Vec<SpawnHandle>)`.
  - `Observer` helper — spawns one fixture in `observer`
    mode + the `core.fixture.observed`-pushing extra
    service that drains into a
    `tokio::sync::mpsc::UnboundedReceiver` returned to the
    test. `Observer::watch_all` builds the multi-namespace
    grant (`core.**`, `plugin.**`, `provider.**`,
    `frontend.**`).
  - `ReadinessGate` helper — wraps the harness side of the
    `core.fixture.ready` extra service; tests await the
    gate before issuing `core.fixture.start`.
  - The harness IS test code; it does not leak into
    `rafaello-core`'s public API.
- **Why.** scope §H1–§H5, pi-3 non-blocking #1 (real
  digest helpers), pi-4 §5 (two-phase readiness handshake).
- **Depends on.** c22.
- **Acceptance.** The c21 + c22 fixture tests are
  refactored to use the harness (their inline plumbing
  collapses). New test
  `tests/harness_lock_builder_round_trip.rs` — builds a
  one-plugin lock via `FixtureLockBuilder`, asserts that
  `compile_plugin` accepts it (returns `Ok(CompiledPlugin)`),
  asserts `validate::lock` accepts it (returns `Ok`).

---

## Group 8 — Supervisor lifecycle: shutdown + Drop + duplicate

### c24 — feat(rafaello-core::supervisor): cooperative `shutdown(self)` — SIGTERM + grace + SIGKILL + `ShutdownReport`

- **What.** Implement `PluginSupervisor::shutdown(self).await
  -> ShutdownReport` per scope §SP1 + §SP5:
  - For each `(canonical, state)` in
    `self.spawned.lock().drain()`:
    - Drop the `RegisteredPlugin` guard (broker fan-out
      stops immediately).
    - `nix::sys::signal::kill(Pid::from_raw(pid as i32),
      Signal::SIGTERM)` — graceful path. On `Errno::ESRCH`
      treat as already exited.
    - Wait up to `config.shutdown_grace` for the watch
      channel to transition to `Some(...)`. On timeout:
      SIGKILL via the same nix call.
    - Drop the `ProxyHandle` (after the child is dead per
      scope §SP5 ordering).
    - Abort the serve-loop `JoinHandle`.
    - Build `ShutdownFailure` per the cached
      `Arc<ReaperOutcome>`: `WaitFailed { kind, message }`
      from `ReaperOutcome::WaitFailed(io_err)`,
      `ReaperPanicked` from
      `ReaperOutcome::ReaperPanicked`, or no failure for
      `Exited(_)` (push to `report.clean` or
      `report.forced` depending on whether SIGKILL was
      needed).
  - Return the `ShutdownReport`.
- **Why.** scope §SP1 (`shutdown(self)` signature), §SP5
  (lifecycle ordering), pi-2 §13 (per-plugin partial
  failures via report), pi-6 §2 (shareable wait error).
- **Depends on.** c23.
- **Acceptance.** Two new tests:
  - `tests/supervisor_shutdown_clean.rs` — spawn fixture in
    `respond_peer_call` (handles SIGTERM by exiting); call
    `supervisor.shutdown().await`; `report.clean`
    contains the canonical; `report.forced` is empty;
    bounded wait < 200ms.
  - `tests/supervisor_shutdown_forced.rs` — spawn fixture
    in a mode that ignores SIGTERM (the c24 agent adds a
    `RFL_FIXTURE_TRAP_SIGTERM=1` env knob to the fixture's
    universal init that installs a SIGTERM handler doing
    nothing — minor extension to c21, the c24 agent inlines
    the change in the commit body); call `shutdown` with
    `shutdown_grace = 50ms`; `report.forced` contains the
    canonical (SIGKILL fired after grace); bounded wait <
    `shutdown_grace * 2`.

### c25 — feat(rafaello-core::supervisor): `Drop` for `PluginSupervisor` — best-effort SIGKILL + reaper handoff

- **What.** Implement `Drop` for `PluginSupervisor` per
  scope §SP5 + pi-6 non-blocking #4:
  - Synchronous best-effort: for each
    `(canonical, state)` in `self.spawned.get_mut()`,
    `nix::sys::signal::kill(Pid::from_raw(pid), SIGKILL)`
    (ignoring ESRCH); abort the serve-loop join handle;
    drop the `RegisteredPlugin` guard.
  - The reaper task (still running, owns the
    `lockin::tokio::SandboxedChild`) continues until the
    OS marks the child dead and publishes the outcome to
    its watch — outstanding `SpawnHandle` clones can still
    observe it. The supervisor does not block waiting (Drop
    cannot await).
- **Why.** scope §SP5 (Drop best-effort), pi-1 §28 (no async
  in Drop), pi-6 non-blocking #4 (don't claim "OS reap" —
  rely on the reaper task).
- **Depends on.** c24.
- **Acceptance.** `tests/supervisor_drop_kills_managed_children.rs`
  — spawn two fixtures in `respond_peer_call` (hold open);
  hold a clone of one `SpawnHandle`; drop the supervisor;
  bounded wait — both fixtures exit (via SIGKILL); the held
  `SpawnHandle::wait().await` resolves to
  `Arc<ReaperOutcome::Exited(s)>` with signal indicating
  SIGKILL.

---

## Group 9 — Proxy + lockin denial proofs

### c26 — feat(rafaello-core::supervisor): proxy startup + env injection end-to-end test

- **What.** No new code (the proxy startup + env injection
  paths are c17 + c18). c26 lands the integration test that
  exercises both end-to-end with a real fixture spawn.
- **Why.** scope §SP4 (proxy + env tested together — the
  positive matrix).
- **Depends on.** c25.
- **Acceptance.** `tests/supervisor_proxy_starts_and_env_injected.rs`
  — fixture A in `respond_peer_call` mode with
  `NetworkPlan::Proxy { allow_hosts: ["example.com"] }`;
  harness calls `core.fixture.dump_env` with
  `RFL_FIXTURE_ENV_KEYS=HTTP_PROXY,HTTPS_PROXY,ALL_PROXY,NO_PROXY,http_proxy,https_proxy,all_proxy,no_proxy,RFL_BUS_FD`.
  Assertions: every uppercase + lowercase `*_PROXY` env is
  `http://127.0.0.1:<port>`; both `NO_PROXY` and `no_proxy`
  are `""`; `RFL_BUS_FD == "3"`. Proxy port non-zero.
  TestHooks::outpost_starts == 1.

### c27 — test(rafaello-core): lockin denial proofs (read + write outside grant)

- **What.** No new code (the lockin grants are passed
  through unchanged from `CompiledPlugin.filesystem` per
  c17). c27 lands the two integration tests.
- **Why.** scope §"Demo bar" negatives, pi-1 §6.4–§6.5
  (kernel-denial verification).
- **Depends on.** c26.
- **Acceptance.** Two tests:
  - `tests/supervisor_lockin_denies_outside_grant_read.rs`
    — fixture A in `respond_peer_call` with grant
    `read_dirs = [<project-root>]` only;
    `RFL_FIXTURE_OPEN_PATH=/etc/hosts`. Harness calls
    `core.fixture.report_open_result`; asserts `ok ==
    false`, `errno` matches one of `EPERM` / `EACCES` /
    `ENOENT` (`matches!` not `==`).
  - `tests/supervisor_lockin_denies_outside_grant_write.rs`
    — A's `write_dirs = [<private-state>]` only;
    `RFL_FIXTURE_WRITE_PATH=<project-root>/forbidden`.
    Harness calls `core.fixture.try_write_path`; asserts
    denial; verifies `<project-root>/forbidden` does not
    exist after.

---

## Group 10 — Cross-plugin scenarios + private-state + manual validation

### c28 — test(rafaello-core): cross-plugin round-trip + private-state + taint round-trip

- **What.** Three integration tests covering the
  cross-plugin and round-trip fan-out paths.
- **Why.** scope §"Demo bar" positives.
- **Depends on.** c27.
- **Acceptance.** Three tests:
  - `tests/supervisor_bus_publish_round_trip_two_plugins.rs`
    — A in `publish_one` mode publishing
    `"plugin.<A>.greet"`; B with subscribe grant on
    `"plugin.<A>.greet"` — observer-style B forwards via
    `core.fixture.observed`. After two-phase readiness
    handshake + `core.fixture.start` to A, B receives one
    event matching the topic + payload. A's
    `publish_with_taint` variant also covered.
  - `tests/supervisor_private_state_dir_writable.rs` —
    A in `respond_peer_call`; harness calls
    `core.fixture.write_private_state`; fixture writes to
    `<RFL_PRIVATE_STATE_DIR>/marker`; test verifies the
    file exists and the path matches
    `project_root.join(".rafaello-plugin-data").join(plan.topic_id).join("marker")`
    (path uses topic-id form per `decisions.md` row 37).
  - `tests/supervisor_taint_round_trip.rs` — A in
    `publish_with_taint` mode with
    `RFL_FIXTURE_TAINT_JSON=[{"source":"test","detail":"x"}]`;
    observer receives event; `event.taint` round-trips
    byte-equal.

### c29 — docs(rafaello-m2): write `manual-validation.md` capturing m2 demo bar evidence

- **What.** Create
  `rafaello/plans/milestones/m2-broker-spawn/manual-validation.md`
  per scope §"Manual validation in `manual-validation.md`":
  - Capture of `nix develop --impure --command cargo test
    --manifest-path rafaello/Cargo.toml -p rafaello-core
    --features test-fixture` output (last 20 lines:
    `<n> tests passed; 0 failed`).
  - Capture of `RUST_LOG=rafaello_core=debug nix develop
    --impure --command cargo test --manifest-path
    rafaello/Cargo.toml -p rafaello-core --features
    test-fixture --test supervisor_spawn_fixture_round_trip
    -- --nocapture` showing broker registration trace +
    fan-out traces.
  - `find rafaello/crates/rafaello-core/src -name '*.rs' |
    sort` listing the new modules (`bus.rs`,
    `bus/publish_msg.rs` — or wherever c06/c07/c12 placed
    them, the c29 agent captures the actual layout —
    `supervisor.rs`, `supervisor/lifecycle.rs`,
    `bin/rfl_bus_fixture.rs`).
  - `ls /proc/<fixture-pid>/fd/` snapshot during a
    `respond_peer_call` fixture run with the documented
    invariants (fd 3 is `socket:[...]`; parent does not
    have a duplicate of the same socket inode).
  - `nix develop --impure --command cargo doc
    --manifest-path rafaello/Cargo.toml -p rafaello-core
    --no-deps` warning-free output.
  - `nix develop --impure --command cargo build
    --manifest-path rafaello/Cargo.toml -p rafaello-core
    --features test-fixture --bin rfl-bus-fixture` green
    output.
  - macOS CI link / job result reference (the c29 agent
    pushes the `rafaello-v0.1` branch to origin during
    this commit and includes the resulting CI URL +
    summary).
- **Why.** scope §"Manual validation", §"Acceptance summary"
  (manual-validation.md required for milestone close).
- **Depends on.** c28.
- **Acceptance.** `manual-validation.md` exists, contains
  every section above. The driver verifies before merging
  that all captures are recent (today's date) and reflect
  the actual `rafaello-v0.1` HEAD, not stale runs from
  earlier commits.

---

## What changed from prior drafts

This is the round-1 draft of `commits.md`. Pi review pending.
