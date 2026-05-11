# m5a — sinks + confirmation protocol + user_grants + rfl-openai — scope

> **Status:** round 1 — first-pass draft. No pi review yet. Pi
> will adversarially critique; iteration to convergence per
> `plans/README.md` Phase 2. The roadmap row for m5
> (`milestones/README.md`) is the pre-ratified definition; this
> document proposes a **split into m5a / m5b** and scopes m5a in
> full, with m5b sketched in Appendix A.

---

## Sizing & split recommendation

**Recommendation: split m5 into m5a (this scope) and m5b
(Appendix A sketch).**

The roadmap row explicitly invites a split — *"May split into
m5a (sinks + confirmation + user_grants) and m5b (taint matching
+ exfil tests) if scoping finds it too big"*. The driver
pre-flight (`milestones/m5-sinks-confirmation/driver-preflight.md`)
makes the same call. I agree, with the cleavage line redrawn
slightly compared to the pre-flight's first sketch.

### Why split

m4 ran 28 plan-row commits + 1 follow-up + 6 retro-drift
commits + Phase-2 docs (~71 commits in the milestone cycle).
Honest forward-looking estimate for a unified m5 by
sub-deliverable, rounded by analogy to m4's bracket:

| Sub-deliverable                                              | Commit estimate |
|--------------------------------------------------------------|-----------------|
| `rfl-openai` provider plugin + fixture lock + tests          | 6–10            |
| Manifest `sinks` consumption + install-time trifecta refusal | 4–6             |
| Confirmation gate (broker-side hold + topics + ACL)          | 6–8             |
| `user_grants` table + matcher                                | 3–4             |
| Slash commands (`/grant`, `/grants list`, `/revoke`)         | 3–4             |
| TUI confirmation modal + input blocking                      | 5–7             |
| Audit log (SQLite table)                                     | 2–3             |
| Per-plugin outstanding tool_request map (m4 §5.1 closer)     | 2–3             |
| Lock-side `check_lock_publish_topic` (m4 §2.6 closer)        | 1–2             |
| Taint propagation rules (arg-value matching)                 | 6–9             |
| Plugin-supplied taint validation + superset enforcement      | 3–4             |
| Exfil/verbatim-flow demo + negatives                         | 4–6             |
| Integration test wiring + `rfl chat` four-tree extension     | 4–6             |
| **Total**                                                    | **49–72**       |

Even the optimistic 49 is past m3 (31 commits) and well past
m2 (28). At 60+ commits per milestone the per-commit Phase 3
walltime + the round-trip cost of pi review on a `commits.md`
that wide become an outright tax — m1's `scope.md` took 6 pi
rounds at ~50 commits, and that was a single new crate without
the cross-cutting protocol surface m5 demands.

### Where to cut

There are three plausible cleavage lines:

1. **Plugin first / protocol second** — ship `rfl-openai` as
   m5a; ship the gate + taint as m5b. **Rejected.** This
   delivers no observable security improvement until m5b lands;
   the gate is the load-bearing m5 deliverable. m5a would
   demo *worse* security than m4 (a real model can call
   sink tools without a gate yet) and the demo bar would be
   embarrassing.

2. **Gate first / taint propagation second** — ship the
   confirmation gate, `user_grants`, slash commands, TUI
   modal, install-time trifecta refusal, *and* `rfl-openai`
   in m5a; ship the broker's taint matching/propagation rules
   plus the exfil demo's verbatim-flow negative in m5b.
   **Selected.** The gate is the canonical m5 win
   (`decisions.md` row 9 is taint-independent — every sink
   call needs `user_grants` or a fresh confirm), so m5a alone
   delivers the headline negative ("user denies → tool
   refused"). Taint propagation makes the prompt's wording
   informative (the verbatim-tool-result-to-sink case becomes
   visibly tainted) and adds the structural superset
   enforcement on plugin-supplied taint, but those layer
   *on top of* a stable gate. Independent landing matches m4's
   "envelope first / consumer later" handoff, which worked.

3. **Plugin + gate scaffolding / everything else** — ship
   `rfl-openai` and a stub gate that always allows in m5a; ship
   the real gate in m5b. **Rejected.** Stubs that always allow
   are worse than no gate (false sense of security in tests),
   and the negative-test surface for m5b would have to undo m5a's
   stub assertions.

### What goes in each half

**m5a — sinks + confirmation + user_grants + rfl-openai (this
scope).** Confirmation gate fires on **any** sink-declaring tool
call lacking a matching `user_grants` entry (per `decisions.md`
row 9, taint-independent). Three confirmation topics
(`core.session.confirm_request`, `core.session.confirm_reply`,
`frontend.tui.confirm_answer`); core-mediated; fail-closed on
60 s timeout. `user_grants` in-memory session table populated by
slash commands (`/grant`, `/grants list`, `/revoke`) and by
`always_allow_session` answers. TUI modal blocks input until
answered. Install-time trifecta refusal turns on (live
`trifecta::evaluate` in `crates/rafaello-core/src/trifecta.rs`
already returns `refuse: bool`; m5a wires it into the install
path with a `--i-know-what-im-doing` override flag visible in
`rfl status`). Bundled `rfl-openai` subprocess plugin speaks
OpenAI Chat Completions wire protocol; ships against a CI stub
endpoint and against the dev LiteLLM proxy via manual
validation. Per-plugin outstanding-tool_request map lands here
because the gate needs it (closes m4 §5.1 / pi-3 M-2). Lock-side
`check_lock_publish_topic` unknown-namespace tightening lands
here (closes m4 §2.6 / m3 §2.7). Audit-log SQLite table records
confirmation outcomes.

Demo bar covers the roadmap's positive plus three of four
negatives: confirmation timeout denies; `always_allow_session`
clears on `rfl chat` restart; install-time trifecta refusal
(roadmap "one-hop trifecta only" sub-bullet, with the explicit
"transitive flows are NOT caught" carve-out).

**m5b — taint propagation + exfil demo (Appendix A).** Taint
propagation matching arg values against recently-emitted
`tool_result` payloads in the same session (security RFC
§7.2.1–§7.2.2); plugin-supplied taint validation via
`in_reply_to` superset rule; broker superset enforcement on
re-emission. Adds the verbatim-flow taint badge to the
confirmation prompt's `details` payload. Exfil demo (the
roadmap's fourth negative — "verbatim tool-result-to-sink flow
blocked at the broker") lands as an end-to-end test: a tool
returns a payload containing an attacker URL; the LLM proposes
`fetch` with that URL verbatim; the gate's prompt now shows
the `{source: "tool", detail: "<canonical>"}` taint and the
test scripts a deny.

**Estimated m5a size: 28–36 commits** (m3 / m4 bracket).
**Estimated m5b size: 16–22 commits** (a focused crate-spanning
follow-up; smaller because it builds on m5a's stable surface).

### Risk of going the wrong direction

**If we go single-milestone:** Phase 3 walltime balloons,
`commits.md` gets more pi rounds (m1's pattern was "wider
surface = more rounds"), and the milestone-end retrospective
has to absorb cross-cutting drift between gate/taint/UI. m4
already showed that 4 retro rounds is realistic at moderate
surface; doubling surface risks 6+ retro rounds and the same
in-flight `commits.md` drift the m1 c31/c32 bundle bug warned
about.

**If we go three-way split (the rejected option 1 above):**
the headline security guarantee is delayed by a milestone; the
project owner sees `rfl-openai` work in isolation with a TUI
that calls sink tools unconfirmed — bad demo, worse optics,
and m5b becomes a more cross-cutting milestone than m4 was.

The selected two-way split mirrors m4's "envelope first,
consumer later" cadence, which the m4 retrospective explicitly
endorsed (§4.3) and which pi-2 ratified. **The pre-flight's
breakdown is preserved with one change**: I move the
**per-plugin outstanding-tool_request map** into m5a (not m5b
as the pre-flight implies) because the gate needs the same map
to track which sink-declared tool_request is held — landing the
data structure once in m5a and reusing it in m5b for stale-id
rejection on `plugin.<id>.tool_result.in_reply_to` is cleaner
than building two siblings.

### What if owner pushes back on the split

If the owner declines the split, fall back to a unified m5
scope.md that reuses §"In scope" verbatim and folds Appendix A
into the §"In scope" body, with the §"Out of scope" deferral
list shortened to remove the m5b items. The driver pre-flight
records the same fallback. I do not recommend this path — the
sizing data above is the strongest argument for split — but
the rewrite is mechanical.

---

## Goal

Land the **first sink-confirmation gate**: a `rfl chat` against
the bundled `rfl-openai` provider can answer a user prompt by
proposing a sink-declaring tool call, and core holds that call
behind a TUI modal until the user answers. Approve → tool runs;
deny / timeout → tool refused. Deliver the canonical
sink-confirmation rule (`decisions.md` row 9: taint-independent;
`user_grants` is the only bypass) and the slash-command surface
(`/grant`, `/grants list`, `/revoke`) that lets a user bypass
prompts for invocations they have already authorised.

The deliverable is:

1. **Bundled `rfl-openai` subprocess plugin** (a new
   `crates/rafaello-openai` workspace member with bin target
   `rfl-openai`). Speaks OpenAI Chat Completions wire protocol
   per `decisions.md` row 38; provider id `"openai"`; subscribes
   to `core.session.user_message` and `core.session.tool_result`;
   publishes on `provider.openai.tool_request` and
   `provider.openai.assistant_message`. Endpoint URL,
   `env.pass` API-key var name, and `network.allow_hosts` are
   install-time configuration in the lock — the dev environment
   uses `https://litellm.thepromisedlan.club/v1` with
   `LITELLM_API_KEY` per `plans/README.md` §"Tooling notes".
   For CI the plugin is pointed at a recorded fixture (a tiny
   `httpmock`-style stub bin shipped in
   `crates/rafaello-openai-stub` only when the `test-fixture`
   workspace feature is on) so the integration tests do not
   require network access. The plugin discovers tool schemas
   from the agent loop's compiled tool-routing table via a new
   bus event (§OA below).

2. **Sink-class consumption.** m1 already plumbed the manifest
   shape (`[provides.tool.<n>] sinks = [...]`,
   `always_confirm`, `grant_match` —
   `crates/rafaello-core/src/manifest/provides.rs:35-39`) and
   the lock projection
   (`crates/rafaello-core/src/lock/bindings.rs:22-37`). m1 also
   already implements `sinks::infer_defaults` and
   `sinks::effective_grant`
   (`crates/rafaello-core/src/sinks.rs`) which compute the
   default sink list per the `decisions.md` row-9-aligned
   conservative table (`network`, `workspace_write`). m5a
   wires this through to the confirmation gate: the gate reads
   `CompiledPlugin.tool_meta.<name>.sinks` (already populated
   per `compile.rs:204` / `:440-463`) and decides whether to
   hold the tool_request.

3. **Confirmation protocol on the bus.** Three new
   `core.session.confirm_*` topics (publisher / subscriber
   table per security RFC §5.6; topic spellings per
   `glossary.md` "Confirmation protocol" entry):

   | Topic                          | Publisher | Subscribers                    |
   |--------------------------------|-----------|--------------------------------|
   | `core.session.confirm_request` | core      | frontends, audit subscriber    |
   | `core.session.confirm_reply`   | core      | the holding agent loop         |
   | `frontend.tui.confirm_answer`  | TUI only  | core (validates, re-emits)     |

   Frontend ACL extended: m4 already grants
   `frontend.tui.user_message`
   (`crates/rafaello/src/lib.rs:308-315`); m5a appends
   `frontend.tui.confirm_answer` to the same set. Both directions
   carry mandatory `request_id` (m4 row 43 — confirm topics
   sit alongside `*.tool_request` / `*.tool_result` /
   `*.assistant_message` / `*.user_message` in the
   correlation-bearing class) and mandatory `in_reply_to` on
   `confirm_answer` per security RFC §7.2.6 row 5.

4. **Broker-side gate** (new
   `crates/rafaello-core/src/gate/mod.rs` module). Subscribes
   internally (`Broker::subscribe_internal` from m4) to
   `core.session.tool_request`. For each event:
   - resolve the target tool's `sinks` from the
     `CompiledPlugin.tool_meta` table (passed in at construct);
   - if `sinks.is_empty()` and `always_confirm == false`,
     pass through (republish to the dispatch path the agent
     loop currently takes — see §AL below);
   - otherwise, look up `user_grants` for a matching entry
     (matcher rules per §UG); if matched, pass through and
     audit;
   - otherwise, hold the tool_request in a per-session map
     keyed by `request_id`, build a
     `core.session.confirm_request` payload, publish it, and
     start a 60 s timeout timer (per security RFC §5.6
     `default = "deny"`).
   - on `core.session.confirm_reply` arrival (re-emitted by core
     after `frontend.tui.confirm_answer` validation), the held
     tool_request is either dispatched or rejected with a
     synthetic `core.session.tool_result` carrying
     `{ok: false, error: "user_denied"}` (the agent loop
     persists this as the `tool_result` entry per the m4 entry
     pipeline).

5. **`user_grants` session table** (new
   `crates/rafaello-core/src/user_grants.rs`). In-memory only
   (`Arc<RwLock<UserGrants>>`); populated by:
   - `/grant` slash command (§SL);
   - the user answering `always_allow_session` (`/`);
   - (deferred to v2 / m6 — provider-extracted proposals;
     security RFC §7.2.4 item 3).
   Cleared on `rfl chat` exit. Never written to the lock.
   Matcher per-tool: structural exact match against the
   `grant_match` JSON-Schema instance the user supplied at
   `/grant` time, or a "any invocation" wildcard if the
   manifest declared no `grant_match` schema.

6. **Slash commands** (`/grant`, `/grants list`, `/revoke`) —
   the TUI's input parser routes any line beginning with `/`
   to a slash-command handler instead of publishing as a
   `frontend.tui.user_message`. The handler synthesises an
   internal call into `user_grants` (no bus round-trip) and
   echoes the outcome as a `core.session.entry.finalized`
   `text` entry.

7. **TUI confirmation modal**. When the TUI subscribes to
   `core.session.confirm_request` and observes a held
   request, it displays a modal entry kind
   (`confirm_request` — see §RC for the new RenderNode variant)
   that blocks the input line until answered. Keys:
   `y` / `a` → allow, `n` / `d` → deny, `s` → always_allow_session
   (clears on restart), `Esc` → deny. Answer is published as
   `frontend.tui.confirm_answer`.

8. **Install-time trifecta refusal**. m1 ships
   `trifecta::evaluate` (`crates/rafaello-core/src/trifecta.rs`)
   which already returns `refuse: bool` honouring the
   `entry.flags.i_know_what_im_doing` flag. m5a turns this on
   at the install path: the `rfl install` command (a new
   bin target) refuses to write a lock entry when
   `evaluate(&lock, &canonical, &ctx).refuse == true` unless
   `--i-know-what-im-doing` is passed; the same flag is
   surfaced loudly in `rfl status` (also new in m5a — the
   bin target gains a `status` subcommand).

9. **Per-plugin outstanding tool_request map** (closes m4
   §5.1 / pi-3 M-2 / m4 §"Out of scope" carryover). The gate
   needs to know which `request_id`s on `core.session.tool_request`
   are currently held; the broker reuses the same map to
   reject `plugin.<id>.tool_result.in_reply_to` citations for
   ids the broker never routed to *that* plugin (security
   RFC §7.2.6 row 1's "must reference the matching
   tool_request previously routed to this plugin" check).
   Lands as broker state managed by the gate module.

10. **Lock-side `check_lock_publish_topic` unknown-namespace
    tightening** (closes m4 §2.6 / m3 §2.7). m1's lock
    validator currently accepts publish topics whose top-level
    segment is not `core` / `provider.<x>` / `plugin.<x>` /
    `frontend.<x>`; the broker rejects them at runtime. m5a
    adds the compile-time check; the wire surface m5a
    introduces (`core.session.confirm_*`,
    `frontend.tui.confirm_answer`) is the natural moment to
    tighten because the install-time trifecta refusal also
    runs in the same pass. Co-located in `validate/`.

11. **Audit-log SQLite table** (`audit_events`) under the
    existing `${PROJECT_ROOT}/.rafaello/state/` SQLite
    database from m3. Records: `id`, `seq`, `at`, `kind`
    (`confirm_request` / `confirm_reply` / `grant_added` /
    `grant_revoked` / `trifecta_overridden` /
    `install_refused`), `payload` JSON, `request_id`. The
    audit subscriber subscribes to
    `core.session.confirm_request` /
    `core.session.confirm_reply` and to slash-command
    internal hooks; writes are append-only.

12. **Integration tests** under `rafaello-core/tests/`,
    `rafaello-openai/tests/`, and `rafaello/tests/` covering
    the §"Demo bar" matrix.

### m5a → m5b boundary

m5a enforces:

- the **structural rule** of `decisions.md` row 9: every
  sink-declaring tool_request lacking a matching `user_grants`
  entry triggers a confirmation prompt, fail-closed on
  60 s timeout;
- the **always_confirm** flag (manifest opt-in, security RFC
  §15.1 #3) for non-sink tools that nonetheless want a prompt;
- the **install-time trifecta refusal** with override flag;
- the per-plugin outstanding-tool_request map (closes m4
  §5.1) and lock-side namespace check (closes m4 §2.6).

m5a does **not** implement:

- **taint matching** — the gate decides "fire prompt or not"
  purely from the target's `sinks` and `user_grants`; it
  does **not** consult the inbound `taint` envelope to
  decide. The envelope is forwarded into the
  `confirm_request.details` payload verbatim from the
  `core.session.tool_request` event so the modal can
  *display* it, but the prompt fires independent of taint
  per `decisions.md` row 9. Taint matching against recent
  result payloads (security RFC §7.2.1–§7.2.2) is m5b.
- **plugin-supplied taint superset enforcement** — m4 already
  discards inbound plugin-supplied taint at the broker
  boundary (§B7 step 8 in m4 scope); m5a does not add the
  superset rule that would reject plugin-supplied taint
  whose `in_reply_to` referent set has a smaller union.
  m5b adds it.
- **the verbatim-tool-result-to-sink negative** from the
  roadmap's fourth negative bullet — without taint
  propagation the prompt cannot *display* the verbatim
  status, and asserting "the gate fired" would be
  redundant with m5a's other gate-fired tests. m5b lands
  the negative as an end-to-end exfil demo.
- **provider-extracted user_grants proposals** (security
  RFC §7.2.4 item 3) — both `rfl-openai` and the gate
  ignore the proposal channel; the only `user_grants`
  populators in m5a are `/grant` and the
  `always_allow_session` answer. m6/v2 territory.

The split mirrors m4 → m5: m4 shipped the envelope so m5
could land matching/propagation atop a stable shape; m5a
ships the gate so m5b can land taint-influenced prompt
wording atop a stable gate.

---

## Inputs

### From the plans tree

- `rafaello/plans/overview.md`:
  - §4.2 (topic grammar — `core.session.confirm_*` and
    `frontend.tui.confirm_answer` are grammar-valid);
  - §4.3 (four namespaces — confirm topics live under `core.*`
    and `frontend.tui.*`);
  - §4.5 (bus event envelopes — `request_id` mandatory on
    confirm_* per the m4 row-43 pattern; `in_reply_to`
    mandatory on `confirm_answer` per row-43 + security RFC
    §7.2.6 row 5);
  - §6.1 (trifecta — m5a wires `trifecta::evaluate` into
    install);
  - §6.2 (the canonical sink rule — m5a's gate enforces this
    verbatim);
  - §6.3 (sink classes; m1's `sinks::infer_defaults` already
    implements the conservative defaults table);
  - §6.4 (`user_grants` semantics — populators 1 and 2 in
    m5a; populator 3 is m5b/m6 territory);
  - §6.6 (confirmation protocol — three topics, core-mediated,
    fail-closed);
  - §7 (tool dispatch — m5a inserts the gate between
    `core.session.tool_request` re-emission and the
    `plugin.<topic-id>.tool_request` dispatch publish);
  - §8.1 (the bundled `rfl-openai` plugin — bundled, not
    built-in; spawn/identity/taint/sink-confirm identical to
    any other plugin; install-configurable endpoint).

- `rafaello/plans/decisions.md`:
  - row 9 (sink confirmation rule, taint-independent — the
    canonical m5a rule);
  - row 10 (user-only taint is provenance, not authorisation —
    `user_grants` is the only bypass);
  - row 11 (one-hop trifecta direct, not transitive — m5a's
    install refusal honours this; the roadmap negative
    "transitive flows are NOT caught" is m5a's third
    install-refusal negative);
  - row 12 (carve-outs by decomposition — m5a does not change
    the carve-out path; touched only as far as the
    install-refusal commit lives in `validate/`);
  - row 13 (`RFL_BUS_FD` — `rfl-openai` is spawned through the
    same supervisor as any plugin, no new fd primitive);
  - row 17 (capability scoped bundles — `sinks::effective_grant`
    already unions `default` ∪ `<tool-name>`);
  - row 26 (helper plugins deferred — `rfl-openai` does not use
    helpers);
  - row 27 (external attach deferred — TUI is the only frontend
    in v1; `frontend.tui.confirm_answer` is the only confirm
    publisher);
  - row 28 (streaming patch ops deferred — the openai plugin
    waits for a complete response and emits one
    `assistant_message` per turn; SSE is not parsed);
  - row 29 (subprocess renderers deferred — the
    `confirm_request` modal is a built-in renderer);
  - row 38 (`rfl-openai` plugin identity, OpenAI Chat
    Completions wire protocol, install-configurable endpoint;
    refines row 21);
  - row 42 (`Publisher` reshape — the gate's internal
    subscriber observes `Publisher::Plugin` /
    `Publisher::Provider` / `Publisher::Frontend` arms
    uniformly);
  - row 43 (`request_id` mandatory on correlation-bearing
    topics — m5a extends the suffix list to include
    `.confirm_request`, `.confirm_reply`, `.confirm_answer`);
  - row 45 (`load = "eager"` is the live string shorthand —
    `rfl-openai`'s manifest uses `load = "eager"`).

- `rafaello/plans/glossary.md` — load-bearing terms used
  verbatim: *Confirmation protocol*, *Sink*, *Sink
  confirmation*, *User grant (session)*, *Trifecta refusal*,
  *Provider plugin*, *Audit log* (added by m5a — see §AL).

- `rafaello/plans/streams/a-security/rfc-security-model.md`:
  - §5 (namespaces) — confirm topics live in `core.*` and
    `frontend.tui.*`;
  - §5.6 (confirmation protocol — wire-shape `request_id`,
    `what`, `summary`, `details`, `default`, `ttl_seconds`);
    payload field names lifted verbatim;
  - §7.1 + §7.1.1 (trifecta — graph scope is one-hop direct,
    not transitive; install-refusal text wins over the
    `--i-know-what-im-doing` override);
  - §7.2.3 (mandatory sink enforcement, the
    cross-tool fix);
  - §7.2.4 (user_grants — populators 1 and 2; populator 3
    deferred);
  - §7.2.5 (sinks declared in manifest — the table m1
    implements is the m5a-honoured shape);
  - §7.2.6 (mandatory `in_reply_to` table — m5a adds row 5
    `frontend.<id>.confirm_answer`);
  - §7.3 (carve-outs — touched only via the validation commit;
    no new carve-out class).

  **Stream A drift to be aware of (do not patch in this
  branch — m5a retro lands the patches per the
  `milestones/README.md` "Stream RFC drift" rule):**
  - §7.4.1 (helper plugins) — deferred per `decisions.md`
    row 26; m5a treats this as background.
  - §10 (v1 summary) — still describes the older "non-user
    taint AND declared sink" formulation; **`overview.md`
    §6.2 wins per the `plans/README.md` workflow rule**, and
    `decisions.md` row 9 is the canonical text. Banner-only
    patch deferred to m5a retro (m4 retro §2.1 carryover for
    the same banner mechanism).

- `rafaello/plans/streams/f-manifest/rfc-manifest-schema.md`:
  - §3, §6 (load/runtime/rpc fields — the live m1 schema
    omits `runtime` and `[rpc]` per `decisions.md` rows 30,
    31; m5a does not touch this);
  - **overview.md §15.1 wins** for the `[provides]` shape:
    `provides.tool.<n>.sinks`, `grant_match`, `always_confirm`
    are the live m1 names (m1 retro §2.1; manifest fields
    are spelled exactly as in §15.1's normative delta).

- `rafaello/plans/streams/e-renderer/rfc-renderer-model.md`:
  - §4 (RenderTree variants) — m5a adds `Confirm` as a new
    variant (built-in only; subprocess renderers are deferred
    per `decisions.md` row 29). Render kind name is
    `confirm_request`, payload shape pinned in §RC below.

### From prior milestones (live state)

- `rafaello/plans/milestones/m4-provider-agent-loop/scope.md`
  §"Out of scope" — the deferral list is m5a's inheritance
  baseline. Items routed to m5a:
  - sink classes (§Si below — most of the schema work is
    already in m1; m5a only adds the consumer);
  - confirmation protocol + UI (§CT, §CG, §TUI);
  - `user_grants`, slash commands (§UG, §SL);
  - **broker-side stale-correlation map on
    `plugin.<id>.tool_result.in_reply_to`** (m4 pi-3 M-2 — the
    gate's outstanding-request map (§OM) doubles as the
    stale-id rejector);
  - `rfl-openai` provider plugin code (§OP);
  - `always_confirm = true` enforcement (§CG step 3);
  - audit-log table (§AL).

- `rafaello/plans/milestones/m4-provider-agent-loop/scope.md`
  §"m4 → m5 boundary" — pins the contract m5a inherits:
  taint envelope present, structurally validated,
  core-supplied origin. m5a does not re-validate the
  envelope; it consumes it via the gate's pass-through and
  m5b's matching layer.

- `rafaello/plans/milestones/m4-provider-agent-loop/retrospective.md`:
  - §2.6 — m1 `check_lock_publish_topic` unknown-namespace
    gap re-filed for m5+. m5a closes it (§M1 below).
  - §5.1 — stale-correlation enforcement on
    `plugin.<id>.tool_result.in_reply_to`; gate's
    outstanding map is the natural reader (§OM).
  - §5.5 production `#[allow(dead_code)]` sites:
    `bus.rs:101` (`ProviderConn.peer`) and
    `supervisor.rs:176` (`SpawnRegistration::Provider`);
    m4 retro names m5's confirmation gate as the natural
    reader. m5a consumes both fields (§CG, §OP).
  - §5.5 production `#[allow(clippy::result_large_err)]` on
    `reemit/mod.rs` and `agent/mod.rs` — m5a may collapse
    these into a workspace boxing convention if the gate's
    error type forces the same shape (the gate's error
    arms are at least as wide as reemit's). Not a hard
    deliverable; tracked as a §"Risks" item.

### Live source baseline (m4-as-shipped)

- `crates/rafaello-core/src/manifest/provides.rs:30-40` — the
  manifest `ToolMeta { sinks, grant_match, always_confirm }`
  shape m5a consumes.
- `crates/rafaello-core/src/lock/bindings.rs:22-37` — the lock
  projection of the same shape.
- `crates/rafaello-core/src/sinks.rs` — `infer_defaults` and
  `effective_grant` already implement the row-9-aligned
  conservative defaults table per §"In scope" item 2.
- `crates/rafaello-core/src/trifecta.rs` — `evaluate` already
  computes the four-tuple including `refuse`; m5a's install
  path consumes the boolean.
- `crates/rafaello-core/src/compile.rs:204` /
  `:440-463` — `tool_meta` projection from lock to
  `CompiledPlugin` (consumed by the gate; no new compiler
  work needed for the routing, only for the gate-side
  consumer).
- `crates/rafaello-core/src/agent/mod.rs:143-217` —
  `handle_tool_request` currently calls
  `broker.publish_for_tool_dispatch(...)` directly. m5a
  inserts the gate between the agent loop's
  `core.session.tool_request` observation and the dispatch
  publish (the call to `publish_for_tool_dispatch` moves
  *behind* the gate's pass-through path; the agent loop
  itself stops driving dispatch directly).
- `crates/rafaello-core/src/bus.rs:590` —
  `Broker::subscribe_internal` is the m4 primitive m5a's
  gate uses to observe `core.session.tool_request` without
  requiring an external bus round-trip.
- `crates/rafaello-core/src/broker_acl.rs:71-78` —
  `FrontendAcl.publish_topics` is the set m5a extends with
  `frontend.tui.confirm_answer`.
- `crates/rafaello/src/lib.rs:308-315` — the `BrokerAcl`
  construction site m5a touches to add the new ACL entry.
- `crates/rafaello-tui/src/lib.rs` — the TUI's input handler
  is the slash-command parser site (§SL).

---

## In scope

Grouped by area; each bullet is intended to be commit-shaped
(commit-row work for `commits.md` happens in the next phase).

### W — workspace dependencies

- **W1.** New crate `crates/rafaello-openai` with
  `Cargo.toml` declaring `rafaello-core`, `fittings`,
  `tokio`, `reqwest = { workspace = true }` (new workspace
  dep), `serde`, `serde_json`, `ulid`. Bin target
  `rfl-openai`. Library target carries the wire-protocol
  client + the bus-side adapter so it can be exercised in
  isolation. Add `reqwest` to the workspace `[dependencies]`
  table.
- **W2.** New crate `crates/rafaello-openai-stub` (gated
  behind workspace feature `test-fixture`, like m4's
  `rafaello-bus-fixture`). Bin target `rfl-openai-stub`
  serves a deterministic `/v1/chat/completions` response
  on a localhost port chosen by the test harness; reads
  request bodies and asserts wire-shape; emits a single
  recorded response.
- **W3.** Workspace fixture lock under
  `rafaello/fixtures/m5a-locks/` containing two manifests:
  `rfl-openai` (active provider; bindings
  `provider = true`, `provider_id = "openai"`,
  `network.mode = "proxy"`,
  `allow_hosts = ["127.0.0.1"]` for the stub /
  `allow_hosts = ["litellm.thepromisedlan.club"]` for the
  manual-validation lock,
  `env.pass = ["RFL_OPENAI_API_KEY"]`) and a sink-declaring
  tool plugin `rafaello-mailcat` (new — §TP below).
- **W4.** Documentation-only Cargo metadata: the workspace
  README (if any) gains a one-line "m5a adds rfl-openai".

### Si — sink-class consumption

m1 already plumbed everything through to
`CompiledPlugin.tool_meta`. m5a only adds the consumer.

- **Si1.** Add `CompiledPlugin::tool_sinks(name: &str) ->
  Option<&[String]>` and `CompiledPlugin::tool_always_confirm(name:
  &str) -> bool` accessors so the gate doesn't reach into
  `tool_meta` directly. Used by §CG step 2.
- **Si2.** Add a sink-class enum
  `SinkClass { Network, VcsPush, Mail, WorkspaceWrite }`
  and a non-exhaustive `Other(String)` arm in
  `crates/rafaello-core/src/sinks.rs`. The four enumerated
  classes match the roadmap row verbatim. The `Other`
  arm covers `exec` and any plugin-author-supplied custom
  string (security RFC §7.2.5). Existing `infer_defaults`
  return type changes from `Vec<String>` to `Vec<SinkClass>`;
  call sites updated.
- **Si3.** A negative test in `rafaello-core/tests/`:
  `tool_meta_with_sinks_drives_gate_decision.rs` — assert
  that a `CompiledPlugin` with
  `tool_meta["mailcat"].sinks = ["mail"]` returns
  `[SinkClass::Mail]` from the new accessor.

### Tr — install-time trifecta refusal

- **Tr1.** New bin target `rfl install` (or extension to
  the existing `rfl` bin if it has one — m5a will create
  the `rfl install` subcommand if absent).
  `rfl install <plugin-source>` materialises a candidate
  lock entry (out-of-scope details are m1's install path —
  m5a touches only the refusal step) and calls
  `trifecta::evaluate` against the candidate lock; if
  `refuse == true`, the install errors out with a typed
  `InstallError::TrifectaRefused { canonical, reads,
  outbound, write }` message that prints the three booleans
  and the override flag.
- **Tr2.** `--i-know-what-im-doing` flag on `rfl install`
  sets `entry.flags.i_know_what_im_doing = true` in the
  candidate lock entry; subsequent `evaluate` passes
  return `refuse == false`.
- **Tr3.** New bin subcommand `rfl status` prints the lock
  contents and **flags any plugin with
  `flags.i_know_what_im_doing == true` in red ANSI**
  (security RFC §7.1 mandates loud surfacing). For
  non-TTY output the same plugins are prefixed with
  `[OVERRIDE]`.
- **Tr4.** Tests in
  `rafaello-core/tests/`:
  - `install_refuses_trifecta_plugin.rs` — install a
    fixture manifest declaring all three of
    `network.mode = "allow_all"`,
    `read_dirs = ["/"]` (outside `${PROJECT_ROOT}`),
    `write_dirs = ["${project}"]`. Assert install
    errors with `TrifectaRefused`.
  - `install_accepts_trifecta_plugin_with_override.rs` —
    same manifest + `--i-know-what-im-doing`; assert
    install succeeds and the lock entry's
    `flags.i_know_what_im_doing == true`.
  - `install_refuses_one_hop_outbound_via_other_plugin.rs`
    — install plugin A whose grant only writes the
    workspace and reads outside the project, but which
    *publishes* on a topic plugin B (already-locked,
    network-open) subscribes to. Assert install errors;
    asserts the one-hop direct check is exercised
    (security RFC §7.1.1).
  - `install_does_not_chase_transitive_outbound.rs` —
    same setup but plugin A→B→C where only C is
    network-open and B does not subscribe to A's
    publish; assert install **accepts** plugin A (the
    transitive flow is the explicit out-of-scope case
    per `decisions.md` row 11). This is the third
    roadmap negative.

### CT — confirmation topics + frontend ACL extension

- **CT1.** Three new topic constants in
  `crates/rafaello-core/src/bus.rs` (or a new
  `topics.rs` module if pi argues for hoisting):
  - `core.session.confirm_request`
  - `core.session.confirm_reply`
  - `frontend.tui.confirm_answer`
- **CT2.** Extend the
  `request_id`-mandatory topic-suffix list (m4 §B0
  table-of-truth, decisions row 43) to include
  `.confirm_request`, `.confirm_reply`, `.confirm_answer`.
  Broker rejects missing `request_id` with the existing
  `MissingRequestId` variant. Per-suffix tests:
  - `broker_publish_core_session_confirm_request_missing_request_id_rejected.rs`
  - `broker_publish_frontend_tui_confirm_answer_missing_request_id_rejected.rs`
- **CT3.** Extend the `in_reply_to`-mandatory rule
  (security RFC §7.2.6 row 5) to
  `frontend.tui.confirm_answer`. Broker rejects with
  `InvalidInReplyTo { reason: Missing }`. Test:
  `broker_publish_frontend_tui_confirm_answer_missing_in_reply_to_rejected.rs`.
- **CT4.** Frontend ACL extension. In
  `crates/rafaello/src/lib.rs:308-315`, add
  `frontend.tui.confirm_answer` alongside the existing
  `frontend.tui.user_message`. Test:
  `frontend_publish_confirm_answer_accepted_by_broker.rs`
  (the grant-only positive, mirrors m4 §F4's pattern).
- **CT5.** Re-emit pipeline (m4's `crates/rafaello-core/src/reemit/mod.rs`)
  gains a fourth arm: `frontend.tui.confirm_answer` inbound
  is canonicalised to `core.session.confirm_reply` after
  validation (the validated answer matches a held
  `confirm_request.request_id`); taint envelope is
  `[{source: "user"}]` per security RFC §7.2.2. Tests:
  - `reemit_frontend_confirm_answer_to_core_session_confirm_reply.rs`
  - `reemit_confirm_answer_unknown_request_id_rejected.rs`
  - `reemit_confirm_answer_synthesises_user_taint.rs`

### CG — confirmation gate

- **CG1.** New module `crates/rafaello-core/src/gate/mod.rs`.
  Public type `ConfirmationGate { broker, acl, controller,
  user_grants, audit, outstanding }`. Constructed by
  `rfl chat` after the broker but before the agent loop;
  spawned as a tokio task that subscribes internally to
  `core.session.tool_request` and `core.session.confirm_reply`.
- **CG2.** Decision logic on each `core.session.tool_request`:
  1. Resolve `dispatch_target` from the event payload
     (m4 already populates this); look up the
     `CompiledPlugin` for that canonical id.
  2. Compute `gate_required = !sinks.is_empty() ||
     always_confirm` via the §Si1 accessors.
  3. If `!gate_required`, pass through (publish
     `plugin.<topic-id>.tool_request` via the existing
     `Broker::publish_for_tool_dispatch` call) and audit
     a `passthrough` event.
  4. If `gate_required`, look up `user_grants` for an
     entry matching `(tool_name, args)`; if matched,
     pass through and audit a `grant_matched` event.
  5. Otherwise, hold the tool_request in
     `outstanding[request_id] = HeldRequest { event,
     deadline }`, build a `ConfirmRequestPayload`
     (§CG3), publish on
     `core.session.confirm_request`, and start a 60 s
     deadline timer.
- **CG3.** `ConfirmRequestPayload` shape (security RFC
  §5.6, payload-shape names lifted verbatim):
  ```json
  {
    "request_id": "<ulid>",
    "what": "tool_call",
    "summary": "<tool> on <plugin> — sinks: [<class>, ...]",
    "details": {
      "tool": "<tool>",
      "args": {...},
      "sinks": ["mail", ...],
      "always_confirm": false,
      "taint": [...]    // forwarded verbatim from m4 envelope
    },
    "default": "deny",
    "ttl_seconds": 60
  }
  ```
  `request_id` is a fresh `ulid` generated by the gate
  (the held tool_request's id lives in `details.tool_call_id`
  for correlation back; the gate's outstanding map keys on
  the gate's own id, mapped through `held.id ↔ confirm.id`).
- **CG4.** On `core.session.confirm_reply` arrival:
  - if `answer == "allow"`, retrieve the held request,
    publish `plugin.<topic-id>.tool_request`, audit
    `confirm_allowed`;
  - if `answer == "deny"`, synthesise a
    `core.session.tool_result` with payload
    `{ok: false, error: "user_denied"}` (the agent loop's
    existing `handle_tool_result` persists it as a
    tool_result entry), audit `confirm_denied`;
  - if `answer == "always_allow_session"`, add a
    `user_grants` entry and treat as `allow`.
- **CG5.** 60 s timeout: a tokio interval task scans the
  outstanding map; on deadline, publishes a synthetic
  `core.session.confirm_reply` with `answer = "deny"` (the
  same path as CG4's deny arm); audit `confirm_timeout`.
- **CG6.** Agent-loop change. The current
  `crates/rafaello-core/src/agent/mod.rs:143` direct call
  to `broker.publish_for_tool_dispatch` is **removed**;
  the agent loop only persists the `tool_call` entry and
  observes the canonical `core.session.tool_request`. The
  gate is now the sole driver of the dispatch publish.
  This is a small but architectural shift; called out
  separately for the commit-row plan.
- **CG7.** Tests in `rafaello-core/tests/`:
  - `gate_passes_through_non_sink_tool_request.rs`
  - `gate_passes_through_user_grant_match.rs`
  - `gate_holds_sink_tool_request_pending_confirm.rs`
  - `gate_dispatches_on_allow.rs`
  - `gate_synthesises_tool_result_on_deny.rs`
  - `gate_times_out_to_deny_after_60s.rs` (uses tokio
    paused time per m3's pattern)
  - `gate_always_confirm_true_holds_non_sink_tool.rs`
  - `gate_always_allow_session_persists_then_clears.rs`
    (the second invocation matches; a fresh `ConfirmationGate`
    constructed afterward — simulating restart — re-prompts).

### OM — outstanding tool_request map (broker-side)

- **OM1.** New broker state
  `BrokerState::outstanding_dispatched: BTreeMap<CanonicalId,
  BTreeMap<JsonRpcId, OutstandingDispatch>>` keyed by
  target plugin canonical id then by request_id. Populated
  by `publish_for_tool_dispatch` (a tool_request is
  routed to plugin X with id N → record (X, N)). Drained
  by the gate on `core.session.tool_result` observation
  for that pair.
- **OM2.** `plugin.<id>.tool_result.in_reply_to` validation
  in the broker's existing `handle_plugin_publish`: the
  cited id must appear in
  `outstanding_dispatched[that_plugin]`. Reject otherwise
  with `BrokerError::StaleRequestId` (m4 already has the
  variant). Closes m4 §5.1 / pi-3 M-2.
- **OM3.** Tests:
  - `broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`
  - `broker_plugin_tool_result_in_reply_to_routed_to_other_plugin_rejected.rs`
    — id N was dispatched to plugin A; plugin B citing N
    fails closed.
  - `broker_outstanding_drained_on_tool_result_observed.rs` —
    after the gate observes the matching tool_result,
    a *second* tool_result citing the same id from the
    same plugin fails (no double-fire).

### UG — user_grants

- **UG1.** New module
  `crates/rafaello-core/src/user_grants.rs`. Type
  `UserGrants { entries: Vec<UserGrant> }` plus
  `UserGrant { tool: String, matcher: GrantMatcher,
  added_at: DateTime<Utc>, source: GrantSource }`.
- **UG2.** `GrantMatcher` enum with arms `Any`,
  `Structural { args_subset: serde_json::Value }`. The
  structural matcher checks that every key in
  `args_subset` is present in the candidate `args` with
  an equal value (recursive on objects; arrays compared by
  elementwise equality). The matcher is intentionally
  conservative: the manifest's optional `grant_match`
  JSON-Schema is not yet used to validate the matcher
  shape in m5a — see §"Architectural choices to ratify".
- **UG3.** `GrantSource` enum `SlashCommand`,
  `AlwaysAllowSession`. (`ProviderProposal` is reserved
  but not constructed in m5a; m5b/m6 territory.)
- **UG4.** API:
  - `UserGrants::add(grant: UserGrant) -> GrantId`
  - `UserGrants::list(&self) -> Vec<(GrantId, &UserGrant)>`
  - `UserGrants::revoke(id: GrantId) -> Result<(), RevokeError>`
  - `UserGrants::matches(tool: &str, args: &Value) ->
    Option<GrantId>`
- **UG5.** Tests:
  - `user_grants_any_matcher_matches_every_invocation.rs`
  - `user_grants_structural_matcher_subset_match.rs`
  - `user_grants_structural_matcher_value_mismatch.rs`
  - `user_grants_revoke_removes_entry.rs`
  - `user_grants_revoke_unknown_id_errors.rs`

### SL — slash commands

- **SL1.** TUI input parser change. Lines beginning with
  `/` are not published as `frontend.tui.user_message`;
  instead they are parsed by a new
  `SlashCommand::parse(input: &str) ->
  Result<SlashCommand, ParseError>` and handed to a
  dispatcher that mutates `UserGrants` directly (no bus
  round-trip — slash commands are TUI-local). The
  outcome is rendered as a `core.session.entry.finalized`
  text entry (informational) so the user sees what
  happened.
- **SL2.** Three slash commands:
  - `/grant <tool> [<key>=<value>]...` — adds a
    `UserGrant { tool, matcher: Structural { args_subset: {key:
    value, ...} } }` (or `Any` if no kv pairs given);
    echoes "granted: <tool> (<grant-id>)".
  - `/grants list` — prints `(<grant-id>) <tool>
    matcher=<repr> source=<source>` per entry; if empty,
    "no grants".
  - `/revoke <grant-id>` — removes the entry; errors
    inline if id is unknown.
- **SL3.** Unknown slash commands are echoed as
  "unknown command: <input>" with the same text-entry
  mechanism. Slash commands themselves are not persisted
  to the SQLite session store as user messages (they're
  not user_messages — they're TUI-local actions); the
  audit log captures grant additions/revocations
  separately (§AL).
- **SL4.** Tests:
  - `tui_slash_grant_adds_user_grant.rs`
  - `tui_slash_grant_with_args_creates_structural_matcher.rs`
  - `tui_slash_grants_list_prints_entries.rs`
  - `tui_slash_revoke_by_id.rs`
  - `tui_slash_unknown_command_renders_error.rs`
  - `tui_user_message_starting_with_slash_not_published.rs`
    — input `/foo` does not generate a
    `frontend.tui.user_message` publish.

### TUI — confirmation modal

- **TUI1.** New input mode in `rafaello-tui`:
  `InputMode::ConfirmModal { request_id, summary,
  details }`. Entered when the TUI's subscriber observes
  `core.session.confirm_request`; exited on
  `frontend.tui.confirm_answer` publish. While in this
  mode, the input line is non-editable and key events
  drive the answer:
  - `y` / `a` / `Enter` → answer `"allow"`
  - `n` / `d` / `Esc` → answer `"deny"`
  - `s` → answer `"always_allow_session"`
- **TUI2.** New RenderNode variant `Confirm { summary,
  details, default, ttl_remaining }`. Rendered in a
  framed modal area above the input line, with a
  ttl countdown updated each second from a tokio
  interval. The detail panel renders `details.taint` as
  a list of `(source, detail)` rows so the operator
  sees provenance when m5b lands the propagation; in
  m5a the list is empty for non-tainted sink calls.
- **TUI3.** When the gate's reply / timeout arrives,
  the TUI exits the modal and re-enables the input
  line. The agent-loop-persisted `tool_call` entry is
  updated to `status: Allowed | Denied | TimedOut`
  via the existing entry `update` path (m3). Status
  rendering: `Pending` shows "running…"; `Denied`
  shows "denied by user"; `TimedOut` shows "no answer
  in 60s".
- **TUI4.** Tests in `rafaello-tui/tests/`:
  - `tui_enters_confirm_modal_on_confirm_request.rs`
  - `tui_y_key_publishes_allow_answer.rs`
  - `tui_n_key_publishes_deny_answer.rs`
  - `tui_s_key_publishes_always_allow_session.rs`
  - `tui_input_blocked_during_confirm_modal.rs`
  - `tui_modal_exits_on_confirm_reply.rs`

### RC — `Confirm` render kind

- **RC1.** `RenderNode::Confirm` added to the renderer
  ADT. The variant carries `summary: String`,
  `details: serde_json::Value`,
  `default: ConfirmDefault`, `ttl_remaining: Duration`.
  Server-side downgrade rule: any frontend whose
  `Capabilities::renderer_kinds` lacks `confirm_request`
  receives a `Callout { kind: warn, child: KeyValue
  { ... } }` — the existing m3 fallback path.
- **RC2.** TUI capability: extend
  `Capabilities::tui_default()` (per m3 retro §2.3) to
  include `confirm_request` and the `Confirm` render
  variant.
- **RC3.** Tests in `rafaello-core/tests/`:
  - `renderer_confirm_kind_renders_for_tui.rs`
  - `renderer_confirm_kind_falls_back_for_minimal_caps.rs`

### OP — `rfl-openai` provider plugin

- **OP1.** Wire-protocol client in
  `crates/rafaello-openai/src/wire.rs`:
  - `ChatCompletionRequest { model, messages, tools? }`
  - `ChatCompletionResponse { id, choices: [{message:
    {role, content?, tool_calls?}}] }`
  - HTTP POST via `reqwest::Client`; the endpoint URL
    is read from the env var `RFL_OPENAI_ENDPOINT_URL`
    (passed in by the supervisor — see §OP4); the API
    key from `RFL_OPENAI_API_KEY` (the lock's
    `env.pass` projects an arbitrary host env var name
    onto this canonical name; see §OP5 — this avoids
    every deployment-specific name like
    `LITELLM_API_KEY` or `OPENAI_API_KEY` leaking into
    the plugin's source).
  - One-shot request/response only — no SSE streaming
    (decisions row 28). The plugin awaits the full
    response body, then publishes one
    `provider.openai.assistant_message` (or one
    `provider.openai.tool_request` per
    tool_call in the response).
- **OP2.** Tool schema discovery: the plugin needs to
  forward the user's tool list to the model. m4's
  agent loop already maintains the
  `BrokerAcl.tool_routes` map (`broker_acl.rs:124-137`).
  m5a adds a new core-only event
  `core.session.tools_advertised` published once at
  `rfl chat` startup with payload
  `{tools: [{name, description, parameters_schema}]}`.
  The provider subscribes to this event and caches the
  list. (This is not in the roadmap but is a
  pragmatic requirement — a chat-completions provider
  cannot propose tool calls without knowing the schema.
  See §"Architectural choices to ratify".)
- **OP3.** Bus-side adapter: subscribes to
  `core.session.user_message` and
  `core.session.tool_result` per the m4 fixture pattern;
  publishes `provider.openai.tool_request` and
  `provider.openai.assistant_message` with mandatory
  `request_id` (fresh ULID per publish) and
  `in_reply_to` populated per security RFC §7.2.6 rows
  2 and 3 (tool_request cites prior tool_result ids;
  assistant_message cites the union of prior tool_result
  and user_message ids it has observed).
- **OP4.** Manifest in
  `rafaello/fixtures/m5a-locks/rafaello-openai/rafaello.toml`
  (and a copy under `crates/rafaello-openai/`):
  ```toml
  schema = 1
  name = "openai"
  version = "0.0.0"
  entry = "bin/rfl-openai"
  rafaello = ">=0.1, <0.2"
  load = "eager"

  [provides]
  provider = "openai"

  [bus]
  subscribes = [
    "core.session.user_message",
    "core.session.tool_result",
    "core.session.tools_advertised",
  ]
  publishes = [
    "provider.openai.tool_request",
    "provider.openai.assistant_message",
  ]

  [capabilities.default.filesystem]
  read_dirs = []
  write_dirs = []

  [capabilities.default.network]
  mode = "proxy"
  allow_hosts = ["litellm.thepromisedlan.club"]   # dev default; lock overrides per deployment
  ```
- **OP5.** Lock binding shape (the install-time
  configuration that carries the deployment-specific
  endpoint URL and env-var name):
  ```toml
  [plugin."builtin:openai@0.0.0".bindings]
  provider     = true
  provider_id  = "openai"

  [plugin."builtin:openai@0.0.0".grant]
  bundles.default.network.mode = "proxy"
  bundles.default.network.allow_hosts = ["litellm.thepromisedlan.club"]
  bundles.default.env.pass = ["LITELLM_API_KEY:RFL_OPENAI_API_KEY"]   # see OP6
  bundles.default.env.set = [
    "RFL_OPENAI_ENDPOINT_URL=https://litellm.thepromisedlan.club/v1",
    "RFL_OPENAI_MODEL=vllm/qwen3.6-27b",
  ]
  ```
  The `RFL_OPENAI_ENDPOINT_URL` and `RFL_OPENAI_MODEL`
  env vars are reserved env vars added to m1's
  `RESERVED_ENV_VARS` list (m1 retro extends to seven;
  m5a adds two more — see §M1.1 below).
- **OP6.** **`env.pass` rename mechanism (light).** The
  plugin's source reads `RFL_OPENAI_API_KEY` only — the
  canonical name. The lock's `env.pass` syntax extension
  `"<host-env>:<plugin-env>"` allows a deployment to
  inject the host's `LITELLM_API_KEY` into the plugin
  as `RFL_OPENAI_API_KEY`. The renaming is performed by
  the supervisor at spawn time. m1's scrubber's reserved
  list (currently rejects the canonical names) needs to
  permit the LHS-of-`:` form to *map onto* a reserved
  name — see §"Architectural choices to ratify": this
  is a small but real schema extension and pi may push
  back on landing it in m5a vs deferring to m6.
- **OP7.** Tests in `rafaello-openai/tests/`:
  - `openai_manifest_compiles.rs`
  - `openai_emits_assistant_message_for_user_message.rs`
    (against the stub server — §W2)
  - `openai_emits_tool_request_when_model_returns_tool_call.rs`
  - `openai_subscribes_to_tools_advertised.rs`
  - `openai_request_carries_tool_schemas.rs`
  - `openai_in_reply_to_populated_for_assistant_message.rs`
  - `openai_in_reply_to_populated_for_tool_request.rs`
  - `openai_endpoint_url_taken_from_env_var.rs`
  - `openai_api_key_taken_from_canonical_env_var.rs`
  - `openai_handles_tool_call_followed_by_assistant_message.rs`
    (multi-turn — model proposes a tool call, observes
    a tool_result, emits a final assistant message)

### TP — `rafaello-mailcat` sink-declaring tool fixture

- **TP1.** New crate `crates/rafaello-mailcat` with bin
  target `rfl-mailcat`. Declares
  `[provides.tools] = ["send-mail"]` and
  `[provides.tool.send-mail] sinks = ["mail"]
  always_confirm = false`. Subscribes to its own
  `plugin.<topic-id>.tool_request`; publishes
  `plugin.<topic-id>.tool_result`. Behaviour: appends
  the request payload to a file named
  `mailcat.log` under the plugin's private state dir
  (the per-plugin private state dir is auto-granted —
  `decisions.md` row 16/37). No actual SMTP. The
  sink class is honest because the plugin's behaviour
  *would* be irreversible if it talked to a real SMTP
  server; for m5a the on-disk log lets the integration
  tests assert the call did or did not happen, without
  network access.
- **TP2.** Optional `[provides.tool.send-mail].grant_match`
  schema referencing
  `crates/rafaello-mailcat/schemas/send-mail-grant.json`:
  ```json
  {
    "type": "object",
    "properties": { "to": {"type": "string"} },
    "required": ["to"]
  }
  ```
  Used by the §UG2 structural matcher to validate
  `/grant send-mail to=alice@example.com`. The schema
  is read at install time (m1 already validates
  presence per scope §M11); m5a does not run JSON-Schema
  validation against grant-supplied args — see §"Out of
  scope" item 5.
- **TP3.** Tests:
  - `mailcat_appends_to_log_on_tool_request.rs`
  - `mailcat_returns_error_on_missing_to_field.rs`
  - `mailcat_manifest_declares_mail_sink.rs`

### AL — audit log

- **AL1.** New SQLite table `audit_events`:
  ```sql
  CREATE TABLE audit_events (
      seq          INTEGER PRIMARY KEY AUTOINCREMENT,
      at           TEXT NOT NULL,           -- ISO-8601
      kind         TEXT NOT NULL,           -- 'confirm_request' | 'confirm_reply' | 'grant_added' | 'grant_revoked' | 'trifecta_overridden' | 'install_refused' | 'gate_passthrough' | 'gate_grant_match'
      request_id   TEXT,                    -- nullable for slash-command events
      payload      TEXT NOT NULL            -- JSON
  );
  ```
- **AL2.** `AuditWriter` consumer wired into the gate
  (writes confirm_request / confirm_reply / passthrough
  / grant_match / timeout) and into `UserGrants`
  (writes grant_added / grant_revoked) and into the
  install path (writes install_refused, trifecta_overridden).
- **AL3.** No bus-side `audit.*` topic; the audit log is
  a **passive sink** read only via SQLite (a future
  `rfl audit` subcommand; not in m5a). Rationale: a bus
  topic would invite plugin subscribers and complicate
  the trust model.
- **AL4.** Tests:
  - `audit_records_confirm_request_event.rs`
  - `audit_records_confirm_reply_event.rs`
  - `audit_records_grant_addition_and_revocation.rs`
  - `audit_records_trifecta_override_at_install.rs`
  - `audit_records_install_refused.rs`
  - `audit_seq_monotonic_per_session.rs`

### M1 — m1 lock-side carryovers

- **M1.1.** Extend m1's `RESERVED_ENV_VARS` (currently
  six per `decisions.md` row 40) to nine —
  `RFL_OPENAI_ENDPOINT_URL`, `RFL_OPENAI_API_KEY`,
  `RFL_OPENAI_MODEL`. Per row 40's pattern, scrubber-level
  rejection at compile/V3 time when present in `env.set`
  raw form (without the §OP6 `<host>:<canonical>` rename
  syntax). The mapped form (`X:RFL_OPENAI_API_KEY`)
  is the documented exception.
- **M1.2.** Lock-side `check_lock_publish_topic`
  unknown-namespace tightening. m1's
  `validate/mod.rs` currently accepts any grammatically
  valid topic in `entry.grant.publishes`; the broker
  rejects unknown namespaces at runtime. m5a adds the
  compile-time check: top-level segment must be `core`,
  `provider`, `plugin`, or `frontend`; deeper-segment
  shape (`provider.<id>.x`, `plugin.<topic-id>.x`,
  `frontend.<id>.x`) follows the existing
  `PublishOnReservedNamespace` /
  `PublishOnFrontendNamespace` /
  `ProviderNamespaceMismatch` rules. New variant
  `PublishUnknownNamespace { topic, top: String }`
  on `m1::ValidateError`. Tests:
  - `lock_validate_publish_unknown_namespace_rejected.rs`
  - `lock_validate_publish_evil_top_segment_rejected.rs`
  - `lock_validate_publish_known_namespaces_accepted.rs`

### CHAT — `rfl chat` orchestration extension

- **CHAT1.** `crates/rafaello/src/lib.rs:run_chat` is
  extended to:
  - construct a `UserGrants` instance (empty);
  - construct an `AuditWriter` against the SQLite path;
  - construct the `ConfirmationGate` and spawn its task;
  - after broker construction, publish the
    `core.session.tools_advertised` event with the
    compiled tool routing table (§OP2).
- **CHAT2.** The four-level orchestration tree from m4
  (`rfl chat` → `rfl-tui` + `rfl-mockprovider` +
  `rfl-readfile`) becomes a *five-tree* in m5a:
  `rfl chat` → `rfl-tui` + `rfl-openai` + `rfl-mailcat`
  (+ `rfl-readfile` and `rfl-mockprovider` retained
  as installed-but-not-active alternatives in the same
  fixture lock for the negatives). Every plugin spawned
  through the existing `PluginSupervisor`. Risk inventory
  (§"Risks") has the leak-mitigation items.
- **CHAT3.** TUI test-mode env hooks (m4's
  `RFL_TUI_TEST_MESSAGE` extended for m5a):
  - `RFL_TUI_TEST_CONFIRM_ANSWER` — `"allow"` / `"deny"`
    / `"always_allow_session"` / `"timeout"` / unset
    (manual). When set, the TUI auto-publishes the
    answer on the next `confirm_request` it observes,
    after a configurable delay
    (`RFL_TUI_TEST_CONFIRM_DELAY_MS`, default 0).
  - `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE` — JSON
    `{"tool": "send-mail", "args_subset": {...}}` —
    auto-issues `/grant ...` before sending the test
    user_message.

### I — integration tests

The §"Demo bar" matrix below is the contract.

Test placement:
- `rafaello-core/tests/` — broker, gate, audit, m1
  validate extension.
- `rafaello-tui/tests/` — modal + slash commands.
- `rafaello-openai/tests/` — provider plugin.
- `rafaello-mailcat/tests/` — sink fixture.
- `rafaello/tests/` — `rfl chat` end-to-end + `rfl
  install` + `rfl status`.

The matrix below enumerates the m5a-introduced files;
m4-carryover positives/negatives are not re-listed (they
continue to pass via the unchanged broker / agent-loop
paths).

---

## Demo bar

The roadmap row's positive + four negatives map as
follows. m5a covers positive + three negatives; the
fourth (verbatim flow blocked at the broker) is m5b.

### Positive (roadmap verbatim)

> Real model call through the configured OpenAI-compatible
> endpoint; model proposes a sink-declaring tool;
> confirmation prompt fires; user accepts → tool runs;
> user denies → tool refused.

Headline integration test:
**`rafaello/tests/rfl_chat_demo_bar_send_mail.rs`** —
spawn `rfl chat` against the m5a fixture lock with
`rfl-openai` (active) + `rfl-mailcat` installed. The CI
fixture points the openai plugin at `rfl-openai-stub`'s
recorded response (a chat completion that proposes a
`send-mail` tool call with `args.to =
"alice@example.com"`). Drive the TUI's
`frontend.tui.user_message` publish via
`RFL_TUI_TEST_MESSAGE="please email alice"`. The test
runs twice with different
`RFL_TUI_TEST_CONFIRM_ANSWER`:

- **`allow` arm:** assert SQLite `entries` table
  contains, in seq order, `text` (user), `tool_call`
  (status `Allowed`), `tool_result` (ok), `text`
  (assistant); the mailcat plugin's `mailcat.log`
  contains one entry; the audit log records a
  `confirm_request` and a `confirm_reply{answer:allow}`.
- **`deny` arm:** assert `entries` contains `text`
  (user), `tool_call` (status `Denied`),
  `tool_result` (`{ok: false, error: "user_denied"}`),
  `text` (assistant — the model's response to the
  denial); mailcat.log is empty; audit log records
  `confirm_reply{answer:deny}`.

### Negative 1 — confirmation timeout denies

`rafaello/tests/rfl_chat_demo_bar_send_mail_timeout.rs`
— same setup but `RFL_TUI_TEST_CONFIRM_ANSWER=timeout`
(the TUI does not publish an answer at all). The test
uses tokio paused time advanced past 60 s. Assert: the
gate publishes a synthetic `core.session.confirm_reply`
with `answer = "deny"`; the entries / mailcat / audit
state matches the deny arm above; the audit log records
a `confirm_timeout` event.

### Negative 2 — `always_allow_session` clears on `rfl chat` restart

`rafaello/tests/rfl_chat_always_allow_session_clears_on_restart.rs`
— first invocation with
`RFL_TUI_TEST_CONFIRM_ANSWER=always_allow_session`;
assert mailcat.log gains one entry. Second invocation
in the same tempdir (same SQLite, same lock) with the
same user message but `RFL_TUI_TEST_CONFIRM_ANSWER`
**unset**; the test injects a deny via
`RFL_TUI_TEST_CONFIRM_DELAY_MS=10` +
`RFL_TUI_TEST_CONFIRM_ANSWER=deny`. Assert: the second
run **prompts again** (the modal entry appears in the
second session's entries) and the deny holds (mailcat.log
unchanged from the first run).

### Negative 3 — install-time trifecta refusal (one-hop, not transitive)

Two tests:
- `rafaello/tests/rfl_install_refuses_one_hop_trifecta.rs`
  — install a fixture plugin that satisfies all three
  trifecta dimensions; assert install errors with
  `TrifectaRefused` and the error names the three
  booleans.
- `rafaello/tests/rfl_install_does_not_chase_transitive_outbound.rs`
  — install plugin A that publishes on a topic B
  subscribes to; B does not have outbound itself, but
  C (subscribing to B's publishes) does. Assert
  install of A **succeeds** because the trifecta
  graph check is one-hop direct only (`decisions.md`
  row 11). Audit log records the install acceptance.

### Negative 4 — verbatim tool-result-to-sink flow blocked at the broker

**Deferred to m5b (Appendix A).** Without taint
propagation, m5a cannot show the verbatim status in
the prompt; an m5a-only "the gate fired" assertion
would be redundant with negatives 1–3.

### Bonus negatives implied by the security RFC / m4 retro

- `rafaello/tests/rfl_chat_always_confirm_true_holds_non_sink_tool.rs`
  — a fixture tool with `sinks = []` and
  `always_confirm = true`. Assert the gate fires the
  prompt even though no sinks are declared.
- `rafaello/tests/rfl_install_status_shows_red_for_override.rs`
  — install a trifecta plugin with
  `--i-know-what-im-doing`; assert `rfl status`
  prints the entry with the red ANSI marker.
- `rafaello-core/tests/broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`
  — closes m4 §5.1 / pi-3 M-2.

---

## Out of scope

The following are explicitly NOT in m5a and not allowed to
sneak in via implementation drift:

1. **Taint matching against recently-emitted tool_result
   payloads** (security RFC §7.2.1–§7.2.2) — m5b. The gate
   is taint-independent in m5a per `decisions.md` row 9.
2. **Plugin-supplied taint superset enforcement on
   re-emission** (security RFC §7.2.6 superset rule) — m5b.
3. **Verbatim tool-result-to-sink exfil demo** — m5b
   (negative 4 above).
4. **Provider-extracted user_grants proposals** (security
   RFC §7.2.4 item 3) — deferred to m6 / v2. The
   `GrantSource::ProviderProposal` arm is reserved but
   never constructed in m5a.
5. **JSON-Schema validation of grant matchers against the
   manifest's `grant_match` schema** — m5a uses the
   structural-subset matcher only (§UG2). The `grant_match`
   schema is parsed and checked for *presence* at install
   time (m1 already does this) but is not used to validate
   `/grant` argument values. Deferred because pulling in a
   JSON-Schema validator (jsonschema or similar) is a
   workspace-dependency expansion that should sit in its
   own milestone debate. Filed as a §"Architectural choices
   to ratify" item.
6. **Multiple active providers, `rfl provider use <id>`,
   provider hot-swap** — deferred to post-v1 (m4 §"Out of
   scope" carryover; overview §8).
7. **Streaming SSE responses from `rfl-openai`** — `decisions.md`
   row 28 (streaming entry patch ops deferred to v2). The
   plugin awaits the full chat-completion response, then
   emits one `assistant_message` per turn.
8. **Helper plugins (`bindings.helper_for`,
   `RFL_HELPER_FD`)** — `decisions.md` row 26 (deferred
   to v2). `rfl-openai` does not use helpers; the
   `[provides] helpers = []` line is implicit.
9. **External UDS-attached frontends, `rfl serve`** —
   `decisions.md` rows 27, 34. The TUI is the only
   frontend principal; `frontend.tui.confirm_answer` is
   the only confirm publisher in m5a.
10. **Subprocess plugin renderers** — `decisions.md` row 29.
    The new `Confirm` render kind is built-in.
11. **Multi-session daemon, attach-multiplexing, branching
    sessions** — post-v1.
12. **Lazy-load / lazy-spawn-on-publish** — out of scope per
    m4 §"Out of scope" carryover. m5a continues to
    eager-spawn every installed plugin via `rfl chat`'s
    orchestration.
13. **`rfl audit` subcommand** — read access to the
    `audit_events` table is via direct SQLite. A CLI
    surface for browsing audit events is m6 polish.
14. **`rfl init`** — materialising the lock with deployment
    defaults is **m6** (per the driver pre-flight). m5a
    ships a hand-written fixture lock for tests + a
    documented manual-validation lock; the user-facing
    "first run" UX is m6's territory.
15. **Audit-log GC / retention policy** — append-only in
    m5a; rotation / size limits are post-v1.
16. **Confirmation answers for tools other than tool_call**
    — security RFC §5.6 lists `tool_call`, `grant_change`,
    `plugin_load`. m5a only fires `what: "tool_call"`
    confirm_requests; `grant_change` and `plugin_load`
    confirmations are out of scope (the only grant-mutation
    surface in m5a is the slash command, which is local to
    the TUI and does not cross the bus). m6 / v2 may add
    `grant_change` confirmations if the install flow grows
    bus-mediated approvals.
17. **macOS interactive smoke testing** — m4 dev work is
    Linux; macOS verified through CI only. macOS CI green
    remains a hard ratification gate (m3 / m4 precedent).
18. **`exec` sink class enforcement** — the
    `SinkClass::Other("exec")` arm is constructible (any
    string can become `Other`) but no fixture in m5a
    declares it; the gate treats it identically to any
    other declared sink (fires the prompt). v1's only
    enumerated classes are `network`, `vcs_push`, `mail`,
    `workspace_write`.
19. **Broker-mediated plugin → plugin confirmation
    requests** — the only confirmation publisher in m5a
    is the gate (core internal). A plugin that wanted to
    request user confirmation for its own internal action
    would have to publish a `plugin.<id>.confirm_request`
    that core re-emits — that path is post-m5b.
20. **OpenAI structured-tool-call argument schema
    validation in the provider** — `rfl-openai` forwards
    whatever the model returns; argument-shape validation
    happens at the tool plugin (`rfl-mailcat` rejects
    missing `to`). m5a does not add a JSON-Schema validator
    in the provider.

Each deferral has an associated decisions.md row (rows 9,
11, 26, 27, 28, 29, 33, 34, 38) or roadmap row pointer
(post-v1) or scope-§-pointer to where the deferred work
will land.

---

## Architectural choices to ratify

Surfaced for pi review and owner sign-off; m5a draft makes a
choice for each but the choices are reversible at scope-round
cost.

### A1. `user_grants` matcher: structural subset vs. JSON-Schema

m5a's draft choice (§UG2): structural subset (`Any` |
`Structural { args_subset }`). The manifest's
`grant_match` JSON-Schema is parsed but unused for runtime
matching.

**Trade-off.** Structural subset is tiny and deterministic;
JSON-Schema is the shape the manifest already carries
(it would honour the manifest author's intent). JSON-Schema
adds a workspace dependency and a new failure mode (invalid
matcher schema → which UI?). Recommendation: ship structural
in m5a; let m6 add JSON-Schema validation if the
deployment surface demands it.

### A2. `core.session.tools_advertised` topic — new in m5a

m5a's draft choice (§OP2): publish a new core-only event
once at `rfl chat` startup carrying the tool list. The
provider subscribes and caches.

**Trade-off.** The roadmap doesn't mention this topic, and
the architecture as-shipped in m4 does not need it (the
mock provider hardcodes its tool name). But a real
chat-completions provider needs to forward tool schemas to
the model. Alternatives:
- ad-hoc fittings RPC `core.tools_list` (request/response,
  not a bus event) — adds an RPC method to core's surface;
- bus-event-on-each-publish (heavier);
- compile-time bake (fragile when tools change).

Recommendation: ship the topic. It is small, additive, and
respects the "bus is the truth" model. Owner may want to
weigh in on naming (`core.session.tools_advertised` vs
`core.tools.advertised`); the former matches m4's
`core.session.*` clustering.

### A3. `env.pass` rename syntax `"<host>:<canonical>"`

m5a's draft choice (§OP6): introduce a small env-var
rename syntax in the lock so deployment-specific names
(`LITELLM_API_KEY`) can be projected onto the plugin's
canonical name (`RFL_OPENAI_API_KEY`).

**Trade-off.** The plugin's source becomes
deployment-agnostic (decisions row 38's intent). But
the syntax is a m1 schema extension; pi may push back on
landing schema work in m5a vs. preferring the plugin's
source to read whatever the host env var is named.
Alternative: each deployment ships a fork of `rfl-openai`
with a different `env.pass`. Rejected — that defeats
"bundled plugin" status.

Recommendation: ship the syntax in m5a as part of M1.1.

### A4. Confirm-modal render kind: built-in vs streaming-aware

m5a's draft choice (§RC): a new built-in `RenderNode::Confirm`
variant with TTL countdown driven by the TUI's tokio
interval, not by streaming entry patches.

**Trade-off.** Streaming entry patches are deferred
(`decisions.md` row 28), so the alternative — a `confirm_request`
entry that is *patched* every second with a fresh TTL
remaining — isn't on the table for v1. The TTL is computed
client-side from the `ttl_seconds` payload + a wall-clock
delta. Acceptable.

### A5. Slash commands — flat parser, not palette UI

m5a's draft choice (§SL): slash commands are flat string
prefixes (`/grant`, `/grants list`, `/revoke`) parsed by
the TUI's input handler, not a palette / autocomplete UI.

The driver pre-flight already pinned this. No alternatives
on the table.

### A6. Audit log: passive SQLite sink only

m5a's draft choice (§AL): the audit log is written to
SQLite by core; no `audit.*` bus topic. Read access in
m5a is via raw SQLite.

**Trade-off.** A bus topic invites plugin subscribers
(security boundary issue). A read-side CLI (`rfl audit`)
could happen in m6. Acceptable.

### A7. `rfl-openai` streaming vs final-only

m5a's draft choice (§OP1): final-only chat completion
response (one HTTP POST, await full body, emit one
`assistant_message`).

`decisions.md` row 28 mandates this for v1 generally
(streaming entry patch ops are v2). A reader might
argue that the *provider* could stream from the endpoint
and still emit a final `assistant_message` to the bus
(internally buffer, externally final-only) — that's
useful when network latency is dominant. Recommendation:
final-only on both sides for m5a; m5b/m6 may revisit if
LiteLLM's tail-latency on `qwen3.6-27b` becomes
embarrassing.

### A8. CI strategy for `rfl-openai`: stub vs recorded fixtures

m5a's draft choice (§W2): a tiny `rfl-openai-stub` bin
that serves a deterministic chat-completion response on a
localhost port chosen by the test harness.

**Trade-off.** Recorded fixtures (e.g. JSON files with
canned responses replayed by a test-only HTTP server)
are an alternative; they're more readable. The stub bin
is more flexible (serves multiple responses; can model
streaming or rate limiting). Pi may push back on shipping
yet-another-bin for the test harness; the alternative is
to fold the stub into `rafaello-bus-fixture`-style helper
inside the openai test suite. Recommendation: keep as a
separate crate behind the `test-fixture` feature, mirroring
m4's `rafaello-bus-fixture` pattern.

### A9. `always_confirm = true` gating: strict vs loose

m5a's draft choice (§CG step 3): `always_confirm = true`
forces a confirm prompt even when `sinks = []`,
**identical** to a sink call's gate. Same `user_grants`
bypass applies.

This matches `overview.md` §15.1 #3 verbatim. Pi may push
back: a non-sink tool with `always_confirm = true` makes
its `grant_match` more semantically loaded ("does the user
want to bypass review"). Acceptable: same matcher rules
apply.

### A10. `rfl-openai`'s name in the lock — `builtin:openai@0.0.0`

m5a's draft choice (§OP5): the bundled plugin's canonical
id uses the `builtin:` source prefix. Other sources are
`github:`, `local:`, etc. (m1 territory).

This is a m1 compile/parse choice; m5a is the first place
a `builtin:` id appears on the wire, so pi may want to
ratify it. Alternative: `bundled:openai@0.0.0`.
Recommendation: `builtin:` matches the binary's location
("built into the rafaello release tree").

---

## Risks

1. **Five-tree orchestration leak surface.** m4 already
   manages a four-tree (`rfl chat` → `rfl-tui` +
   `rfl-mockprovider` + `rfl-readfile`); m5a adds
   `rfl-openai` and `rfl-mailcat`. Mitigation: extend
   m2's `RFL_FIXTURE_MAX_LIFETIME` self-timeout pattern
   into the two new fixtures (m4 retro §5.4 says the
   pattern held in m4); extend m4's SIGCHLD-style cleanup
   to cover all five children; reuse the deterministic
   test_done signal pattern.

2. **`reqwest` is a heavy workspace dep.** Pulls in a
   large transitive set (rustls, hyper, tokio-tls).
   Alternative: hand-rolled hyper client. Mitigation:
   accept the dep — `reqwest` is the de facto OpenAI-Chat
   client choice; the alternative is more code that we
   own. Pi-feedback risk: pi may push back on the dep
   weight; willing to discuss.

3. **The CI stub server requires an unused localhost
   port.** Reuse m4's `rafaello-bus-fixture`-style port
   selection (bind to `127.0.0.1:0`, read assigned port,
   pass to `rfl-openai` via the `RFL_OPENAI_ENDPOINT_URL`
   env var injected at test setup). No platform-specific
   syscalls.

4. **Manual-validation against the LiteLLM proxy
   requires `LITELLM_API_KEY`.** CI will not have this.
   The headline `rfl_chat_demo_bar_send_mail.rs` test
   uses the stub. Manual-validation runs the same test
   shape against the real proxy and is documented in
   `manual-validation.md` per the m4 pattern.

5. **`always_allow_session` correctness across `rfl chat`
   restart.** The whole point of the second roadmap
   negative is that `user_grants` clears on restart; if
   the test harness or a session-store quirk persists
   the answer somehow, the test silently passes when it
   should fail. Mitigation: the negative asserts both
   the modal-appeared signal *and* the mailcat unchanged;
   double-trigger reduces false-positive risk.

6. **`tools_advertised` event's idempotence.** If the
   provider re-subscribes (e.g. on reconnect after a
   hypothetical fittings drop), it expects the topic to
   have been re-broadcast. m5a's broker does not have
   replay-on-subscribe (decisions row 41 covers replay
   only on `core.session.entry.finalized`). Mitigation:
   `rfl-openai` caches the list at first observation;
   the broker publishes once at startup; reconnect is
   not a v1 path because plugins never reconnect within
   a session. If they crash, the supervisor terminates
   the session.

7. **Slash-command parsing collides with future user
   intent.** A user typing `/path/to/file` as part of a
   real conversation triggers `unknown command`.
   Mitigation: documented in §SL3; the user sees a
   clear "unknown command" message and can re-type with
   a leading space. Acceptable for v1; a richer parser
   (e.g. require `/` *and* a known verb) is m6.

8. **`Confirm` render kind interactions with the entry
   store.** Confirm modals are *not* persisted as
   entries (they are transient UX); only the resulting
   `tool_call` entry status update is persisted. The
   `core.session.confirm_request` event is a transient
   bus event; it does not pass through the agent loop's
   entry-persistence path. Mitigation: m5a's agent loop
   change in §CG6 is the explicit pivot; a test
   (`agent_loop_does_not_persist_confirm_request_event.rs`)
   asserts no `entries` row appears for a
   `confirm_request` event.

9. **Audit-log writes contend with session-store
   writes.** Both share the SQLite database. m3's
   session store uses connection-per-task with WAL.
   m5a's audit writer reuses the same connection pool.
   No new locking contracts; risk is bounded.

10. **m1 schema extension for `env.pass` rename
    syntax (§OP6 / A3).** Touching m1 schema in m5a is
    mild scope-creep. Mitigation: the change is purely
    additive (the existing `["FOO", "BAR"]` form
    continues to work; only the `"FOO:BAR"` form is
    new). If pi pushes back, fall back to per-deployment
    plugin forks — explicitly rejected above but
    available as a fallback.

11. **The `result_large_err` clippy carryover from m4
    §5.5.** `gate/mod.rs` and the audit writer's error
    types are likely as wide as `reemit/mod.rs`. m5a may
    add two more `#[allow(clippy::result_large_err)]`
    sites unless we land a workspace boxing convention.
    Mitigation: defer the boxing convention; document
    each new allow with a one-line rationale; track for
    a future cleanup pass (same disposition as m4).

12. **macOS CI gate carries forward.** m5a introduces no
    new platform-specific syscalls (`reqwest` rustls
    works on both). Default expectation: macOS CI green
    from day one. Push to CI as the W1/W2 commits land
    (m2 §5.7 push-to-CI-early lesson).

13. **`rfl install` is a new bin subcommand.** m4 ran on
    a hand-written fixture lock; m5a is the first
    milestone where install actually does work
    (refusing trifecta plugins). The install path
    itself is large; m5a only owns the *refusal step*
    — the materialise-from-source path is m1's existing
    `Lock::from_toml` plus a new "install candidate"
    helper. Risk: I underestimate the materialise
    plumbing. Mitigation: in commits.md drafting,
    bound the install command's surface to "given a
    candidate `PluginEntry` constructed by hand or by
    a m1 helper, refuse if trifecta refuses; otherwise
    write to lock". The full network-fetch /
    digest-verify install path is m6 territory.

14. **Stream A drift carryover patches.** §10 banner
    fix and any `confirm_*` schema additions to Stream A's
    body land as anticipated retro drift, **not in this
    branch**. Pi may catch a missing patch; m5a retro is
    the natural place.

---

## Manual validation

The companion `manual-validation.md` (Phase 3) records:

1. **Real-network demo.** Run `rfl chat` against the
   dev LiteLLM proxy with a fixture lock pointing
   `RFL_OPENAI_ENDPOINT_URL` at the proxy and
   `RFL_OPENAI_API_KEY` projected from `LITELLM_API_KEY`.
   Type "please email alice@example.com that I'll be
   late" — the model proposes `send-mail`; the modal
   fires; the operator presses `y`; the mailcat plugin's
   on-disk log gains an entry. Same flow with `n` shows
   the deny path.
2. **Slash-command demo.** Within the same session,
   type `/grant send-mail to=alice@example.com`; verify
   `/grants list` shows the entry; re-issue the same
   user message; confirm no modal fires this time
   (the gate matches the grant); type `/revoke <id>`;
   re-issue; confirm modal fires again.
3. **Trifecta refusal demo.** Run `rfl install` against
   a fixture manifest declaring all three trifecta
   dimensions; observe the typed error. Re-run with
   `--i-know-what-im-doing`; observe install succeeds;
   run `rfl status` and confirm the red ANSI marker.
4. **macOS CI green** capture (run URL recorded in
   `manual-validation.md` §4 per the m4 pattern).
5. **TUI keyboard interaction.** A short interactive
   walk asserting every documented key (y / a / Enter
   / n / d / Esc / s) drives the expected answer.
6. **Audit-log inspection.** After a session, dump
   `audit_events` and assert the rows match the
   operator's actions.

CI cannot exercise (1) because `LITELLM_API_KEY` isn't
present; the headline integration test uses
`rfl-openai-stub` as substitute. (4) is captured by the
post-merge driver sweep, mirroring m4.

---

## Internal split (driver guidance for `commits.md`)

Suggested grouping; `commits.md` picks final granularity.
Pi review may reshape.

1. **Workspace + crate scaffolds + m1 reserved-env
   extension (W1-W4 + M1.1)** — ~3 commits. The
   `rafaello-openai`, `rafaello-openai-stub`,
   `rafaello-mailcat` crate skeletons land here separately
   from logic. `reqwest` workspace-dep addition is its
   own commit.
2. **m1 lock-side namespace tightening (M1.2)** —
   ~1 commit. Closes m4 §2.6.
3. **Sink-class consumer (Si1-Si3) + per-plugin
   outstanding map (OM1-OM3)** — ~2 commits. Both add
   small data structures + tests against the existing
   broker.
4. **Confirmation topics + frontend ACL extension
   (CT1-CT5)** — ~2-3 commits. The
   `request_id`-mandatory list extension is grouped with
   the topic constants; the frontend ACL extension is
   its own commit; the re-emit pipeline arm lands
   alongside the canonical-emit logic.
5. **Render kind + TUI capability (RC1-RC3)** —
   ~1 commit.
6. **Confirmation gate (CG1-CG7)** — ~3-4 commits. The
   gate's decision logic is the largest single module;
   passthrough vs hold vs reply paths each merit their
   own commit. The agent-loop pivot (CG6) is its own
   commit because it removes the m4 dispatch path; pi
   will want this commit isolated.
7. **`user_grants` (UG1-UG5)** — ~2 commits. The
   matcher and the API surface each merit a commit.
8. **Slash commands (SL1-SL4)** — ~2 commits. Parser +
   dispatcher; tests bundled.
9. **TUI confirmation modal (TUI1-TUI4)** — ~2-3 commits.
   Input mode + key handling + tests.
10. **Audit log (AL1-AL4)** — ~2 commits. Schema migration
    + writer.
11. **Install-time trifecta refusal (Tr1-Tr4)** —
    ~2-3 commits. The `rfl install` subcommand,
    `rfl status` red marker, and the four tests. The
    transitive-not-chased test is its own commit because
    it asserts a deliberate non-feature.
12. **`rafaello-mailcat` fixture (TP1-TP3)** —
    ~2 commits.
13. **`rfl-openai` provider plugin (OP1-OP7)** —
    ~4-5 commits. Wire client, bus adapter, tools
    advertised consumer, integration tests; the
    multi-turn and tool-call-detection tests are
    bundled.
14. **`rfl chat` orchestration extension (CHAT1-CHAT3)** —
    ~2 commits. Wiring + test-mode env hooks.
15. **Demo-bar headline + manual validation** —
    ~2 commits. The two `rfl_chat_demo_bar_send_mail*`
    tests + `manual-validation.md` skeleton.

Forced-monolithic commits called out explicitly:

- **Step 4 + step 5 may need to land as a single
  workspace cutover** if `BusEvent` constructor sites are
  forced to acquire a new field for confirmation-topic
  validation. Round-1 cut: keep separate; pi may merge.
- The agent-loop pivot (CG6) is a m0-c08-class API change
  inside the agent loop; it is the *only* place where
  m4 behaviour changes shape in m5a. Commit body must
  call this out.

Realistic total: **~28-36 commits sequential**. m4 took
28 plan-row commits at comparable surface (broker +
provider + agent loop + two fixtures + orchestration);
m5a's surface is similar (gate + ACL + provider + two
fixtures + slash commands + modal + audit + install).
Pi round budget: **plan for 6-8 scope rounds** (m4 took
6; m3 took 22 but that included round-counting for
m3's outsized rendering surface). No m5a-i / m5a-ii
split anticipated.

---

## Acceptance summary

m5a is done when:

- Every named test in §"Demo bar" + §I matrices is
  implemented and passes. Tests may split or merge during
  `commits.md` drafting as long as the named behaviours
  are all covered.
- `nix develop --impure --command cargo test
  --manifest-path rafaello/Cargo.toml --workspace
  --features test-fixture` green on Linux inside the
  devshell.
- **macOS CI green is a hard ratification gate**
  (m3 / m4 precedent); the same `cargo test --workspace
  --features test-fixture` job on `macos-latest` must be
  green before retrospective ratification, with the only
  exception being tests explicitly gated
  `#[cfg(target_os = "linux")]`.
- `nix develop --impure --command cargo build
  --manifest-path rafaello/Cargo.toml --workspace --bins
  --features rafaello-core/test-fixture` green. Verifies
  `rfl`, `rfl-tui`, `rfl-mockprovider`, `rfl-readfile`,
  `rfl-openai`, `rfl-openai-stub`, `rfl-mailcat`, and
  `rfl-bus-fixture` all build.
- `nix develop --impure --command cargo doc
  --manifest-path rafaello/Cargo.toml --workspace
  --no-deps` warning-free.
- `manual-validation.md` records:
  - the interactive `rfl chat` allow + deny demo against
    the dev LiteLLM proxy (the operator-facing positive);
  - the slash-command demo (grant → silent invocation →
    revoke → modal fires);
  - the install-trifecta refusal demo;
  - the macOS CI URL.
- `retrospective.md` written with anticipated drift items
  addressed:
  - **Stream A §10 v1-summary banner patch** —
    `decisions.md` row 9 wins (`overview.md` §6.2 already
    says so); m5a lands a banner-only patch to the
    security RFC §10. Already deferred by
    `milestones/README.md` §"Stream RFC drift".
  - **Stream A §5.6 confirm-payload schema additions** —
    Stream A's confirm payload schema already matches
    the m5a wire shape (security RFC §5.6); banner may
    add a wire-shape pointer to `crates/rafaello-core/src/gate/`
    for navigability (mirrors m4's pointer additions).
  - **`decisions.md` row for the `Confirm` render kind**
    — new ratified row documenting the kind name and
    payload shape.
  - **`decisions.md` row for the `audit_events` table** —
    optional; recording-only would suffice.
  - **`decisions.md` row for the `env.pass` rename
    syntax** if §A3 lands as drafted — required, because
    it extends m1's schema in a forward-compatible way.
  - **`overview.md` §4.6 reserved env-vars table** —
    add `RFL_OPENAI_ENDPOINT_URL`, `RFL_OPENAI_API_KEY`,
    `RFL_OPENAI_MODEL`.
  - **`glossary.md`** — add an `Audit log` entry
    (table-passive, append-only) and adjust the
    `Confirmation protocol` entry to point at m5a's
    `gate/` module.
  - **m4 §5.5 `bus.rs:101` and `supervisor.rs:176`
    `#[allow(dead_code)]` removed** — m5a's gate
    consumes both fields; allows can drop in the
    commit that introduces the read-side.
- No follow-up Stream RFC drift is owed by m5a beyond the
  items above. m5a does NOT modify Stream A's body in
  this branch (banner-only, m1 / m3 / m4 precedent).

m5a ships the **first sink-confirmation gate**: a real
model proposes a sink-declaring tool call, core holds it
behind a TUI modal, the user answers, and the audit log
records the round-trip. Every later piece of v1's
security story (m5b's taint propagation, m6's polish)
inherits this gate's wire surface unchanged.

---

## Appendix A — m5b scope sketch (~1 page)

> This appendix is **not** the m5b scope.md. It is the
> carve-out plan so pi and the owner can see the split's
> shape before ratifying m5a. The actual m5b scope.md
> drafts after m5a closes (Phase 3 retrospective).

### A.1 Goal

Land **taint-aware confirmation prompts** and the
**verbatim tool-result-to-sink exfil demo** that closes
the roadmap row's fourth negative. m5a's gate fires
identically; m5b makes the modal *informative* about
provenance and adds the structural superset enforcement
that prevents plugin-supplied taint stripping.

### A.2 Sub-deliverables

1. **Taint propagation** (`crates/rafaello-core/src/reemit/taint_match.rs`)
   — when core re-emits `core.session.tool_request`, match
   each arg value against a per-session map of recently
   emitted `core.session.tool_result` payload values
   (literal hash + substring containment per security RFC
   §7.2.1). Matches union their taint into the canonical
   envelope. Map keyed by `(session_id, value_hash)` →
   `Vec<TaintEntry>`; refreshed on each tool_result with
   a TTL (default 5 minutes — choice to ratify).
2. **Plugin-supplied taint validation via `in_reply_to`
   superset rule** (broker side) — when a plugin publishes
   `plugin.<id>.tool_result` with a non-empty `taint`,
   the broker verifies the published `taint` is a
   **superset** of the union of taints from every event
   referenced in `in_reply_to`. The published taint is
   discarded at re-emission boundary (m4 already does this);
   the *check* is m5b's addition. Reject with new
   `BrokerError::TaintSupersetViolated` variant.
3. **Broker superset enforcement on re-emission** —
   every `provider.<id>.*` and `frontend.tui.*` re-emit
   already strips inbound taint and synthesises canonical
   from publisher identity (m4). m5b adds: when the
   re-emit pipeline observes `in_reply_to`, the
   synthesised envelope must be a superset of the
   referenced events' taints. Tests for the superset path.
4. **Confirmation prompt's `details.taint` populated
   from the canonical envelope.** m5a already forwards
   the field but the field is empty when no provenance
   exists. m5b's matching populates it; the TUI modal's
   render of `details.taint` becomes informative.
5. **Verbatim exfil demo**:
   `rafaello/tests/rfl_chat_demo_bar_verbatim_exfil_blocked.rs`
   — a `rafaello-fetch` fixture tool returns
   `{content: "https://evil.example.com/leak"}`; the
   model proposes `web-fetch {url:
   "https://evil.example.com/leak"}` verbatim; the gate's
   prompt `details.taint` shows `[{source: "tool",
   detail: "<rafaello-fetch canonical>"}]`; the TUI's
   automated answer (via `RFL_TUI_TEST_CONFIRM_ANSWER=deny`)
   blocks the call. Asserts: tainted prompt fires, mailcat
   /web-fetch log empty.
6. **Lock-side bindings: a third sink-declaring tool
   fixture** (`rafaello-fetch` with
   `sinks = ["network"]`) under
   `rafaello/fixtures/m5b-locks/`.

### A.3 m5a inheritance baseline

m5b inherits: gate (§CG), confirm topics (§CT), user_grants
(§UG), slash commands (§SL), TUI modal (§TUI), audit log
(§AL), `Confirm` render kind (§RC), `rfl-openai` plugin
(§OP), per-plugin outstanding map (§OM), install-time
trifecta refusal (§Tr).

### A.4 Estimated size

16-22 commits across:

- 6-9 commits for taint matching + superset enforcement
  + propagation;
- 3-4 commits for the verbatim exfil demo + the new
  fetch fixture;
- 3-4 commits for the TUI / audit-log enrichment of
  taint provenance;
- 2-3 commits for retro drift and Stream A patches
  (§7.2.1, §7.2.6 row 1's "must reference the matching
  tool_request previously routed to this plugin" — m5a
  closes the routed-to-this-plugin check via the
  outstanding map, but the superset half is m5b).

Pi round budget: 4-6 scope rounds (m4 was 6 for a wider
surface; m5b is narrower).

### A.5 m5b's `decisions.md` row candidates

- Taint matching algorithm — literal hash + substring
  containment (per security RFC §7.2.1); explicit
  non-coverage of laundered/transformed flows (CaMeL
  v2 territory).
- Plugin-supplied taint discard policy — m4 already
  established the canonical envelope is core-supplied;
  m5b's superset check adds an extra rejection signal.
- TTL on the per-session value→taint map (default
  proposed: 5 minutes; pi may want a smaller window).

### A.6 m5b → m6 boundary

m5b ships v1's full security story. m6 is polish:
`rfl init` materialising the lock, documentation pass,
Homebrew formula, `rfl audit` read CLI, and the
release-engineering work. No further security primitives.

---

*End of m5a scope round 1.*
