# m4 — provider fixture + secure agent loop + read-only tool + taint envelope — scope

> **Status:** round-1 — initial draft. Pi review pending.

## Goal

Land the **first end-to-end agent loop**: a `rfl chat` invocation
against a bundled deterministic **mock provider plugin** can answer
"what's in README.md" by emitting a `read-file` tool call, having
core dispatch it to a bundled **read-file tool plugin**, ferrying
the result back to the provider, and rendering the assistant reply
in the m3 TUI. m4 is the structural moment where m1's manifest /
lock / compiler, m2's broker / supervisor, and m3's TUI / session
machinery compose into the canonical 5-step tool-dispatch path
that overview §7 describes. Every later milestone (m5 sinks +
confirmation, m6 the real OpenAI-compatible provider) inherits
m4's provider supervision + agent loop + canonical taint envelope
without modification.

The deliverable is:

1. **Provider plugin supervision** via the existing
   `rafaello_core::supervisor::PluginSupervisor` (m2). m2's row-39
   refusal (`SpawnError::InvalidPlan { reason:
   InvalidPlanReason::ProviderNotInM2 }`) is removed; entries with
   `bindings.provider = true` now spawn through the same path as
   any other plugin. The supervisor wires the
   `Publisher::Provider { provider_id, topic_id }` (new — row 42
   follow-through) into broker registration and injects two new
   env vars (`RFL_PROVIDER_ID`, `RFL_PROVIDER_ACTIVE`) so the
   provider child knows its identity. No new supervisor type is
   introduced (see "Lock-correspondence claim, extended" below).
2. **Broker extension** in `rafaello_core::bus` /
   `rafaello_core::broker_acl` / `rafaello_core::error`:
   - `BusEvent.request_id: Option<JsonRpcId>` lands as a
     first-class envelope field (overview §4.5: "m2 omits this
     field; m4 adds it").
   - `Publisher::Provider { provider_id: String, topic_id:
     TopicId }` variant (row 42 — m2 staged the reshape, m4
     adds the third arm).
   - `BrokerAcl` gains provider registration: `register_provider`
     / `handle_provider_publish` symmetric to the plugin path;
     `try_reserve_provider_registration`; provider publish
     authority gated by the `provider.<provider-id>.*`
     namespace.
   - `BrokerError` grows `ProviderNotInAcl`,
     `ProviderAlreadyRegistered`, `ProviderNotRegistered` and the
     `Publisher::Provider` arm flows through the existing
     `PublishOutsideGrant`, `UnknownNamespace`,
     `PublishOnReservedNamespace`, `InvalidInReplyTo`,
     `InvalidPayload`, `InvalidTopic` variants.
   - **`taint` envelope enforcement** on `core.session.tool_*`:
     - presence + structural validation: every event on
       `core.session.tool_request` and `core.session.tool_result`
       must carry a non-empty `taint: [{source, detail?}, …]`
       with source ∈ `{"user", "provider", "tool", "system"}`
       (overview §4.5; security RFC §7.2.1–7.2.2);
     - **plugin-supplied taint on `core.*` is rejected**: only
       core may write a `core.session.tool_*` event, so taint
       arriving in the plugin's `bus.publish` payload on its own
       namespace is **carried into the re-emit** by core only
       after validation (it may be empty or absent on the
       `provider.<id>.*` / `plugin.<id>.*` side; core synthesises
       the canonical envelope).
   - **`in_reply_to` enforcement** extends m2's `tool_result` /
     `rpc_reply` rule to `provider.<id>.tool_request`,
     `provider.<id>.assistant_message`, and (existing) `tool_result`
     on both `plugin.*` and `core.*` (security RFC §7.2.6 table).
   - New `core.session.*` topics are **introduced as wire-active
     in m4**: `core.session.tool_request`,
     `core.session.tool_result`, `core.session.assistant_message`,
     `core.session.user_message`. None of these existed in m3
     (m3 only finalised `core.session.entry.finalized`).
3. **Frontend ACL extension** (m3 retrospective §2.10 handover):
   the `rfl chat` `BrokerAcl` construction in `rafaello::run_chat`
   extends the `tui` `FrontendAcl.publish_topics` to include
   `frontend.tui.user_message`; `handle_frontend_publish` exercises
   the existing m3 namespace machinery, and core re-emits the
   validated `frontend.tui.user_message` as
   `core.session.user_message`.
4. **Core re-emit pipeline** in a new
   `rafaello_core::reemit` module:
   - subscribes to `provider.<active-provider-id>.tool_request`,
     `provider.<active-provider-id>.assistant_message`, and
     `plugin.<topic-id>.tool_result` (the symmetric inbound path);
   - validates the inbound event (`in_reply_to`, payload schema);
   - synthesises the canonical `taint` envelope from the
     publishing principal's identity (`{source: "provider",
     detail: provider_id}` for provider re-emits;
     `{source: "tool", detail: canonical_id}` for tool-result
     re-emits — security RFC §7.2.1);
   - re-emits as `core.session.tool_request` /
     `core.session.tool_result` / `core.session.assistant_message`
     via `Broker::publish_core_with_taint` (new sibling of
     existing `publish_core`).
5. **Agent loop module** `rafaello_core::agent`: a
   `AgentLoop` task subscribes to `core.session.user_message`,
   `core.session.assistant_message`, and the re-emitted
   `core.session.tool_request`; routes tool requests through the
   tool-routing table from `BrokerAcl.tool_routes` (m1's compiled
   tool→canonical map, already present); publishes
   `plugin.<topic-id>.tool_request` to the bound tool plugin; and
   forwards `core.session.tool_result` to the active provider via
   the existing fan-out path (provider's `subscribe_patterns`
   include `core.session.tool_result`).
6. **`rafaello-mockprovider` crate** (`crates/rafaello-mockprovider`)
   with bin target `rfl-mockprovider`: a deterministic subprocess
   plugin whose manifest declares `provides.provider = "mock"`,
   subscribes to `core.session.user_message` and
   `core.session.tool_result`, publishes on `provider.mock.*`. The
   plugin is content-pattern driven: a `core.session.user_message`
   matching `/what(?:'s| is) in (?<path>\S+)/` emits a
   `provider.mock.tool_request` for `read-file` with
   `{path: "<path>"}`; on receiving the corresponding
   `core.session.tool_result`, it emits an
   `provider.mock.assistant_message` whose payload echoes the file
   contents with a "Here's what's in <path>:\n…" prefix; any other
   message emits a single `provider.mock.assistant_message`
   echoing the input ("echo: <message>"). No network egress.
7. **`rafaello-readfile` crate** (`crates/rafaello-readfile`) with
   bin target `rfl-readfile`: a subprocess tool plugin whose
   manifest declares `provides.tools = ["read-file"]` with
   `sinks = []`. Subscribes to its own
   `plugin.<topic-id>.tool_request` topic (m1's compiler-inserted
   auto-subscribe per `broker_acl.rs:98`); publishes on
   `plugin.<topic-id>.tool_result`. Reads the requested path from
   a grant of `read_dirs = [PROJECT_ROOT]`, returns
   `{ok: true, content: <utf8>}` on success or
   `{ok: false, error: <reason>}` on `NotFound` / `PermissionDenied`
   / `NotUtf8`.
8. **Tool dispatch wiring** on the core side: m1 already compiles
   `bindings.tools` → `tool_routes: BTreeMap<String, CanonicalId>`
   (`broker_acl.rs:124-137`). m4 surfaces this through
   `Broker::tool_route(name: &str) -> Option<CanonicalId>` and the
   `AgentLoop` consumes it. Conflicting tool declarations remain
   compile-time errors (m1 territory; m4 adds a fixture lock that
   uses the `lock.session.tool_owner` disambiguation path so the
   tool-routes map is well-defined under shared names).
9. **m2 supervisor row-39 refusal removed**: the
   `InvalidPlanReason::ProviderNotInM2` arm in
   `supervisor.rs:414-419` is deleted; the
   `provider_lock_entries_refused.rs` m2 test (verify exact name
   under §M2 below) flips into a positive
   `provider_plugin_spawns_through_supervisor.rs` test. The
   `ProviderNotInM2` variant of `InvalidPlanReason` is removed
   from `error.rs:401-403` (source-breaking; one consumer is the
   m2 test). Synthetic-stub-test successor pattern is named here
   (per `plans/README.md` "Synthetic-stub tests need a planned
   successor").
10. **Integration tests** under
    `rafaello-core/tests/`, `rafaello-mockprovider/tests/`,
    `rafaello-readfile/tests/`, and `rafaello/tests/` exercising
    the demo bar (positive and the six named negatives).

### m4 → m5 boundary

m4 enforces the `taint` envelope's **presence + structural
shape + core-supplied origin**. Specifically:

- the envelope is required on `core.session.tool_request` and
  `core.session.tool_result`;
- the envelope's `source` must be a known taxon and
  `detail` must be a non-empty string when present;
- plugin-supplied taint on the inbound provider / plugin
  namespaces does not flow verbatim to `core.*` — core
  computes the canonical entry from the publishing
  principal's identity per security RFC §7.2.1.

m4 does **not** implement:

- the taint **propagation** rules (e.g. tool-result taint
  feeding back into the next tool-request's taint when the
  arg matches a recent result payload — security RFC
  §7.2.1–§7.2.2);
- the taint-matching / superset enforcement on tool-result
  re-emission (the m4 re-emit only verifies envelope shape,
  not the superset relation);
- the **broker-side sink gate** that consumes the envelope on
  sink-class tool requests (overview §6.2). m4's only tool is
  `read-file` with `sinks = []`, so this gate has no consumer
  yet.
- `user_grants`, `/grant` slash commands,
  `core.session.confirm_request` / `confirm_reply` /
  `confirm_answer`. These are m5.

The split is **load-bearing**: m4 ships the envelope so that
m5's gate can be wired against an envelope that *already exists
on the wire and is validated at the broker*; m5 then adds
matching/propagation atop a stable envelope shape.

## Lock-correspondence claim, extended

The m2 / m3 "lock-correspondence is API-level only" claim (m2 retro
§2.6; m3 retro §2.8) carries into m4 with one explicit decision:

**Default: extend `PluginSupervisor` to handle
`bindings.provider = true` entries; introduce no new supervisor
type.** Rationale:

- m2's supervisor already spawns plugins by `CompiledPlugin` plan
  (`supervisor.rs:259-419`). The only "providers are different"
  surface m2 ships is the row-39 refusal at line 414 — there is no
  separate code path that would otherwise duplicate.
- The provider-vs-plain-plugin distinction at runtime is **the
  topic-namespace publish authority**, which lives in
  `broker_acl.rs:99-103` (`PluginAcl.provider_id: Option<String>`)
  and in the broker's `handle_*_publish` dispatch — not in the
  supervisor. m4's `handle_provider_publish` consumes the same
  `PluginAcl` field; the supervisor path is unchanged below the
  refusal-removal commit.
- A separate `ProviderSupervisor` would force a second
  TestHooks copy, a second `ManagedSpawn` shape, a second
  Drop/shutdown path. None of these earn their complexity.

The supervisor's public entry point remains
`PluginSupervisor::spawn(plan: &CompiledPlugin, paths:
&SpawnPaths)`. A `CompiledPlugin` with `provider_id = Some(_)` and
`bindings.provider = true` (m1's `BrokerAcl::compile` already
maps `bindings.provider` into `PluginAcl.provider_id` per
`broker_acl.rs:99-103`) is now a valid input. The supervisor
spot-checks remain identical (path validation, reserved-env-var
rejection, network policy parse) — they apply uniformly.

The one supervisor-internal change in m4 is the **broker
registration call site** at the end of the spawn pipeline: instead
of `register_plugin(canonical, peer)`, providers go through a new
`register_provider(canonical, provider_id, peer)` so the broker
records the provider's distinct publish authority. The choice
between two methods vs one polymorphic call falls to "two methods"
because the `Publisher::Plugin` vs `Publisher::Provider` distinction
must be observable at registration time and the
`RegisteredPlugin` / `RegisteredProvider` RAII guards have
distinct Drop paths against distinct `BrokerState` maps.

The `PluginSupervisor` retains its name (no rename). Frontends
remain on `FrontendSupervisor` (m3 territory; not changed in m4).

## Inputs

- `rafaello/plans/overview.md` end-to-end, especially:
  - §4.3 (four namespaces — `provider.<provider-id>.*` finally
    becomes live);
  - §4.4 (provider plugins + core re-emit rule);
  - §4.5 (bus event envelopes — `request_id` v1 status says **m4
    adds it**; banner explicitly names m4);
  - §4.6 (reserved env vars — m4 adds `RFL_PROVIDER_ID` and
    `RFL_PROVIDER_ACTIVE`);
  - §6 (grant compiler — note v1 sinks/confirmation gate is m5;
    m4 only enforces taint envelope presence + origin);
  - §7 (tool dispatch — the canonical 5-step path m4 implements);
  - §8 (provider model — lock's `[session].provider_active`
    pins the active provider);
  - §11 / §12.
- `rafaello/plans/decisions.md` rows **3, 4, 5, 6, 7, 8, 10, 13,
  16, 17, 18, 20, 22, 23, 32, 33, 37, 38, 39, 40, 41, 42**:
  - row **6** — provider plugins publish on
    `provider.<provider-id>.*`; core re-emits.
  - row **7** — mandatory taint on `core.session.tool_*`;
    structured `{source, detail}`; populated by core, not plugins.
  - row **8** — mandatory `in_reply_to` on tool_result, RPC reply,
    confirm_answer, provider tool_request, and provider
    assistant_message. m4 owns the provider-tool_request and
    provider-assistant_message slots.
  - row **20** — `core.session.*` topic spelling.
  - row **39** — m2 supervisor refuses `bindings.provider = true`;
    **m4 removes this refusal**.
  - row **40** — reserved env-var list. m4 adds
    `RFL_PROVIDER_ID` and `RFL_PROVIDER_ACTIVE` to
    `supervisor::RESERVED_ENV_VARS` (m1's `scrubber.rs`
    `RESERVED_ENV_VARS` also extends symmetrically — this is
    the m4 §M1.1 backreach if it surfaces; default is "add to
    both lists in the same commit because m1 v3 catches them
    pre-compile and m2 supervisor catches them at spawn").
  - row **41** — `replay: bool` envelope flag on
    `core.session.entry.finalized` (m3 wire shape; m4 does not
    change it but it is load-bearing because m4's TUI replay
    path remains the m3 path).
  - row **42** — `Publisher` shape; **m4 adds the
    `Provider` publisher variant**.
- `rafaello/plans/glossary.md`. Especially "Provider plugin",
  "Taint", "`in_reply_to`", "Canonical `core.*` event",
  "Tool dispatch".
- `rafaello/plans/streams/a-security/rfc-security-model.md`:
  - §5.4 + §5.4.1 — provider tool_request path, taint synthesis
    from publisher origin, result-routing back path.
  - §7.2.1–§7.2.2 — taint origin rules. m4 implements the
    *origin* half (envelope built from publisher identity); the
    *propagation* half (matching arg values to recent results) is
    m5.
  - §7.2.6 — `in_reply_to` required-fields table.
  - §10 v1 summary — the caveat that overview §6.2 wins on the
    sink rule (m5 territory).
- `rafaello/plans/streams/f-manifest/rfc-manifest-schema.md` —
  `provides.provider`, `provides.tools`, and the per-tool
  `[provides.tool.<n>]` block. m1 already validates these
  fields end-to-end; m4 is the first milestone where the
  validated fields **drive runtime authority** (provider
  publish namespace + tool routing target).
- `rafaello/plans/milestones/m3-tui-sessions/scope.md` — read
  end-to-end for **format/structure** and:
  - §2.7 `check_lock_publish_topic` unknown-namespace gap
    (recorded for m4; only file a §M1 commit if a user-facing
    failure surfaces — default is no commit).
  - §2.10 frontend ACL `publish_topics = []` — m4's **first
    action** is to extend the TUI's `FrontendAcl` to allow
    publishing `frontend.tui.user_message`. Load-bearing for the
    demo bar: the user's typed message must reach core.
  - §5.9 `frontend_register_with_broker.rs` granularity gap —
    m4 hardening pass; lands a dedicated test file.
- `rafaello/plans/milestones/m3-tui-sessions/retrospective.md`
  §2.7, §2.10, §5.9 confirming the carryovers.
- `rafaello/plans/milestones/m2-broker-spawn/scope.md` — esp.
  §B (broker), §SP (supervisor), §B3 (publish-authority rules),
  §F (lockin enforcement).
- `rafaello/plans/milestones/m2-broker-spawn/retrospective.md`
  §2.1 (provider-refusal guard rationale) and §5.1 (TestHooks
  fault-injection mechanism, used here too).
- `rafaello/plans/milestones/m1-manifest/scope.md` §C (compile
  module) for what `bindings.provider` / `bindings.tools` carry
  into m4 via m1's `compile_plugin` and `broker_acl::compile`.
- Live m2/m3 code surface (cited line numbers as of branch tip):
  - `rafaello/crates/rafaello-core/src/bus.rs` — `Broker`,
    `BusEvent`, `PublisherIdentity` (currently `Core | Plugin |
    Frontend`), `PublishMsg`, `TaintEntry`, `RegisteredPlugin`,
    `RegisteredFrontend`, `handle_plugin_publish`,
    `handle_frontend_publish`, `publish_core`, `publish_boot`,
    `publish_core_internal`, `fan_out`.
  - `rafaello/crates/rafaello-core/src/broker_acl.rs` —
    `BrokerAcl { plugins, tool_routes, frontends }`, `AttachId`,
    `FrontendAcl`, `PluginAcl { topic_id, publish_topics,
    subscribe_patterns, auto_subscribes, provider_id }`,
    `compile(lock)`.
  - `rafaello/crates/rafaello-core/src/supervisor.rs` —
    `PluginSupervisor`, `SpawnPaths`, `SpawnHandle`,
    `ManagedSpawn`, `RESERVED_ENV_VARS` (line 49-56),
    `TestHooks { inject_pre_spawn_fault,
    inject_post_spawn_pre_register_fault,
    inject_post_register_fault }`, the row-39 refusal block at
    lines 414-419.
  - `rafaello/crates/rafaello-core/src/error.rs` —
    `BrokerError`, `Publisher { Core, Plugin, Frontend }` (line
    289-293; m4 adds `Provider`), `InReplyToReason`,
    `InvalidPlanReason::ProviderNotInM2` (lines 401-403; m4
    deletes), `SpawnError`, `FrontendSpawnError`.
  - `rafaello/crates/rafaello-core/src/session/mod.rs` —
    `SessionStore`, `SessionController`, `StoredEntry`,
    `SessionError`. m4 adds a `SessionController::record_message`
    method (or reuses `finalize_entry`) to persist the
    user_message / assistant_message / tool_call / tool_result
    entries — see §AL below.
  - `rafaello/crates/rafaello-core/src/validate/mod.rs` —
    `check_publish_topic` (manifest, line 359-380),
    `check_lock_publish_topic` (lock, line 382-414).
  - `rafaello/crates/rafaello/src/lib.rs` `run_chat` — the m3
    wiring path the m4 frontend ACL extension edits in §F1.
- `rafaello/Cargo.toml` workspace deps — verified present at
  draft time: `serde_json`, `tokio`, `nix`, `serde`,
  `fittings-*`, `outpost`, `lockin`, `tempfile`, `serial_test`,
  `tracing-test`, `tracing-subscriber`. **No new workspace
  dependencies are required by m4 round 1.**
- The fittings + lockin + outpost public APIs (used unchanged
  from m2/m3).

## In scope

Per-commit granularity is the driver's call when drafting
`commits.md`; this section names public API surface and the
test matrix.

### W — workspace dependencies

m4 does **not** add new third-party crates. Two new in-tree crates
land:

- **W1 (workspace `Cargo.toml`).** Extend `members` from
  `["crates/rafaello", "crates/rafaello-core", "crates/rafaello-tui"]`
  to `["crates/rafaello", "crates/rafaello-core",
  "crates/rafaello-tui", "crates/rafaello-mockprovider",
  "crates/rafaello-readfile"]`. No `[workspace.dependencies]`
  edits.
- **W2 (new crate `rafaello-mockprovider`).** Cargo manifest at
  `rafaello/crates/rafaello-mockprovider/Cargo.toml`:
  - `[package] name = "rafaello-mockprovider"; version = "0.0.0";
    edition = "2021";`
  - `[lib]`
  - `[[bin]] name = "rfl-mockprovider"; path =
    "src/bin/rfl_mockprovider.rs"`
  - `[dependencies]`: `rafaello-core = { path =
    "../rafaello-core" }`, `tokio`, `tracing`,
    `tracing-subscriber`, `fittings-core`, `fittings-server`,
    `fittings-client`, `fittings-transport`, `serde`,
    `serde_json`, `async-trait`, `anyhow`, all via
    `workspace = true`.
  - `[dev-dependencies]`: `tempfile`, `serial_test`,
    `tracing-test`, all `workspace = true`.
- **W3 (new crate `rafaello-readfile`).** Cargo manifest at
  `rafaello/crates/rafaello-readfile/Cargo.toml`. Same dep
  shape as W2 (`bin/rfl_readfile.rs`); no extra runtime
  dependencies.
- **W4 (`rafaello-core/Cargo.toml`).** No edits required —
  `rafaello-core` already pulls every dep m4's new modules
  (`agent`, `reemit`) need. Round-1 default: leave
  `rafaello-core/Cargo.toml` untouched. If the agent loop's
  scheduling primitives need a `futures = "0.3"` dep, that lands
  in the W4 commit (a single-line workspace alias `futures = "0.3"`
  is already pulled in transitively via `fittings-*` so the
  default is "no new dep").

### B — broker extension: provider principals + envelope + taint

The m2/m3 broker `Broker` is extended in three orthogonal
directions: (1) provider as a third registration principal,
(2) `request_id` as a first-class envelope field, (3) taint
envelope validation on `core.session.tool_*`. These three may
land as separate commits or be bundled — driver picks; pi may
prefer they bundle because they all touch `BusEvent` /
`Publisher` / `BrokerError` together.

- **B1.** Extend `Publisher` (`error.rs:289-293`):
  ```rust
  #[derive(Debug)]
  #[non_exhaustive]
  pub enum Publisher {
      Core,
      Plugin(CanonicalId),
      Frontend(AttachId),
      Provider {
          canonical: CanonicalId,    // for diagnostic / logging
          provider_id: String,       // the public id (e.g. "mock")
      },
  }
  ```
  The new variant carries both the canonical id (so log lines
  remain traceable to a specific plugin) and the public
  `provider_id` (the namespace authority key). m1's
  `PluginAcl.provider_id: Option<String>` is the source of
  truth — when the supervisor calls `register_provider`, it
  reads it from the ACL.
- **B2.** Extend `BusEvent` (`bus.rs:35-44`):
  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct BusEvent {
      pub topic: String,
      pub payload: serde_json::Value,
      pub publisher: PublisherIdentity,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub request_id: Option<JsonRpcId>,         // NEW in m4
      #[serde(skip_serializing_if = "Option::is_none")]
      pub in_reply_to: Option<Vec<JsonRpcId>>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub taint: Option<Vec<TaintEntry>>,
  }
  ```
  - `request_id` carries the publisher-assigned correlation id;
    `JsonRpcId` is the same type used inside `in_reply_to` and is
    re-exported via `crate::bus::JsonRpcId`. Generated on the
    publishing side; preserved verbatim by the broker; consumed
    by subscribers correlating against `in_reply_to`. Required
    on every event that may be cited by a future `in_reply_to`
    (tool_request, user_message, assistant_message — every
    "request-shaped" topic). Optional on result/reply topics
    that close a prior `request_id` (tool_result, rpc_reply).
    Schema-side validation lives in the per-publisher `PublishMsg`
    parsing path (B6 below).
  - **`PublishMsg`** (`bus.rs:17-26`) grows
    `request_id: Option<JsonRpcId>` symmetrically. Plugins
    set it on their `bus.publish` calls; the broker passes it
    into the emitted `BusEvent`.
- **B3.** Extend `PublisherIdentity` (`bus.rs:46-52`):
  ```rust
  #[derive(Debug, Clone, Serialize)]
  #[serde(tag = "kind", rename_all = "snake_case")]
  pub enum PublisherIdentity {
      Core,
      Plugin { canonical: String, topic_id: String },
      Frontend { attach_id: String },
      Provider { canonical: String, provider_id: String, topic_id: String },
  }
  ```
  The `topic_id` is included for diagnostic symmetry with
  `Plugin` and because providers also have a hashed topic-id
  (m4's `rfl-mockprovider` declares `provides.tools = []` but
  may still publish on `plugin.<topic-id>.*` if it ever needed
  to — it does not in m4, but the symmetric encoding keeps
  the v2 surface minimal).
- **B4.** `BrokerError` extensions:
  ```rust
  #[error("provider `{0}` not in broker ACL")]
  ProviderNotInAcl(CanonicalId),
  #[error("provider `{0}` not registered with broker")]
  ProviderNotRegistered(CanonicalId),
  #[error("provider `{0}` already registered with broker")]
  ProviderAlreadyRegistered(CanonicalId),
  #[error("envelope missing required `request_id` on `{topic}` (publisher {publisher:?})")]
  MissingRequestId { publisher: Publisher, topic: String },
  #[error("missing or invalid `taint` envelope on `{topic}` from publisher {publisher:?}: {reason}")]
  InvalidTaint { publisher: Publisher, topic: String, reason: String },
  ```
  - `MissingRequestId` fires when a request-shaped topic
    (`*.tool_request`, `*.user_message`, `*.assistant_message`)
    arrives without `request_id`.
  - `InvalidTaint` covers the four taint-envelope failure modes
    (missing, empty array, unknown source taxon, plugin-
    supplied on a `core.*` topic). The `reason` string is a
    structured token (see B7 below).
- **B5.** Provider registration surface, symmetric to plugin
  / frontend:
  ```rust
  pub struct RegisteredProvider {
      broker: Arc<BrokerInner>,
      canonical: Option<CanonicalId>,
  }
  // Drop releases the registry slot in BrokerState.providers.

  impl Broker {
      pub fn try_reserve_provider_registration(
          &self, canonical: &CanonicalId,
      ) -> Result<(), BrokerError>;

      pub fn register_provider(
          &self,
          canonical: CanonicalId,
          provider_id: String,
          peer: PeerHandle,
      ) -> Result<RegisteredProvider, BrokerError>;

      pub fn contains_provider(&self, canonical: &CanonicalId) -> bool;

      pub fn handle_provider_publish(
          &self,
          canonical: &CanonicalId,
          raw_params: &serde_json::Value,
      ) -> Result<(), BrokerError>;
  }
  ```
  The `BrokerState` (`bus.rs:62-65`) grows
  `providers: BTreeMap<CanonicalId, ProviderConn>` alongside
  `registry` and `frontends`. `register_provider` checks that
  the ACL's `PluginAcl.provider_id` matches the
  `provider_id` arg (round-1 defaults to a hard error if they
  diverge — `BrokerError::ProviderIdMismatch`; alternative is
  to drop the arg and read from ACL only — pi-preferred path
  TBD).
- **B6.** `handle_provider_publish` (mirror of m2
  `handle_plugin_publish` and m3 `handle_frontend_publish`):
  1. Verify the provider is registered (`ProviderNotRegistered`
     otherwise).
  2. Parse `PublishMsg` (now including `request_id`).
  3. `validate_topic`.
  4. Namespace dispatch on `segments[0]`:
     - `"core" | "plugin" | "frontend"` →
       `PublishOnReservedNamespace { publisher: Provider, … }`.
     - `"provider"` — must be ≥3 segments and
       `segments[1] == provider_id` (the public id stored in
       the registry, not the topic-id), otherwise
       `PublishOnReservedNamespace`.
     - other → `UnknownNamespace`.
  5. Exact-string check against
     `PluginAcl.publish_topics` (which m1 already validates is
     a `provider.<id>.*` subset for `provider = true` entries
     via `check_lock_publish_topic`).
  6. **`in_reply_to` enforcement** per security RFC §7.2.6:
     - `provider.<id>.tool_request` MUST have
       `in_reply_to = [<user_message_id>]` (length 1; references
       the `core.session.user_message.request_id` that the
       provider is responding to);
     - `provider.<id>.assistant_message` MUST have
       `in_reply_to = [<user_message_id> or
       <tool_result_request_id>]` (length 1);
     - `provider.<id>.rpc_reply` (future) MUST have
       `in_reply_to = [<call_request_id>]`.
     Missing / empty array / multiple entries → `InvalidInReplyTo`.
  7. **`request_id` requirement**: `tool_request`,
     `assistant_message`, `user_message` → required;
     `tool_result`, `rpc_reply` → optional (these are *responses*,
     not requests). Round-1 cut: `request_id` on a request topic
     missing → `MissingRequestId`.
  8. Emit the `BusEvent` with `PublisherIdentity::Provider {…}`;
     **the broker carries `msg.taint` verbatim to the inbound
     event** — provider-side taint is preserved at this layer
     because the core re-emit (CR below) is what computes the
     canonical `core.*` taint, and the inbound provider event
     is fanned out to the agent loop, the session controller,
     and any other subscribers granted the
     `provider.<id>.**` pattern.
  9. Fan out via the existing `fan_out` path (extended in B9
     below to include `providers` recipients).
- **B7.** Taint envelope validation. m4's enforcement is **at
  re-emission time** (CR §below), not at the inbound publish
  point — the broker accepts whatever `taint` the provider sends
  on the `provider.<id>.*` namespace (an empty / absent envelope
  is fine at that layer) and **core synthesises the canonical
  envelope when it re-emits to `core.*`**. The hard enforcement
  is therefore on the `publish_core_with_taint` path, not on
  `handle_provider_publish`. Concretely:
  - **`Broker::publish_core_with_taint(topic, payload,
    request_id, in_reply_to, taint)`** is a new method that:
    - validates the topic is `core.*`;
    - validates `taint` is non-empty for `topic` matching the
      `core.session.tool_request` / `core.session.tool_result`
      pattern (per overview §4.5 / row 7);
    - validates each `TaintEntry.source ∈ {"user", "provider",
      "tool", "system"}` (security RFC §7.2.1 taxon);
    - emits the `BusEvent` with the supplied
      `request_id` / `in_reply_to` / `taint`.
  - The existing `publish_core` becomes a thin wrapper that
    calls `publish_core_with_taint(topic, payload, None, None,
    None)`; on `core.session.tool_*` it errors
    `InvalidTaint { reason: "missing" }`. (Defence in depth:
    a future core path that forgot to set taint cannot publish
    a tool_* event.)
  - **Plugin-supplied taint rejection**: if a plugin or
    provider attempts to publish on its own namespace **and
    that namespace would be re-emitted to `core.session.tool_*`**,
    the re-emit path enforces that the synthesised taint is
    `[{source: <publisher-kind>, detail: <publisher-id>}]`,
    not whatever the publisher attached. The publisher's
    optional `msg.taint` is **discarded at the re-emit
    boundary** — overview §4.4 / §4.5 reconciliation note
    ("Core synthesises and validates them before fan-out;
    plugin authors never write `taint` directly").
- **B8.** Topic validation lifecycle for the new
  `core.session.*` topics:
  - `core.session.tool_request`, `core.session.tool_result`,
    `core.session.assistant_message`, `core.session.user_message`
    are grammar-valid by construction — no `validate_topic`
    change is needed.
  - The frontend's subscribe pattern `core.session.**` (m3 default)
    already covers them; no ACL change needed for the TUI to
    receive them.
  - The provider's manifest `subscribes` set must include
    `core.session.user_message` and `core.session.tool_result`
    (m4 §PR1 lock fixture); m1's `validate::lock` accepts these
    today (no manifest schema change).
- **B9.** Fan-out (`bus.rs:546-625`) gains a third recipient
  band — `providers`. Same shape as `plugin_recipients` and
  `frontend_recipients`: build the recipient list under the
  state lock, drop the lock, then per-recipient
  `peer.notify("bus.event", value.clone())`. Subscribers
  receive events on `provider.<id>.**` patterns; round-1
  exclusion rule: a provider does not receive its own
  re-published `core.session.tool_request` (the agent loop
  does), to avoid an obvious feedback loop. Specifically, if
  the publisher is `Core` and the re-emit was synthesised from
  a `provider.<id>.tool_request`, fan-out **excludes** the
  source provider from the recipient set. This is the m4
  analogue of m2's `result-routing protection` (`bus.rs:310-318`).
- **B10.** `BrokerAcl` defence-in-depth pattern revalidation
  (m2 §B10, m3 §B6 carryover) — m4 adds nothing structural
  here; the validation already iterates `plugins.publish_topics`
  / `frontends.publish_topics` per their existing rules.
  **However**, m4 adds a constraint check: for any
  `PluginAcl` with `provider_id = Some(_)`, every
  `publish_topics` entry must be `provider.<id>.*` and the
  `<id>` segment must equal the `provider_id`. m1's
  `check_lock_publish_topic` already does this on the lock
  side; the broker-side defence-in-depth check makes it a
  second gate so a hand-mutated `BrokerAcl` cannot bypass.
  New test:
  `broker_construct_with_provider_publish_id_mismatch_rejected.rs`.

### F — frontend ACL extension (m3 retro §2.10 handover)

- **F1.** Edit `rafaello/crates/rafaello/src/lib.rs` `run_chat`
  (lines 142-153). m3's `publish_topics: BTreeSet::new()`
  becomes:
  ```rust
  let mut publish_topics = BTreeSet::new();
  publish_topics.insert("frontend.tui.user_message".to_string());
  ```
  The frontend's subscribe pattern set is unchanged
  (`core.session.**`, `core.lifecycle.**`).
- **F2.** New core re-emit: the broker subscribes (in-process,
  not via the bus ACL) to `frontend.tui.user_message` and
  re-emits as `core.session.user_message`. Spec lives in §CR
  below; this row pins the *grant* side.
- **F3.** TUI publishes the user's typed message. The
  `rafaello-tui` library/bin gains a small piece of code: when
  the user presses Enter on the prompt input, the TUI calls
  `peer.notify("bus.publish", {topic:
  "frontend.tui.user_message", payload: {text: <input>},
  request_id: <fresh JsonRpcId>})`. The request_id flows into
  the re-emitted `core.session.user_message`.
- **F4.** New positive test in `rafaello-core/tests/`:
  `frontend_register_with_broker.rs` (m3 retro §5.9
  granularity gap — stand-alone test for the registration
  happy path) and
  `frontend_publish_user_message_reemitted_as_core_session_user_message.rs`.

### PS — provider-side supervisor changes

- **PS1.** Remove m2's row-39 refusal at
  `supervisor.rs:414-419`. Delete the entire `if let Some(provider_id) =
  acl_provider_id { return Err(SpawnError::InvalidPlan {…
  ProviderNotInM2…}) }` block. The `acl_provider_id: Option<String>`
  retained at line 341 is now consumed by step PS3.
- **PS2.** Delete `InvalidPlanReason::ProviderNotInM2`
  (`error.rs:401-403`). Source-breaking inside `rafaello-core`;
  the only out-of-tree consumer is m2's
  `tests/provider_lock_entries_refused.rs` (verify exact file
  name by inspecting the m2 test dir — round-1 cut assumes the
  filename matches what scope §M2 below references). Per
  `plans/README.md` synthetic-stub-test successor rule (m2
  §3.3): m4 §M2 row below names the successor.
- **PS3.** Inject `RFL_PROVIDER_ID` and `RFL_PROVIDER_ACTIVE`
  into the child env when the spawn plan has
  `acl_provider_id = Some(_)`:
  - `RFL_PROVIDER_ID = <provider_id>` (e.g. `"mock"`) — the
    public namespace authority key.
  - `RFL_PROVIDER_ACTIVE = "1"` if this provider is the
    `[session].provider_active` per the lock; `"0"` otherwise.
    Round-1 cut: m4 only spawns the active provider via the
    agent loop's lazy-spawn path, so this var is always `"1"`
    in m4 practice; the variable exists so a future "multiple
    providers installed, one active" scenario (overview §8)
    doesn't need a re-spawn semantics change.
  - **Decision and defence**: `RFL_PROVIDER_ID` is the new
    primary env var. Alternative considered: rely on
    `RFL_PLUGIN` + the bindings.toml carried in the lock
    (overview §4.6 lists only `RFL_BUS_FD`, `RFL_PLUGIN`,
    `RFL_HELPER_FD`). Rejection rationale: `RFL_PLUGIN` is the
    *canonical id* (`<source>:<name>@<version>`), not the
    public provider id. Forcing the provider plugin to parse
    canonical-id strings to discover its own provider-id is
    ugly; an explicit env var is cleaner and matches the
    pattern m2 set with `RFL_TOPIC_ID` (row 40).
- **PS4.** Extend m2's `RESERVED_ENV_VARS` (`supervisor.rs:49-56`):
  ```rust
  const RESERVED_ENV_VARS: &[&str] = &[
      "RFL_BUS_FD",
      "RFL_PLUGIN",
      "RFL_HELPER_FD",
      "RFL_PROJECT_ROOT",
      "RFL_PRIVATE_STATE_DIR",
      "RFL_TOPIC_ID",
      "RFL_PROVIDER_ID",          // NEW in m4
      "RFL_PROVIDER_ACTIVE",      // NEW in m4
  ];
  ```
- **PS5.** Extend m1's `scrubber.rs` `RESERVED_ENV_VARS` to
  match (row 40 mirror). m1 v3 catches reserved-name use at
  manifest compile time; the m4 §M1.1 row records this as the
  m1 back-reach (default: same commit as PS4 — the two lists
  must move together).
- **PS6.** Provider broker registration. At the broker
  registration step in `PluginSupervisor::spawn` (currently
  `Broker::register_plugin` for non-provider plugins), branch
  on `acl_provider_id`:
  ```rust
  let registered: ProviderOrPlugin = match acl_provider_id.clone() {
      Some(pid) => ProviderOrPlugin::Provider(
          self.broker.register_provider(plan.canonical.clone(),
              pid, peer.clone())?,
      ),
      None => ProviderOrPlugin::Plugin(
          self.broker.register_plugin(plan.canonical.clone(),
              peer.clone())?,
      ),
  };
  ```
  with a `ProviderOrPlugin` newtype enum (or `Either`)
  carrying the appropriate RAII guard into `ManagedSpawn`.
- **PS7.** `ManagedSpawn` (`supervisor.rs:155-168`) field
  `registered: Option<RegisteredPlugin>` becomes
  `registered: Option<ProviderOrPlugin>` (or two parallel
  optional fields if pi prefers explicit shapes). Drop
  unconditionally releases the right registry slot.
- **PS8.** `SupervisorConnectionService` and any
  fittings-bound dispatchers learn about the new
  `bus.publish` source: when the publish comes from a
  provider-bound peer, it routes to
  `Broker::handle_provider_publish` instead of
  `handle_plugin_publish`. m2's
  `BusPublishService::call(method="bus.publish", …)` (in
  `supervisor.rs:1005-1036`) already dispatches by
  `canonical`; m4 extends the dispatcher to check
  `broker.contains_provider(canonical)` first.

### CR — core re-emit pipeline

A new module `rafaello-core/src/reemit/mod.rs` ("reemit" =
re-emission). The module owns the in-process subscriber to
the four wire paths that produce `core.session.*` events:

- `frontend.tui.user_message` → `core.session.user_message`
- `provider.<id>.tool_request` → `core.session.tool_request`
  + `plugin.<topic-id>.tool_request` (the routed tool-request,
  see §AL below — actually the agent loop does the second hop;
  the re-emit path stops at `core.session.tool_request`)
- `provider.<id>.assistant_message` → `core.session.assistant_message`
- `plugin.<topic-id>.tool_result` → `core.session.tool_result`

- **CR1.** New struct `ReemitRouter` constructed at `rfl chat`
  startup with handles to `Broker`, `BrokerAcl` (for the
  `tool_routes` map), and the `[session].provider_active`
  canonical id from the lock. The router subscribes via an
  in-process channel hooked into the broker's fan-out
  (round-1 mechanism: `Broker::subscribe_internal(pattern,
  Sender<BusEvent>)` — new method that registers an in-process
  recipient with no fittings transport; events are deep-cloned
  into the supplied channel before the bus's `fan_out`
  outbound notifications run). The internal subscriber is not
  ACL-gated because it is part of core's trusted internal
  composition.
- **CR2.** Re-emission steps for **provider →
  core.session.tool_request**:
  1. Receive `BusEvent { topic: "provider.mock.tool_request",
     payload, publisher: Provider{…, provider_id}, request_id:
     Some(rid), in_reply_to: Some([user_msg_id]), taint: _ }`
     from the internal subscriber.
  2. Validate `payload` deserialises to
     `{tool: String, args: Value}`.
  3. Look up `BrokerAcl.tool_routes.get(&payload.tool)` —
     if missing, emit a `core.lifecycle.tool_dispatch_rejected`
     core event with reason `unknown_tool` and *do not* re-emit
     a `core.session.tool_request`. (This is the "tool plugin
     called directly by another plugin (not via core
     re-emission) doesn't reach the dispatch path" negative —
     unknown tools cannot be routed.)
  4. Synthesise canonical taint:
     `vec![TaintEntry{source: "provider".into(), detail:
     Some(provider_id.clone())}]`. (Security RFC §7.2.1 — the
     origin half. Propagation half deferred to m5.)
  5. Call `broker.publish_core_with_taint(
       "core.session.tool_request",
       json!({tool: <name>, args: <args>, dispatch_target:
         <canonical-id>}),
       Some(rid),                  // forwarded from provider
       Some(vec![user_msg_id]),    // forwarded
       Some(taint))`.
- **CR3.** Re-emission steps for **plugin →
  core.session.tool_result**:
  1. Receive `BusEvent { topic: "plugin.<topic-id>.tool_result",
     payload, publisher: Plugin{canonical, topic_id},
     request_id: maybe, in_reply_to: Some([tool_request_id]), … }`.
  2. Look up `canonical` in `BrokerAcl.plugins` to confirm it
     is a known tool plugin (defence in depth).
  3. Synthesise taint: `vec![TaintEntry{source: "tool".into(),
     detail: Some(canonical.to_string())}]`. (m4: origin only.
     m5 will additionally `concat` the originating
     tool_request's taint per security RFC §7.2.2.)
  4. `publish_core_with_taint("core.session.tool_result",
     payload, None /* result is a response, not a request */,
     Some([tool_request_id]), Some(taint))`.
- **CR4.** Re-emission steps for **provider →
  core.session.assistant_message**: payload pass-through;
  taint = `[{source: "provider", detail: provider_id}]`;
  `in_reply_to` forwarded; `request_id` forwarded.
- **CR5.** Re-emission for **frontend.tui.user_message →
  core.session.user_message**: payload pass-through; taint =
  `[{source: "user", detail: None}]` (security RFC §7.2.1
  user-source taxon); `in_reply_to = None` (user messages
  initiate a turn); `request_id` forwarded from the frontend's
  publish. **Validation**: the frontend's publish must carry
  `request_id` (security RFC §7.2.6); if missing,
  `MissingRequestId` is returned to the frontend via the
  publish error path and no `core.session.user_message` is
  re-emitted.
- **CR6.** Active-provider scoping. The router subscribes to
  `provider.<active-id>.**` only (round-1 cut: m4 installs
  exactly one provider plugin, named `mock`). If a future
  multi-provider scenario surfaces (m6+), the router gains a
  `set_active_provider(canonical)` call that updates the
  pattern; m4 takes the single-static-pattern path for
  simplicity.
- **CR7.** Re-emit failure semantics: a re-emit that hits
  `BrokerError::InvalidTaint` etc. (which would be a core bug,
  not user input) logs at `tracing::error!` and emits a
  `core.lifecycle.reemit_rejected` event for observability.
  No process kill; the next inbound event still attempts a
  re-emit.

### AL — agent loop module

A new module `rafaello-core/src/agent/mod.rs`. Owns the dispatch
half of the canonical 5-step path (overview §7):

- **AL1.** `pub struct AgentLoop { broker: Broker, acl:
  BrokerAcl, session: Arc<SessionController> }`. Constructed at
  `rfl chat` startup after the `Broker`, `ReemitRouter`, and
  `SessionController` are wired.
- **AL2.** `AgentLoop::start(&self) -> tokio::task::JoinHandle<()>`
  spawns a tokio task that holds an in-process subscriber to
  `core.session.user_message`, `core.session.tool_request`,
  `core.session.tool_result`, and `core.session.assistant_message`.
- **AL3.** Per `core.session.user_message` event:
  - persist as a `kind: "user_message"` entry via
    `SessionController::finalize_entry`;
  - no further action — the provider plugin is the consumer
    (via fan-out on its subscribe set).
- **AL4.** Per `core.session.assistant_message` event:
  - persist as a `kind: "assistant_message"` entry via
    `SessionController::finalize_entry`. (Re-emit via the
    existing m3 wire shape; the TUI already renders
    `text` / `assistant_message` kinds via the m3
    renderer registry, with author = `Assistant`.)
- **AL5.** Per `core.session.tool_request` event (the re-emitted
  version with `dispatch_target: <canonical-id>` in payload —
  see CR2):
  - persist as a `kind: "tool_call"` entry (overview §11 /
    Stream E §3 — `tool_call` is a built-in renderer kind);
  - publish a tool-side request: the agent loop synthesises
    `plugin.<target-topic-id>.tool_request` with the same
    `request_id`, `in_reply_to`, `taint` envelope, and a
    payload `{tool, args}` (the `dispatch_target` field is
    stripped from the inner payload).
  - This publish goes through a new
    `Broker::publish_for_tool_dispatch(canonical:
    &CanonicalId, payload, request_id, in_reply_to, taint)`
    method that mirrors `publish_core_with_taint` but with
    publisher `PublisherIdentity::Core` (the agent loop is a
    core component) and a topic of the form
    `plugin.<topic-id>.tool_request`. The method validates that
    the supplied canonical is in `BrokerAcl.plugins` and that
    the topic-id matches. **This is the only path from
    `core.session.tool_request` to a tool plugin** — overview
    §7 architectural commitment.
- **AL6.** Per `core.session.tool_result` event:
  - persist as a `kind: "tool_result"` entry;
  - no further action — the provider plugin observes the
    re-emitted event (it subscribes to
    `core.session.tool_result` per its manifest) and uses the
    `in_reply_to` correlation to match against its
    outstanding `tool_request`.
- **AL7.** Active-provider pinning per overview §8: m4 reads
  `lock.session.provider_active` once at `rfl chat` startup;
  the agent loop and the reemit router are configured for that
  single provider. If `provider_active` is unset
  (`rfl init`-less mode), m4 falls back to "tool-less LLM
  client" mode where the agent loop is not started — but
  m4's demo bar requires a populated provider, so this branch
  is `manual-validation.md` territory only.
- **AL8.** Cancellation / shutdown: the AgentLoop task
  observes a `tokio::sync::watch::Receiver<bool>` shutdown
  signal; on signal, it drops its subscriber and exits. m4's
  `rfl chat` wires the shutdown signal to the same trigger
  m3's `forward_child_stderr` task observes.

### PR — `rafaello-mockprovider` subprocess plugin

- **PR1.** Manifest at
  `rafaello/fixtures/rafaello-mockprovider/rafaello.toml`:
  ```toml
  [plugin]
  name = "mockprovider"
  source = "builtin"
  version = "0.0.0"

  [provides]
  provider = "mock"

  [bus]
  subscribes = ["core.session.user_message", "core.session.tool_result"]
  publishes = ["provider.mock.tool_request",
               "provider.mock.assistant_message"]

  [capabilities.default.filesystem]
  read_dirs = []
  write_dirs = []

  [capabilities.default.network]
  mode = "deny"

  [load]
  eager = true
  ```
  The corresponding lock entry (m4 fixture) pins
  `bindings.provider = true`, `bindings.provider_id = "mock"`.
  `lock.session.provider_active = <canonical>`. The manifest's
  `subscribes` set is preserved verbatim into
  `entry.grant.subscribes`; m1's `BrokerAcl::compile` reads it
  into the ACL.
- **PR2.** Bin target `src/bin/rfl_mockprovider.rs`:
  - Reads `RFL_BUS_FD`, `RFL_PROVIDER_ID`,
    `RFL_PROVIDER_ACTIVE`, `RFL_PLUGIN` from env;
  - constructs a fittings `Server` on `RFL_BUS_FD`;
  - handles `bus.event` notifications with the canonical
    matcher: parse the payload as a `BusEvent`, branch on
    `topic`:
    - `core.session.user_message` → run the deterministic
      matcher:
      - if regex `(?i)what(?:'s| is) in (?<path>\S+)` matches,
        publish
        `bus.publish({topic: "provider.mock.tool_request",
        payload: {tool: "read-file", args: {path: "<path>"}},
        request_id: <fresh>, in_reply_to:
        [<user_message.request_id>]})`.
      - else publish
        `bus.publish({topic: "provider.mock.assistant_message",
        payload: {text: "echo: <input>"}, request_id:
        <fresh>, in_reply_to: [<user_message.request_id>]})`.
    - `core.session.tool_result` → publish
      `bus.publish({topic: "provider.mock.assistant_message",
      payload: {text: "Here's what's in <path>:\n<content>"},
      request_id: <fresh>, in_reply_to:
      [<tool_result.request_id-as-cited-in-its-in_reply_to>]})`.
- **PR3.** Determinism: the mock provider does not call into
  any time-of-day, RNG, or filesystem outside its private
  state dir. Every input message produces the same output
  message. Tests rely on this.
- **PR4.** **Decision: separate crate
  (`rafaello-mockprovider`) rather than a `[[bin]]` inside
  `rafaello-tui` or `rafaello-core`.** Defence:
  - matches the "ships as a plugin" architecture
    (decision row 21 / refined row 38);
  - keeps `rafaello-core` test-isolation tight (no
    workspace-internal cycle from a core test to a fixture
    bin in another crate);
  - the m3 precedent (rafaello-tui as its own crate with
    `rfl-tui` bin) establishes the pattern;
  - the fixture's dev-only nature is encoded via
    `required-features = ["test-fixture"]` on the bin
    target — round-1 cut wires this so the production
    `cargo build --release` of `rafaello-mockprovider` is
    available as a normal plugin install path for v2, but
    the integration tests bind the bin via `CARGO_BIN_EXE_*`
    inside the `rafaello-mockprovider` crate's tests.

### TP — `rafaello-readfile` tool plugin

- **TP1.** Manifest at
  `rafaello/fixtures/rafaello-readfile/rafaello.toml`:
  ```toml
  [plugin]
  name = "readfile"
  source = "builtin"
  version = "0.0.0"

  [provides]
  tools = ["read-file"]

  [provides.tool.read-file]
  sinks = []
  always_confirm = false

  [bus]
  subscribes = []                # m1 auto-subscribe inserts plugin.<id>.tool_request
  publishes = []                 # auto-publishes plugin.<id>.tool_result (see below)

  [capabilities.default.filesystem]
  read_dirs = ["${PROJECT_ROOT}"]
  write_dirs = []

  [capabilities.default.network]
  mode = "deny"

  [load]
  eager = false
  triggers = [{ kind = "tool", tool = "read-file" }]
  ```
  - The `auto_subscribes = ["plugin.<topic-id>.tool_request"]`
    entry is *compiler-inserted* per m1 — see
    `broker_acl.rs:98`. The manifest does not state it.
  - **`publishes` open question**: m1's `check_publish_topic`
    requires manifest authors to declare the tool_result
    topic (line 359-380 — `plugin` namespace is allowed for
    self-publishes). Round-1 cut sets
    `publishes = ["plugin.<topic-id>.tool_result"]` *but*
    the manifest cannot know its own topic-id at author
    time (the topic-id is derived from the canonical id at
    install). **Decision**: m1's manifest validator accepts
    a literal placeholder `plugin.<topic-id>.tool_result`
    where `<topic-id>` is the literal string (m1 substitutes
    the hashed form at lock-compile time). Round-1 cut
    files this as a Risk — if m1's substitution logic
    doesn't exist, the m4 §M1 backreach adds it. Pi may
    push back; the alternative is to special-case
    "tool plugins auto-publish `plugin.<own-topic-id>.tool_result`
    without manifest declaration" but that bakes a
    runtime-side rule into the broker that conflicts with
    overview §5.1's "lock is the grant" rule.
- **TP2.** Bin target `src/bin/rfl_readfile.rs`:
  - Reads `RFL_BUS_FD`, `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`,
    `RFL_PRIVATE_STATE_DIR`, `RFL_PLUGIN` from env;
  - on receipt of `bus.event` with topic
    `plugin.<own-topic-id>.tool_request`, parses payload as
    `{tool: "read-file", args: {path: String}}`:
    - resolve `path` against `RFL_PROJECT_ROOT` if relative;
      reject paths that escape (canonicalize + ancestor
      check);
    - read the file (utf8-only — m4 cut);
    - publish `plugin.<topic-id>.tool_result` with payload
      `{ok: true, content: <utf8>}` (or `{ok: false, error:
      <reason>}`) and `in_reply_to = [<request_id>]`.
- **TP3.** Read-only grant intersection: `read_dirs =
  ["${PROJECT_ROOT}"]` ensures the lockin sandbox sees the
  project root. The demo bar's "what's in README.md" prompt
  resolves to `${PROJECT_ROOT}/README.md`. m1's existing
  `${PROJECT_ROOT}` placeholder expansion (compile time) is
  the substitution path; m4 does not add a new placeholder.
- **TP4.** Same separate-crate rationale as §PR4. Lives at
  `rafaello/crates/rafaello-readfile/`.

### TD — tool dispatch wiring (core side)

- **TD1.** `Broker::tool_route(name: &str) ->
  Option<CanonicalId>`: a thin accessor over
  `self.0.acl.tool_routes.get(name)`. Public on `Broker`.
- **TD2.** `AgentLoop` consumes `tool_route` in step AL5 to
  resolve `dispatch_target`. Conflicting tool declarations are
  m1's territory (resolved via `lock.session.tool_owner`); m4
  does not add disambiguation logic.
- **TD3.** Round-1 cut: tool dispatch is **one-shot per
  `core.session.tool_request`** — the agent loop does not
  retry on transient failures or implement a request queue.
  A tool plugin that doesn't respond within a future
  configurable timeout is m5 territory (alongside sink
  confirmation timeouts which already need a timer
  primitive); m4's demo bar exercises the synchronous happy
  path only.

### M2 — remove m2 supervisor's row-39 refusal

Per `plans/README.md` "Synthetic-stub tests need a planned
successor": m4 names the successor up front.

- **M2.1.** Identify the m2 test: scope §SP / m2 §Risk list
  reference the row-39 `ProviderNotInM2` negative. The likely
  filename is
  `rafaello/crates/rafaello-core/tests/supervisor_refuses_provider_lock_entry.rs`.
  **Action at commits.md time**: verify the exact filename
  via `ls rafaello/crates/rafaello-core/tests/` and update the
  successor reference. Round-1 placeholder is the name above.
- **M2.2.** **Successor pattern**: delete the negative test
  (the synthetic refusal is gone) and add a positive test
  `provider_plugin_spawns_through_supervisor.rs` that:
  - builds a fixture `CompiledPlugin` with `bindings.provider =
    true`, `bindings.provider_id = "mock"`;
  - spawns through `PluginSupervisor::spawn`;
  - asserts the spawn succeeds (`SpawnHandle::wait_ready`
    resolves);
  - asserts `Broker::contains_provider(canonical) == true`;
  - asserts a `provider.mock.tool_request` publish by the
    child succeeds (via the existing fixture-mode
    `frontend_bus_publish` pattern, extended to provider
    publishes).
  This is the **named successor** that closes the
  synthetic-stub gap for m4.
- **M2.3.** Note in the commit body: "deletes
  `supervisor_refuses_provider_lock_entry.rs`; adds
  `provider_plugin_spawns_through_supervisor.rs`. Synthetic
  stub successor per `plans/README.md`."

### H6 — TestHooks taxonomy

- **H6.1.** m4 reuses m3's three TestHooks inject points
  (`inject_pre_spawn_fault`,
  `inject_post_spawn_pre_register_fault`,
  `inject_post_register_fault` — `supervisor.rs:194-199`).
  No new inject points are needed: provider spawn-time
  failures (env-var rejection, registration conflict) are
  exercised by the existing inject points, since the only
  m4-added branch in the spawn pipeline is the
  `register_provider` vs `register_plugin` choice at the
  registration step — both flow through the same
  post-register inject window.
- **H6.2.** **Provider-publish-rejected mid-spawn**: m4
  does not introduce a new inject point for this; the
  scenario is covered by the existing negative tests
  (`broker_publish_provider_unknown_id_rejected.rs` etc.
  below in §I) without needing a TestHooks hook.
- **H6.3.** Round-1 explicit statement: **m4 adds no new
  TestHooks inject points**. Default is to reuse m3's
  three.

### M1 — m1 publishes-grant patches if user-facing failures surface

- **M1.1.** **Reserved env-var list extension (`scrubber.rs`)
  for `RFL_PROVIDER_ID` + `RFL_PROVIDER_ACTIVE`**. Same
  rationale as decisions row 40 — additive, m1 v3 catches
  collisions pre-compile. **Default: land in the same commit
  as PS4.** This is the only m4-required m1 back-reach.
- **M1.2.** **`check_lock_publish_topic` unknown-namespace
  gap** (m3 retro §2.7): default is **no commit**. Filed if
  a user-facing failure surfaces during m4 implementation.
  If a fixture's hand-authored lock with an unknown namespace
  blows up at runtime in a way that surprises the
  per-commit agent, the §M1.2 commit lands a one-line patch
  (replacing the `_ => {}` arm in
  `validate/mod.rs:411` with an
  `Err(ValidationError::LockPublishUnknownNamespace { … })`).
  Round-1 documents the rationale (hand-authored locks are
  `--allow-unsafe`; runtime rejection is sufficient defence)
  to keep the default "no commit" stable through pi review.
- **M1.3.** **Manifest `publishes` literal `<topic-id>`
  substitution** (referenced in §TP1): if m1 does not
  currently accept the literal-placeholder form, the §M1.3
  commit lands the substitution. Round-1 default:
  investigate at commits.md drafting time by reading m1's
  `compile.rs` for the substitution path; if the
  substitution exists, no commit; if not, lands in m4.

### I — integration test suite

The §"Demo bar" matrix below is the contract.

Test placement follows the m3 rule (Cargo `CARGO_BIN_EXE_<name>`
is only reliable inside the bin's own package):

- **`rafaello-core/tests/`** — broker, agent loop, re-emit
  pipeline, m2 supervisor (incl. the provider-positive
  spawn), m3 frontend ACL extension.
- **`rafaello-mockprovider/tests/`** — anything spawning
  `rfl-mockprovider` (uses
  `env!("CARGO_BIN_EXE_rfl-mockprovider")`).
- **`rafaello-readfile/tests/`** — anything spawning
  `rfl-readfile`.
- **`rafaello/tests/`** — the headline `rfl chat` end-to-end
  test against the full plugin tree.

#### Positive matrix

`rafaello-core/tests/`:

- `broker_register_provider_happy_path.rs` — construct a
  broker with a `PluginAcl` carrying `provider_id =
  Some("mock")`; call `register_provider`; assert the guard
  drops cleanly and `contains_provider == true` during its
  lifetime.
- `broker_publish_provider_topic_authorised.rs` — register a
  provider; publish `provider.mock.assistant_message` (in
  `publish_topics`); assert the fan-out reaches a subscribed
  in-process recipient and the emitted `BusEvent` has
  `publisher: Provider { canonical, provider_id: "mock",
  topic_id }`, `request_id: Some(_)`, `in_reply_to:
  Some([_])`.
- `broker_publish_provider_carries_request_id.rs` — exercise
  the new `BusEvent.request_id` round-trip from `PublishMsg`
  to the emitted event.
- `broker_publish_core_with_taint_happy_path.rs` —
  `publish_core_with_taint("core.session.tool_request", …,
  taint=[{source: "provider", detail: "mock"}])` succeeds;
  fan-out delivers an event whose `taint` matches.
- `reemit_provider_tool_request_to_core_session_tool_request.rs`
  — drive a provider publish; observe the re-emitted
  `core.session.tool_request` with canonical taint
  `[{source: "provider", detail: "mock"}]`,
  `dispatch_target` payload field populated.
- `reemit_plugin_tool_result_to_core_session_tool_result.rs`
  — drive a plugin tool_result publish; observe canonical
  re-emit with taint `[{source: "tool", detail: <canonical>}]`,
  `in_reply_to` forwarded.
- `reemit_frontend_user_message_to_core_session_user_message.rs`
  — drive a frontend `frontend.tui.user_message`; observe
  canonical re-emit with taint `[{source: "user"}]`.
- `agent_loop_dispatches_tool_request_to_target_plugin.rs` —
  drive a `core.session.tool_request` with
  `dispatch_target` set; the agent loop publishes the
  corresponding `plugin.<topic-id>.tool_request`.
- `agent_loop_persists_user_message_entry.rs` — assert a
  `core.session.user_message` event causes a row in the
  `entries` table with `kind = "user_message"`.
- `agent_loop_persists_tool_call_entry.rs`,
  `agent_loop_persists_tool_result_entry.rs`,
  `agent_loop_persists_assistant_message_entry.rs` —
  analogous.
- `provider_plugin_spawns_through_supervisor.rs` — the
  successor named in §M2.2 above.
- `frontend_register_with_broker.rs` — the m3 retro §5.9
  granularity gap closer.
- `frontend_publish_user_message_reemitted_as_core_session_user_message.rs`
  — m3 §2.10 handover completion.

`rafaello-mockprovider/tests/`:

- `mockprovider_emits_tool_request_for_read_file_pattern.rs`
  — spawn `rfl-mockprovider` against an in-test broker
  fixture; deliver a synthetic `core.session.user_message`
  with text "what's in README.md"; observe a
  `provider.mock.tool_request` with
  `{tool: "read-file", args: {path: "README.md"}}` and
  proper `request_id` / `in_reply_to`.
- `mockprovider_emits_echo_assistant_message_on_no_match.rs`
  — same setup, payload "hello"; observe
  `provider.mock.assistant_message` with `{text: "echo:
  hello"}`.
- `mockprovider_emits_assistant_message_on_tool_result.rs` —
  inject a `core.session.tool_result` with content "Hello!";
  observe `provider.mock.assistant_message` with
  `text` beginning `"Here's what's in"`.

`rafaello-readfile/tests/`:

- `readfile_returns_content_for_existing_file.rs` — spawn
  `rfl-readfile` against a tempdir project root containing a
  `README.md`; deliver a synthetic
  `plugin.<own-topic-id>.tool_request` for
  `{path: "README.md"}`; observe `tool_result` with `ok:
  true, content: "<file body>"`.
- `readfile_errors_for_missing_file.rs`,
  `readfile_errors_for_outside_project_root.rs`,
  `readfile_errors_for_non_utf8.rs` — analogous error paths.

`rafaello/tests/`:

- `rfl_chat_demo_bar_read_file.rs` — **headline test, lands
  at the end of the milestone.** Spawn `rfl chat` against a
  tempdir project root containing a `README.md` with known
  content; the `rafaello.lock` is pre-materialised with
  `rfl-mockprovider` (active) + `rfl-readfile` installed.
  Drive the TUI's `frontend.tui.user_message` publish via a
  test-mode env hook (`RFL_TUI_TEST_MESSAGE="what's in
  README.md"` — new env hook in §F3 / rafaello-tui), or via
  the existing `RFL_HARNESS_FIXTURES` style. Assert (in
  order):
  - SQLite `entries` table contains rows of kinds
    `user_message`, `tool_call`, `tool_result`,
    `assistant_message` in seq order;
  - the combined stderr stream contains the canonical
    `"rfl-tui: bus.event topic=core.session.entry.finalized
    seq=N"` lines for `N = 0..=3`;
  - the assistant message's text begins with
    `"Here's what's in README.md:"` followed by the file's
    body.

#### Negative matrix

The roadmap row enumerates six negative demos. The mapping
to test files:

- **`tool_result` missing `in_reply_to` rejected** →
  `rafaello-core/tests/broker_plugin_tool_result_missing_in_reply_to_rejected.rs`
  — *extends m2's existing test* (m2 already enforces
  `in_reply_to` on `tool_result`/`rpc_reply`); m4 adds a
  symmetric test on the `core.*` re-emit path
  (`reemit_plugin_tool_result_missing_in_reply_to_rejected.rs`)
  showing that the re-emit refuses to emit a
  `core.session.tool_result` without `in_reply_to`.
- **Provider tool_request with stale/unknown id fails closed**
  →
  `rafaello-core/tests/broker_provider_tool_request_missing_in_reply_to_rejected.rs`
  + `reemit_provider_tool_request_unknown_user_message_id_rejected.rs`
  (the second test shows that an `in_reply_to` citing a
  request_id never seen on `core.session.user_message`
  results in the re-emit path rejecting with a `core.lifecycle.reemit_rejected`
  event and no `core.session.tool_request` is fanned out).
- **Tool plugin called directly by another plugin (not via core
  re-emission) doesn't reach the dispatch path** →
  `rafaello-core/tests/cross_plugin_tool_request_blocked_at_broker.rs`
  — a non-provider plugin attempts to publish on
  `plugin.<other-topic-id>.tool_request`; m2 already rejects
  this with `PublishOnReservedNamespace` (the plugin can
  only publish on its own `plugin.<own-topic-id>.*`); m4
  adds a test that explicitly names the dispatch-path
  violation. Plus
  `cross_provider_request_to_tool_only_routes_via_core.rs`:
  show that a tool plugin receiving a request from
  `core.session.tool_request` directly (i.e. the tool plugin
  subscribes to `core.session.tool_request` rather than its
  own `plugin.<id>.tool_request`) does not dispatch — the
  re-emit path emits to `core.*` for *observation*, the
  agent loop alone reaches the tool plugin via the
  per-plugin namespace. The test asserts the tool plugin's
  subscribe pattern set does not include
  `core.session.tool_request` (m1 grant compiler should
  warn or refuse — round-1 records this as a Risk to
  validate with m1).
- **Tool requested outside its grant denied at lockin** →
  `rafaello-readfile/tests/readfile_denied_outside_grant.rs`
  — spawn `rfl-readfile` with `read_dirs = [<tempdir-A>]`;
  request `{path: "<tempdir-B>/foo"}`; tool result is
  `ok: false, error: "path denied"`.
- **Bus event missing the `taint` envelope rejected** →
  `rafaello-core/tests/broker_publish_core_session_tool_request_missing_taint_rejected.rs`
  — call `publish_core` directly on
  `core.session.tool_request` (without taint); broker errors
  `InvalidTaint { reason: "missing" }`. Plus
  `broker_publish_core_session_tool_result_missing_taint_rejected.rs`.
- **Plugin-supplied (rather than core-supplied) taint
  rejected** →
  `rafaello-core/tests/reemit_discards_plugin_supplied_taint_on_core_session_tool_request.rs`
  — drive a provider publish with `taint: [{source: "user"}]`
  (the provider trying to launder a tool_request as
  user-originated); the re-emit synthesises the canonical
  `[{source: "provider", detail: "mock"}]` and the
  emitted `core.session.tool_request` carries only that;
  the test asserts the provider's claimed taint is **not**
  in the emitted envelope.

Plus the m2-supervisor symmetry tests:

- `broker_publish_provider_id_mismatch_rejected.rs` —
  provider publishes on `provider.other.foo`;
  `PublishOnReservedNamespace`.
- `broker_publish_provider_two_segment_topic_rejected.rs` —
  `provider.mock`; symmetric to m2's plugin / m3's frontend
  two-segment rule.
- `broker_publish_provider_unknown_namespace_rejected.rs` —
  `evil.foo` from a provider; `UnknownNamespace`.
- `broker_publish_provider_outside_grant_rejected.rs` —
  `provider.mock.confidential` not in `publish_topics`;
  `PublishOutsideGrant`.
- `broker_register_provider_unknown_canonical_rejected.rs` —
  `ProviderNotInAcl`.
- `broker_register_provider_duplicate_rejected.rs` —
  `ProviderAlreadyRegistered`.

### H — test harness

m4 reuses m3's harness primitives where possible. New
additions:

- **H1.** `MockProviderHandle` — a struct in
  `rafaello-mockprovider/tests/common/` wrapping a spawned
  `rfl-mockprovider` child + the in-test broker fixture; same
  shape as m2's `m2_harness::FixtureHandle` and m3's
  `FrontendExtraServiceFactory`. Exposes
  `publish_user_message(&self, text: &str) -> JsonRpcId` and
  `recv_event(&self) -> BusEvent`.
- **H2.** `ReadFileToolHandle` — analogous for `rfl-readfile`.
- **H3.** `assert_origin_taint(event: &BusEvent, source:
  &str, detail: Option<&str>)` — common helper in
  `rafaello-core/tests/common/`. m4 grows the existing
  `common::session_test_kit` module.
- **H4.** **`assert_reemit_happened(event: &BusEvent)`** vs
  **`assert_fixture_published(event: &BusEvent)`** — paired
  asserts that distinguish whether an event on
  `core.session.tool_*` came through the re-emit path (the
  one m4 implements) or directly from a fixture publishing
  on `core.*` (which the broker now rejects in m4 — the
  m4 taint envelope check + `core` namespace ACL together
  prevent this). The pair encodes the m4 contract: in-test
  events on `core.*` either come from core's re-emit path
  or do not exist.

## Out of scope

The following are explicitly NOT in m4 and not allowed to
sneak in via implementation drift:

- **Sink classes, confirmation UI, `user_grants`, taint
  matching / propagation, the broker-side sink gate that
  consumes the envelope on sink calls, slash commands
  (`/grant`, `/grants list`, `/revoke`)** — all m5.
- **`rfl-openai` (the bundled default provider plugin
  per decisions row 38) and any OpenAI-Chat-Completions
  wire protocol code** — m5 (lands alongside sinks +
  confirmation) and m6 (end-to-end against a real
  endpoint).
- **Multiple active providers**, `rfl provider use <id>`
  command runtime semantics, provider hot-swap mid-session
  — post-v1 (overview §8 names the lock mutation, but
  m4's `provider_active` is read once at startup).
- **Sink-class on `read-file`**, `always_confirm = true`
  on `read-file` — even though m1 schema validates the
  field, m4's only tool is the read-only fixture and the
  enforcement path is m5. (Plain-language note: if
  `always_confirm = true` were set on `read-file` in m4,
  the broker has no confirmation path, so the request
  would either deadlock or pass-through. m4 manifests for
  fixtures set `always_confirm = false` to keep the m4
  surface unambiguous.)
- **Streaming entry patch ops** (`stream_state: "open"` /
  `"patch"`, `core.session.entry.appended` /
  `core.session.entry.patched` notifications). m4
  continues to emit `core.session.entry.finalized` with
  `stream_state: "final"` only (decisions row 28).
- **Helper plugins** (`bindings.helper_for`,
  `RFL_HELPER_FD`) — deferred to v2 per decisions row 26.
- **External UDS-attached frontends, `rfl serve`** —
  decisions rows 27, 34.
- **Subprocess plugin renderers** — decisions row 29.
  m4 reuses m3's built-in renderers for
  `user_message`/`tool_call`/`tool_result`/`assistant_message`
  via the existing `text`/`tool_call`/`tool_result` renderer
  registry; no new renderer kinds.
- **Multi-session daemon, attach-multiplexing, branching
  sessions** (`parent` field non-NULL) — post-v1.
- **Lazy-load orchestrator beyond what m2's supervisor
  already does**. m4's `rfl chat` spawns the provider
  plugin eagerly (`load.eager = true` per fixture
  manifest) and the tool plugin on first dispatch
  (`load.triggers.kind = "tool"`); the broader lazy-load
  policy + `/plugin start --skip-eager` flag are
  later-milestone territory.
- **Provider plugin renderers** — the assistant_message
  kind renders as `text` in m4; no plugin-side render
  customisation.
- **TUI command palette / slash commands** — m5+.
- **Audit log table** — m5 (confirmation answers audit).
- **macOS interactive smoke testing** — m4 dev work is
  Linux; macOS verified through CI only. macOS CI green
  remains a hard gate (m3 precedent).

## Risks

1. **`request_id` rollout requires a workspace cutover.**
   Adding `BusEvent.request_id` is source-breaking for
   every `BusEvent` consumer: m2 broker tests, m3 session
   tests, the m3 TUI test harness. Round-1 mitigation:
   land the cutover as **one consolidated commit** (m0
   §4.1 precedent) so the per-commit green-bar holds.
   The field is `Option<JsonRpcId>` with `serde(default)`
   so JSON deserialisation of m3-era payloads
   continues to work (None on absence), but
   constructor-site updates are required wherever a
   `BusEvent` literal is built. Document the size in the
   commit body.
2. **`Publisher::Provider` reshape break-radius.** Less
   severe than `request_id` (the variant is additive); but
   any code that exhaustively matches `Publisher` needs
   updating. m2 / m3 use `Publisher` only inside error
   variants — round-1 cut: confirm at commits.md time by
   `grep -rn 'match.*Publisher\|Publisher::' rafaello/`
   and pre-name affected call sites. Likely all internal.
3. **Provider plugin spawn introduces a new lockin failure
   mode if `RFL_PROVIDER_ID` env-var injection collides
   with m1's reserved-list (row 40).** Mitigation: PS4 +
   M1.1 extend both lists in the same commit. The
   fixture manifest must not declare `RFL_PROVIDER_ID` in
   its `env.set` / `env.pass`; m1 v3 catches this at
   compile.
4. **Tool-result routing back to the provider requires the
   provider to subscribe to `core.session.tool_result`.**
   Verify at commits.md time that m1's manifest validator
   accepts that subscription on a `provider = true`
   plugin. `bus.subscribes` is a freeform pattern list per
   m1; `core.session.**` is grammar-valid. Round-1 default:
   no issue, but flagged for verification.
5. **The demo bar uses `read-file` against `README.md` —
   the read-only grant must intersect with the project
   root.** The fixture lock pins `read_dirs =
   ["${PROJECT_ROOT}"]`; the headline test's tempdir is
   the project root. The test writes a fixture `README.md`
   into the tempdir before spawning `rfl chat`. Pi may
   push back: alternative is to read a specific
   sub-fixture (e.g. `notes.md`); round-1 stays with
   `README.md` because the roadmap row uses it explicitly.
6. **Reemit-as-internal-subscriber vs subscribe-on-fittings
   side**. The reemit router (CR1) subscribes
   *internally* — no fittings round-trip — to avoid the
   serialisation cost of every provider event going out
   over the broker's notify channel then back into core.
   The mechanism is a new
   `Broker::subscribe_internal(pattern, Sender<BusEvent>)`.
   Risk: this adds a side-channel that bypasses the
   broker's ACL. Mitigation: the side-channel is
   constructed only inside `rafaello-core` (no public
   `Sender` exposed to plugins) and the recipient is
   always Core; the publish-authority side is still ACL'd.
   The internal subscriber lives **alongside** fan-out,
   not before it, so the side-channel cannot leak events
   that would not have reached external subscribers.
7. **Two-level subprocess chain in the headline test.**
   m3's `rfl_chat_demo_bar.rs` already spawns `rfl chat`
   which spawns `rfl-tui`. m4 extends to a *four-level*
   chain (`rfl chat` → `rfl-tui` + `rfl-mockprovider` +
   `rfl-readfile`). Leak risk if any layer panics.
   Mitigations:
   - extend m2's fixture self-timeout
     (`RFL_FIXTURE_MAX_LIFETIME` — m3 retro §2.9) into
     `rfl-mockprovider` and `rfl-readfile`;
   - extend the existing SIGCHLD-style cleanup in
     `rfl chat` to cover all three children;
   - the deterministic test_done signal pattern from m3
     reused.
8. **m1's `publishes` literal-placeholder substitution
   may not exist** (§TP1 open question). Round-1 cut:
   investigate at commits.md drafting time; if absent,
   the §M1.3 commit lands the substitution. Worst-case
   workaround: hand-compute the topic-id from the
   canonical id and write the literal into the manifest
   (fragile but unblocks m4 while §M1.3 is pending).
9. **Provider-id mismatch detection at registration**
   (B5): the `register_provider` arg `provider_id` could
   diverge from the ACL's `PluginAcl.provider_id`.
   Round-1 cut: hard-error
   `BrokerError::ProviderIdMismatch`. Pi may prefer the
   simpler "drop the arg, read from ACL" path — that's
   a 5-line refactor if pi asks.
10. **`request_id` on the frontend side**. m3's TUI does
    not generate JSON-RPC ids today (m3's frontend
    `publish_topics = []`). m4's §F3 introduces TUI-side
    id generation; the mechanism reuses the existing
    fittings client's id allocator on the frontend's
    `PeerHandle`, but a fresh `JsonRpcId::from(uuid)` is
    needed for the `bus.publish` envelope's `request_id`
    field (distinct from the fittings RPC id of the
    `bus.publish` notification itself). Risk: confusion
    between the two id spaces. Mitigation: spell it out in
    the TUI's publish helper; document the distinction
    inline.
11. **macOS CI gate carries forward**. m3 made macOS CI a
    hard ratification gate. m4 introduces no new
    platform-specific syscalls (the agent loop uses only
    tokio + existing fittings transport; the new crates
    have no FS-syscall paths beyond standard Rust I/O).
    Default expectation: macOS CI green from day one. m2
    §5.7 push-to-CI-early lesson applies — push the new
    crates to CI as the W2/W3 commits land, not at
    retrospective.
12. **Tool plugin lazy-load timing**. The fixture sets
    `load.triggers = [{kind = "tool", tool = "read-file"}]`
    so `rfl-readfile` is spawned on first dispatch. The
    agent loop's `publish_for_tool_dispatch` must
    therefore trigger a supervisor spawn before the
    publish reaches the broker. m2/m3 do not yet
    implement lazy-spawn-on-publish — round-1 cut: m4
    bypasses lazy-load by making `rfl-readfile` eager
    (`load.eager = true`) in the m4 fixture lock, even
    though its manifest declares `load.eager = false`.
    Lock-side `bindings` overrides manifest `load`
    per m1 (the lock is the grant); m4's `rfl init`
    fixture writes `eager = true`. Pi may push back if
    this conflates lazy-load with m4 scope — the
    alternative is to land a lazy-spawn primitive in
    m4 (significantly more surface). Round-1 takes the
    eager-fixture path; lazy-spawn is m5+ scope.

## Internal split (driver guidance for `commits.md`)

Suggested grouping; `commits.md` picks final granularity.
Pi review may reshape. m4's surface is high (new broker
publisher class + new envelope field + two new plugin
crates + new agent loop + new re-emit module) — expect
**~20-26 commits sequential**, comparable to m3's 31.

1. **Workspace + crate scaffolds + m1 reserved-env
   extension (M1.1)** (W1-W4 + M1.1): ~2-3 commits. The
   `rafaello-mockprovider` and `rafaello-readfile`
   crate skeletons (Cargo.toml + lib.rs + bin
   placeholder) land here, separate from the actual
   plugin logic.
2. **Broker envelope cutover** (B1-B4): **one
   consolidated workspace cutover commit** for
   `BusEvent.request_id` + `Publisher::Provider` +
   `BrokerError` variants. Per m0 §4.1, breaking trait
   changes with multiple in-tree consumers cannot be
   staged. ~1 commit (large).
3. **Broker provider registration + handle_provider_publish**
   (B5-B6): ~2 commits. The RAII guard +
   namespace dispatch separation.
4. **Taint-envelope enforcement** (B7) +
   `publish_core_with_taint` +
   `MissingRequestId` / `InvalidTaint` variants: ~2
   commits.
5. **Fan-out extension to provider recipients + provider
   defence-in-depth ACL check** (B8-B10): ~1 commit.
6. **Frontend ACL extension (F1-F4)** + retro §5.9 test
   gap (`frontend_register_with_broker.rs`): ~1-2 commits.
7. **m2 row-39 refusal removal + supervisor provider
   path** (PS1-PS8 + M2.1-M2.3): ~2 commits. Synthetic
   stub successor lands in the same commit as the m2
   test deletion per `plans/README.md`.
8. **Re-emit pipeline (CR1-CR7)**: ~2-3 commits, one per
   wire direction or one consolidated. Includes
   `subscribe_internal` mechanism.
9. **Agent loop (AL1-AL8)** + tool dispatch surface
   (TD1-TD3): ~2 commits.
10. **`rafaello-mockprovider` plugin (PR1-PR4)** with
    its own integration tests: ~2 commits.
11. **`rafaello-readfile` plugin (TP1-TP4)** with its
    own integration tests: ~2 commits.
12. **Demo-bar headline + manual validation** (the
    `rfl_chat_demo_bar_read_file.rs` test +
    `manual-validation.md`): ~2 commits.

Forced-monolithic commits called out explicitly:

- **Step 2 (broker envelope cutover)** is the m4
  equivalent of m0 c08's API cutover. The commit body
  must say so.
- Step 7 bundles the m2 refusal removal with its
  positive successor test (synthetic-stub-successor
  rule).

Realistic total: **~22 commits sequential**. No m4a /
m4b split anticipated — the surface threads through
broker + supervisor + agent loop + two plugin crates
without natural chasms. If a split materialises during
Phase 3 (e.g. the agent loop blows out a budget),
owner-ratified mid-milestone; default is "ship m4 as
one milestone".

## Acceptance summary

m4 is done when:

- Every named test in the §"Positive" and §"Negative"
  matrices is implemented and passes. Tests may split or
  merge during `commits.md` drafting as long as the named
  behaviours are all covered.
- `nix develop --impure --command cargo test
  --manifest-path rafaello/Cargo.toml --workspace --features
  test-fixture` green on Linux inside the devshell.
- **macOS CI green is a hard ratification gate** (m3
  precedent); the `cargo test --workspace --features
  test-fixture` job on `macos-latest` must be green
  before retrospective ratification, with the only
  exception being tests explicitly gated
  `#[cfg(target_os = "linux")]` (carried forward from
  m3's frontend-handle-drop test).
- `nix develop --impure --command cargo build
  --manifest-path rafaello/Cargo.toml --workspace --bins
  --features rafaello-core/test-fixture` green. Verifies
  `rfl`, `rfl-tui`, `rfl-mockprovider`, `rfl-readfile`,
  and `rfl-bus-fixture` all build.
- `nix develop --impure --command cargo doc --manifest-path
  rafaello/Cargo.toml --workspace --no-deps` warning-free.
- `manual-validation.md` records an interactive `rfl chat`
  run against the fixture lock that demonstrates the demo
  bar (user types "what's in README.md", sees the file's
  contents rendered as an assistant message) plus the
  macOS CI URL.
- `retrospective.md` written with anticipated drift items
  addressed:
  - **Stream A security-RFC §10 v1-summary patch** — the
    overview §6.2 wording wins on the sink rule; m4 lands
    a banner-only patch to `streams/a-security/rfc-security-model.md`
    §10 pointing at overview §6.2 and decisions row 9.
    Already deferred by `milestones/README.md`
    §"Stream RFC drift".
  - **`PublisherIdentity::Provider` schema additions to
    Stream A.** Symmetric to m3's banner addition for
    `Frontend`; the wire-schema banner expands to include
    the new variant.
  - **decisions.md row for the `BusEvent.request_id`
    rollout** — new ratified row documenting that
    `request_id` is now mandatory on request-shaped
    topics (`*.tool_request`, `*.user_message`,
    `*.assistant_message`) at the broker.
  - **decisions.md row for the `Publisher::Provider`
    variant landing** — refines row 42.
  - **m1's `check_lock_publish_topic` unknown-namespace
    gap** (m3 retro §2.7) — m4 may close it if a
    failure surfaced; otherwise it stays as runtime-only
    enforcement and is re-filed for m5+.
  - **Provider-side env-var documentation in
    overview §4.6** — `RFL_PROVIDER_ID` and
    `RFL_PROVIDER_ACTIVE` get added to the reserved env
    vars table.
- No follow-up Stream RFC drift is owed by m4 BEYOND the
  items above. m4 does NOT modify Stream A's body in this
  branch (banner-only, m1 / m3 precedent).

m4 ships the first running **agent**: a user types
"what's in README.md" and gets the file's contents back as
an assistant reply, with every step (user_message,
tool_call, tool_result, assistant_message) flowing
through the canonical core re-emit + taint envelope path.
Every later milestone (m5 sinks + confirmation, m6 the
real OpenAI-compatible provider end-to-end) layers on
this primitive.
