# m5a scope.md round-1 pi review

> Verdict: blocking
>
> Counts: B/7 M/8 N/6

I reviewed `scope.md` against the pre-flight, the ratified roadmap row, overview/decisions/glossary, Stream A/F, m4 boundary/retro, prior pi-review style, and the live `rafaello/crates` state on this worktree.

The split direction is plausible, but this draft is not implementation-ready. The main blockers are concrete wiring contradictions: TUI-local slash commands cannot mutate core memory, confirmation correlation is not single-valued, deny/timeout emits invalid m4 `tool_result` events, `tools_advertised` is published before the provider can receive it, OpenAI env/secret handling conflicts with the live scrubber/lock schema, the new `rfl install` surface is underspecified, and the outstanding-request map is assigned to the wrong component/race boundary.

## Blockers

### B-1. Slash commands mutate core-owned `user_grants` from the separate TUI process

**Anchor:** §Goal items 5-6 (`scope.md:273-292`), §SL (`scope.md:892-928`), §CHAT1 (`scope.md:1210-1217`).

**Issue:** The draft says `user_grants` is an `Arc<RwLock<UserGrants>>` in core memory, but also says the TUI slash-command handler mutates it directly with “no bus round-trip”. That is impossible under the ratified process model: the TUI is a separate frontend process speaking over the bus, not an in-process module sharing core heap state.

**Ratified conflict:** `overview.md` §3 says: “Frontends are bus principals … **always separate processes from core** — there is no in-process frontend in v1.” `overview.md` §6.4 says `user_grants` “lives in core memory; process exit clears it.”

**Smallest acceptable fix:** Make core the sole mutator of `UserGrants`. Either:

- add a core-validated frontend command topic (e.g. `frontend.tui.slash_command` → `core.session.command_result`) with ACL, payload schemas, auditing, and rejection tests; or
- keep slash parsing in TUI only as a lexical convenience, but publish a typed request to core for `/grant`, `/grants list`, and `/revoke`.

Then update §SL, §AL, §CHAT, tests, and negative coverage for malformed commands and unknown revoke ids. Do not claim TUI-local direct mutation.

### B-2. Confirmation protocol correlation is not single-valued

**Anchor:** §Goal item 3 (`scope.md:227-246`), §CT (`scope.md:712-747`), §CG3 (`scope.md:776-797`), §CG4-5 (`scope.md:798-812`).

**Issue:** The draft mixes payload-level `request_id`, envelope `BusEvent.request_id`, and `in_reply_to` without pinning equality or freshness rules. `ConfirmRequestPayload` contains `request_id`; the m4-style bus envelope also has a mandatory `request_id`; `confirm_answer` is said to carry both mandatory `request_id` and mandatory `in_reply_to`; `confirm_reply` is consumed by the gate, but its envelope/payload correlation is never specified. This will make stale/double answers, timeout answers, and audit correlation implementation-specific.

**Ratified/source conflict:** Stream A §5.6 payload schema uses `request_id` inside the three confirmation payloads; Stream A §7.2.6 row 5 requires `frontend.<id>.confirm_answer` to have exactly one `in_reply_to` referencing the matching `core.session.confirm_request`. m4 decision row 43 only established the live `BusEvent.request_id` rule for `.tool_request`, `.tool_result`, `.assistant_message`, and `.user_message`; extending it to `.confirm_*` is m5 work and needs a table-of-truth like m4 §B0.

**Smallest acceptable fix:** Add a CT0 “confirmation correlation table” before CT1. For each of the three topics, specify:

- envelope `request_id` source (fresh event id vs confirm id);
- payload `request_id` semantics and whether it must equal the envelope id;
- `in_reply_to` cardinality;
- stale/unknown/double-answer behavior;
- timeout-vs-late-answer behavior.

Add tests for missing envelope id, missing `in_reply_to`, unknown id, duplicate answer, and late answer after timeout. Use one canonical id as the key in the held-confirmation map.

### B-3. Deny/timeout emits an invalid `core.session.tool_result` under the live m4 envelope rules

**Anchor:** §Goal item 4 (`scope.md:265-270`), §CG4-5 (`scope.md:798-812`), demo positive/timeout (`scope.md:1286-1308`).

**Issue:** The deny path says the gate synthesises a `core.session.tool_result` with payload `{ok:false,error:"user_denied"}`. It does not specify a fresh `request_id`, `in_reply_to = [held_tool_request_id]`, or non-empty `taint`. The live m4 broker rejects `core.session.tool_result` without `request_id` and non-empty taint, and the live agent loop persists tool results by reading `event.in_reply_to[0]`.

**Live-source evidence:** `bus.rs` requires `request_id` for suffix `tool_result` (`REQUEST_ID_REQUIRED_SUFFIXES`) and `publish_core_with_taint` rejects `core.session.tool_result` when `taint` is missing/empty. `agent/mod.rs::handle_tool_result` returns early if `event.in_reply_to[0]` is absent.

**Smallest acceptable fix:** Pin the synthetic deny/timeout event shape:

- `request_id`: fresh result id;
- `in_reply_to`: exactly `[held_tool_request.request_id]`;
- `taint`: forwarded from the held tool_request or a specified core/system taint entry, but non-empty;
- payload schema: the same wire shape expected by `handle_tool_result`, including how `error` maps into the persisted `ToolResultPayload`.

Add a unit test that publishes the synthetic event through `Broker::publish_core_with_taint` and proves the live agent-loop persistence path records the denied tool result.

### B-4. `core.session.tools_advertised` can be missed by `rfl-openai`

**Anchor:** §OP2 (`scope.md:1007-1019`), §CHAT1 (`scope.md:1210-1217`), Risk 6 (`scope.md:1657-1667`).

**Issue:** CHAT1 says `rfl chat` publishes `core.session.tools_advertised` “after broker construction”. In the live m4 orchestration, broker construction happens before the provider process is spawned/registered. The broker has no replay-on-subscribe for arbitrary events, and Risk 6 explicitly acknowledges there is no replay-on-subscribe. If the event is published before `rfl-openai` is registered, the provider never receives the tool schemas, so the headline demo cannot rely on a model-proposed tool call.

**Live-source evidence:** m4 `run_chat` constructs `Broker`, then `PluginSupervisor`, then spawns provider/tool plugins, then starts `ReemitRouter`/`AgentLoop` and TUI. `Broker::fan_out` only delivers to currently registered peers/internal subscribers; decision row 41 replay is only for `core.session.entry.finalized`.

**Smallest acceptable fix:** Specify a delivery handshake. Options:

- publish `tools_advertised` after the active provider is spawned and registered, and add a provider-ready/observed-tools test; or
- expose a core request (`core.tools_list`) the provider calls after start; or
- have `rfl-openai` receive the tool schema via env/file at spawn.

Whichever option lands, add an end-to-end test proving `rfl-openai` has the tool list before the first user message.

### B-5. OpenAI env/API-key configuration conflicts with the live lock schema and scrubber

**Anchor:** §W3 (`scope.md:625-634`), §OP5-6 (`scope.md:1063-1095`), §M1.1 (`scope.md:1182-1189`), §A3 (`scope.md:1504-1520`).

**Issue:** The draft has three incompatible env stories:

1. W3 says fixture lock uses `env.pass = ["RFL_OPENAI_API_KEY"]`.
2. OP5 says lock uses `env.pass = ["LITELLM_API_KEY:RFL_OPENAI_API_KEY"]` and `env.set = ["RFL_OPENAI_ENDPOINT_URL=…"]`.
3. M1.1 says `RFL_OPENAI_*` become reserved, but the mapped form may target a reserved name.

The live lock schema has `GrantEnv { pass: Vec<String>, set: BTreeMap<String,String> }`, not an array of `KEY=VALUE` strings for `env.set`. The live scrubber rejects reserved `RFL_*` names and strips `*_KEY` secret-looking env vars unless the override path is used. As written, the fixture either fails validation/compile or starts without an API key.

**Ratified conflict:** Decision row 38 says endpoint URL, API-key env-var name, and `allow_hosts` are install-time configuration; it does not ratify a canonical `RFL_OPENAI_API_KEY` rename scheme. `overview.md` §4.6 reserves core-injected `RFL_*` names; new reserved names need a precise exception if plugins are meant to read them.

**Smallest acceptable fix:** Choose exactly one env model and make it parseable:

- simplest: defer rename syntax; let `rfl-openai` read a configured env-var name supplied by lock/env (e.g. `RFL_OPENAI_API_KEY_ENV=LITELLM_API_KEY`) while `env.pass = ["LITELLM_API_KEY"]` is explicitly allowed; or
- fully specify the `env.pass` rename schema, lock TOML syntax, scrubber exception, supervisor remapping, reserved-name exception, and tests.

Also correct OP5 to valid lock TOML (`[...grant.bundles.default.env.set] RFL_OPENAI_MODEL = "..."` or the live equivalent), and add tests for secret-scrubber behavior with the LiteLLM key.

### B-6. `rfl install`/`rfl status` is required by acceptance but the install surface is not scoped enough to implement

**Anchor:** §Goal item 8 (`scope.md:303-312`), §Tr (`scope.md:662-710`), Risk 13 (`scope.md:1719-1732`), demo negative 3 (`scope.md:1325-1339`).

**Issue:** The draft simultaneously says m5a adds `rfl install <plugin-source>`/`rfl status` integration tests and says the materialise-from-source path is out of scope / “m1's install path” / “a new install candidate helper”. There is no live user-facing install command in m4 to extend; m1 has parser/validator/compiler pieces, not a CLI that resolves plugin sources, computes digests, snapshots manifests, and writes lock entries. The implementation agent will have to invent what `plugin-source` means and how much of install is real.

**Roadmap conflict:** The roadmap requires one-hop trifecta guardrail “at install time,” but `rfl init` materialisation is explicitly m6. A minimal m5a install command is acceptable, but it must be specified.

**Smallest acceptable fix:** Bound the m5a install CLI mechanically. For example: `rfl install --from-lock-entry <toml>` or `rfl install --fixture <dir>` reads a local manifest/package dir, computes the existing digest pair, snapshots a candidate `PluginEntry`, runs `validate::lock`/`trifecta::evaluate`, and writes `rafaello.lock`. Explicitly defer network fetch/update/review UI. Then align all tests and manual validation with that exact interface.

### B-7. The outstanding `tool_request` map is assigned to the wrong race boundary

**Anchor:** §Goal item 9 (`scope.md:314-322`), §OM (`scope.md:834-858`), m4-retro inheritance (`scope.md:545-550`).

**Issue:** OM says broker state is populated by `publish_for_tool_dispatch` but “drained by the gate on `core.session.tool_result` observation.” That is too late and in the wrong component for the m4 carryover. The validation must happen atomically in `handle_plugin_publish` when the plugin publishes `plugin.<id>.tool_result`; otherwise a second or stale result can pass broker intake before a gate observer drains a later canonical event. The gate also tracks held confirmations, not dispatched-to-plugin requests; conflating those maps invites races around allow/timeout.

**Ratified/source conflict:** Security RFC §7.2.6 row 1 says `plugin.<id>.tool_result` must reference the matching `tool_request` previously routed to this plugin. m4 retro §5.1 explicitly filed this as a broker-side stale-correlation gap. The live m4 `handle_plugin_publish` already enforces cardinality for `tool_result`/`rpc_reply`; m5a must add the stale-id lookup there.

**Smallest acceptable fix:** Split the maps:

- gate-held confirmations: keyed by confirm id / held tool_request id, owned by `ConfirmationGate`;
- broker outstanding dispatches: keyed by target canonical + tool_request id, populated by `publish_for_tool_dispatch`, checked and drained atomically in `handle_plugin_publish` for `tool_result`.

Add a duplicate-result test where two `plugin.<id>.tool_result` publishes race; the second must fail at broker intake, not after re-emission.

## Major

### M-1. The m5a/m5b split needs an explicit owner decision before further scope iteration

**Anchor:** §Sizing (`scope.md:12-174`), demo mapping (`scope.md:1260-1346`).

The split is plausible and probably right, but it changes what “m5 complete” means. The ratified roadmap m5 demo includes the verbatim tool-result-to-sink negative. This draft defers that to m5b and makes m5a cover only three of four negatives. Smallest fix: mark the split itself as an owner-judgment item in the preamble/convergence section, and make m5a acceptance say “m5a is not the full m5 roadmap row; m5b remains required before m5 is closed.”

### M-2. `rfl-openai` wire/error behavior is too underspecified for an implementation handoff

**Anchor:** §OP1/OP3/OP7 (`scope.md:987-1108`), §A7/A8 (`scope.md:1554-1598`).

The OpenAI plugin section names request/response structs but does not specify: HTTP non-200 mapping; auth failure behavior; timeout/retry policy; `model` resolution from `RFL_OPENAI_MODEL`; malformed JSON; `choices=[]`; multiple choices; `finish_reason`; `tool_calls[*].id/function.name/function.arguments` parsing; invalid JSON arguments; multiple tool calls ordering; or whether a final assistant content plus tool_calls emits both. These are exactly the edge cases that decide whether the provider is deterministic enough for m5a CI and manual validation. Add a small wire-shape table and negative tests for at least auth failure/non-200, malformed tool-call args, unknown tool name, and multiple tool_calls.

### M-3. Multiple pending confirmations and races are not specified

**Anchor:** §CG2-5 (`scope.md:757-812`), §TUI1-4 (`scope.md:933-965`), §CHAT3 (`scope.md:1227-1238`).

OpenAI Chat Completions can return multiple `tool_calls`, and the bus can observe parallel sink requests. The TUI modal is a single `InputMode::ConfirmModal`, but the scope does not say whether core queues, serializes, denies the second request, or allows multiple pending confirmations. It also does not specify the race where `always_allow_session` is granted while another matching sink call is already pending. Add a policy and tests for queue ordering, stale modal answers, parallel matching grants, and timeout while another modal is active.

### M-4. Confirm modal is both a transient frontend overlay and a new RenderNode/entry kind

**Anchor:** §Goal item 7 (`scope.md:294-301`), §TUI2-3 (`scope.md:943-958`), §RC (`scope.md:967-983`), Risk 8 (`scope.md:1677-1687`).

The draft says the TUI displays a modal “entry kind (`confirm_request` — see §RC)” and adds `RenderNode::Confirm`, but Risk 8 says confirm modals are not persisted as entries and do not pass through the entry-persistence path. If the modal is frontend-internal from `core.session.confirm_request`, no RenderNode/capability/downgrade work is needed. If it is an entry/render kind, then core must finalize an entry and other frontends need downgrade semantics. Choose one. Smallest fix: for m5a, make it a TUI-internal overlay driven directly by the bus event; drop §RC unless there is a concrete persisted entry consumer.

### M-5. `user_grants` matcher semantics contradict the ratified `grant_match` purpose and the draft itself

**Anchor:** §Goal item 5 (`scope.md:273-284`), §UG2 (`scope.md:867-875`), §A1 (`scope.md:1467-1480`), Out-of-scope item 5 (`scope.md:1375-1384`).

The top-level deliverable says matching is against the `grant_match` JSON-Schema instance supplied at `/grant` time; UG2 says structural subset and no JSON-Schema validation; A1 says the manifest JSON-Schema is parsed but unused. Stream A §7.2.4 says matching “uses the matcher schema declared in the tool's manifest,” and overview §15.1 made `grant_match` a load-bearing field. If m5a intentionally ships structural matching, that is a new decision row / owner call, not just a local implementation detail. Also specify how a tool whose manifest changes mid-session is handled (lock-pinned `bindings.tool_meta` should win).

### M-6. The acceptance claim that m5a removes m4 dead-code suppressions is not supported by the scope

**Anchor:** m4 inheritance (`scope.md:551-555`), acceptance (`scope.md:1923-1926`).

The draft says m5a consumes `ProviderConn.peer` and `SpawnRegistration::Provider`, but no m5a section actually reads either field. The gate publishes through `Broker::publish_for_tool_dispatch`; it does not need direct provider peer access. `SpawnRegistration` is RAII-only by design. Requiring those `#[allow(dead_code)]` removals in acceptance will create fake work or churn. Smallest fix: either specify the real read-side use, or drop this acceptance bullet and keep the m4 retro follow-up open.

### M-7. Test matrix under-covers security negatives introduced by the new surfaces

**Anchor:** §CT/CG/UG/SL/TUI tests (`scope.md:720-965`), demo matrix (`scope.md:1260-1359`).

Missing named negatives include: `confirm_answer` missing `in_reply_to`; stale/unknown confirm id; duplicate confirm answer; late answer after timeout; malformed answer string; `/grant` with no args if broad grants are allowed (needs explicit scary wording); `/grant` malformed `key=value`; `/revoke` unknown id in TUI; slash command from any non-TUI path if B-1 adds a bus command; grant revoked while a matching confirmation is pending; and grant matched against the wrong tool/plugin. Add named tests before `commits.md` so Phase 3 does not choose ad hoc behavior.

### M-8. The sink enum cutover is broader than necessary and risks churn in m1/m4 code

**Anchor:** §Si2 (`scope.md:647-655`).

Live lock/compiler structs store `sinks: Vec<String>` and validation accepts known plus custom sink classes. Replacing `infer_defaults`' return type with `Vec<SinkClass>` forces a cross-crate cutover and TOML conversion churn. The gate only needs a parsed view. Smallest fix: keep stored/default inference as `Vec<String>` and add a non-invasive parser/accessor (`tool_sink_classes`) returning parsed `SinkClass` for gate UI/logic, unless there is a concrete safety reason to change the storage type.

## Nits

### N-1. §Goal item 1 points at a non-existent §OA

**Anchor:** `scope.md:207-209`.

“via a new bus event (§OA below)” should be `§OP2` (or create an `OA` section if intended).

### N-2. `fittings` dependency name is not a live workspace crate name

**Anchor:** §W1 (`scope.md:610-617`).

m4 crates depend on `fittings-core`, `fittings-server`, `fittings-client`, `fittings-transport`, etc., not a single `fittings` crate. Spell the actual dependencies to avoid another m4-style placeholder chase.

### N-3. “new bin target `rfl install`” is imprecise

**Anchor:** §Tr1 (`scope.md:664-666`).

`rfl` already exists as the CLI binary; `install`/`status` are subcommands, not new bin targets. Reword to “new `rfl install` subcommand”.

### N-4. Stray slash after `always_allow_session`

**Anchor:** §Goal item 5 (`scope.md:273-279`).

“The user answering `always_allow_session` (`/`);” looks like a typo. Remove `(/)` or explain it.

### N-5. Live reserved-env count is stale

**Anchor:** §M1.1 (`scope.md:1182-1185`).

The live scrubber list contains seven names after m4 (`RFL_PROVIDER_ID` included), not “six per row 40”. Reword to “currently seven in live source; m5a adds …” after deciding B-5.

### N-6. Negative 2 says the env var is unset and set at the same time

**Anchor:** demo Negative 2 (`scope.md:1312-1323`).

It says the second invocation has `RFL_TUI_TEST_CONFIRM_ANSWER` unset, then says the test injects deny via `RFL_TUI_TEST_CONFIRM_DELAY_MS=10` + `RFL_TUI_TEST_CONFIRM_ANSWER=deny`. Pick one phrasing: e.g. “no pre-existing grant; automated TUI answers deny after 10ms.”

## Convergence call

Blocking count: **7**. Major count: **8**. Nit count: **6**.

My read: convergence is plausible in 2-3 more scope rounds if the next draft resolves the process-boundary and wire-shape blockers mechanically. This is not a “rewrite from zero,” but it is more than polish: the confirmation protocol table, slash-command authority path, synthetic deny event, OpenAI env model, and install CLI boundary need explicit design choices before `commits.md`.

Owner-judgment items to surface before further rounds:

1. The **m5a/m5b split** itself: acceptable only if owner agrees that m5a is not the full m5 roadmap row and m5b remains mandatory for the verbatim-exfil negative.
2. The **`user_grants` matcher** deviation from JSON-Schema/`grant_match` semantics.
3. The **env.pass rename / canonical OpenAI env** schema extension versus a simpler deployment-configured API-key env name.
4. Whether the confirm modal is **TUI-internal transient UI** or a persisted/rendered entry kind.
