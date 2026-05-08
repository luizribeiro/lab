# Pi review 1 — rafaello v1 milestones overview

Review target: `rafaello/plans/milestones/README.md` as introduced by
`45d201d docs(rafaello-milestones): draft v1 milestones overview`.

Verdict: **do not ratify as-is**. The roadmap is useful and close to a
workable implementation plan, but it currently contradicts the converged
v1 architecture in several load-bearing places. The biggest issues are
silent v1 scope cuts, manifest/lock work arriving too late, a temporary
non-v1 tool-dispatch path, and branch/workflow drift from
`plans/README.md`.

## Summary of blocking findings

1. **The “confirmed deferrals” section silently overrides the source of
   truth.** Helper plugins, frontend principals, streaming patch ops, and
   subprocess renderers are all listed as deferred in the milestones doc,
   but `overview.md` / `decisions.md` currently commit them to v1.
2. **The branch model conflicts with the top-level milestone workflow.**
   The milestone draft says all work accumulates on `rafaello-v0.1` and
   `main` waits until v1 is demo-ready; `plans/README.md` says each
   milestone branch merges to `main` after its retrospective.
3. **Manifest/lock/schema prerequisites are sequenced too late.**
   `overview.md` says Stream F manifest deltas must land before m1
   implementation, but the roadmap spawns hardcoded plugins before the
   real manifest/lock/grant/compiler path exists.
4. **Tool dispatch is introduced before its security envelope.** m3 adds
   an agent loop with one tool before provider plugin routing, sink
   confirmation, taint, `in_reply_to`, and `user_grants` are present.
5. **Several demos are underspecified relative to the architecture.**
   They prove happy paths but not the negative/security invariants that
   make the design valuable.

## Blocking findings in detail

### 1. Scope drift: “confirmed deferrals” contradict `overview.md`

`milestones/README.md:25-40` says the following primitives stay in
`decisions.md` but are not implemented in v1:

- helper plugins (`bindings.helper_for`, `RFL_HELPER_FD`);
- frontend principals beyond TUI;
- streaming patch ops (`stream_state: "open"` / `"patch"`);
- subprocess plugin renderers;
- plugin-level interception;
- Capsa runtime backend.

Only the last two are clearly aligned with the current architecture.
The first four contradict the single source of truth:

- **Helper plugins are a v1 primitive.**
  - `overview.md:757-785` specifies helper plugin spawn, lock bindings,
    `RFL_HELPER_FD`, no bus fd, lifecycle ownership by core.
  - `overview.md:964-965` lists helper plugins as load-bearing decision
    14.
  - `decisions.md` row 14 says “Helper plugins are a v1 primitive
    (`bindings.helper_for`, `RFL_HELPER_FD`).”
  - `overview.md:1295-1309` includes helper plugins in the explicit
    “In v1” scope cut.

- **Frontends are first-class v1 bus principals, not merely a reserved
  namespace.**
  - `overview.md:787-823` specifies local-spawned TUI and
    external-attached UDS frontends as v1 frontend flavours.
  - `overview.md:966-967` lists frontend principals as load-bearing
    decision 15.
  - `decisions.md` row 15 says frontends are first-class bus principals
    with UDS+token attach for external frontends.
  - `overview.md:917-928` says `rfl serve` runs without TUI and waits for
    external frontends to connect to the attach socket.
  - What is deferred is **network-attached TCP frontends**, not UDS
    frontend principals (`overview.md:809-811`, `overview.md:1322`).

- **Streaming patch/finalization is currently in v1.**
  - `overview.md:855-899` specifies `core.session.entry.appended`,
    `core.session.entry.patched`, and `core.session.entry.finalized`, and
    states that dropped intermediate patches are acceptable because
    `finalized` is authoritative.
  - `overview.md:981-982` pins `core.session.entry.*` as a load-bearing
    spelling decision.
  - `decisions.md` row 20 mirrors that topic decision.

- **Subprocess renderers are currently in v1.**
  - `overview.md:835-873` describes rendering as in-process for built-in
    kinds and subprocess for plugin-provided kinds, with
    `renderer.render` and daemon-side cache.
  - `overview.md:888-890` calls out `renderer.render` as a request/
    response method on the renderer plugin’s fittings server.
  - `overview.md:1295-1309` includes the renderer model in the “In v1”
    scope cut.

If the owner has intentionally cut these items, that is fine, but the
cut must be represented as architecture work first: add explicit
reversal/deferral rows to `decisions.md`, patch `overview.md`’s v1 scope
cut, and then update milestones. The milestone overview should not be the
first place where these v1 commitments change.

### 2. Branch model conflicts with `plans/README.md`

The milestone draft says:

- all work accumulates on `rafaello-v0.1`;
- per-milestone branches rebase into it;
- `main` stays at the `rafaello-design` merge until v1 is demo-ready;
- `rafaello-v0.1` merges to `main` only at the end
  (`milestones/README.md:62-72`).

The top-level workflow says after every milestone:

- pi reviews the full milestone diff;
- retrospective updates land;
- **the milestone branch merges to `main` with linear history**
  (`plans/README.md:52-60`).

Pick one model and update both docs. This is not just process trivia:
retrospectives, owner ratification, and “overview/decisions drift is
fixed before proceeding” have different enforcement strength depending
on whether each milestone lands on `main` or only on a long-running v1
branch.

My recommendation: for a paper-to-code v1 effort, a long-running
`rafaello-v0.1` integration branch is defensible, but then
`plans/README.md` must explicitly say milestone merges target
`rafaello-v0.1`, and `main` only receives owner-ratified design updates
or the final v1 merge. Alternatively keep the current top-level workflow
and let each milestone land on `main` behind incomplete but tested CLI
surfaces.

### 3. Manifest / lock / Stream F work is sequenced too late

The current roadmap has:

- m1: core skeleton + bus broker + sandboxed plugin spawn using a
  hardcoded test plugin (`milestones/README.md:47`);
- m2: manifest + lock + grant flow + lockin policy compilation
  (`milestones/README.md:48`).

This conflicts with the architecture’s own prerequisite:

- `overview.md:1041-1058` says Stream F manifest fields are missing and
  the precise schema delta “must adopt before m1 implementation.”
- `overview.md:1060-1078` defines the required `[provides]` block and
  lock snapshots for tools, provider id, sinks, grant match,
  `always_confirm`, and helpers.
- `overview.md:1092-1117` defines `helper_for` and install-time
  validation.
- `overview.md:1147-1149` says items 1–4 are real Stream F work for m1.

A hardcoded m1 plugin creates exactly the path the architecture is trying
to avoid: a plugin can be spawned without a lock entry whose bindings are
the single source of runtime authority. That may be acceptable for a
throwaway transport test, but it should not become the first
`rafaello-core` milestone.

Additionally, m2’s parenthetical “Manifest format (no `runtime`, no
`[rpc]`, `openrpc.json` sibling)” is not yet an authoritative cut in the
current docs. Stream F currently describes `runtime` and `[rpc]` in
`streams/f-manifest/rfc-manifest-schema.md`; `overview.md:1041-1149`
requires a `[provides]` delta but does not clearly ratify removing
`runtime` / `[rpc]`. If this is a new owner decision, it needs a
`decisions.md` row and a Stream F/overview reconciliation before
implementation milestones depend on it.

Recommendation: move manifest/lock/grant/compiler ahead of real plugin
spawn, or make m1’s spawn use a hand-authored lock fixture with the same
bindings shape that m2 will later write.

### 4. Tool dispatch is introduced before the security envelope

m3 currently includes “agent loop dispatches tool calls” and a mock
provider (`milestones/README.md:49`). m4 then adds `rfl-litellm`, sink
declarations, confirmation protocol (`line 50`). m5 then adds taint,
`in_reply_to`, `user_grants`, and trifecta (`line 51`).

The architecture says these are not optional later hardening layers; they
are the shape of the only valid tool path:

- `overview.md:570-610`: mandatory taint propagation and the canonical
  sink-confirmation rule.
- `overview.md:631-643`: `user_grants` is the only confirmation bypass.
- `overview.md:655-665`: confirmation protocol.
- `overview.md:667-691`: provider namespace → core validation/gating →
  tool plugin is the only path from LLM-shaped output to tools.
- `overview.md:703-706`: providers publish on `provider.<provider-id>.*`
  and core re-emits canonical `core.*`.
- `decisions.md` rows 7–10 capture these as load-bearing decisions.

Do not land a temporary m3 agent loop that dispatches directly to a tool
or uses a built-in mock provider outside the plugin/provider namespace
model. If a mock provider is useful, make it a locked subprocess provider
fixture and route it through the same bus/security envelope as
`rfl-litellm`.

### 5. m0 is overloaded unless scoped tightly

m0 currently includes “PeerHandle, ServiceContext, error preservation,
JsonRpcId migration, cancellation semantics, bounded notify” and demos
MCP server outbound notifications, bidirectional calls, JS-SDK interop,
and tests for every RFC bullet (`milestones/README.md:46`).

That is a large amount of transport surgery for a single milestone:

- `Request.id: Option<JsonRpcId>` and response id preservation;
- predefined JSON-RPC error preservation;
- bounded notification channel and deadlock refactor;
- cancellation semantics and malformed cancellation handling;
- bidirectional `PeerHandle` with outbound server-side calls;
- `Client::with_service` inbound request handling;
- interop tests.

It may still be acceptable because it is self-contained in `fittings/`,
but the milestone scope should explicitly say m0 can split if the first
`scope.md` finds more than a small PR series. The acceptance criteria
should be grouped by RFC area so agents can land independent commits
without producing a mega-change.

### 6. m3 is too broad as written

m3 bundles:

- a separate `rafaello-tui` crate;
- `rfl serve` daemon mode;
- inherited-fd or attach-socket frontend attach;
- agent loop;
- tool-call dispatch;
- built-in renderers;
- turn-by-turn rendering;
- mock provider.

That spans frontend process model, session lifecycle, bus attach,
rendering, provider interaction, and tool dispatch. It is likely too big
for the “small number of sessions” sizing rule in
`milestones/README.md:8-10`.

Split it. A cleaner boundary is:

- one milestone for TUI/daemon/session/rendering with static or recorded
  entries, no tool dispatch;
- one milestone for provider fixture + agent loop + secure tool dispatch.

### 7. m4 and m5 split the confirmation/taint story in a risky way

m4 adds sink declarations and confirmation. m5 adds taint,
`in_reply_to`, `user_grants`, and exfiltration tests. But confirmation
UI, sink metadata, and `user_grants` are tightly coupled:

- `overview.md:577-585`: any sink call requires confirmation unless a
  matching `user_grants` entry exists.
- `overview.md:631-643`: `user_grants` entries come from slash command,
  `always_allow_session`, or provider structured grant proposal.
- `overview.md:970-977` in the security RFC cross-reference table makes
  `in_reply_to` required for provider requests and confirm answers.

You can stage implementation internally, but the first milestone that
claims “real model proposes a sink-declaring tool; confirmation prompt;
user accepts/denies” should also define what happens to repeated
invocations and the `always_allow_session` path, or clearly mark that it
is a temporary UX-only confirmation with no final bypass semantics yet.

### 8. Demo gaps and negative tests

The milestone demos mostly show happy paths. Add explicit negative demos
or tests for the architectural invariants:

- **m1/m2 spawn path:** plugin cannot publish outside its namespace;
  core rejects invalid topic/pattern; spawn refuses missing or stale lock
  digest; lockin denies out-of-grant file/network access.
- **manifest/install:** conflicting tool bindings are lock-time errors;
  mutable-local plugin source is visibly labelled; digest mismatch
  triggers re-confirmation; default sink inference works for network and
  write grants.
- **frontend attach:** external attach rejects wrong token, token reuse,
  duplicate attach id, and publish attempts outside
  `frontend.<attach-id>.*`.
- **tool dispatch:** provider cannot bypass core by sending directly to a
  plugin; missing `in_reply_to` is rejected where required; provider
  tool request with stale/unknown correlation id fails closed.
- **confirmation:** timeout denies; deny path does not dispatch to tool;
  `always_allow_session` creates only in-memory grant; restart clears it.
- **taint/exfil:** verbatim result-to-sink flow is blocked or confirmed;
  filesystem-write sink is treated as a sink, not just network.

The current m5 demo (“scripted exfiltration attempt blocked at the
broker”) is good but too late if earlier milestones already ship tool
routing demos.

## Hidden dependencies to make explicit

- **Lock shape before broker ACL.** The broker’s publish/subscribe ACL is
  derived from lock bindings. Even a skeleton broker needs either a real
  lock parser or a lock-shaped fixture.
- **Stream F reconciliation before plugin examples.** Plugin examples
  need final answers for `[provides]`, `helper_for`, OpenRPC placement,
  `runtime`, tool sinks, and `always_confirm`.
- **Provider fixture before agent loop.** The agent loop should consume a
  provider plugin event stream, not a direct in-process mock API, unless
  that in-process mock is explicitly test-only and cannot leak into the
  runtime architecture.
- **Frontend attach before confirmation UX.** `core.session.confirm_*`
  relies on frontends being authenticated bus principals; confirmation
  UI should not be implemented as a privileged side channel.
- **Renderer capability negotiation before plugin renderers.** If
  subprocess renderers stay in v1, `frontend.hello` and server-side
  downgrade need to exist before plugin-renderer demos are meaningful.
- **Session persistence before reliable rendering demos.** Renderer replay
  and final authoritative frames depend on entry persistence under
  `.rafaello/state/`.

## Deferrals: what is currently valid vs invalid

Valid deferrals already aligned with the architecture:

- Capsa runtime backend: deferred to v2 (`overview.md:1315-1317`,
  `decisions.md` row 2).
- Plugin-level interception / CaMeL middleware behaviour: v2 territory;
  v1 ships primitives, not CaMeL itself (`overview.md:1315`).
- TCP/network-attached frontends: deferred; UDS frontends remain v1
  (`overview.md:809-811`, `overview.md:1322`).
- Dynamic/per-method sandbox policies: deferred; v1 flattens and enforces
  per-method in core (`overview.md:1323-1324`).

Invalid deferrals unless preceded by architecture changes:

- helper plugins;
- UDS frontend principals beyond the local TUI;
- streaming patch/finalized entry model;
- subprocess plugin renderers.

If the project owner wants the leaner cut implied by the milestone draft,
then add explicit rows to `decisions.md` reversing or deferring decisions
14, 15, the renderer/subprocess-renderer commitment, and the streaming
patch commitment, and patch `overview.md` §10, §11, §12, §13, and §16.

## Answers to the five open questions

### 1. Is m1 → m2 → m3 ordering correct, or should `rafaello.lock` come before plugin spawning?

`rafaello.lock` / manifest / grant compiler should come before any real
plugin spawning. The first plugin we run should either:

- go through the real install/grant flow, or
- be driven by a hand-authored lock fixture with the exact final lock
  bindings shape.

Do not create a hardcoded spawn path that later gets retrofitted. The
whole architecture is “manifest is request, lock is grant, lockin is
enforcer” (`overview.md:431-499`, `overview.md:551-557`), so an
unlocked spawn path should be limited to a fittings-only test binary, not
`rafaello-core` runtime design.

### 2. Is splitting m3 warranted?

Yes. Split m3 into at least:

1. daemon/session/frontend/rendering without agent loop or tool dispatch;
2. provider fixture + agent loop + secure tool dispatch.

The current m3 is too large and crosses too many subsystem boundaries.
It also risks introducing tool dispatch before confirmation/taint are in
place.

### 3. Should a built-in mock provider exist before m4?

Yes, but only as a test/bundled **subprocess provider plugin fixture**
that goes through the same lock, bus namespace, and provider-active path
as any other provider. Avoid a built-in core provider because
`overview.md:708-755` explicitly says even the default LiteLLM provider
is bundled but not built into core.

A deterministic mock provider plugin is useful for tests and demos before
`rfl-litellm`; it should publish `provider.<provider-id>.tool_request`
and `provider.<provider-id>.assistant_message`, and let core re-emit
canonical `core.*` events.

### 4. Is “no third-party plugin renderers in v1” worth a `decisions.md` row?

Yes — if that is the actual decision. It affects plugin authors and
changes the renderer architecture. But it cannot live only in the
milestone README, because the current overview includes subprocess/plugin
renderers in v1 (`overview.md:835-873`).

So either:

- keep plugin/subprocess renderers in v1 and remove the deferral; or
- add a `decisions.md` row deferring third-party/subprocess renderers,
  patch `overview.md` §11 and §16, and make m3/m6 reflect built-in-only
  rendering.

### 5. Is m6 polish better folded into m5?

Keep m6 separate. Release readiness, docs, packaging, cross-platform nix
builds, Homebrew formula, and end-to-end manual validation are real work.
Folding them into m5 will either hide release risk or make m5 too large.

However, remove or ratify open-ended feature scope like “additional
built-in tools if useful.” If `write-file` or `run-shell` are v1 tools,
make them explicit in the relevant security/tooling milestone with sink
semantics and demos. If they are opportunistic polish, keep them out of
v1 acceptance criteria.

## Suggested high-level reorder

A safer sequencing that preserves the architecture:

1. **m0 — fittings v1.**
   Land JSON-RPC id migration, error preservation, cancellation,
   bounded notifications, and bidirectional `PeerHandle`. Keep it
   self-contained in `fittings/`. Split internally by RFC area if needed.

2. **m1 — manifest/lock/grant/compiler foundation.**
   Reconcile Stream F first. Implement manifest parsing, lock shape,
   digest pinning, grant review, lockin policy compilation, topic-id
   derivation, bindings snapshot, and negative compiler tests. This may
   use a minimal fixture plugin but should not require full broker
   runtime.

3. **m2 — rafaello-core broker + locked plugin spawn.**
   Add minimal `rafaello-core`, spawn plugins only from locked entries,
   bind principals to inherited fds, enforce publish/subscribe ACLs,
   support `bus.publish` fan-out, and prove lockin denial for
   out-of-grant operations.

4. **m3 — sessions, daemon/frontend attach, and built-in rendering.**
   Implement `rfl serve`, local-spawned `rfl-tui`, UDS attach/token,
   `frontend.hello`, session entry persistence, built-in renderers, and
   final-frame rendering. If streaming patches remain v1, include
   append/patch/finalized semantics here; otherwise first patch the
   architecture to final-only.

5. **m4 — provider fixture + secure agent loop + one read-only tool.**
   Add a deterministic mock provider as a subprocess plugin fixture,
   route through `provider.<id>.*`, enforce `in_reply_to` where required,
   dispatch a read-only tool via `core.session.tool_request`, and reject
   direct/bypass paths. This gives the “what’s in README.md” demo without
   introducing LiteLLM yet.

6. **m5 — LiteLLM provider, sinks, confirmation, taint, `user_grants`,
   exfiltration demo.**
   Add bundled `rfl-litellm`, real model path, sink declarations,
   confirmation UI, `always_allow_session`, slash grants, taint matching,
   one-hop trifecta guardrail, and scripted exfiltration tests. If this
   is too large, split into m5a confirmation/sinks and m5b taint/exfil,
   but do not let a sink-capable tool dispatch without the gate.

7. **m6 — release readiness.**
   Close test gaps, docs, packaging, cross-platform build, manual
   litellm session capture, and retrospective cleanup. No opportunistic
   new tools unless owner-ratified.

## Concrete edits requested before ratification

- Remove or reclassify the invalid “confirmed deferrals” in
  `milestones/README.md:25-40`, or patch `overview.md` / `decisions.md`
  first to make them true.
- Align branch model between `milestones/README.md` and
  `plans/README.md`.
- Move manifest/lock/compiler work before core plugin spawn, or constrain
  m1 to a lock-shaped fixture.
- Split m3.
- Ensure the first agent-loop/tool-dispatch milestone uses provider
  namespace + core re-emission + `in_reply_to` + confirmation/taint hooks,
  even if some gates initially operate in a no-sink/read-only mode.
- Expand demos with negative/security tests, especially around ACL,
  digest/grant mismatch, attach auth, confirmation timeout, and bypass
  rejection.
