# m4 — provider fixture + secure agent loop + read-only tool + taint envelope — scope

> **Status:** round-2 — addresses `pi-review-1.md` (b/8 h/5 m/4
> l/3). Pi recovered from the auto-compact and added the 3 Low
> items + Summary after the initial fold; the Low items landed
> as a small follow-on commit (`docs(rafaello-m4): scope.md
> round-2 follow-up — pi-review-1 Low items`).
> Trajectory: r1 8/5/4/3.
>
> Round-2 fixes (by pi-1 number):
>
> Blockers:
> - **B-1** New §C `rfl chat` orchestration section spells out
>   lock load → V3 → `compile_plugin` → `broker_acl::compile` →
>   `PluginSupervisor::new` → eager spawn of provider + tool →
>   shutdown. Lazy-spawn is **explicitly out of scope**; both
>   plugins are eager in the m4 fixture lock (Risk #12
>   resolved). New negatives for missing/invalid lock and
>   missing-provider/missing-tool spawn failures.
> - **B-2** Fixture manifests rewritten in the live m1 schema
>   (`schema = 1`, top-level `name`/`version`/`entry`/
>   `rafaello = ">=0.1, <0.2"`), `[load]` shape per m1 §M6,
>   `${project}` not `${PROJECT_ROOT}` (m1 closed placeholder
>   set per `manifest/capability_path_template.rs:17`), and
>   sibling `openrpc.json` files. New §PR3/§TP3 compile-tests
>   for both fixtures land before any subprocess test.
> - **B-3** Taint rule is now mechanically single-valued:
>   inbound `provider.*` / `plugin.*` `msg.taint` is **discarded
>   at the broker** for tool-shaped topics (`*.tool_request`,
>   `*.tool_result`); core re-emit synthesises the canonical
>   envelope solely from publisher-principal identity. B6
>   rewritten; the negative test renamed
>   `reemit_discards_plugin_supplied_taint_on_core_session_tool_request.rs`
>   keeps the "discards and replaces" semantics, and a sibling
>   `broker_provider_tool_request_with_supplied_taint_discards.rs`
>   asserts the broker drops the inbound `taint` before fan-out
>   to the internal subscriber.
> - **B-4** `in_reply_to` model aligned with security RFC
>   §7.2.6 (lines 1027-1036) verbatim:
>   - `provider.<id>.tool_request`: **required, ≥0 entries**,
>     each citing a `core.session.tool_result.request_id` the
>     provider has already received.
>   - `provider.<id>.assistant_message`: **required, ≥0
>     entries** (the conversation context the message is
>     replying to).
>   - `plugin.<id>.tool_result`: required, exactly one entry,
>     citing the matching tool_request (m2 already enforces).
>   - `frontend.<id>.user_message`: **optional** (user messages
>     are roots).
>   `core.session.tool_result.request_id` is **required** on
>   the wire (per security RFC §5.4.1 line 418 "request_id …
>   match[es] the subscribed tool binding"). New negative tests
>   added for provider `assistant_message` missing / empty /
>   stale-id `in_reply_to` (§I negatives).
> - **B-5** Provider inbound (`provider.<id>.tool_request` /
>   `assistant_message`) is **internal-intake-only**: the
>   broker validates publish authority + `in_reply_to`, sends
>   the validated `BusEvent` to the trusted `ReemitRouter`
>   in-process channel, and **does not fan out to external
>   subscribers**. B6/B9 rewritten. New positive test
>   `broker_provider_event_not_fanned_to_external_subscribers.rs`.
> - **B-6** `plugin.<topic-id>.tool_result` publish-grant
>   resolved **inside m4** as a compiler-inserted auto-publish:
>   `broker_acl::compile` extends the existing auto-subscribe
>   logic (line 98) to also auto-add
>   `plugin.<topic-id>.tool_result` to `publish_topics` for
>   any plugin with non-empty `bindings.tools`. The manifest
>   never declares the topic; m4 §M1.3 lands the compiler
>   change (m1 back-reach, additive — same shape as m1's
>   existing `auto_subscribes` insertion). §TP1 manifest now
>   has `publishes = []`.
> - **B-7** m2 row-39 test name fixed to the live file
>   `supervisor_spawn_provider_lock_refused.rs`. Successor
>   uses real `SpawnHandle::wait()` (not the invented
>   `wait_ready` — `wait_ready` exists on `FrontendHandle`,
>   not `SpawnHandle`; `supervisor.rs:134` is authoritative).
> - **B-8** Entry-construction rules pinned in §AL3/AL4/AL5/AL6:
>   user/assistant messages → `Entry { kind: "text", author:
>   User/Assistant, payload: TextPayload { text, markdown:
>   false } }`; tool requests/results → existing
>   `ToolCallPayload { id, name, args, status }` and
>   `ToolResultPayload { call_id, ok, content: RenderNode,
>   details }`. New tests assert rendered tree shape, not just
>   DB `kind` strings.
>
> High:
> - **H-1** `RFL_PROVIDER_ACTIVE` dropped. `rfl chat` only
>   spawns the active provider (single provider in m4), so the
>   env var is unnecessary. `RFL_PROVIDER_ID` retained.
>   PS3/PS4/PS5/Risk #3 updated.
> - **H-2** Provider `assistant_message` in_reply_to negative
>   matrix added (missing / empty array / unknown
>   tool_result id).
> - **H-3** Outside-grant test split into two: the existing
>   plugin-level path-traversal negative
>   (`readfile_errors_for_outside_project_root.rs`) **plus** a
>   sandbox-level negative `readfile_lockin_denies_outside_grant.rs`
>   that invokes `std::fs::read` directly on a path outside
>   `read_dirs` (bypassing the plugin's own ancestor check) and
>   asserts the lockin sandbox rejects it.
> - **H-4** Mock-provider parser pinned: strip trailing
>   `[.?!,;:]` from the captured path, store `request_id →
>   path` in an in-memory `BTreeMap` so the assistant reply
>   knows what file was requested. Tests cover `README.md?`,
>   missing files, multibyte UTF-8 content.
> - **H-5** `ProviderIdMismatch` dropped.
>   `register_provider(canonical, peer)` reads `provider_id`
>   from `PluginAcl.provider_id` (B5 simplified).
>
> Mediums:
> - **M-1** `subscribe_internal` now returns an RAII
>   `InternalSubscription` guard; bounded `tokio::sync::mpsc`
>   channel (default capacity 256) with drop-on-full +
>   `tracing::warn!`; specified to fire **before** external
>   fan-out (the internal subscriber observes the canonical
>   ordering); tests for unregister-on-drop and slow-receiver
>   behaviour added.
> - **M-2** TUI test-mode env hook `RFL_TUI_TEST_MESSAGE`
>   specified in new §T1; lands on rafaello-tui's
>   `ENV_PASS_ALLOWLIST`; `rfl chat`'s pass-list extended;
>   send timing pinned to "after `frontend.ready` resolves".
>   New test `rafaello-tui/tests/tui_sends_test_message_after_ready.rs`.
> - **M-3** Regex dep dropped. Mock-provider parser is a
>   hand-written deterministic matcher (case-fold prefix
>   match on `"what's in "` / `"what is in "`).
> - **M-4** Commit budget revised upward to ~26-32 commits;
>   pi-round budget acknowledged at 8+ for scope.
>
> Lows (follow-on commit):
> - **L-1** `Publisher` and `PublisherIdentity` enum shapes
>   pinned to one canonical form across the doc.
>   `Publisher::Provider { canonical: CanonicalId, provider_id:
>   String }` is the broker-error / authority enum;
>   `PublisherIdentity::Provider { canonical: String,
>   provider_id: String, topic_id: String }` is the wire-side
>   serialised event identity. Top-level deliverable item 1 +
>   B2 list entry rewritten to match §B1/§B3.
> - **L-2** Tool-name spelling convention pinned: **manifest
>   tool name and bus payload `tool:` field use `"read-file"`
>   (kebab-case)**; Rust crate / bin / module identifiers use
>   `read_file` / `readfile` (snake-case where required by
>   syntax). The security RFC's `read_file` taint-source
>   example (§7.2.1 around line 880) is a *taint source label*
>   (free-form `detail` string), not the routing key; m4
>   treats it as illustrative and does not adopt it as the
>   canonical spelling. All bus payload snippets in this
>   scope.md already use `"read-file"`; new §"Naming
>   conventions" subsection at the end of §TP records the
>   rule.
> - **L-3** Frontend-user-message taint-synthesis bullet
>   sharpened in §CR5: the broker's re-emit of
>   `frontend.tui.user_message` → `core.session.user_message`
>   sets `taint = [{source: "user", detail: None}]` per
>   security RFC §7.2.1 (lines 878-886). The frontend's
>   inbound `msg.taint` is **discarded** (consistent with the
>   provider/plugin discard rule in §B6 step 8); a fresh
>   `request_id` is assigned by core if the frontend did not
>   supply one. New test
>   `reemit_user_message_synthesises_user_taint.rs` asserts
>   the canonical envelope shape; sibling test
>   `reemit_user_message_discards_frontend_supplied_taint.rs`
>   proves a TUI that publishes `taint: [{source:
>   "provider"}]` cannot launder a message as
>   provider-originated.

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
   `Publisher::Provider { canonical, provider_id }` (new — row 42
   follow-through; exact shape pinned in §B1) into broker
   registration and injects one new
   env var (`RFL_PROVIDER_ID`) so the
   provider child knows its identity. No new supervisor type is
   introduced (see "Lock-correspondence claim, extended" below).
2. **Broker extension** in `rafaello_core::bus` /
   `rafaello_core::broker_acl` / `rafaello_core::error`:
   - `BusEvent.request_id: Option<JsonRpcId>` lands as a
     first-class envelope field (overview §4.5: "m2 omits this
     field; m4 adds it").
   - `Publisher::Provider { canonical: CanonicalId,
     provider_id: String }` variant (row 42 — m2 staged the
     reshape, m4 adds the third arm). Exact shape defined in
     §B1; the sibling `PublisherIdentity::Provider`
     (`bus.rs`-side serialised shape) carries an additional
     `topic_id` for symmetry with `Plugin` (§B3).
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
   `supervisor.rs:414-419` is deleted; the live m2 test
   `rafaello/crates/rafaello-core/tests/supervisor_spawn_provider_lock_refused.rs`
   (verified at draft time via `ls`) is deleted and a positive
   `provider_plugin_spawns_through_supervisor.rs` replaces it.
   The `ProviderNotInM2` variant of `InvalidPlanReason` is
   removed from `error.rs:401-403` (source-breaking; the only
   consumer is the m2 test). Synthetic-stub-test successor
   pattern is named in §M2 (per `plans/README.md`).
10. **`rfl chat` orchestration extension** (`crates/rafaello/src/lib.rs`):
    `run_chat` is extended to load `rafaello.lock` from the
    project root, run V3 validation (m1 `validate::validate_lock`),
    compile per-plugin `CompiledPlugin` plans via
    `compile_plugin`, compile the `BrokerAcl` via
    `broker_acl::compile`, construct the `PluginSupervisor`, and
    **eagerly spawn the active provider plus every installed
    tool plugin** before wiring the TUI. Plugin children are
    held through to shutdown via `SpawnHandle` clones; SIGTERM
    on `rfl chat` exit reaps cleanly. See §C below for the full
    step-by-step.
11. **Integration tests** under
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
  - §4.6 (reserved env vars — m4 adds `RFL_PROVIDER_ID`;
    `RFL_PROVIDER_ACTIVE` dropped per pi-1 H-1);
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
    `RFL_PROVIDER_ID` (only, per H-1) to
    `supervisor::RESERVED_ENV_VARS` and to m1's
    `scrubber.rs` `RESERVED_ENV_VARS` in the same commit
    (m1 v3 catches reserved-name use pre-compile; m2
    supervisor catches at spawn).
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
  #[error("invalid `taint` envelope on `{topic}` from publisher {publisher:?}: {reason}")]
  InvalidTaint { publisher: Publisher, topic: String, reason: TaintReason },
  ```
  And m2's existing `InReplyToReason` (`error.rs:312-316`) is
  extended:
  ```rust
  #[non_exhaustive]
  pub enum InReplyToReason {
      Missing,
      EmptyArray,
      UnexpectedMultiple,
      StaleRequestId { id: JsonRpcId },  // NEW in m4 — pi-1 B-4
  }
  ```
  A new structured `TaintReason` enum:
  ```rust
  #[non_exhaustive]
  pub enum TaintReason {
      Missing,                   // None on core.session.tool_*
      EmptyArray,                // Some(vec![])
      UnknownSource { source: String },
  }
  ```
  - `MissingRequestId` fires when a request-shaped topic
    (`*.tool_request`, `*.user_message`, `*.assistant_message`)
    arrives without `request_id`.
  - `InvalidTaint` covers the three canonical-side
    taint-envelope failure modes (missing, empty array,
    unknown source taxon). It is **never** fired by
    `handle_provider_publish` / `handle_plugin_publish` —
    inbound supplied taint is discarded (B6 step 8), not
    rejected.
  - `StaleRequestId` is the §7.2.6 enforcement signal for
    "in_reply_to cites an id the publisher hasn't observed"
    (provider tool_request / assistant_message).
  - **No `ProviderIdMismatch` variant** (pi-1 H-5):
    `register_provider(canonical, peer)` derives the
    `provider_id` from `PluginAcl.provider_id` directly; there
    is no caller-supplied id to mismatch against.
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
          peer: PeerHandle,
      ) -> Result<RegisteredProvider, BrokerError>;
      // provider_id is derived from PluginAcl.provider_id
      // inside register_provider; ProviderNotInAcl fires when
      // the ACL lacks the canonical or has provider_id = None
      // (the plan is malformed — m1 v3 already catches this,
      // but defence-in-depth at the broker is cheap).

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
  `registry` and `frontends`. `register_provider` reads the
  `provider_id` exclusively from `PluginAcl.provider_id`
  (pi-1 H-5 — no caller-supplied arg, no mismatch state).
  Round-2 default: if the `PluginAcl` for `canonical` has
  `provider_id = None`, registration returns
  `ProviderNotInAcl` (the ACL was built without recognising
  this plugin as a provider — almost certainly a m1
  compilation bug, but a typed error is the safer surface).
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
  6. **`in_reply_to` enforcement** per security RFC §7.2.6
     (lines 1027-1036, authoritative table):
     - `provider.<id>.tool_request`: **required, ≥0 entries**.
       Each entry must reference a
       `core.session.tool_result.request_id` the provider has
       already observed (the broker maintains a per-provider
       in-flight `BTreeSet<JsonRpcId>` of tool_result ids it
       has fanned out to this provider; an empty `[]` is
       valid — first-turn requests cite nothing).
       Missing field → `InvalidInReplyTo { reason: Missing }`;
       unknown / stale id in the list →
       `InvalidInReplyTo { reason: StaleRequestId }` (new
       variant — see B4 patch below).
     - `provider.<id>.assistant_message`: **required, ≥0
       entries**, each citing the conversation context the
       message is replying to (a prior
       `core.session.tool_result.request_id` or
       `core.session.user_message.request_id`). Same
       in-flight-map enforcement.
     - `plugin.<id>.tool_result`: m2 already enforces "required,
       exactly one entry"; m4 extends with
       `StaleRequestId` validation against the per-plugin
       outstanding-tool_request map the agent loop maintains.
     - `frontend.<id>.user_message`: **optional** per §7.2.6
       row 5 (user messages are roots; no taint to inherit).
     - `plugin.<a>.rpc_reply`: m2 already enforces "required,
       exactly one entry"; out of m4 scope for stale-id (no
       in-flight `rpc_call` map yet — m5+).
  7. **`request_id` requirement**: `tool_request`,
     `assistant_message`, `user_message` (request-shaped, may
     be cited by a future `in_reply_to`) → required;
     `tool_result`, `rpc_reply` (responses) → required on
     `core.session.tool_result` (per security RFC §5.4.1 line
     418), optional on the inbound plugin-side
     `plugin.<id>.tool_result` (the inbound event carries
     the same id that surfaces as the canonical
     `request_id` on the re-emitted `core.*` event — but the
     broker may synthesise a fresh id if the inbound is
     absent, since the outstanding-tool_request map already
     knows the correlation). Round-2 cut: **all request-shaped
     topics require `request_id`; all response-shaped topics
     have `request_id: Option`** with synthesis on the
     re-emit side if absent.
  8. **`taint` discard rule** (pi-1 B-3 fix): the inbound
     `msg.taint` is **never carried into the canonical `core.*`
     event** for tool-shaped topics. The broker has two
     options for the *inbound* `BusEvent` it places on the
     internal subscriber channel (B7 below):
     - **Option A (round-2 default): strip `taint` to `None`
       at the broker boundary** for `provider.<id>.*` and
       `plugin.<id>.tool_result` topics. The inbound event
       carries no provenance claim; the canonical envelope is
       computed exclusively by the re-emit synthesis (CR §below)
       from the publishing principal's identity.
     - Option B (rejected): error on supplied `taint` with
       `InvalidTaint`. Rejected because v1 plugins / providers
       may set `taint` for their own audit reasons on their
       own namespace (overview §4.5 says "plugin authors add
       to `taint` for their own published events"); a hard
       error would force every provider to omit it explicitly.
     The negative test
     `reemit_discards_plugin_supplied_taint_on_core_session_tool_request.rs`
     proves the discard: a provider publish with `taint:
     [{source: "user"}]` produces a canonical
     `core.session.tool_request` with `taint: [{source:
     "provider", detail: "mock"}]` — the provider's claim is
     **not** present in the emitted envelope.
  9. Emit the `BusEvent` with `PublisherIdentity::Provider
     { canonical, provider_id, topic_id }`, `taint = None`
     (per step 8). Hand off to step B7 (internal intake).
- **B7.** Provider / tool-result inbound is **internal-intake-
  only** (pi-1 B-5 fix). The validated `BusEvent` produced by
  `handle_provider_publish` (and the analogous tool-result
  inbound branch of `handle_plugin_publish` — see B7b) is
  routed into the trusted `ReemitRouter` queue, **not the
  external fan-out path**.
  - **`BrokerInner` grows an internal-subscriber field**:
    `internal_subscribers: Mutex<Vec<InternalSubscriberSlot>>`,
    where each slot holds a `Sender<BusEvent>` and a subscribe
    pattern. The `Sender` is created by
    `Broker::subscribe_internal(pattern) ->
    (Receiver<BusEvent>, InternalSubscription)`; the
    `InternalSubscription` is an RAII guard whose Drop removes
    the slot.
  - **For provider inbound** (B6 step 9), the broker calls
    `notify_internal_subscribers(&event)` and **does not**
    invoke `fan_out(..)`. Subscriber lists are walked once
    inside the state lock; the lock is dropped; per-recipient
    `try_send` runs after. External subscribers (plugins,
    frontends, other providers) never see the inbound
    `provider.<id>.*` event.
  - **For `plugin.<id>.tool_result` inbound** (B7b): m2
    already implements a "result-routing protection"
    short-circuit at `bus.rs:310-318` that *skips* external
    fan-out for `tool_result` / `rpc_reply` topics. m4
    extends that branch to **also** call
    `notify_internal_subscribers(&event)` so the ReemitRouter
    sees the inbound tool_result. The external-fan-out skip
    remains; the internal-intake side is additive.
  - The `core.session.*` canonical events are emitted by the
    re-emit path through a new `publish_core_with_taint`
    helper (B8 below). Those canonical events **do** flow
    through `fan_out` to external subscribers.
- **B7b.** Per-provider outstanding-tool_result map. To
  enforce the §7.2.6 "must reference a tool_result the
  provider has already received" rule, the broker tracks
  outstanding ids per `Publisher::Provider`. New shape on
  `BrokerInner`:
  ```rust
  // Keyed by provider canonical id.
  provider_observed_results:
      Mutex<BTreeMap<CanonicalId, BTreeSet<JsonRpcId>>>,
  ```
  Inserted on every `core.session.tool_result` fan-out
  delivered to a given provider; cleared when the matching
  `provider.<id>.tool_request` cites it (the cited id is
  *consumed* — first-time-use semantics; round-2 default).
  An empty `in_reply_to: []` on a provider tool_request is
  valid (first-turn). An id not in the set →
  `InvalidInReplyTo { reason: StaleRequestId }`.
  - Analogous structure for `assistant_message`: the agent
    loop may cite either a tool_result id or the originating
    user_message id; the broker therefore unions
    `provider_observed_results` with a
    `provider_observed_user_messages` set populated on
    `core.session.user_message` fan-out.
- **B8.** Taint-envelope synthesis on canonical re-emission.
  - **`Broker::publish_core_with_taint(topic, payload,
    request_id, in_reply_to, taint)`** is a new method that:
    - validates the topic is `core.*`;
    - **for `core.session.tool_request` /
      `core.session.tool_result`**: validates `taint` is
      `Some(non_empty_vec)` and every entry's `source ∈
      {"user", "provider", "tool", "system"}` (security RFC
      §7.2.1 taxon); on failure → `InvalidTaint`;
    - for other `core.*` topics: `taint` may be `None`;
    - emits the `BusEvent` with the supplied
      `request_id` / `in_reply_to` / `taint` and fans out
      externally (this is the only path that produces
      canonical `core.session.tool_*` events).
  - The existing `publish_core` becomes a thin wrapper that
    calls `publish_core_with_taint(topic, payload, None,
    None, None)`. Publishing `publish_core(
    "core.session.tool_request", _)` now errors
    `InvalidTaint { reason: "missing" }` — defence in depth
    against a future core path that forgot to use the
    taint-aware variant.
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
  band — `providers` — but **only for canonical `core.*`
  events**, never for inbound `provider.<id>.*` events
  (which are internal-intake-only per B7). Same shape as
  `plugin_recipients` and `frontend_recipients`: build the
  recipient list under the state lock, drop the lock, then
  per-recipient `peer.notify("bus.event", value.clone())`.
  Provider subscribers receive `core.session.tool_result`,
  `core.session.user_message` events on the patterns their
  manifest declares (e.g. `["core.session.user_message",
  "core.session.tool_result"]` for the mock provider).
  Round-2 exclusion rule: when fan-out emits a
  `core.session.tool_request` that was synthesised from a
  particular provider's `tool_request`, that provider is
  **excluded** from the recipient set (the agent loop alone
  consumes the canonical tool_request). This is the m4
  analogue of m2's `result-routing protection`
  (`bus.rs:310-318`).
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

### C — `rfl chat` orchestration extension

m3's `run_chat` (`crates/rafaello/src/lib.rs:107-220`) constructs
a `BrokerAcl` with empty `plugins` / `tool_routes` maps and never
loads a lock (`lib.rs:154-157`). m4 replaces that hard-coded ACL
construction with a real lock-driven path. This section is the
load-bearing orchestration spec for the demo bar — without it,
the headline test has no plugin tree to drive.

- **C1.** Project-root + lock-load. Step 1 (project root
  canonicalisation) is unchanged from m3. New step 1b: load
  `<project_root>/rafaello.lock` via `Lock::load(&path)` (m1
  surface: `rafaello-core/src/lock/lock_file.rs`). On missing
  lock → `RflChatError::LockNotFound { path }` (new variant);
  on parse error → `RflChatError::LockParse { source }`.
- **C2.** V3 validation. Call `validate::validate_lock(&lock)`
  (m1 surface; round-1 cut: confirm exact symbol name at
  commits.md time — m1's V3 entry point is in
  `validate/mod.rs`). On error → `RflChatError::LockValidation
  { source }`. m4 cannot skip V3 because the broker ACL
  compilation `spot_check_v3` path
  (`broker_acl.rs:88`) expects V3 to have already run.
- **C3.** Per-plugin plan compilation. For each
  `(canonical, entry)` in `lock.plugins`, call
  `compile_plugin(&lock, canonical)` (m1 surface) → `CompiledPlugin`.
  Collect into `BTreeMap<CanonicalId, CompiledPlugin>`. On
  error → `RflChatError::CompilePlugin { canonical, source }`.
- **C4.** Broker ACL compilation. `let acl =
  broker_acl::compile(&lock)?`. Then **extend** the resulting
  `acl.frontends` with the `tui` `FrontendAcl` entry per §F
  below (subscribe = `["core.session.**", "core.lifecycle.**"]`,
  publish = `["frontend.tui.user_message"]`). The m1 compiler
  emits `acl.frontends = BTreeMap::new()` (`broker_acl.rs:142`);
  the frontend wiring stays in `rfl chat` because frontends
  are not plugins.
- **C5.** Broker + supervisor construction.
  `let broker = Broker::new(acl)?;` (existing). New:
  `let supervisor = PluginSupervisor::new(broker.clone(),
  SupervisorConfig::default());` (m2 surface — verify exact
  ctor signature at commits.md time; round-1 cut matches
  `supervisor.rs:259-280` shape).
- **C6.** Compute `SpawnPaths` per plugin. For each
  `(canonical, plan)` in C3's map:
  - `project_root = <C1 project root>`;
  - `private_state_dir = project_root /
    ".rafaello-plugin-data" / plan.topic_id` (per decisions
    row 37). The dir is created in C7 by the supervisor (m2
    §SP4); m4 does not pre-create.
- **C7.** Eager spawn of the active provider. Look up
  `lock.session.provider_active: Option<String>` (m1 surface,
  `lock/session.rs:11`). On `None` → m4 demo bar cannot run;
  surface `RflChatError::NoActiveProvider`. On `Some(canonical_str)`:
  - parse via `CanonicalId::parse`;
  - look up the matching `CompiledPlugin` in C3's map;
  - call `supervisor.spawn(&plan, &paths).await?` →
    `SpawnHandle`; store in a `Vec<SpawnHandle>` held in
    `run_chat`'s local scope. On error →
    `RflChatError::ProviderSpawnFailed { canonical, source }`.
- **C8.** Eager spawn of every installed tool plugin. For
  each `(canonical, entry)` where
  `!entry.bindings.tools.is_empty() && !entry.bindings.provider`:
  - `supervisor.spawn(&plan, &paths).await?`;
  - store the `SpawnHandle` in the same `Vec`.
  - **Rationale**: m4 eager-loads every tool to avoid
    introducing lazy-spawn-on-publish in this milestone (Risk
    #12 round-1). The fixture lock has exactly one tool
    plugin (`rfl-readfile`). m5+ may add lazy-spawn once a
    second tool exists.
- **C9.** Reemit router + agent loop construction (after
  plugin spawns so the broker registry is populated):
  - `let router = ReemitRouter::new(broker.clone(),
    acl.tool_routes.clone(), provider_canonical.clone()); let
    router_join = router.start();`
  - `let agent = AgentLoop::new(broker.clone(), acl.clone(),
    Arc::new(controller)); let agent_join = agent.start();`
  Both tasks subscribe before the TUI starts (C10), so the
  user_message → reemit → tool_request → tool_result chain
  is wired before any input arrives.
- **C10.** Frontend (TUI) spawn — unchanged from m3 except
  the `CompiledFrontend.env.pass` allowlist grows to include
  `RFL_TUI_TEST_MESSAGE` (per §T1 / M-2).
- **C11.** Wait loop. `tokio::select!` on:
  - `handle.wait_ready()` → on `Ok(Ok(()))`, run
    `controller.replay_history` (m3 path);
  - TUI exit (`handle.wait().await`) → trigger shutdown;
  - shutdown signal (Ctrl-C / SIGTERM).
- **C12.** Shutdown. `supervisor.shutdown(grace).await`
  reaps every spawned plugin (m2 surface). `router_join`
  and `agent_join` observe the shared shutdown
  `watch::Receiver<bool>` and exit. The TUI handle reaps
  via m3's path. Order: signal shutdown → wait for tasks
  → supervisor.shutdown → drain stderr forwarder.
- **C13.** New `RflChatError` variants (additions to
  m3's existing enum in `rafaello/src/lib.rs`):
  - `LockNotFound { path: PathBuf }`,
  - `LockParse { source: LockError }`,
  - `LockValidation { source: ValidationError }`,
  - `CompilePlugin { canonical: CanonicalId, source: CompileError }`,
  - `NoActiveProvider`,
  - `ProviderSpawnFailed { canonical: CanonicalId, source: SpawnError }`,
  - `ToolSpawnFailed { canonical: CanonicalId, source: SpawnError }`.
- **C14.** Negatives for the orchestration path:
  - `rfl_chat_missing_lock_errors.rs` — no `rafaello.lock` at
    project root; exit non-zero with `LockNotFound`.
  - `rfl_chat_invalid_lock_errors.rs` — corrupt TOML;
    `LockParse`.
  - `rfl_chat_lock_validation_fails.rs` — lock with an
    invalid `bindings.tools` entry; `LockValidation`.
  - `rfl_chat_no_active_provider_errors.rs` — valid lock
    with `session.provider_active = None`; `NoActiveProvider`.
  - `rfl_chat_provider_spawn_failure_propagates.rs` —
    fixture lock points at a non-existent provider binary;
    `ProviderSpawnFailed`.

### T — TUI test-mode env hook (pi-1 M-2)

- **T1.** New env var **`RFL_TUI_TEST_MESSAGE`** read by the
  TUI binary (`rafaello-tui/src/bin/rfl_tui.rs`) at startup:
  - If set and non-empty, after the TUI's
    `peer.call("frontend.ready", …)` resolves and the
    `BusEventHandler` is registered, the TUI publishes a
    single `frontend.tui.user_message` containing
    `{text: <env-value>}` and a freshly-allocated
    `request_id` (a new `JsonRpcId::String` synthesised
    from a `Ulid::new().to_string()`).
  - If unset, the TUI runs the normal interactive prompt.
  - The env var is added to `rafaello-tui`'s
    `ENV_PASS_ALLOWLIST` (the in-crate constant that
    documents what the bin reads) and to
    `rafaello/src/lib.rs`'s `ENV_PASS_ALLOWLIST` for
    `CompiledFrontend.env.pass` so `rfl chat` propagates
    it to the spawned child.
  - **Test**:
    `rafaello-tui/tests/tui_sends_test_message_after_ready.rs`
    — spawn `rfl-tui` in `RFL_TUI_TEST_MODE=1` with
    `RFL_TUI_TEST_MESSAGE="what's in README.md"`; in the
    parent-side broker fixture, register a callback on the
    `FrontendReadyService`; await the ready signal, then
    await the `frontend.tui.user_message` publish; assert
    the payload's `text` matches.

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
- **PS3.** Inject **`RFL_PROVIDER_ID`** (only) into the child
  env when the spawn plan has `acl_provider_id = Some(_)`:
  - `RFL_PROVIDER_ID = <provider_id>` (e.g. `"mock"`) — the
    public namespace authority key, sourced from
    `PluginAcl.provider_id` via the broker ACL.
  - **`RFL_PROVIDER_ACTIVE` is NOT injected** (pi-1 H-1):
    `PluginSupervisor::spawn` does not receive
    session-activeness state (`&CompiledPlugin` + `&SpawnPaths`
    only — `supervisor.rs:310-314`); routing activeness via
    a new spawn-plan field or a new spawn arg adds surface
    without a v1 consumer. m4's `rfl chat` spawns only the
    active provider (§C7), so every provider child is active
    by construction. Multi-provider activeness is m6+ scope.
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
  ];
  ```
- **PS5.** Extend m1's `scrubber.rs` `RESERVED_ENV_VARS` to
  match (row 40 mirror) — add `RFL_PROVIDER_ID` only. m1 v3
  catches reserved-name use at manifest compile time; the m4
  §M1.1 row records this as the m1 back-reach (default: same
  commit as PS4 — the two lists must move together).
- **PS6.** Provider broker registration. At the broker
  registration step in `PluginSupervisor::spawn` (currently
  `Broker::register_plugin` for non-provider plugins), branch
  on `acl_provider_id`:
  ```rust
  let registered: ProviderOrPlugin = match acl_provider_id {
      Some(_) => ProviderOrPlugin::Provider(
          self.broker.register_provider(plan.canonical.clone(),
              peer.clone())?,
      ),
      None => ProviderOrPlugin::Plugin(
          self.broker.register_plugin(plan.canonical.clone(),
              peer.clone())?,
      ),
  };
  ```
  with a `ProviderOrPlugin` newtype enum (or `Either`)
  carrying the appropriate RAII guard into `ManagedSpawn`.
  The provider id is read from `PluginAcl.provider_id`
  inside `register_provider` (pi-1 H-5).
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
  canonical id from the lock. The router subscribes via the
  new internal-subscriber API (pi-1 M-1 — lifecycle and
  backpressure specified up front):
  ```rust
  impl Broker {
      /// Register an in-process subscriber. The returned
      /// guard removes the slot on Drop. The channel is
      /// bounded (default cap 256) with drop-on-full
      /// + tracing::warn!; ordering is deterministic
      /// (internal notify runs **before** external fan-out
      /// inside the broker's lock-drop sequence, so the
      /// router observes events at or before any external
      /// subscriber sees them — pi-1 M-1 ordering rule).
      pub fn subscribe_internal(
          &self,
          patterns: Vec<String>,
          capacity: usize,
      ) -> (mpsc::Receiver<BusEvent>, InternalSubscription);
  }

  pub struct InternalSubscription {
      broker: Arc<BrokerInner>,
      slot_id: u64,
  }
  // Drop releases the slot from BrokerInner.internal_subscribers.
  ```
  The internal subscriber is not ACL-gated because it is
  part of core's trusted internal composition (it can only
  be constructed by code with a `&Broker`, which means
  inside `rafaello-core`). The `slot_id` is monotonically
  assigned so a Drop after the broker has already cleared
  state (shutdown) is a no-op rather than corrupting an
  unrelated slot.
  - **Bounded behaviour**: on `try_send` failure (channel
    full), the broker logs `tracing::warn!(slot_id, topic,
    "internal subscriber dropped event — channel full")`
    and continues to the next subscriber. No backpressure
    on the publisher.
  - **Send failure on receiver-dropped**: `try_send`
    returns `TrySendError::Closed`; the broker logs at
    `tracing::debug!` and the slot is left in place until
    Drop on the matching `InternalSubscription` clears it.
  - **Ordering relative to external fan-out**: the broker
    notifies internal subscribers first (inside the same
    `fan_out` body, before iterating plugin / frontend /
    provider recipients). This is load-bearing for the
    reemit router because the canonical `core.*` event
    must be observable to the router before any external
    consumer can re-publish based on it.
  - **Tests**:
    `broker_internal_subscriber_unregister_on_drop.rs`,
    `broker_internal_subscriber_drops_event_when_full.rs`,
    `broker_internal_subscriber_fires_before_external_fan_out.rs`.
- **CR2.** Re-emission steps for **provider →
  core.session.tool_request**:
  1. Receive `BusEvent { topic: "provider.mock.tool_request",
     payload, publisher: Provider{…, provider_id}, request_id:
     Some(rid), in_reply_to: Some(prior_tool_result_ids),
     taint: None (broker discarded inbound — B6/B8) }` from
     the internal subscriber.
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
     origin half. Propagation half — unioning the cited
     tool_results' taint — deferred to m5.)
  5. Call `broker.publish_core_with_taint(
       "core.session.tool_request",
       json!({tool: <name>, args: <args>, dispatch_target:
         <canonical-id>}),
       Some(rid),                          // forwarded from provider
       Some(prior_tool_result_ids),        // forwarded — ≥0 entries
                                           // per §7.2.6 row 2
       Some(taint))`.
- **CR3.** Re-emission steps for **plugin →
  core.session.tool_result**:
  1. Receive `BusEvent { topic: "plugin.<topic-id>.tool_result",
     payload, publisher: Plugin{canonical, topic_id},
     request_id: <inbound, may be None>, in_reply_to:
     Some([tool_request_id]), taint: None (broker discarded) }`.
  2. Look up `canonical` in `BrokerAcl.plugins` to confirm it
     is a known tool plugin (defence in depth).
  3. Synthesise the canonical `request_id`. The result event
     **must carry `request_id`** on the canonical wire per
     security RFC §5.4.1 line 418 ("request_id, tool name,
     and result schema match the subscribed tool binding").
     - If the inbound carries `request_id`, forward it.
     - If the inbound omits it (the tool plugin author chose
       not to set one), the agent loop's per-plugin
     `outstanding_tool_requests: BTreeMap<JsonRpcId,
     ToolDispatchContext>` (AL5) holds the originating
     `tool_request.request_id`, which is reused as the
     result's canonical `request_id`. This guarantees the
     provider can correlate.
  4. Synthesise taint: `vec![TaintEntry{source: "tool".into(),
     detail: Some(canonical.to_string())}]`. (m4: origin only.
     m5 will additionally `concat` the originating
     tool_request's taint per security RFC §7.2.2.)
  5. `publish_core_with_taint("core.session.tool_result",
     payload, Some(canonical_request_id),
     Some([tool_request_id]), Some(taint))`.
- **CR4.** Re-emission steps for **provider →
  core.session.assistant_message**: payload pass-through;
  taint = `[{source: "provider", detail: provider_id}]`;
  `in_reply_to` forwarded; `request_id` forwarded.
- **CR5.** Re-emission for **frontend.tui.user_message →
  core.session.user_message** (pi-1 L-3 — pinned). The
  user-message root event is the only canonical `core.*` event
  whose taint source is `"user"`; the synthesis lives here, not
  on any other code path.
  1. Receive `BusEvent { topic: "frontend.tui.user_message",
     payload, publisher: Frontend{attach_id: "tui"},
     request_id: <maybe>, in_reply_to: None, taint: <broker
     discarded — see step 3> }` from the internal subscriber.
  2. Validate payload deserialises to `{text: String}`.
  3. **Discard any frontend-supplied `taint`** (consistent
     with the provider/plugin discard rule in §B6 step 8; the
     broker already strips it before delivering to the
     internal subscriber, but the re-emit path re-asserts the
     invariant by ignoring the inbound field even if a future
     refactor stops stripping at the broker).
  4. **Synthesise canonical user-source taint**:
     `vec![TaintEntry { source: "user".into(), detail: None
     }]`. Per security RFC §7.2.1 (lines 878-886) the
     user-source label carries no `detail` because the user
     is a singleton principal in the v1 trust model. m4 is
     the **only** point in v1 where the `"user"` taxon
     originates.
  5. `request_id` handling: if the frontend supplied one,
     forward it; if absent, synthesise a fresh
     `JsonRpcId::String(Ulid::new().to_string())` —
     `core.session.user_message.request_id` must be present
     on the canonical wire so a provider can cite it in a
     future `assistant_message.in_reply_to`. (Round-1 errored
     on missing `request_id`; round-2 + L-3 cut: synthesise
     instead, since the user is a singleton and a missing
     id is benign.)
  6. `in_reply_to` is forced to `None`: user messages are
     conversation roots and inherit no prior taint (security
     RFC §7.2.6 row 5 confirms the optional + root semantic).
  7. Call `publish_core_with_taint(
       "core.session.user_message",
       json!({text: <text>}),
       Some(canonical_request_id),
       None,                              // user messages are roots
       Some(vec![TaintEntry{source: "user".into(), detail: None}]),
     )`.
  Tests:
  `reemit_user_message_synthesises_user_taint.rs`,
  `reemit_user_message_discards_frontend_supplied_taint.rs`,
  `reemit_user_message_synthesises_request_id_when_absent.rs`.
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
  - persist as an `Entry` with `kind = "text"`,
    `metadata.author = EntryAuthor::User`, `payload =
    TextPayload { text, markdown: false }` (pi-1 B-8 fix —
    m3's renderer registry has no `user_message` kind;
    `renderer/mod.rs:112-122` lists the eight built-in
    kinds, and `Entry::new_text` is the canonical
    constructor). The wire payload's
    `core.session.user_message.text` field maps directly
    into `TextPayload.text`.
  - The TUI renders this as a user-attributed text bubble
    via the existing `TextRenderer` + the
    `EntryAuthor::User` distinction (m3 renderer pipeline).
  - No further action — the provider plugin is the consumer
    (via fan-out on its subscribe set).
- **AL4.** Per `core.session.assistant_message` event:
  - persist as `Entry` with `kind = "text"`,
    `metadata.author = EntryAuthor::Assistant`, `payload =
    TextPayload { text, markdown: false }` (the canonical
    `Entry::new_text` shape at `entry/mod.rs:109-118`).
- **AL5.** Per `core.session.tool_request` event (the re-emitted
  version with `dispatch_target: <canonical-id>` in payload —
  see CR2):
  - persist as `Entry` with `kind = "tool_call"`,
    `metadata.author = EntryAuthor::Assistant`, `payload =
    ToolCallPayload { id: <request_id-as-string>, name:
    <tool>, args: <args>, status: ToolCallStatus::Pending }`
    (`entry/payloads.rs:47-52`). The `id` field is the
    canonical `request_id` rendered as a string so the
    matching `tool_result` entry (AL6) can be correlated
    via `ToolResultPayload.call_id`;
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
  - persist as `Entry` with `kind = "tool_result"`,
    `metadata.author = EntryAuthor::Tool`, `payload =
    ToolResultPayload { call_id: <in_reply_to[0]
    as-string>, ok, content: RenderNode::Text { ... } or
    Code { ... } from the file body, details }`
    (`entry/payloads.rs:56-62`). Round-2 cut: the readfile
    tool's content is wrapped as
    `RenderNode::CodeBlock { code: <content>, lang: None }`
    so the TUI renders it inside a code block. Future
    tools may emit richer `RenderNode`s.
  - **Also update** the prior `tool_call` entry's status
    field to `ToolCallStatus::Ok` / `Error` via a new
    `SessionStore::update_entry` helper, or simpler:
    round-2 cut leaves the `tool_call`'s status at
    `Pending` and lets the `tool_result` entry carry the
    final state. Pi may push back on the update path; the
    simpler "no in-place updates" cut keeps SQLite
    append-only.
  - No further action — the provider plugin observes the
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
  `rafaello/fixtures/rafaello-mockprovider/rafaello.toml`, in
  the live m1 schema (pi-1 B-2 fix; m1 scope §M1 + §M6 + §M8
  + §M10 + the closed placeholder set at
  `manifest/capability_path_template.rs:17`):
  ```toml
  schema   = 1
  name     = "mockprovider"
  version  = "0.0.0"
  entry    = "bin/rfl-mockprovider"
  rafaello = ">=0.1, <0.2"

  [provides]
  provider = "mock"

  [bus]
  subscribes = ["core.session.user_message",
                "core.session.tool_result"]
  publishes  = ["provider.mock.tool_request",
                "provider.mock.assistant_message"]

  [capabilities.default.filesystem]
  read_dirs  = []
  write_dirs = []

  [capabilities.default.network]
  mode = "deny"

  [load]
  eager = true
  ```
  Plus a sibling `openrpc.json` (required by m1 §M10 — the
  package-level validator refuses without it). The m4 mock
  provider exposes no JSON-RPC methods of its own
  (interaction is bus-only) but the OpenRPC document is still
  required; the minimum valid shape is an `openrpc` version
  + an `info` block + `methods: []`.
  The corresponding lock entry (m4 fixture) pins
  `bindings.provider = true`, `bindings.provider_id = "mock"`.
  `lock.session.provider_active = "<canonical>"` (the canonical
  id of this plugin entry).
- **PR2.** Bin target `src/bin/rfl_mockprovider.rs`. The
  provider is a small `tokio` binary that reads env, opens
  the fittings peer on `RFL_BUS_FD`, holds three pieces of
  per-session state, and runs a content-pattern matcher
  driven by `bus.event` notifications.
  - **Env**: `RFL_BUS_FD`, `RFL_PROVIDER_ID` (always
    `"mock"`), `RFL_PLUGIN` (canonical id, for logging),
    `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`,
    `RFL_PRIVATE_STATE_DIR`. (No `RFL_PROVIDER_ACTIVE` —
    pi-1 H-1.)
  - **State** (all `Mutex<…>`, single tokio task):
    - `outstanding: BTreeMap<JsonRpcId, String>` — maps
      provider-issued `tool_request.request_id` → captured
      path. Lets `tool_result` handling know what file was
      requested (pi-1 H-4).
    - `last_user_message: Option<JsonRpcId>` — the most
      recent `core.session.user_message.request_id` seen,
      so a no-tool path can cite it in
      `assistant_message.in_reply_to` (per security RFC
      §7.2.6 row 3: assistant_message ≥0 entries citing
      conversation context).
    - `seen_tool_results: BTreeSet<JsonRpcId>` — the
      `core.session.tool_result.request_id` values seen, so
      a follow-up `tool_request` can cite a prior result
      per §7.2.6 row 2 (≥0 entries citing tool_results the
      provider has already received).
  - **Handler logic** per `bus.event`:
    - `topic == "core.session.user_message"`:
      - extract `payload.text` and `bus_event.request_id`;
      - update `last_user_message` to that id;
      - run the deterministic matcher (pi-1 M-3 — no
        regex dep). The matcher:
        1. lowercases the input and trims;
        2. strips a leading `"what's in "` or `"what is in "`
           (case-fold prefix match); if neither prefix
           matches → echo path;
        3. takes the remaining slice, splits on the first
           ASCII whitespace, takes segment 0 as the
           candidate path;
        4. strips trailing punctuation in `[.?!,;:]`;
        5. rejects empty / pure-whitespace residues
           (falls through to echo).
      - on **match**: synthesise a fresh `request_id` (a
        ULID stringified, since the provider has no JSON-RPC
        connection-scoped id to allocate — m4 cut: ULIDs
        wrap as `JsonRpcId::String`); record `outstanding[request_id] =
        path`; publish
        `provider.mock.tool_request` with:
        - payload `{tool: "read-file", args: {path:
          "<path>"}}`,
        - `request_id: <fresh>`,
        - `in_reply_to:
          seen_tool_results.iter().cloned().collect()`
          (≥0 entries citing every prior tool_result the
          provider has observed — typically `[]` on
          first turn). Per §7.2.6 row 2.
      - on **no-match** (echo path): publish
        `provider.mock.assistant_message` with payload
        `{text: format!("echo: {}", input)}`,
        `request_id: <fresh>`,
        `in_reply_to:
          [last_user_message.unwrap()]` (citing the
          conversation context — §7.2.6 row 3 ≥0 entries;
          m4 cut always cites the immediate user_message
          when echoing, since that's the only context).
    - `topic == "core.session.tool_result"`:
      - extract `in_reply_to[0]` (the cited
        `tool_request.request_id`) and `payload.content`;
      - **append `bus_event.request_id` to
        `seen_tool_results`** (so the next provider
        tool_request can cite it per §7.2.6 row 2);
      - look up `outstanding[in_reply_to[0]]` to recover
        the original path (pi-1 H-4 — explicit
        request_id → path mapping); on miss, log + ignore
        (the provider never issued the matching request);
      - on hit, publish
        `provider.mock.assistant_message` with
        - payload `{text: format!("Here's what's in {}:\n{}",
          path, content_text)}` where `content_text` is
          extracted from `ToolResultPayload.content`
          (m4 wraps as `RenderNode::CodeBlock` per AL6;
          the mock provider unwraps it back to text — a
          fragile shape, but acceptable for the
          deterministic fixture);
        - `request_id: <fresh>`;
        - `in_reply_to: [<bus_event.request_id of the
          tool_result>]` (one entry citing the result the
          message is replying to).
  - **Round-2 explicit decision**: the mock provider does
    not subscribe to `core.session.assistant_message` (its
    own re-emitted output) — that would be a feedback loop;
    the canonical-`core`-subscribe set is exactly the
    manifest's `subscribes`
    (`["core.session.user_message",
    "core.session.tool_result"]`).
- **PR3 (compile-test).** Before any subprocess test runs,
  the fixture manifest must pass m1's
  `Manifest::parse_at(<fixtures/rafaello-mockprovider/>)`
  + `validate_with_package` (pi-1 B-2). Lands as
  `rafaello-mockprovider/tests/mockprovider_manifest_compiles.rs`
  and gates every other `rafaello-mockprovider/tests/`
  entry via plan ordering in `commits.md`.
- **PR4.** Determinism: the mock provider does not call into
  any time-of-day, RNG, or filesystem outside its private
  state dir. Every input message produces the same output
  message. Tests rely on this.
- **PR5.** **Decision: separate crate
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
  `rafaello/fixtures/rafaello-readfile/rafaello.toml`, live
  m1 schema (pi-1 B-2 + B-6 fixes):
  ```toml
  schema   = 1
  name     = "readfile"
  version  = "0.0.0"
  entry    = "bin/rfl-readfile"
  rafaello = ">=0.1, <0.2"

  [provides]
  tools = ["read-file"]

  [provides.tool.read-file]
  sinks          = []
  always_confirm = false

  [bus]
  subscribes = []   # m1 compiler auto-inserts
                    #   plugin.<topic-id>.tool_request
                    # (broker_acl.rs:98)
  publishes  = []   # m1 compiler auto-inserts
                    #   plugin.<topic-id>.tool_result
                    # (NEW in m4 §M1.3 — see below)

  [capabilities.default.filesystem]
  read_dirs  = ["${project}"]
  write_dirs = []

  [capabilities.default.network]
  mode = "deny"

  [load]
  eager = true   # m4 round-2: eager-load every tool to
                 # avoid introducing lazy-spawn-on-publish
                 # (§C8 / Risk #12)
  ```
  Plus a sibling `openrpc.json` (m1 §M10) declaring the
  `read-file` tool's wire shape — the readfile plugin exposes
  its tool through bus events, not JSON-RPC methods, so the
  OpenRPC document carries `methods: []` like the mock
  provider. m1's validator does not require `methods` to
  enumerate tools (tools are bus-level, not RPC-level).
  - **`${project}` placeholder**: the live m1 closed set is
    `${project}`, `${home}`, `${plugin}`, `${cache}`,
    `${state}` (`manifest/capability_path_template.rs:17`).
    `${PROJECT_ROOT}` is **not** valid (pi-1 B-2). m1
    substitutes `${project}` to the project root at compile
    time.
  - **`plugin.<topic-id>.tool_result` auto-publish** (pi-1
    B-6 resolution): the m1 compiler is extended in m4 §M1.3
    to auto-insert
    `format!("plugin.{}.tool_result", topic_id)` into the
    `PluginAcl.publish_topics` for any plugin with non-empty
    `bindings.tools`, identical in shape to the existing
    `auto_subscribes` insertion at `broker_acl.rs:98`. The
    manifest never declares the topic; authors with a custom
    tool topic add it explicitly in `[bus].publishes`. The
    change is purely additive on the manifest side (no
    literal `<topic-id>` placeholder syntax introduced) and
    closes the round-1 "non-existent placeholder
    substitution" gap.
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
- **TP3 (compile-test).** Same as PR3 — fixture manifest +
  `openrpc.json` parse + `validate_with_package` green
  before any subprocess test runs. Lands as
  `rafaello-readfile/tests/readfile_manifest_compiles.rs`.
- **TP4.** Read-only grant intersection: `read_dirs =
  ["${project}"]` (the live m1 closed placeholder per
  `manifest/capability_path_template.rs:17`) ensures the
  lockin sandbox sees the project root. The demo bar's
  "what's in README.md" prompt resolves to
  `<project_root>/README.md`. m1's existing `${project}`
  placeholder expansion (compile time) is the substitution
  path; m4 does not add a new placeholder.
- **TP5.** Same separate-crate rationale as §PR5. Lives at
  `rafaello/crates/rafaello-readfile/`. The lockin
  read_dirs intersection (project root) is computed by m1
  via the `${project}` substitution at compile time —
  m1's `manifest/capability_path_template.rs:17` is the
  canonical resolver.
- **TP6 (naming conventions).** Pi-1 L-2 pinned. Two
  spellings appear in the m4 surface and they refer to
  different layers:
  - **`"read-file"`** (kebab-case, single-segment) is the
    **manifest tool name** (`[provides] tools =
    ["read-file"]`) and the **bus payload `tool:` field**
    routing key — i.e. the public identity that flows
    through `BrokerAcl.tool_routes` and any
    `provider.<id>.tool_request` payload's `tool` field.
    Every bus snippet in this scope.md uses `"read-file"`
    (no exceptions).
  - **`read_file` / `readfile`** (snake-case) is permitted
    inside **Rust identifiers** where syntax requires
    (`crate rafaello-readfile`, `bin rfl-readfile`, module
    paths, struct names). Crate / bin file names use the
    `rafaello-readfile` / `rfl-readfile` kebab-case form
    consistent with the workspace's `rafaello-tui` /
    `rafaello-mockprovider` precedent.
  - The security RFC's `read_file` taint-source label
    (`streams/a-security/rfc-security-model.md` §7.2.1
    around lines 878-886) is a *free-form `detail` string*
    on a `TaintEntry`, **not a routing key**. m4 does not
    adopt that spelling as the canonical tool name. If a
    future taint-propagation pass (m5) wants to round-trip
    the tool name into `TaintEntry.detail`, it uses the
    same `"read-file"` spelling as the manifest, not the
    RFC's illustrative `read_file`.

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

- **M2.1.** Identify the m2 test: the live file (verified via
  `ls rafaello/crates/rafaello-core/tests/`) is
  `rafaello/crates/rafaello-core/tests/supervisor_spawn_provider_lock_refused.rs`
  (pi-1 B-7 fix — the live name; m2 retrospective entry at
  line 100-104 confirms).
- **M2.2.** **Successor pattern**: delete
  `supervisor_spawn_provider_lock_refused.rs` (the synthetic
  refusal is gone) and add a positive test
  `provider_plugin_spawns_through_supervisor.rs` that:
  - builds a fixture `CompiledPlugin` with `bindings.provider =
    true`, `bindings.provider_id = "mock"`;
  - spawns through `PluginSupervisor::spawn`;
  - awaits `SpawnHandle::wait()` (the **real** API at
    `supervisor.rs:134-148`; `wait_ready` is on
    `FrontendHandle` not `SpawnHandle` — pi-1 B-7). Round-2
    cut: the test uses `try_wait()` to confirm the handle
    is *not* yet terminal at a fixed sleep point, then
    triggers shutdown and asserts `wait()` resolves with
    `ReaperOutcome::Exited(_)`. (Pattern matches m2's
    existing `supervisor_spawn_fixture_happy_path.rs`.)
  - asserts `Broker::contains_provider(canonical) == true`
    while the spawn is live;
  - asserts a `provider.mock.tool_request` publish by the
    child reaches the broker's internal subscriber (via the
    existing fixture-mode `frontend_bus_publish` pattern,
    extended to provider publishes — round-2 cut: add a
    `provider_bus_publish` fixture mode to
    `rfl-bus-fixture` that issues a synthetic provider
    publish).
  This is the **named successor** that closes the
  synthetic-stub gap for m4.
- **M2.3.** Note in the commit body: "deletes
  `supervisor_spawn_provider_lock_refused.rs`; adds
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
  for `RFL_PROVIDER_ID`** (only — `RFL_PROVIDER_ACTIVE`
  dropped per pi-1 H-1). Same rationale as decisions row 40
  — additive, m1 v3 catches collisions pre-compile.
  **Default: land in the same commit as PS4/PS5.** This is
  one of two required m1 back-reaches in m4 (the other is
  M1.3 — auto-publish).
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
- **M1.3.** **Compiler-inserted `plugin.<topic-id>.tool_result`
  auto-publish** (pi-1 B-6 resolution; **required**, not
  contingent). m4 extends `rafaello-core/src/broker_acl.rs`
  `compile`:
  ```rust
  let mut publish_topics = entry.grant.publishes.clone();
  if !entry.bindings.tools.is_empty() {
      publish_topics.push(format!("plugin.{}.tool_result",
                                   topic_id_str));
  }
  ```
  Identical in shape to the existing `auto_subscribes`
  insertion at `broker_acl.rs:98` (`format!("plugin.{}.tool_request",
  topic_id_str)`). Defence: a plugin with empty
  `bindings.tools` never gets the auto-publish; an existing
  manifest that already declares
  `plugin.<topic-id>.tool_result` in `publishes` would
  duplicate — m1 v3 already rejects literal `<topic-id>`
  (illegal chars `<` `>` per `validate/mod.rs:359-365`), so
  the only way that string reaches `entry.grant.publishes`
  is a hand-mutated lock; m4 dedupes inside the compiler via
  `publish_topics.sort(); publish_topics.dedup();` defensively.
  New tests: `broker_acl_auto_publishes_tool_result_topic.rs`
  (positive — confirm the topic appears in
  `PluginAcl.publish_topics`),
  `broker_acl_auto_publish_absent_for_non_tool_plugin.rs`
  (negative — empty `tools` → no auto-publish).

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
- `reemit_user_message_synthesises_user_taint.rs` (pi-1 L-3)
  — assert the user-source synthesis at §CR5 step 4
  produces exactly `[{source: "user", detail: None}]`,
  regardless of inbound `taint`.
- `reemit_user_message_discards_frontend_supplied_taint.rs`
  (pi-1 L-3) — TUI publishes with `taint: [{source:
  "provider", detail: "mock"}]`; the canonical re-emit
  carries `[{source: "user", detail: None}]` only.
- `reemit_user_message_synthesises_request_id_when_absent.rs`
  (pi-1 L-3) — frontend publish omits `request_id`; the
  canonical event nonetheless carries a synthesised
  `request_id`.
- `agent_loop_dispatches_tool_request_to_target_plugin.rs` —
  drive a `core.session.tool_request` with
  `dispatch_target` set; the agent loop publishes the
  corresponding `plugin.<topic-id>.tool_request`.
- `agent_loop_persists_user_message_entry.rs` — assert a
  `core.session.user_message` event causes a row in the
  `entries` table with `kind = "text"`, `metadata.author =
  EntryAuthor::User`, and `payload.text` matching the
  inbound message (pi-1 B-8).
- `agent_loop_persists_assistant_message_entry.rs` —
  analogous: `kind = "text"`, `author = Assistant`.
- `agent_loop_persists_tool_call_entry.rs` — `kind =
  "tool_call"`, `payload = ToolCallPayload { id,
  name: "read-file", args, status: Pending }`.
- `agent_loop_persists_tool_result_entry.rs` — `kind =
  "tool_result"`, `payload = ToolResultPayload { call_id,
  ok: true, content: RenderNode::CodeBlock { .. }, details:
  None }`.
- `provider_plugin_spawns_through_supervisor.rs` — the
  successor named in §M2.2 above.
- `frontend_register_with_broker.rs` — the m3 retro §5.9
  granularity gap closer.
- `frontend_publish_user_message_reemitted_as_core_session_user_message.rs`
  — m3 §2.10 handover completion.
- `broker_provider_event_not_fanned_to_external_subscribers.rs`
  (pi-1 B-5) — register a plugin with subscribe pattern
  `provider.mock.**`; issue a
  `provider.mock.tool_request` from a registered provider;
  assert the registered plugin's peer notify count remains
  zero (the internal ReemitRouter receives the event; no
  external subscriber does).
- `broker_internal_subscriber_unregister_on_drop.rs` (pi-1
  M-1) — see §CR1.
- `broker_internal_subscriber_drops_event_when_full.rs`
  (pi-1 M-1).
- `broker_internal_subscriber_fires_before_external_fan_out.rs`
  (pi-1 M-1).
- `broker_acl_auto_publishes_tool_result_topic.rs` (pi-1
  B-6 / §M1.3 positive).
- `broker_acl_auto_publish_absent_for_non_tool_plugin.rs`
  (pi-1 B-6 / §M1.3 negative).
- `provider_assistant_message_in_reply_to_missing_rejected.rs`
  (pi-1 H-2) — provider publishes
  `provider.mock.assistant_message` with no `in_reply_to`
  field at all; broker rejects with `InvalidInReplyTo {
  reason: Missing }`.
- `provider_assistant_message_in_reply_to_stale_id_rejected.rs`
  (pi-1 H-2) — `in_reply_to: [<id-never-observed>]`;
  broker rejects with `StaleRequestId`.
- `provider_tool_request_in_reply_to_stale_id_rejected.rs`
  (pi-1 H-2) — analogous for tool_request.

`rafaello-mockprovider/tests/`:

- `mockprovider_manifest_compiles.rs` (pi-1 B-2) — load the
  fixture manifest + sibling `openrpc.json` via m1's
  `Manifest::parse_at(...)` and assert successful parse +
  `validate_with_package` green.
- `mockprovider_emits_tool_request_for_read_file_pattern.rs`
  — spawn `rfl-mockprovider` against an in-test broker
  fixture; deliver a synthetic `core.session.user_message`
  with text `"what's in README.md"`; observe a
  `provider.mock.tool_request` with
  `{tool: "read-file", args: {path: "README.md"}}`,
  `request_id: Some(_)`, and `in_reply_to: []` (no prior
  tool_results observed — §7.2.6 row 2).
- `mockprovider_strips_trailing_punctuation_from_path.rs`
  (pi-1 H-4) — input `"what's in README.md?"` produces
  `args.path = "README.md"` (the `?` is stripped).
- `mockprovider_records_request_id_to_path_mapping.rs`
  (pi-1 H-4) — issue two consecutive
  `core.session.user_message` events for distinct paths
  ("what's in a.txt", "what's in b.txt"); assert the
  internal `outstanding` map records both ids; then
  inject a `tool_result` citing the second; assert the
  assistant_message text references `b.txt`, not `a.txt`.
- `mockprovider_emits_echo_assistant_message_on_no_match.rs`
  — same setup, payload `"hello"`; observe
  `provider.mock.assistant_message` with `{text: "echo:
  hello"}` and `in_reply_to: [<user_message.request_id>]`.
- `mockprovider_emits_assistant_message_on_tool_result.rs` —
  drive a request, then inject a `core.session.tool_result`
  with content "Hello!"; observe
  `provider.mock.assistant_message` whose `text` begins
  `"Here's what's in"` and whose `in_reply_to =
  [<tool_result.request_id>]`.
- `mockprovider_handles_multibyte_utf8_path.rs` (pi-1 H-4)
  — input `"what's in données.txt"` produces
  `args.path = "données.txt"` correctly.

`rafaello-readfile/tests/`:

- `readfile_manifest_compiles.rs` (pi-1 B-2) — analogous to
  the mockprovider compile test; load + validate the
  fixture manifest + `openrpc.json`.
- `readfile_returns_content_for_existing_file.rs` — spawn
  `rfl-readfile` against a tempdir project root containing a
  `README.md`; deliver a synthetic
  `plugin.<own-topic-id>.tool_request` for
  `{path: "README.md"}`; observe `tool_result` with `ok:
  true, content: "<file body>"`.
- `readfile_errors_for_missing_file.rs`,
  `readfile_errors_for_non_utf8.rs` — analogous error paths.
- `readfile_errors_for_outside_project_root.rs` — request
  path that resolves outside `read_dirs`; plugin-level
  ancestor check rejects with `ok: false, error: "path
  denied"`. (Pi-1 H-3: keeps this as the plugin-level
  negative.)
- `readfile_lockin_denies_outside_grant.rs` (pi-1 H-3) —
  **lockin-level** negative. The readfile bin gains a
  `RFL_READFILE_TEST_BYPASS_GUARD=1` env (test-only) that
  skips the in-plugin ancestor check and calls
  `std::fs::read` on the raw input path. The test spawns
  the plugin with that env set, requests a file outside
  `read_dirs`, and asserts the resulting `tool_result`
  carries `ok: false, error: <io::ErrorKind::PermissionDenied
  rendered>` — i.e. the sandbox denied the read, not the
  plugin's own ancestor check.

`rafaello/tests/`:

- `rfl_chat_missing_lock_errors.rs`,
  `rfl_chat_invalid_lock_errors.rs`,
  `rfl_chat_lock_validation_fails.rs`,
  `rfl_chat_no_active_provider_errors.rs`,
  `rfl_chat_provider_spawn_failure_propagates.rs` (pi-1
  B-1) — orchestration negatives per §C14.
- `rfl_chat_demo_bar_read_file.rs` — **headline test, lands
  at the end of the milestone.** Spawn `rfl chat` against a
  tempdir project root containing a `README.md` with known
  content; the `rafaello.lock` is pre-materialised with
  `rfl-mockprovider` (active) + `rfl-readfile` installed.
  Drive the TUI's `frontend.tui.user_message` publish via a
  test-mode env hook (`RFL_TUI_TEST_MESSAGE="what's in
  README.md"` per §T1), or via the existing
  `RFL_HARNESS_FIXTURES` style. Assert (in order):
  - SQLite `entries` table contains rows of kinds
    `text` (user), `tool_call`, `tool_result`,
    `text` (assistant) in seq order, distinguished by
    `metadata.author` (`User` / `Assistant` / `Tool`);
    test asserts via the canonical `Entry` shape, not via
    the kind string alone (pi-1 B-8);
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
  (per §7.2.6 row 2 the field is **required** even if it is
  `[]`; absent field → `Missing`)
  + `broker_provider_tool_request_stale_id_rejected.rs`
  (an `in_reply_to` citing a `tool_result.request_id` never
  observed by this provider per §B7b's
  `provider_observed_results` set →
  `InvalidInReplyTo { reason: StaleRequestId { id } }`).
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
  `rafaello-readfile/tests/readfile_lockin_denies_outside_grant.rs`
  (pi-1 H-3) — the lockin-level negative described in §TP
  above. The plugin-level path-traversal check is
  exercised separately by
  `readfile_errors_for_outside_project_root.rs`. Both must
  pass; the H-3 fix is to ensure the lockin path is
  independently tested without the plugin's ancestor check
  short-circuiting it.
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

- `broker_publish_provider_id_segment_mismatch_rejected.rs`
  — a provider registered as `provider_id = "mock"` publishes
  on `provider.other.foo`; `PublishOnReservedNamespace`
  (the `provider.<id>` segment must match the registered id
  per §B6 step 4).
- `broker_publish_provider_two_segment_topic_rejected.rs` —
  `provider.mock`; symmetric to m2's plugin / m3's frontend
  two-segment rule.
- `broker_publish_provider_unknown_namespace_rejected.rs` —
  `evil.foo` from a provider; `UnknownNamespace`.
- `broker_publish_provider_outside_grant_rejected.rs` —
  `provider.mock.confidential` not in `publish_topics`;
  `PublishOutsideGrant`.
- `broker_register_provider_unknown_canonical_rejected.rs` —
  `ProviderNotInAcl` (canonical absent from `BrokerAcl.plugins`
  or present with `provider_id = None` per §B5 round-2 cut).
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
  m4 reuses m3's built-in renderers exclusively: user
  and assistant bus messages map to `kind = "text"` with
  distinct `metadata.author` (User vs Assistant); tool
  call/result events map to `kind = "tool_call"` /
  `"tool_result"` with the canonical
  `ToolCallPayload` / `ToolResultPayload` shapes from
  `entry/payloads.rs`. **No new entry kinds, no new
  renderers** (pi-1 B-8 fix).
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
   ["${project}"]`; the headline test's tempdir is
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
8. **Compiler-inserted `tool_result` auto-publish** —
   pi-1 B-6 closed the round-1 open question. m4 §M1.3
   adds the auto-publish in `broker_acl::compile`; the
   readfile manifest's `[bus].publishes` is empty.
   Risk: the m1 grant compiler is touched in m4, so an
   m1 regression must be caught. Mitigation: the two
   §M1.3 tests
   (`broker_acl_auto_publishes_tool_result_topic.rs` +
   `broker_acl_auto_publish_absent_for_non_tool_plugin.rs`)
   plus a re-run of m1's existing
   `broker_acl_extraction.rs` test suite.
9. **Provider-id mismatch detection at registration** —
   pi-1 H-5 closed this. `register_provider(canonical,
   peer)` reads `provider_id` from `PluginAcl.provider_id`;
   no caller-supplied id, no mismatch state, no
   `BrokerError::ProviderIdMismatch` variant.
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
12. **Lazy-load is out of scope** (pi-1 B-1 resolution). m4
    eager-spawns every installed plugin via §C7/C8; the
    fixture manifests set `load.eager = true` for both
    plugins. m2/m3 ship no lazy-spawn-on-publish primitive
    and m4 does not introduce one. Out of scope explicitly
    documents this; m5+ will revisit when a second tool
    plugin lands.

## Internal split (driver guidance for `commits.md`)

Suggested grouping; `commits.md` picks final granularity.
Pi review may reshape. m4's surface is high (new broker
publisher class + new envelope field + two new plugin
crates + new agent loop + new re-emit module + new `rfl
chat` orchestration) — expect **~26-32 commits sequential**
(pi-1 M-4 revised upward from round-1's optimistic
~22), comparable to m3's 31.

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
6. **Frontend ACL extension (F1-F4) + TUI test-mode env
   hook (T1)** + retro §5.9 test gap
   (`frontend_register_with_broker.rs`): ~2-3 commits.
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
12. **`rfl chat` orchestration (C1-C13) + orchestration
    negatives (C14)**: ~3-4 commits. Lock-load + V3 +
    compile_plugin per-plugin + supervisor construction +
    eager spawn + shutdown; orchestration negatives land
    alongside.
13. **Demo-bar headline + manual validation** (the
    `rfl_chat_demo_bar_read_file.rs` test +
    `manual-validation.md`): ~2 commits.

Forced-monolithic commits called out explicitly:

- **Step 2 (broker envelope cutover)** is the m4
  equivalent of m0 c08's API cutover. The commit body
  must say so.
- Step 7 bundles the m2 refusal removal with its
  positive successor test (synthetic-stub-successor
  rule).

Realistic total: **~26-32 commits sequential** (pi-1 M-4
revised upward from round-1's 22). m3 took 31 plan-row
commits at comparable surface area; m4 adds new broker
publisher class + `request_id` envelope cutover + provider
registration + internal-subscriber primitive + agent loop +
re-emit pipeline + two new plugin crates + `rfl chat`
orchestration + manifest fixtures. Pi round budget:
**plan for 8+ scope rounds** (m3 took 22; m2 took 8).
No m4a / m4b split anticipated — the surface threads
through broker + supervisor + agent loop + two plugin
crates without natural chasms. If a split materialises
during Phase 3, owner-ratified mid-milestone; default is
"ship m4 as one milestone".

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
    overview §4.6** — `RFL_PROVIDER_ID` gets added to the
    reserved env vars table (pi-1 H-1 dropped
    `RFL_PROVIDER_ACTIVE`).
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
