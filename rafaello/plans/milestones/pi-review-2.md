# Pi review 2 — revised rafaello v1 milestones overview

Review target: the revised `rafaello/plans/milestones/README.md` after
round-1 feedback, plus the accompanying architecture-doc patches in:

- `915e80d docs(rafaello-decisions): record v1 deferrals and manifest simplifications`
- `27ad1d4 docs(rafaello-overview): patch §16 v1 scope cut to match deferrals`
- `461cad9 docs(rafaello): align branch model with rafaello-v0.1 accumulation`
- `a9cca35 docs(rafaello-milestones): rewrite per pi-review-1`

Verdict: **much improved, but not ratifiable yet**.

The milestone sequencing is now basically right: manifest/lock/compiler
precedes spawn, frontend/rendering is split from the agent loop, the mock
provider is correctly a subprocess plugin fixture, and the branch model is
aligned. The remaining blockers are mostly incomplete architecture-doc
reconciliation after the new v1 deferrals, plus one security-shape issue
around taint staging.

## Executive summary

Round 1's largest roadmap issues are addressed:

- m1/m2 order is fixed: manifest/lock/compiler now lands before plugin
  spawn.
- The previous oversized m3 is split into frontend/rendering (m3) and
  agent-loop/tool-dispatch (m4).
- The mock provider is now a locked subprocess plugin fixture, not a
  built-in core shortcut.
- Branch model is now consistently `rafaello-v0.1` accumulation.
- Demos include explicit negative/security tests.
- m6 no longer has opportunistic extra tools.

However, the deferral patch only updated `overview.md` §16 and
`decisions.md`. Earlier sections of `overview.md` still state that
helper plugins, external UDS frontends, streaming patch ops, and
subprocess renderers are in v1. Because `overview.md` is the source of
truth, this inconsistency must be fixed before owner ratification.

## Round-1 finding verdicts

### R1 finding 1 — “confirmed deferrals” silently override the source of truth

**Verdict: partially fixed, still blocking.**

The good part: the revised milestone README no longer asserts unbacked
scope cuts. It now points to explicit decision rows:

- helper plugins deferred: `decisions.md` row 26;
- external UDS-attached frontends deferred: row 27;
- streaming entry patch ops deferred: row 28;
- subprocess plugin renderers deferred: row 29.

`overview.md` §16 was also patched to include those deferrals in the v1
scope cut.

The remaining problem: earlier `overview.md` sections still contradict
those new rows. See blocking finding 1 below for details. Until the
whole overview is internally consistent, implementers can follow a stale
section and build the wrong v1.

### R1 finding 2 — branch model conflicts with top-level workflow

**Verdict: fixed.**

`plans/README.md` now says milestone branches merge into the
long-running `rafaello-v0.1` integration branch, and `main` waits until
v1 is demo-ready. The milestone README says the same thing and cites
`decisions.md` row 33.

This resolves the process contradiction from round 1.

### R1 finding 3 — manifest / Stream F prerequisites are too late

**Verdict: fixed at the milestone level; stream RFC drift remains.**

The revised roadmap puts manifest / lock / grant / compiler foundation in
m1, before broker + locked plugin spawn in m2. This fixes the dangerous
hardcoded-spawn-first path.

The remaining issue is not the milestone order; it is documentation
consistency. Decisions rows 30–32 say v1 omits `runtime`, drops `[rpc]`,
requires an `openrpc.json` sibling, and compiles via the lockin Rust
builder API. But Stream F still describes `runtime` and `[rpc]`. Either
patch Stream F now or explicitly mark the drift so m1 implementers know
which source wins.

### R1 finding 4 — tool dispatch introduced before its security envelope

**Verdict: mostly fixed, but one blocker remains.**

The revised roadmap moves agent-loop/tool-dispatch to m4, after
manifest/lock and broker/spawn. It also makes the mock provider a locked
subprocess plugin and routes through provider namespace + core re-emit.
That addresses the major bypass concern.

Remaining blocker: m4 introduces canonical `core.session.tool_request` /
`tool_result`, but m5 adds taint propagation. Decisions rows 7–8 and the
overview make taint part of the mandatory canonical event envelope. m4
should include the mandatory taint field shape and `in_reply_to`
enforcement; m5 can add richer value-matching taint propagation and the
sink/exfiltration story.

### R1 finding 5 — m0 overloaded unless scoped tightly

**Verdict: fixed enough for roadmap ratification.**

m0 now explicitly says it may split internally by RFC area if scoping
finds more than a small PR series. The demo covers outbound
notifications, bidirectional `PeerHandle::call`, cancellation, JS-SDK
interop, and malformed/edge-case tests.

That is still a large milestone, but it is self-contained in `fittings/`
and the split note is adequate at roadmap granularity.

### R1 finding 6 — m3 too broad

**Verdict: fixed.**

The old m3 has been split:

- m3: sessions, daemon, local TUI, built-in rendering, no provider, no
  agent loop, no tool dispatch;
- m4: provider fixture, secure agent loop, one read-only tool.

This is a much better boundary.

### R1 finding 7 — m4/m5 split confirmation/taint story riskily

**Verdict: partially fixed.**

The revised m5 keeps LiteLLM, sinks, confirmation, taint,
`user_grants`, and exfiltration demo together, with an explicit m5a/m5b
split option if too large. That is better than separating confirmation
from the rest of the security story.

But m4 still needs the mandatory taint envelope for canonical events, as
noted above. Confirmation can wait until m5 because m4's first tool is
read-only/non-sink; the canonical event schema should not wait.

### R1 finding 8 — demos lack negative/security tests

**Verdict: fixed.**

Every milestone now has negative/security invariants in its demo bar.
The revised demos cover namespace rejection, lockin denial, missing lock
entry, invalid topic grammar, duplicate daemon, forbidden frontend
publish, missing `in_reply_to`, stale provider ids, confirmation timeout,
restart-cleared grants, broker-blocked exfiltration, and explicit
one-hop-only guardrail scope.

## Blocking findings for round 2

### 1. `overview.md` still contradicts the new deferrals

**Severity: blocking.**

Claude patched `overview.md` §16, but earlier sections still say the
newly deferred items are v1 commitments.

Concrete stale sections:

- **Goals still promise deferred scope.**
  - `overview.md:51-53`: “Multiple frontends over one bus” says daemon
    mode lets web/IDE/email frontends attach over JSON-RPC.
  - `overview.md:54-57`: “CaMeL-as-plugin is buildable on v1” says every
    primitive CaMeL needs, including helper plugins, is committed in
    this document.

- **Process model still includes helpers, plugin renderers, external
  frontends, and attach sockets.**
  - `overview.md:130-150`: diagram includes helper process,
    plugin-renderer process, TCP/UDS frontend path, and web/IDE/email
    frontends.
  - `overview.md:164-177`: helper plugins and external frontends are
    described as v1 process-model participants.
  - `overview.md:181-192`: trust posture includes helpers and external
    frontend principals as if both are v1.

- **Manifest/lock section still includes deferred helper/renderer
  bindings as required v1 fields.**
  - `overview.md:450-459`: manifest/lock text includes renderer
    registrations and `helper_for` in the v1 authority snapshot.
  - `overview.md:500-514`: required manifest fields still list
    `[[renderers]]` and `helper_for`, and says the NEW rows must land
    before m1 implementation.

- **Bus/fittings section still uses deferred examples as v1 drivers.**
  - `overview.md:238-253`: examples include `helper.spawn`,
    `renderer.render`, `frontend.hello`, and CaMeL helper-spawn handshake
    as v1 peer-call drivers.

- **§9 still defines helper plugins as a v1 primitive.**
  - `overview.md:757-785`: helper plugins are fully specified as v1.

- **§10 still defines external-attached frontends and `frontend.hello`.**
  - `overview.md:787-823`: web/IDE/email UDS attach and capability
    negotiation are still written as v1.

- **§11 still defines streaming patches and subprocess renderers.**
  - `overview.md:855-873`: appended/patched/finalized streaming and
    `renderer.render` subprocess renderer cache are still written as v1.

- **§13 load-bearing decisions are stale.**
  - `overview.md:930-982`: decisions 14, 15, 19, and 20 still state the
    pre-deferral v1 commitments without noting rows 26–29.

This is the main blocker. `decisions.md` rows 26–29 and `overview.md`
§16 now say the right thing, but the source-of-truth overview is
internally inconsistent. Patch the earlier sections, not just §16.

### 2. Stream RFC drift is now large enough to call out explicitly

**Severity: blocking unless explicitly documented as known drift.**

The repository convention allows stream RFCs to drift, with `overview.md`
winning. But after decisions rows 26–32, the drift is broad enough that
m1/m2 implementers could easily follow stale stream text.

Examples:

- **Stream A still treats helper plugins and external UDS frontends as
  v1.**
  - `streams/a-security/rfc-security-model.md:508-560`: external
    frontend attach/auth flow is specified as v1.
  - `streams/a-security/rfc-security-model.md:1076-1215`: helper plugin
    primitive is specified as accepted in v1.

- **Stream E still treats streaming patches and subprocess renderers as
  v1.**
  - `streams/e-renderer/rfc-renderer-model.md:34-43`: entries flow as
    appended/patched/finalized.
  - `streams/e-renderer/rfc-renderer-model.md:230-246`: open/patch/final
    protocol is specified.
  - `streams/e-renderer/rfc-renderer-model.md:321-350`: subprocess
    `renderer.render` is specified.

- **Stream F still has manifest fields now removed by decisions rows
  30–31.**
  - `streams/f-manifest/rfc-manifest-schema.md:35-58`: `runtime` is still
    in the top-level shape.
  - `streams/f-manifest/rfc-manifest-schema.md:60-80`: `[rpc]` is still
    specified.

Acceptable fixes:

1. Patch the stream RFCs now to match the new v1 cut; or
2. Add an explicit “known drift after decisions rows 26–32” section in
   `overview.md` and/or the milestone README, listing exactly which
   stream sections are stale and what implementers should follow instead.

I prefer patching at least Stream F before m1, because m1 directly
implements the manifest parser and install validation. Stream A/E drift
can be deferred if the overview clearly marks it.

### 3. m4 still delays mandatory taint too far

**Severity: blocking for roadmap ratification.**

m4 introduces canonical `core.session.tool_request` and
`core.session.tool_result`, plus `in_reply_to` enforcement. m5 then adds
“taint propagation (`{source, detail}`) on `tool_request` /
`tool_result`.”

This staging is not quite right. Per the current architecture:

- `decisions.md` row 7: mandatory taint on `core.session.tool_request`
  and `core.session.tool_result`, populated by core.
- `decisions.md` row 8: mandatory `in_reply_to` where taint inherits.
- `overview.md:570-610`: taint and sink confirmation are part of the
  core broker/tool gate, not an optional later envelope.

For m4's read-only, non-sink tool, it is fine that sink confirmation and
`user_grants` wait until m5. But the canonical bus event schema should
already include the mandatory taint envelope in m4. At minimum m4 should
emit and validate empty/minimal taint arrays on canonical tool events,
plus `in_reply_to`. m5 can then add:

- value-matching taint propagation from previous tool results into later
  tool requests;
- sink-aware confirmation prompts;
- `user_grants` bypass matching;
- exfiltration tests.

Suggested milestone wording change:

- m4: “Mandatory canonical event envelope: `in_reply_to` and `taint`
  fields present and validated; taint may be empty/minimal for the
  read-only fixture path.”
- m5: “Full taint propagation/matching and sink/exfiltration semantics.”

### 4. `rfl serve` semantics are unclear after external attach deferral

**Severity: blocking unless clarified.**

Decision row 27 defers external UDS-attached frontend principals to v2.
The revised milestones say v1 has a local-spawned TUI only.

But m3 still ships `rfl serve` daemon mode:

- `milestones/README.md:53`: “`rfl serve` daemon … local-spawned by
  `rfl chat` (no external attach in v1).”
- `overview.md:1298-1301`: §16 still lists `rfl serve` among v1 CLI
  subcommands.

If external attach is deferred and TUI is the only frontend, what does a
headless public `rfl serve` do? It cannot wait for external frontends to
connect, because that attach path is no longer v1.

Acceptable fixes:

1. Define `rfl serve` in v1 as an internal/test-oriented core daemon that
   accepts only test injection or local-spawned TUI attachment via an
   inherited fd; or
2. Defer the public `rfl serve` CLI until external attach returns in v2,
   and make `rfl chat` spawn core + TUI directly for v1; or
3. Keep a non-public library/test harness daemon and remove `rfl serve`
   from the user-facing v1 CLI list.

Right now the roadmap and §16 imply a public daemon command, but the only
useful user-visible daemon mode was just deferred.

## Focus-area review

### Sequencing

The high-level order is now sound:

`m0 fittings → m1 manifest/lock/compiler → m2 broker + locked spawn → m3 sessions/TUI/rendering → m4 provider fixture + read-only tool dispatch → m5 LiteLLM/sinks/confirmation/taint/user_grants/exfil → m6 release`

This fixes the major round-1 sequencing issue. The only sequencing tweak
needed is moving the mandatory taint envelope shape from m5 into m4, with
m5 keeping the richer propagation and sink logic.

### Sizing

- m0 remains large, but the internal split note is adequate.
- m1 is appropriately data-transform/test-heavy and has no runtime
  dependency except on final schema choices.
- m2 is a reasonable first runtime milestone.
- m3 is now much better sized after the split, but renderer panic
  isolation should be explicit if the demo promises renderer crashes do
  not crash the TUI.
- m5 is still large, but the m5a/m5b split note is acceptable because the
  tightly coupled security story is acknowledged.

### Demo gaps

The demo bars are much stronger than round 1. Remaining demo clarifications:

- m3 “renderer crash for one kind doesn't crash the TUI” should specify
  how in-process built-in renderer panics are isolated, since there are
  no subprocess renderers in v1.
- m4 should add taint-envelope presence/validation negative tests, not
  just `in_reply_to` tests.
- m3 should clarify what mechanism injects static fixture entries if
  external attach is deferred.

### Hidden dependencies

Most hidden dependencies from round 1 are now surfaced. Remaining ones:

- m1 depends on final Stream F schema decisions being reflected somewhere
  implementable, not just in `decisions.md` prose.
- m3 depends on a clear v1 definition of local TUI attachment / daemon
  lifecycle after external UDS attach is deferred.
- m4 depends on canonical bus event schemas being stable enough to include
  taint and `in_reply_to` from the start.

### Scope drift / deferrals

The decisions log now records the deferrals, which is good. But the
architecture doc patch is incomplete. The key rule should be: if a v1
primitive is deferred in rows 26–29, every earlier overview section that
previously treated it as v1 must either be patched or explicitly marked
as historical/pre-deferral text.

### Branch model

Fixed. `plans/README.md`, the milestones README, and `decisions.md` row
33 now agree on long-running `rafaello-v0.1` accumulation.

## Non-blocking concerns

1. **m5 remains large.** The built-in “may split into m5a/m5b” note is
   acceptable; enforce it during `scope.md` if the commit list grows.
2. **Built-in renderer crash isolation needs a concrete mechanism.** With
   subprocess renderers deferred, “renderer crash doesn't crash TUI” means
   panic isolation around in-process Rust renderers, not process crash
   isolation. That is feasible but should be explicit in m3 scope.
3. **Decision rows say “Owner-approved” but status remains `proposed`.**
   Not fatal, but clarify status semantics. If owner approval has
   happened, consider `ratified`; if not, avoid saying owner-approved.
4. **`frontend.<attach-id>.*` is reserved but still active for TUI.**
   The wording “namespace reserved” can be misread as no frontend
   namespace in v1. The TUI still needs a `frontend.tui.*` principal for
   confirmation answers; external attach is what is deferred.

## Bottom line: four must-fix items before ratification

1. **Patch `overview.md` beyond §16.** Update sections 1, 3, 5, 9, 10,
   11, 13, and 15 as needed so the whole overview matches decisions rows
   26–32. In particular, remove or mark as v2: helper plugins, external
   UDS attach, `frontend.hello`, streaming patch ops, and subprocess
   `renderer.render`.

2. **Clarify or patch stream RFC drift.** Either patch Stream A/E/F now,
   or add an explicit known-drift note listing stale sections and saying
   the overview/decisions rows 26–32 win. Stream F should probably be
   patched before m1 because m1 implements its schema.

3. **Move mandatory taint envelope into m4.** m4 should include canonical
   `taint` fields and validation on `core.session.tool_request` /
   `tool_result`, even if taint is empty/minimal for the read-only tool
   path. m5 can add full matching/propagation, sinks, confirmation,
   `user_grants`, and exfiltration tests.

4. **Define or defer public `rfl serve` for v1.** Under “TUI only, no
   external attach,” headless `rfl serve` needs a clear v1 purpose. Either
   define it as internal/test-only, keep it only as part of `rfl chat`'s
   spawned core lifecycle, or defer the public command until external
   attach returns.

After those four fixes, the milestone overview should be ready for owner
ratification at roadmap granularity.
