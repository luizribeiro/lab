# m5a — sinks + confirmation protocol + user_grants + rfl-openai — scope

> **Status:** round 2 — addresses `scope-pi-review-1.md`
> (7 blocking / 8 major / 6 nit). All 7 B and 8 M findings are
> resolved (mostly fold; M-6 pushed back with rationale). All
> 6 N folded. The split itself (M-1) is confirmed: the roadmap
> row in `milestones/README.md` pre-authorises *"May split into
> m5a (sinks + confirmation + user_grants) and m5b (taint
> matching + exfil tests) if scoping finds it too big."* The
> convergence-time owner ping covers the split decision; the
> §"Acceptance summary" now states explicitly that **m5a is not
> the full m5 roadmap row; m5b remains required before m5 is
> closed**.
>
> Round-2 fixes by pi-1 finding (one line each):
> - **B-1** Slash commands now publish typed
>   `frontend.tui.slash_command` events; core (the `UserGrants`
>   owner) is the sole mutator; outcome echoed via
>   `core.session.command_result`. §SL rewritten end-to-end.
> - **B-2** New §CT0 confirmation correlation table pinning
>   envelope vs. payload `request_id`, `in_reply_to`
>   cardinality, stale/duplicate/late behaviour, and the
>   single-canonical-id key for the held-confirmation map.
> - **B-3** Synthetic deny / timeout `core.session.tool_result`
>   shape pinned: fresh envelope `request_id`, `in_reply_to =
>   [held_tool_request.request_id]`, taint
>   `[{source: "system", detail: "user_denied"}]` /
>   `[{source: "system", detail: "confirm_timeout"}]`, payload
>   wire-shape matches `agent/mod.rs::handle_tool_result`'s
>   reader exactly. §CG4 / §CG5 rewritten with the canonical
>   builder helper named.
> - **B-4** `tools_advertised` replaced by a `core.tools_list`
>   fittings request/response method on core (the provider
>   pulls the schema after spawn-handshake completes); avoids
>   the broker-replay race entirely. §OP2 rewritten; §CHAT1
>   updated.
> - **B-5** Adopted pi's "simplest" env model per the round-2
>   prompt: `env.pass = ["LITELLM_API_KEY"]` verbatim (no
>   rename); plugin reads the API-key env-var name from
>   `RFL_OPENAI_API_KEY_ENV` (set via `env.set`). §A3
>   (rename-syntax extension) deleted; §OP5/§OP6 rewritten;
>   §M1.1 reduced to two new reserved names
>   (`RFL_OPENAI_ENDPOINT_URL`, `RFL_OPENAI_MODEL`); the
>   correct live count is **seven** post-m4 (per
>   `crates/rafaello-core/src/scrubber.rs:23-31`), m5a takes
>   it to nine. The lock TOML's `env.set` is now a TOML map
>   (`BTreeMap<String, String>`) per the live `GrantEnv`
>   shape, not the array-of-`KEY=VALUE` form the round-1 draft
>   wrote.
> - **B-6** `rfl install` bound to `rfl install --fixture
>   <dir>`: reads a local manifest + package directory,
>   computes existing digest pair, snapshots a candidate
>   `PluginEntry`, runs `validate::lock` and
>   `trifecta::evaluate`, writes `rafaello.lock`. Network
>   fetch / update / review-UI explicitly out of scope (m6).
>   §Tr fully rewritten.
> - **B-7** Outstanding-tool_request map split: the broker
>   owns `outstanding_dispatched` (populated by
>   `publish_for_tool_dispatch`, drained / checked atomically
>   in `handle_plugin_publish` for `plugin.<id>.tool_result`);
>   the gate owns the held-confirmations map (keyed by held
>   tool_request id). §OM rewritten. New duplicate-result test.
> - **M-1** Owner-judgment item resolved per the round-2
>   prompt's pre-authorisation reading; acceptance bullet added
>   ("m5a is not the full m5 roadmap row").
> - **M-2** §OP gains a wire-shape table (HTTP non-200
>   mapping, auth failure, retry/timeout, `model` resolution,
>   malformed-JSON / empty-`choices` / multiple-choices /
>   multiple-`tool_calls` / invalid-arg-JSON / unknown-tool /
>   final-content-with-tool-calls behaviour) plus four named
>   negative tests.
> - **M-3** §CG gains an explicit multi-pending policy: held
>   confirmations queue by held-id arrival; the TUI modal
>   serialises (one prompt visible at a time, next prompt
>   pops on close); concurrent `always_allow_session` /
>   matching grant arrival short-circuits any matching held
>   request. Tests for queue ordering, stale modal answers,
>   parallel matching grants, and timeout-during-active-modal.
> - **M-4** §RC and `RenderNode::Confirm` deleted entirely.
>   The TUI shows a transient overlay driven directly by the
>   `core.session.confirm_request` bus event; no entry is
>   persisted; no renderer-tree work. §TUI revised to
>   "TUI-internal overlay".
> - **M-5** Owner-judgment item resolved per the round-2
>   prompt's reading: the lock's
>   `bindings.tool_meta.<n>.grant_match` JSON-Schema is the
>   matcher source of truth, lock-pinned (manifest re-reads
>   are not honoured mid-session). m5a's `/grant` argument
>   shape is a **structural template** (key/value pairs) the
>   user supplies; core compiles it to a JSON-Schema-conformant
>   matcher object, validates *that object* against the
>   lock's `grant_match` schema at `/grant` time, and uses
>   structural-subset semantics at runtime against incoming
>   tool_request args. The JSON-Schema is therefore the
>   *shape contract* on the matcher template, not a runtime
>   validator on every tool call (which would cost a
>   per-call schema-compile). §UG2 rewritten.
> - **M-6** Pushed back. The dead-code-removal acceptance
>   bullet from round 1 is dropped; the m4 retro follow-up
>   stays open. The gate does not directly read
>   `RegisteredProvider.peer` (it publishes through
>   `Broker::publish_for_tool_dispatch`), and
>   `SpawnRegistration` is RAII-by-design. Manufacturing a
>   read-side just to satisfy the allow-removal would be
>   exactly the "fake work" pi flagged. See §"Acceptance
>   summary".
> - **M-7** Test matrix expanded with the named negatives pi
>   listed (confirm correlation, slash-command malformed
>   forms, stale modal answers, grant-vs-pending races).
> - **M-8** §Si2 rewritten: `Vec<String>` storage retained
>   throughout m1/m4; m5a adds `tool_sink_classes(name) ->
>   Vec<SinkClass>` parser-accessor only. No cross-crate
>   cutover.
> - **N-1** §OA reference fixed to §OP2.
> - **N-2** Spelled out `fittings-core`, `fittings-server`,
>   `fittings-client`, `fittings-transport` in §W1.
> - **N-3** "new bin target `rfl install`" → "new
>   `rfl install` subcommand on the existing `rfl` binary".
> - **N-4** Stray `(/)` removed.
> - **N-5** Reserved-env count: seven currently, m5a adds two
>   to nine.
> - **N-6** Negative 2 wording aligned: "no pre-existing
>   grant; automated TUI answers `deny` after 10ms".
>
> The roadmap row for m5 (`milestones/README.md`) is the
> pre-ratified definition; this document scopes **m5a in
> full** with m5b sketched in Appendix A.

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
   `provider.openai.assistant_message`. Endpoint URL, the
   API-key env-var name, and `network.allow_hosts` are
   install-time configuration in the lock — the dev environment
   uses `https://litellm.thepromisedlan.club/v1` with
   `LITELLM_API_KEY` per `plans/README.md` §"Tooling notes". For
   CI the plugin is pointed at a recorded fixture (a tiny
   `httpmock`-style stub bin shipped in
   `crates/rafaello-openai-stub` only when the `test-fixture`
   workspace feature is on) so the integration tests do not
   require network access. The plugin discovers tool schemas by
   calling the new core fittings RPC method `core.tools_list`
   after spawn handshake (§OP2 — request/response, not a bus
   event, to avoid the broker-replay race pi-1 B-4 caught).

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
   `crates/rafaello-core/src/user_grants.rs`). In-memory in
   **core** only (`Arc<RwLock<UserGrants>>` lives inside the
   `rfl chat` core process — pi-1 B-1 corrected the round-1
   draft's claim that the TUI mutates this directly; the TUI
   is a separate process per `overview.md` §3 and cannot share
   core heap state). Populated by:
   - a typed `frontend.tui.slash_command` bus event from the
     TUI (§SL);
   - the user answering `always_allow_session` on a
     confirmation prompt;
   - (deferred to v2 / m6 — provider-extracted proposals;
     security RFC §7.2.4 item 3).
   Cleared on `rfl chat` exit. Never written to the lock.
   Matcher: lock-pinned `bindings.tool_meta.<n>.grant_match`
   JSON-Schema is the *shape contract* on the matcher
   template; runtime matching is structural-subset against
   incoming tool_request `args`. Manifest changes mid-session
   are ignored (the lock is the source of truth — m1
   precedent). See §UG2 for the full semantics (revised in
   round 2 per pi-1 M-5).

6. **Slash commands** (`/grant`, `/grants list`, `/revoke`) —
   the TUI's input parser detects lines beginning with `/`
   and publishes a typed `frontend.tui.slash_command` bus
   event (new ACL grant) instead of
   `frontend.tui.user_message`. Core (which owns
   `UserGrants`) validates the command, mutates the table,
   and re-emits a `core.session.command_result` event the
   TUI renders inline as a transient text line (no
   `core.session.entry.finalized`; slash commands are not
   conversation history). Audit log records the grant
   addition / revocation. See §SL for full payload schemas
   and ACL deltas.

7. **TUI confirmation overlay** — a transient TUI-internal
   modal rendered when the TUI's bus subscriber observes a
   `core.session.confirm_request` event. **Not** a render
   kind, **not** a persisted entry (pi-1 M-4). Blocks the
   input line until answered. Keys: `y` / `a` → allow,
   `n` / `d` → deny, `s` → always_allow_session, `Esc` →
   deny. Answer is published as
   `frontend.tui.confirm_answer`.

8. **Install-time trifecta refusal**. m1 ships
   `trifecta::evaluate` (`crates/rafaello-core/src/trifecta.rs`)
   which already returns `refuse: bool` honouring the
   `entry.flags.i_know_what_im_doing` flag. m5a turns this on
   at the install path: a **new `rfl install --fixture <dir>`
   subcommand** on the existing `rfl` binary (pi-1 B-6 / N-3)
   reads a local manifest + package directory, computes the
   existing digest pair, snapshots a candidate `PluginEntry`,
   runs `validate::lock` and `trifecta::evaluate`, and writes
   `rafaello.lock`. Refuses when `evaluate(...).refuse ==
   true` unless `--i-know-what-im-doing` is passed; the flag
   is surfaced loudly in a new `rfl status` subcommand on the
   same binary. Network fetch / update / review-UI
   explicitly out of scope (m6 / `rfl init` territory). See
   §Tr.

9. **Broker-owned outstanding-dispatched map** (closes m4
   §5.1 / pi-3 M-2 / m4 §"Out of scope" carryover). The
   broker (not the gate — pi-1 B-7) maintains
   `BrokerState::outstanding_dispatched: BTreeMap<CanonicalId,
   BTreeMap<JsonRpcId, _>>` populated atomically by
   `publish_for_tool_dispatch` and checked / drained
   atomically in the existing `handle_plugin_publish` for
   `plugin.<id>.tool_result`. Stale or duplicate
   tool_results are rejected at intake with the existing
   `BrokerError::StaleRequestId` variant. The gate's
   held-confirmations map is a separate structure keyed by
   held `tool_request.request_id`, owned by
   `ConfirmationGate`. See §OM.

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
  - **No render-tree changes in m5a.** Round 1's draft added
    a `RenderNode::Confirm` variant; pi-1 M-4 correctly
    flagged that the modal is transient TUI-internal UI, not
    a persisted entry. Removed in round 2. The TUI overlay
    consumes the bus event directly (§TUI). Other frontends
    (none in v1 — `decisions.md` row 27) would either
    subscribe to `core.session.confirm_request` and render
    their own UI or ignore it.

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
    broker's `outstanding_dispatched` map (§OM) is the
    stale-id rejector; lives in the broker, not the gate, per
    pi-1 B-7);
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
    `supervisor.rs:176` (`SpawnRegistration::Provider`).
    Round 1 claimed m5a would consume both; pi-1 M-6
    correctly flagged that the gate publishes through
    `Broker::publish_for_tool_dispatch` and does not need
    direct provider peer access, and that
    `SpawnRegistration` is RAII-by-design. **m5a does not
    promise to remove these allows.** They stay open as m4
    retro follow-ups; if a reader emerges naturally during
    Phase 3 (e.g. the gate gains a typed
    `ProviderConn::send_confirm` direct path that the
    confirmation router uses), the allow drops with that
    commit. Otherwise they remain.
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
  `Cargo.toml` declaring `rafaello-core`, `fittings-core`,
  `fittings-server`, `fittings-client`, `fittings-transport`
  (live workspace crate names per
  `crates/rafaello-mockprovider/Cargo.toml:19-22` —
  pi-1 N-2), `tokio`, `reqwest = { workspace = true }`
  (new workspace dep), `serde`, `serde_json`, `ulid`. Bin
  target `rfl-openai`. Library target carries the
  wire-protocol client + the bus-side adapter so it can be
  exercised in isolation. Add `reqwest` to the workspace
  `[dependencies]` table.
- **W2.** New crate `crates/rafaello-openai-stub` (gated
  behind workspace feature `test-fixture`, like m4's
  `rafaello-bus-fixture`). Bin target `rfl-openai-stub`
  serves a deterministic `/v1/chat/completions` response
  on a localhost port chosen by the test harness; reads
  request bodies and asserts wire-shape; emits a recorded
  response (or two, for multi-turn tests).
- **W3.** Workspace fixture lock under
  `rafaello/fixtures/m5a-locks/` containing two manifests:
  `rfl-openai` (active provider; bindings
  `provider = true`, `provider_id = "openai"`,
  `network.mode = "proxy"`,
  `allow_hosts = ["127.0.0.1"]` for the stub /
  `allow_hosts = ["litellm.thepromisedlan.club"]` for the
  manual-validation lock,
  `env.pass = ["LITELLM_API_KEY"]` verbatim per pi-1 B-5)
  and a sink-declaring tool plugin `rafaello-mailcat` (new
  — §TP below).
- **W4.** Documentation-only Cargo metadata: the workspace
  README (if any) gains a one-line "m5a adds rfl-openai".

### Si — sink-class consumption

m1 already plumbed everything through to
`CompiledPlugin.tool_meta` (`compile.rs:204` /
`:440-463`). The live storage type is `Vec<String>`. m5a
only adds the consumer; storage type **does not change**
(pi-1 M-8).

- **Si1.** Add accessors on `CompiledPlugin`:
  - `tool_sinks(name: &str) -> Option<&[String]>` —
    returns the raw stored list;
  - `tool_sink_classes(name: &str) -> Vec<SinkClass>` —
    parser that maps each string to the §Si2 enum (the
    *only* place the typed enum appears; storage stays
    string);
  - `tool_always_confirm(name: &str) -> bool`.
- **Si2.** Add a sink-class enum
  `SinkClass { Network, VcsPush, Mail, WorkspaceWrite,
  Other(String) }` in `crates/rafaello-core/src/sinks.rs`.
  Used by the gate's UI (`ConfirmRequestPayload.summary`
  formatting) and by the m5b matching layer. **The
  existing `Vec<String>` storage in
  `bindings.tool_meta.<n>.sinks`,
  `compile.rs::ToolMeta.sinks`, and the m1 validator's
  acceptance set is unchanged.** No cross-crate cutover
  (pi-1 M-8). `infer_defaults` continues to return
  `Vec<String>`; the parser is layered over it.
- **Si3.** A positive test in `rafaello-core/tests/`:
  `tool_meta_with_sinks_drives_gate_decision.rs` — assert
  that a `CompiledPlugin` with
  `tool_meta["send-mail"].sinks = ["mail"]` returns
  `vec![SinkClass::Mail]` from `tool_sink_classes`, and
  that `tool_sinks` returns the underlying
  `&["mail".to_string()]`.

### Tr — install-time trifecta refusal

Bound to **`rfl install --fixture <dir>`** per pi-1 B-6:
reads a local manifest + package directory (the same
shape m4's fixture lock construction uses), computes the
digest pair via `digest::manifest_digest` +
`digest::content_digest`, snapshots a candidate
`PluginEntry`, runs `validate::lock` and
`trifecta::evaluate`, and writes `rafaello.lock`. Network
fetch / update / review-UI are explicitly **m6 / `rfl
init` territory** — m5a does not invent a plugin-source
URL scheme, does not implement download, does not handle
update or review.

- **Tr1.** New `rfl install` subcommand on the existing
  `rfl` binary (`crates/rafaello/src/main.rs` —
  pi-1 N-3). Signature:
  `rfl install --fixture <PACKAGE_DIR>
  [--lock <LOCK_PATH>] [--i-know-what-im-doing]
  [--allow-credential-paths]`.
  - Reads `<PACKAGE_DIR>/rafaello.toml` via
    `Manifest::parse`; runs
    `manifest::validate_with_package(&manifest_path,
    &package_dir, &manifest)`.
  - Resolves canonical id from manifest
    (`<source>:<name>@<version>` — `source` defaults to
    `local` for `--fixture` inputs; m6 may extend).
  - Computes `manifest_digest = digest::manifest_digest(&
    manifest.canonical_bytes())` and
    `content_digest = digest::content_digest(&package_dir)`.
  - Synthesises a default `Grant` from the manifest's
    `[capabilities.default]` (verbatim — m5a does not
    invent a review UI; what the manifest asks for is
    what the user grants by passing `--fixture`).
  - Constructs a candidate `PluginEntry` and merges into
    `Lock::from_toml(&existing_lock_text)?`.
  - Runs `validate::lock(&candidate, &LockValidationContext
    { ... })` — m1's existing V3 path.
  - Runs `trifecta::evaluate(&candidate, &canonical, &ctx)`;
    if `refuse == true` and
    `--i-know-what-im-doing` was not passed, returns
    `InstallError::TrifectaRefused { canonical, reads,
    outbound, write }` with the three booleans printed in
    the error message.
  - With `--i-know-what-im-doing`, sets
    `entry.flags.i_know_what_im_doing = true` before the
    second `trifecta::evaluate` call (which then returns
    `refuse == false`); the install proceeds and the
    flag is persisted to the lock.
  - Writes the merged lock back to `<LOCK_PATH>` (default:
    `${PROJECT_ROOT}/rafaello.lock`).
- **Tr2.** `--allow-credential-paths` sets
  `entry.flags.allow_credential_paths = true` per security
  RFC §7.3 / `decisions.md` row 12. Unchanged behaviour
  from m1's lock-flag semantics; m5a only exposes the CLI
  surface.
- **Tr3.** New `rfl status` subcommand on the existing
  `rfl` binary. Reads `${PROJECT_ROOT}/rafaello.lock`,
  prints one row per plugin with the canonical id, the
  bindings summary, and any active flags. Plugins with
  `flags.i_know_what_im_doing == true` are rendered with
  red ANSI (security RFC §7.1's "loud surfacing"
  requirement). Non-TTY output uses the `[OVERRIDE]`
  prefix instead.
- **Tr4.** Tests in `rafaello/tests/`:
  - `rfl_install_fixture_writes_lock.rs` — happy path:
    install a benign `rafaello-readfile`-shaped fixture;
    assert the lock gains the entry with the expected
    digests.
  - `rfl_install_refuses_trifecta_plugin.rs` — install a
    fixture manifest declaring all three trifecta
    dimensions; assert exit code non-zero, stderr
    contains `TrifectaRefused` and the three booleans.
  - `rfl_install_accepts_trifecta_plugin_with_override.rs`
    — same manifest + `--i-know-what-im-doing`; assert
    install succeeds and the lock entry's
    `flags.i_know_what_im_doing == true`.
  - `rfl_install_refuses_one_hop_outbound_via_other_plugin.rs`
    — install plugin A into a lock that already has
    plugin B (network-open) subscribing to A's published
    topic. Assert install fails; the one-hop direct check
    fires (security RFC §7.1.1).
  - `rfl_install_does_not_chase_transitive_outbound.rs`
    — same setup but A→B→C where only C is network-open
    and B does not subscribe to A's publish; assert
    install **accepts** plugin A (the transitive
    non-feature, `decisions.md` row 11). This is the
    third roadmap negative.
  - `rfl_status_prints_red_for_override_flag.rs` — TTY
    capture; `rfl_status_prints_override_prefix_for_non_tty.rs`
    — pipe stdout to a buffer.

### CT — confirmation topics + frontend ACL extension

#### CT0 — confirmation correlation table-of-truth (pi-1 B-2)

This table is the single source for confirm-protocol
correlation. m4 §B0's `request_id` table-of-truth pattern
extended to confirm topics. Every other §CT/§CG row cites
this table.

| Topic                          | Envelope `request_id`                              | Payload `request_id`                                                  | `in_reply_to`                                                          | Stale / unknown                                                                          | Duplicate                                                                            | Late (after timeout)                                                          |
|--------------------------------|----------------------------------------------------|-----------------------------------------------------------------------|------------------------------------------------------------------------|------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------|--------------------------------------------------------------------------------|
| `core.session.confirm_request` | fresh ULID = the gate's **held-confirmation key**  | identical to envelope `request_id` (single id, not two)               | exactly `[held_tool_request.request_id]` (one entry, the held call)   | n/a (core publishes; broker accepts)                                                     | gate enforces single-fire per held tool_request; second publish is a logic bug       | n/a                                                                            |
| `frontend.tui.confirm_answer`  | fresh ULID per answer publish (TUI generates)      | identical to envelope `request_id` (the TUI's answer-event id)        | exactly `[confirm_request.request_id]` (the held-confirmation key)    | broker rejects via `BrokerError::InvalidInReplyTo { reason: Missing }`; re-emit drops    | gate's first-write-wins on the held key; second answer audited as `late_duplicate`   | gate has already published the synthetic deny; second answer audited and dropped |
| `core.session.confirm_reply`   | fresh ULID = the reply event's id                  | identical to envelope `request_id`                                    | exactly `[confirm_request.request_id]` (forwarded verbatim by re-emit)| n/a (core publishes after re-emit validation)                                            | n/a (gate publishes once per held tool_request)                                      | n/a                                                                            |

Implications, pinned:

1. **Single canonical id** = the gate's
   `confirm_request.request_id`. The envelope `request_id`
   *equals* the payload `request_id` for all three topics
   (no two-id-space confusion). The held-confirmation map
   keys on this id.
2. The held-tool-request's `request_id` (a *separate* id —
   the tool-call correlation) is carried in
   `core.session.confirm_request.in_reply_to[0]` (per the
   m4 `in_reply_to` mechanism for any event that "inherits"
   from another). The audit log records both.
3. **Timeout vs. late answer.** When the 60 s deadline
   fires, the gate publishes the synthetic
   `core.session.tool_result` (deny path — see §CG5) and
   removes the held entry. A `confirm_answer` arriving
   *after* removal: the broker accepts the publish (its
   `in_reply_to` references a syntactically-valid prior
   event); the re-emit pipeline checks the held map, finds
   nothing, and **drops the answer with an audit-log
   entry** `confirm_late`. No `core.session.confirm_reply`
   is emitted. The TUI's overlay has already exited (it
   exited on its own timeout countdown — see §TUI3); a
   late keystroke that ends up publishing simply
   ghost-clicks.
4. **Duplicate answer** (two answers for the same held
   key): the second answer's re-emit finds the held entry
   already-resolved; audit-logs `confirm_duplicate` and
   drops.
5. **Unknown id** (`in_reply_to` references an id that
   was never held): re-emit rejects with
   `ReemitError::ConfirmAnswerUnknownId`; broker drops the
   re-emit; audit logs `confirm_unknown`.

- **CT1.** Three new topic constants in
  `crates/rafaello-core/src/bus.rs` (or a new
  `topics.rs` module if pi argues for hoisting):
  - `core.session.confirm_request`
  - `core.session.confirm_reply`
  - `frontend.tui.confirm_answer`
- **CT2.** Extend the
  `request_id`-mandatory topic-suffix list
  (`bus.rs::REQUEST_ID_REQUIRED_SUFFIXES`, m4 §B0
  table-of-truth / decisions row 43) to include
  `.confirm_request`, `.confirm_reply`, `.confirm_answer`.
  Broker rejects missing `request_id` with the existing
  `MissingRequestId` variant. Per-suffix tests:
  - `broker_publish_core_session_confirm_request_missing_request_id_rejected.rs`
  - `broker_publish_core_session_confirm_reply_missing_request_id_rejected.rs`
  - `broker_publish_frontend_tui_confirm_answer_missing_request_id_rejected.rs`
- **CT3.** Extend the `in_reply_to`-mandatory rule
  (security RFC §7.2.6 row 5) to
  `frontend.tui.confirm_answer` and (m5a addition)
  `core.session.confirm_reply`. Broker rejects with
  `InvalidInReplyTo { reason: Missing }`. Tests:
  - `broker_publish_frontend_tui_confirm_answer_missing_in_reply_to_rejected.rs`
  - `broker_publish_frontend_tui_confirm_answer_in_reply_to_too_many_rejected.rs`
    (the row-5 cardinality is **exactly one**)
  - `broker_publish_core_session_confirm_reply_missing_in_reply_to_rejected.rs`
- **CT4.** Frontend ACL extension. In
  `crates/rafaello/src/lib.rs:308-315`, add
  `frontend.tui.confirm_answer` and (per §SL below)
  `frontend.tui.slash_command` alongside the existing
  `frontend.tui.user_message`. Tests:
  - `frontend_publish_confirm_answer_accepted_by_broker.rs`
  - `frontend_publish_slash_command_accepted_by_broker.rs`
  - `frontend_publish_unknown_topic_rejected.rs` (a
    `frontend.tui.evil_topic` publish fails the new
    explicit ACL set)
- **CT5.** Re-emit pipeline (m4's
  `crates/rafaello-core/src/reemit/mod.rs`) gains a fourth
  arm: `frontend.tui.confirm_answer` inbound is
  canonicalised to `core.session.confirm_reply` after
  validation. Validation steps (per CT0 implications):
  1. Envelope `request_id` present (broker already
     checked); payload `request_id` equals envelope id.
  2. `in_reply_to` is exactly one entry; that id appears
     in the gate's held-confirmation map.
  3. The held entry has not already been resolved
     (otherwise → `ConfirmAnswerDuplicate`).
  4. The answer string is one of `"allow" | "deny" |
     "always_allow_session"` (otherwise →
     `ConfirmAnswerMalformed`).
  5. Synthesise canonical taint
     `[{source: "user", detail: None}]` per security RFC
     §7.2.2.
  6. Publish `core.session.confirm_reply` via
     `Broker::publish_core_with_taint` with `in_reply_to
     = [confirm_request.request_id]`.

  Tests:
  - `reemit_frontend_confirm_answer_to_core_session_confirm_reply.rs`
  - `reemit_confirm_answer_unknown_request_id_audit_logged.rs`
  - `reemit_confirm_answer_late_after_timeout_audit_logged.rs`
  - `reemit_confirm_answer_duplicate_audit_logged.rs`
  - `reemit_confirm_answer_malformed_string_rejected.rs`
  - `reemit_confirm_answer_synthesises_user_taint.rs`

### CG — confirmation gate

- **CG1.** New module `crates/rafaello-core/src/gate/mod.rs`.
  Public type `ConfirmationGate { broker, acl, controller,
  user_grants, audit, held }` where `held: BTreeMap<JsonRpcId,
  HeldConfirmation>` is the gate's own held-confirmations
  map (keyed by the **single canonical id** per CT0 — the
  `confirm_request.request_id`). **Distinct** from the
  broker's `outstanding_dispatched` map (§OM, owned by the
  broker, keyed by `(target_canonical, tool_request.request_id)`).
  Constructed by `rfl chat` after the broker but before
  the agent loop; spawned as a tokio task that subscribes
  internally (via `Broker::subscribe_internal`) to
  `core.session.tool_request` and
  `core.session.confirm_reply`.
- **CG2.** Decision logic on each `core.session.tool_request`:
  1. Resolve `dispatch_target` from the event payload
     (m4 already populates this); look up the
     `CompiledPlugin` for that canonical id.
  2. Compute `gate_required = !sinks.is_empty() ||
     always_confirm` via the §Si1 accessors.
  3. If `!gate_required`, pass through (publish
     `plugin.<topic-id>.tool_request` via the existing
     `Broker::publish_for_tool_dispatch` call); audit
     `gate_passthrough`.
  4. If `gate_required`, look up `user_grants` for an
     entry matching `(tool_name, args)` per §UG2;
     if matched, pass through and audit
     `gate_grant_match`.
  5. Otherwise, allocate `confirm_id = ULID::new()`; insert
     `held[confirm_id] = HeldConfirmation { tool_request:
     event.clone(), deadline: Instant::now() + 60s,
     dispatch_target: canonical }`; build the
     `ConfirmRequestPayload` (§CG3); publish via
     `Broker::publish_core_with_taint("core.session.confirm_request",
     payload, taint = [{source: "system", detail:
     "confirm_request"}], in_reply_to =
     Some(vec![event.request_id.clone()]))`; audit
     `confirm_request`.
- **CG3.** `ConfirmRequestPayload` shape (Stream A §5.6
  schema, payload field names lifted verbatim):
  ```json
  {
    "request_id": "<confirm_id>",
    "what": "tool_call",
    "summary": "<tool> via <plugin> — sinks: [<class>, ...]",
    "details": {
      "tool_call_id": "<held tool_request.request_id>",
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
  Per CT0: payload `request_id` equals envelope
  `request_id` (the canonical confirm key);
  `details.tool_call_id` carries the held
  `tool_request.request_id` (the second id space, for
  audit-log correlation and TUI display).
- **CG4.** On `core.session.confirm_reply` arrival
  (re-emitted by core after the §CT5 validation chain):
  the reply's `in_reply_to[0]` *is* the confirm key; look
  up `held[confirm_key]`. If absent, audit `confirm_late`
  and drop. If present, dispatch on `payload.answer`:
  - **`"allow"`**: publish the held tool_request via
    `Broker::publish_for_tool_dispatch` (the same call
    the m4 agent loop made directly); remove from
    `held`; audit `confirm_allowed`.
  - **`"deny"`**: synthesise a `core.session.tool_result`
    via the helper `gate::synthesise_deny_tool_result`
    (§CG4a); remove from `held`; audit `confirm_denied`.
  - **`"always_allow_session"`**: insert a `UserGrant`
    matching `(tool, args)` exactly via
    `UserGrants::add(UserGrant { tool, matcher:
    Structural::from_args(args), source:
    AlwaysAllowSession })`; then take the `"allow"`
    branch (publish the held request, remove,
    audit `confirm_allowed_with_session_grant`).
- **CG4a.** **Synthetic deny `core.session.tool_result`
  shape** (pi-1 B-3 — pinned to compile cleanly under
  the live m4 envelope rules and consume cleanly by
  the live `agent/mod.rs::handle_tool_result`):
  ```rust
  pub fn synthesise_deny_tool_result(
      held: &HeldConfirmation,
      reason: DenyReason,   // UserDenied | ConfirmTimeout
  ) -> PublishCoreArgs {
      PublishCoreArgs {
          topic: "core.session.tool_result",
          payload: serde_json::json!({
              "ok": false,
              "error": match reason {
                  DenyReason::UserDenied     => "user_denied",
                  DenyReason::ConfirmTimeout => "confirm_timeout",
              },
              "content": "",                     // handle_tool_result reads this
          }),
          request_id:  Some(JsonRpcId::from(Ulid::new())), // fresh result id
          in_reply_to: Some(vec![held.tool_request.request_id.clone()]),
          taint:       Some(vec![TaintEntry {
              source: "system".into(),
              detail: Some(match reason {
                  DenyReason::UserDenied     => "user_denied".into(),
                  DenyReason::ConfirmTimeout => "confirm_timeout".into(),
              }),
          }]),
      }
  }
  ```
  Wire facts the helper guarantees, all checked against
  live m4 source:
  - `request_id` is `Some(_)` because
    `bus.rs::REQUEST_ID_REQUIRED_SUFFIXES` includes
    `tool_result`;
  - `taint` is non-empty
    (`{source: "system", detail: <reason>}`) because
    `Broker::publish_core_with_taint` rejects
    `core.session.tool_result` with empty/missing taint
    (m4 §B7);
  - `in_reply_to[0]` is the held tool_request's
    `request_id` because
    `agent/mod.rs::handle_tool_result` reads
    `event.in_reply_to[0]` to derive `call_id` for the
    persisted `ToolResultPayload.call_id`;
  - the payload has `ok: false`, an `error` string, and
    a `content` field (empty) so
    `handle_tool_result`'s
    `obj.get("content").and_then(|v| v.as_str()).unwrap_or_default()`
    path produces a valid `RenderNode::Code { code: "",
    lang: None }` for the persisted entry. The
    operator's TUI sees a
    `tool_result` entry with `ok: false, error:
    "user_denied"` rendered through the existing m3
    rendering pipeline. No new render kind, no
    persistence-layer change.
- **CG5.** 60 s timeout: each `held` insertion schedules a
  `tokio::time::sleep_until(deadline)` task; on fire, the
  task acquires the gate's lock, checks `held[confirm_id]`
  is still present (not raced by an arriving allow/deny),
  and if so:
  1. publishes the synthetic deny `core.session.tool_result`
     via §CG4a with `reason = ConfirmTimeout`;
  2. removes the held entry;
  3. audit-logs `confirm_timeout`.
  Tests use `tokio::time::pause` per m3's idiom.
- **CG6.** Agent-loop change. The current
  `crates/rafaello-core/src/agent/mod.rs:143` direct call
  to `broker.publish_for_tool_dispatch` is **removed**;
  the agent loop only persists the `tool_call` entry and
  observes the canonical `core.session.tool_request`. The
  gate is now the sole driver of the dispatch publish.
  This is a small but architectural shift; called out
  separately for the commit-row plan. Test:
  `agent_loop_does_not_dispatch_tool_request_directly.rs`
  — assert that with no gate constructed, a
  `core.session.tool_request` produces no
  `plugin.<id>.tool_request` publish.
- **CG7.** **Multi-pending policy** (pi-1 M-3). OpenAI
  Chat Completions can return multiple `tool_calls` in a
  single response; sink calls can therefore arrive in
  rapid succession. The gate's policy:
  - **Hold queue is unbounded** (one entry per held
    `tool_request.request_id`); each gets its own 60 s
    deadline.
  - **TUI overlay serialises**: the TUI shows one
    confirm prompt at a time (the *oldest* held by
    `confirm_id` arrival order — held order corresponds
    to publish order via core's single-threaded re-emit).
    The next prompt becomes visible after the current
    one closes (allow/deny/timeout). The TUI maintains
    its own queue of pending `core.session.confirm_request`
    events and pops the next on close. While a prompt
    is visible, additional `confirm_request` events
    accumulate silently; the input line stays blocked.
    A small "+N more pending" badge surfaces when the
    queue is non-empty.
  - **`always_allow_session` short-circuit**: when the
    gate inserts a new `UserGrant`, it walks the
    `held` map and resolves any pending entries whose
    `(tool, args)` newly match (audit:
    `gate_grant_match_short_circuit`). The TUI's
    overlay, on observing that a held entry has been
    resolved server-side, drops the corresponding
    queued prompt before showing it (the
    `core.session.confirm_reply` events arrive on the
    TUI's bus subscription and the TUI tracks
    held→reply correlation for queue-pruning).
  - **Stale modal answer**: if the operator's answer
    arrives *after* the held entry was resolved by
    short-circuit, the §CT0 duplicate / late paths
    catch it.
  - **Per-held-entry timeout** is independent: timing
    out the visible prompt does not affect queued
    prompts (they each have their own deadline).
  Tests:
  - `gate_two_concurrent_sink_calls_serialise_in_tui.rs`
  - `gate_grant_short_circuits_pending_held_entry.rs`
  - `gate_late_answer_after_grant_short_circuit_audit_logged.rs`
  - `gate_per_held_timeout_independent.rs`
- **CG8.** Tests in `rafaello-core/tests/`:
  - `gate_passes_through_non_sink_tool_request.rs`
  - `gate_passes_through_user_grant_match.rs`
  - `gate_holds_sink_tool_request_pending_confirm.rs`
  - `gate_dispatches_on_allow.rs`
  - `gate_synthesises_deny_tool_result_with_pinned_shape.rs`
    (asserts the §CG4a wire shape exactly: `request_id`
    Some, `in_reply_to` matches held id, `taint`
    non-empty with `source: "system"`)
  - `gate_synthetic_deny_persists_through_agent_loop.rs`
    (publishes the synthetic event through
    `Broker::publish_core_with_taint` and asserts
    `agent/mod.rs::handle_tool_result` records a
    `tool_result` entry with `ok: false`)
  - `gate_times_out_to_deny_after_60s.rs` (paused-time;
    asserts the timeout reason is `confirm_timeout`)
  - `gate_always_confirm_true_holds_non_sink_tool.rs`
  - `gate_always_allow_session_creates_grant_and_dispatches.rs`
  - `gate_always_allow_session_grant_clears_on_restart.rs`
    (a fresh `ConfirmationGate` constructed afterward —
    simulating `rfl chat` restart — re-prompts; the
    grant is in-memory only).
  - `gate_late_confirm_answer_audit_logged.rs`
  - `gate_duplicate_confirm_answer_audit_logged.rs`
  - `gate_unknown_confirm_answer_audit_logged.rs`

### OM — outstanding-dispatched map (broker-side, atomically checked)

Owned by the broker (not the gate — pi-1 B-7). Validates
the m4 retro §5.1 / security RFC §7.2.6 row 1
"`plugin.<id>.tool_result` must reference the matching
tool_request previously routed to this plugin" check at
**broker intake**, atomically inside `handle_plugin_publish`
— before the result reaches re-emit, before the gate sees
it, and before any external subscriber observes it. The
gate's separate held-confirmations map (§CG1) is *not*
this; conflating them was the round-1 design bug.

- **OM1.** New broker state
  `BrokerState::outstanding_dispatched: BTreeMap<CanonicalId,
  BTreeMap<JsonRpcId, OutstandingDispatch>>` keyed by
  target plugin canonical id then by tool_request
  `request_id`. Populated atomically inside
  `Broker::publish_for_tool_dispatch` (a tool_request
  routed to plugin X with id N → record `(X, N)` in the
  same critical section that hands the event to the
  fittings transport).
- **OM2.** Validation in
  `Broker::handle_plugin_publish` for topic suffix
  `tool_result`:
  - extract the publisher's canonical id from the
    `Publisher::Plugin { canonical, .. }` arm;
  - extract `id = event.in_reply_to[0]`;
  - check `outstanding_dispatched[canonical].contains_key(&id)`;
    if absent, return `BrokerError::StaleRequestId
    { canonical, id }` (m4 already has the variant) —
    the publish is rejected before fan-out, before
    re-emit, before any subscriber sees the event;
  - if present, **drain the entry** (`remove`) before
    fan-out so a duplicate `tool_result` from the same
    plugin citing the same id fails the next call.
- **OM3.** Tests in `rafaello-core/tests/`:
  - `broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`
    — plugin A publishes `tool_result` citing an id
    nothing was dispatched for; broker rejects.
  - `broker_plugin_tool_result_in_reply_to_routed_to_other_plugin_rejected.rs`
    — id N was dispatched to plugin A; plugin B
    publishing `tool_result` citing N fails closed.
  - `broker_plugin_tool_result_duplicate_publish_rejected.rs`
    — plugin A publishes twice with the same id; the
    second publish fails at intake with `StaleRequestId`
    (the first drained the entry).
  - `broker_plugin_tool_result_race_two_concurrent_publishes.rs`
    — spawn two tasks publishing `tool_result` with the
    same id from the same plugin concurrently; assert
    exactly one succeeds, exactly one fails with
    `StaleRequestId` (atomic intake check).
  - `broker_outstanding_dispatched_populated_by_publish_for_tool_dispatch.rs`
    — direct `BrokerState` accessor in `#[cfg(test)]`
    asserts the map is populated synchronously.

### UG — user_grants

- **UG1.** New module
  `crates/rafaello-core/src/user_grants.rs`. Type
  `UserGrants { entries: BTreeMap<GrantId, UserGrant> }`
  plus `UserGrant { tool: String, plugin: CanonicalId,
  matcher: GrantMatcher, added_at: DateTime<Utc>, source:
  GrantSource }`. The `plugin` field pins the grant to a
  specific plugin canonical id (so a `/grant send-mail`
  granted while plugin A owns the tool name does not
  silently authorise plugin B if a future `rfl provider
  tool` reassigns the name — the matcher checks
  `(plugin, tool, args)`, not just `(tool, args)`).
- **UG2.** **Matcher semantics** (pi-1 M-5, resolved per
  the round-2 prompt's reading of Stream A §7.2.4 +
  overview §15.1).

  The lock's
  `bindings.tool_meta.<tool>.grant_match` is a
  JSON-Schema **shape contract on the matcher template**.
  m5a's matching is the smallest-acceptable conformant
  implementation:

  1. **At `/grant` time** (slash command processing in
     core — see §SL3): core compiles the user-supplied
     `key=value` list into a JSON object — the
     "matcher template" — and validates that template
     against the lock's `grant_match` schema using the
     `jsonschema` crate (workspace dep added in §W1).
     - Schema-validation failure → `core.session.command_result
       { ok: false, error: "matcher schema mismatch:
       <jsonschema diagnostic>" }`; no entry is added.
     - Schema absent in `grant_match` → the template is
       accepted as-is (the manifest declared no shape
       contract, so the structural-subset matcher
       applies broadly).
     - **Lock-pinned: `bindings.tool_meta` is read once,
       at gate construction; manifest changes mid-session
       are not re-read** (m1 lock-correspondence
       precedent, m4 §"Lock-correspondence claim").
  2. **At runtime** (gate matching against incoming
     `tool_request.args`, §CG2 step 4): structural-subset
     match — every key in the matcher template must
     appear in the request `args` with a deep-equal
     value. Recursive on JSON objects; arrays compared
     element-wise; missing template key → no match;
     extra args keys → still match (subset semantics).

  Concretely:

  ```rust
  pub enum GrantMatcher {
      /// `/grant <tool>` with no key/value pairs and no
      /// schema declared → matches every invocation of
      /// the tool. Surfaced loudly in `/grants list`.
      Any,
      /// `/grant <tool> <k>=<v> ...` → template object;
      /// matches iff request args is a structural superset.
      Structural { template: serde_json::Value },
  }
  ```

  Why this resolves the round-1 contradiction: round-1
  said "structural subset" while leaving the manifest
  schema unused. m5a now uses the schema **at /grant
  time** to validate the *shape* of the user's template
  (Stream A §7.2.4's "uses the matcher schema declared
  in the tool's manifest" — the schema constrains what a
  valid matcher *is*, not what every tool call looks
  like). Runtime matching stays cheap structural-subset
  to avoid per-call schema compilation. A future m6
  could promote runtime matching to full schema
  validation if profiling justifies it.

- **UG3.** `GrantSource` enum `SlashCommand`,
  `AlwaysAllowSession`. (`ProviderProposal` is reserved
  but not constructed in m5a; m5b/m6 territory.)
- **UG4.** API:
  - `UserGrants::add(grant: UserGrant) -> GrantId`
  - `UserGrants::list(&self) -> Vec<(GrantId, &UserGrant)>`
  - `UserGrants::revoke(id: GrantId) -> Result<(), RevokeError>`
  - `UserGrants::matches(plugin: &CanonicalId, tool:
    &str, args: &Value) -> Option<GrantId>`
- **UG5.** Tests:
  - `user_grants_any_matcher_matches_every_invocation_of_tool.rs`
  - `user_grants_structural_matcher_subset_match.rs`
  - `user_grants_structural_matcher_value_mismatch.rs`
  - `user_grants_structural_matcher_missing_key.rs`
  - `user_grants_structural_matcher_extra_args_still_matches.rs`
  - `user_grants_plugin_pinned_does_not_match_other_plugin.rs`
  - `user_grants_template_validated_against_lock_schema_at_grant_time.rs`
  - `user_grants_template_schema_mismatch_rejected.rs`
  - `user_grants_template_no_schema_declared_accepted.rs`
  - `user_grants_revoke_removes_entry.rs`
  - `user_grants_revoke_unknown_id_errors.rs`
  - `user_grants_revoke_during_pending_confirmation_does_not_short_circuit.rs`
    (pi-1 M-7 — revoking a grant after a `tool_request`
    has already been short-circuited as
    `gate_grant_match` does not retroactively un-allow
    the in-flight call; revoking before the next call
    blocks the next one).

### SL — slash commands (bus-mediated)

Per pi-1 B-1: the TUI is a separate process and **cannot
mutate core's `UserGrants` directly**. Slash commands are
typed bus events; core (the `UserGrants` owner) is the
sole mutator. Two new topics added to the frontend ACL.

- **SL1.** TUI input parser change. Lines beginning with
  `/` are detected by a new
  `SlashCommand::parse(input: &str) -> Result<SlashCommand,
  ParseError>` in `crates/rafaello-tui`. Parsed commands
  are serialised into a typed payload and published on
  `frontend.tui.slash_command`. Lines that fail to parse
  are still published (with `kind: "unknown"` and the raw
  input) so core's audit log captures the attempt.
- **SL2.** New bus topic
  `frontend.tui.slash_command` (frontend ACL grant added
  in §CT4). Payload schema:
  ```json
  {
    "request_id": "<ulid>",                  // CT0-class envelope id mandatory
    "command": "grant" | "list_grants" | "revoke" | "unknown",
    "args": {
      // for "grant":     { "tool": "...", "plugin": "<canonical>?", "template": {...} }
      // for "revoke":    { "grant_id": "..." }
      // for "list_grants": {}
      // for "unknown":   { "raw": "<input>" }
    }
  }
  ```
  Mandatory envelope `request_id` per the same suffix
  rule m5a adds for `confirm_*` (§CT2 — slash_command is
  added to the same suffix list because the response
  must correlate). Mandatory `in_reply_to` is **not**
  required (`frontend.tui.slash_command` is a *root*
  event, like `frontend.tui.user_message`; there is no
  prior event whose taint it inherits).
- **SL3.** New core handler in
  `crates/rafaello-core/src/user_grants.rs::handle_slash_command`
  (or a new `crates/rafaello-core/src/slash.rs` module if
  pi prefers separation). The handler:
  - subscribes to `frontend.tui.slash_command` via
    `Broker::subscribe_internal`;
  - validates the payload shape (`Result<SlashCommand,
    InvalidCommand>`);
  - if `command == "grant"`: looks up the lock's
    `bindings.tool_meta[tool].grant_match` schema (if
    present); validates the supplied `template`
    against it via the `jsonschema` crate; on success
    inserts a `UserGrant`;
  - if `command == "list_grants"`: enumerates entries;
  - if `command == "revoke"`: looks up by id; removes;
  - if `command == "unknown"`: no mutation;
  - publishes a `core.session.command_result` event
    (new core-only topic; added to the
    `request_id`-mandatory list and to core publish
    authority) with payload:
    ```json
    {
      "request_id": "<ulid>",                          // fresh
      "in_reply_to": ["<slash_command request_id>"],
      "ok": true | false,
      "kind": "grant" | "list_grants" | "revoke" | "unknown",
      "message": "human-readable summary",
      "details": { ... }                               // grant-id, list, error message
    }
    ```
  - audit-logs `grant_added` / `grant_revoked` /
    `grant_list` / `slash_unknown`.
- **SL4.** TUI rendering: the TUI subscribes to
  `core.session.command_result` (no new ACL grant
  required — TUI's subscribe pattern already covers
  `core.session.**`) and renders the result as a
  **transient inline text line** above the input
  (single-line callout, distinct visual treatment from
  conversation entries). Not persisted as a
  `core.session.entry.finalized`. The user can scroll
  back to see prior command results in the same chat
  buffer until the TUI is restarted.
- **SL5.** Tests:
  - `tui_slash_grant_publishes_typed_event.rs`
  - `tui_slash_grant_with_args_template_object.rs`
  - `tui_slash_unknown_command_publishes_unknown_kind.rs`
  - `tui_user_message_starting_with_slash_not_published.rs`
    — input `/foo` does not generate a
    `frontend.tui.user_message`; produces a
    `frontend.tui.slash_command` instead.
  - `core_slash_command_grant_handler_inserts_user_grant.rs`
  - `core_slash_command_grant_template_schema_mismatch_publishes_ok_false.rs`
  - `core_slash_command_grant_no_schema_template_accepted.rs`
  - `core_slash_command_revoke_unknown_id_publishes_ok_false.rs`
  - `core_slash_command_list_grants_returns_entries.rs`
  - `core_slash_command_malformed_payload_rejected.rs`
  - `core_slash_command_publishes_command_result_correlated.rs`
    (asserts `in_reply_to` matches the slash request id)
  - `frontend_publish_slash_command_missing_request_id_rejected.rs`
  - `audit_log_records_grant_added_with_plugin_pin.rs`

### TUI — confirmation overlay (TUI-internal, transient)

Per pi-1 M-4: the modal is **TUI-internal UI**, not a
persisted entry kind. It consumes the
`core.session.confirm_request` bus event directly and
publishes `frontend.tui.confirm_answer` on user input.
**No `RenderNode::Confirm`, no entry persistence, no
server-side downgrade.** The round-1 §RC section is
deleted.

- **TUI1.** New input mode in `rafaello-tui`:
  `InputMode::ConfirmOverlay { confirm_id, summary,
  details, ttl_remaining, queued_count }`. Entered when
  the TUI's bus subscriber observes
  `core.session.confirm_request`. While in this mode the
  input line is non-editable; key events drive the
  answer:
  - `y` / `a` / `Enter` → publish
    `frontend.tui.confirm_answer { answer: "allow",
    in_reply_to: [confirm_id] }`
  - `n` / `d` / `Esc` → answer `"deny"`
  - `s` → answer `"always_allow_session"`
- **TUI2.** Overlay rendering: a framed area above the
  input line with the summary, the args, the sinks list,
  the (m5a-empty / m5b-populated) taint list, and a TTL
  countdown ticked from a `tokio::time::interval(1s)`.
  No render-tree work — the overlay is painted directly
  by the TUI's existing ratatui pipeline. **The
  countdown is purely UI** (deadline enforcement is
  server-side per §CG5); a stale countdown that fires
  before the server-side timeout merely repaints "0s
  remaining" and waits for the synthetic deny event to
  arrive.
- **TUI3.** Multi-pending queue (per §CG7). The TUI
  maintains a `VecDeque<PendingConfirm>` of
  `core.session.confirm_request` events whose answer
  hasn't been published. The current overlay corresponds
  to the queue head. On exit (allow/deny/timeout/
  short-circuit), the next is popped and shown. While
  queued, the overlay shows "+N more pending" in the
  frame. Short-circuited entries (server-side
  `core.session.confirm_reply` arriving for a
  not-yet-shown queued confirm) are silently dropped from
  the queue.
- **TUI4.** Status display for the persisted `tool_call`
  entry: when `core.session.tool_result` for the
  corresponding `request_id` arrives (allow / deny /
  timeout), the existing m3 entry-update path renders
  the result row beneath the call row — no overlay-side
  state mutation needed; the renderer pipeline just
  paints the new `tool_result` entry as the m3 / m4
  pipeline already does.
- **TUI5.** Tests in `rafaello-tui/tests/`:
  - `tui_enters_overlay_on_confirm_request.rs`
  - `tui_y_key_publishes_allow_answer.rs`
  - `tui_n_key_publishes_deny_answer.rs`
  - `tui_esc_key_publishes_deny_answer.rs`
  - `tui_s_key_publishes_always_allow_session.rs`
  - `tui_input_blocked_during_overlay.rs`
  - `tui_overlay_exits_on_confirm_reply_via_bus.rs`
  - `tui_two_concurrent_confirm_requests_serialise.rs`
    (asserts queue head shown, "+1 more pending"
    rendered)
  - `tui_short_circuited_pending_overlay_silently_dropped.rs`
  - `tui_overlay_does_not_persist_entry_for_confirm_request.rs`
    (asserts no `core.session.entry.finalized` is
    emitted for the modal itself).

### OP — `rfl-openai` provider plugin

- **OP1.** Wire-protocol client in
  `crates/rafaello-openai/src/wire.rs`. Wire-shape table
  (pi-1 M-2 — pinned for handoff):

  | Aspect                             | m5a behaviour                                                                                                                |
  |------------------------------------|-------------------------------------------------------------------------------------------------------------------------------|
  | Request struct                     | `ChatCompletionRequest { model: String, messages: Vec<Msg>, tools: Option<Vec<ToolDecl>>, tool_choice: Option<...> }`        |
  | Response struct                    | `ChatCompletionResponse { id: String, choices: Vec<Choice>, usage: Option<Usage> }`                                          |
  | `Choice` shape                     | `{ index: u32, message: Msg, finish_reason: String }`                                                                        |
  | `Msg` shape                        | `{ role: "user" \| "assistant" \| "tool" \| "system", content: Option<String>, tool_calls: Option<Vec<ToolCall>>, tool_call_id: Option<String> }` |
  | `ToolCall` shape                   | `{ id: String, type: "function", function: { name: String, arguments: String } }` (`arguments` is JSON-encoded as a string per OpenAI spec) |
  | HTTP method / path                 | `POST <RFL_OPENAI_ENDPOINT_URL>/chat/completions` (the URL comes pre-suffixed with `/v1` per OP5)                            |
  | Auth header                        | `Authorization: Bearer <api-key value>`; key value read from the env var **named by `RFL_OPENAI_API_KEY_ENV`** (see OP5)     |
  | Streaming                          | Disabled — `stream: false` in the request body. SSE handling deferred to v2 per `decisions.md` row 28.                       |
  | Timeout                            | Single 60 s per-request timeout; no retries in m5a. Failure → `provider.openai.assistant_message` with `text: "<error>"` and a structured `details` field (see OP1a) |
  | HTTP non-200                       | 4xx → emit assistant_message `"openai: client error <status>: <body excerpt>"`; 5xx → `"openai: server error <status>"`; both audit-logged via `core.session.entry.finalized` |
  | Auth failure (401/403)             | Specifically named in OP1a: emit `"openai: auth failed (<status>); check API key env var"` |
  | Connection error / timeout         | `"openai: transport error: <reqwest::Error display>"`                                                                        |
  | Malformed JSON response            | `"openai: malformed response: <serde error>"` — log full body to stderr for `manual-validation.md` capture                   |
  | Empty `choices`                    | Treat as a no-op turn — emit a single assistant_message `"(no response)"`; do not panic                                      |
  | Multiple `choices`                 | Use `choices[0]` only; log a stderr warning if `len > 1`                                                                     |
  | `finish_reason` handling           | `"stop"` / `"length"` → emit assistant_message; `"tool_calls"` → emit one `provider.openai.tool_request` per `tool_calls[i]`; other reasons → log + treat as `"stop"` |
  | Mixed final content + tool_calls   | Per OpenAI spec a single response can carry both `content` and `tool_calls`. m5a emits the `assistant_message` first (preserving narration), then one `tool_request` per `tool_calls[i]` in array order |
  | `tool_calls[i].function.arguments` parse error | Emit `assistant_message "openai: invalid tool args from model: <serde error>"`; do **not** emit the malformed tool_request (the bus would reject the args anyway) |
  | Unknown tool name (model proposes a tool not in `core.tools_list` cache) | Emit assistant_message `"openai: model proposed unknown tool '<name>'"`; do not emit tool_request |
  | Multiple `tool_calls` in one response | Each is published as a separate `provider.openai.tool_request` with a fresh `request_id`, all carrying the same `in_reply_to` (the user_message id that triggered the round) |
  | `model` resolution                 | Read from `RFL_OPENAI_MODEL` env var (set by the supervisor per OP5); default if unset: `"gpt-4o-mini"` (audit-logged warning since the dev default `vllm/qwen3.6-27b` is the operator's intent) |
  | Conversation history forwarded     | The plugin maintains a per-session in-memory `Vec<Msg>` constructed from observed `core.session.user_message` (`role: "user"`), prior `assistant_message` (`role: "assistant"`), and `core.session.tool_result` (`role: "tool"`, `tool_call_id` from `in_reply_to[0]`). |

- **OP1a.** Error mapping helper
  `crates/rafaello-openai/src/error.rs::map_to_assistant`
  produces the deterministic strings above. Tests for
  401/403/500/timeout/malformed-json/empty-choices.
- **OP2.** **Tool schema discovery via fittings RPC**
  (pi-1 B-4). Round 1 used a `core.session.tools_advertised`
  bus event published at startup; pi-1 B-4 correctly
  flagged that `Broker::fan_out` only delivers to
  *registered* peers, the broker has no replay-on-subscribe
  for arbitrary topics (decision row 41 covers replay
  only for `core.session.entry.finalized`), and the
  provider is registered *after* core's startup publish
  fires. Replaced with a **request/response fittings RPC
  method on core**: `core.tools_list`. Wire shape:
  ```rust
  // request:  no params
  // response: { tools: Vec<ToolSchema> }
  // ToolSchema { name, description?, parameters_schema: serde_json::Value }
  ```
  - Method registered on core's fittings server (the
    same server that hosts the bus broker per m2).
    `BrokerAcl.fittings_methods` (m2 already exists)
    extended with `core.tools_list`.
  - `rfl-openai` calls
    `peer.call("core.tools_list", json!({}))` once after
    completing the fittings handshake, before
    subscribing to `core.session.user_message`. The
    response is cached on the plugin's heap. A failed
    call is fatal: the plugin exits with a typed error
    that `rfl chat`'s supervisor catches as
    `SpawnError::PostHandshakeFailure` (m4 already
    surfaces this category).
  - The schema source is the same `BrokerAcl.tool_routes`
    map plus `CompiledPlugin.tool_meta` from m1; no new
    compile-time work. Core synthesises one
    `ToolSchema` per `tool_routes` entry by reading the
    target plugin's `bindings.tool_meta` (sinks /
    `grant_match` are not forwarded to the model — the
    model only needs name + parameters).
  - Test: `core_tools_list_rpc_returns_compiled_tool_routes.rs`
    (the broker-internal test); test:
    `openai_calls_tools_list_after_handshake.rs` (the
    provider-side integration test against a stubbed
    core).
  - The two related m4 dead-code allows
    (`ProviderConn.peer`, `SpawnRegistration::Provider`)
    are *still* not naturally read by m5a per pi-1 M-6;
    `core.tools_list` is a normal fittings server method,
    not a peer-direct call. The allow-removal stays a
    m4 retro follow-up.
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
- **OP5.** **Lock binding shape** (env model resolved
  per pi-1 B-5 / round-2 prompt: simplest path — no
  rename syntax, plugin reads the API-key env-var
  *name* from `RFL_OPENAI_API_KEY_ENV`, the *value*
  from whichever env var that names). The lock TOML
  uses the **live `GrantEnv` shape** (`pass:
  Vec<String>`, `set: BTreeMap<String, String>` per
  `crates/rafaello-core/src/lock/grant.rs:66-70`),
  not the round-1 array-of-`KEY=VALUE` form pi-1 B-5
  caught:
  ```toml
  [plugin."builtin:openai@0.0.0".bindings]
  provider     = true
  provider_id  = "openai"

  [plugin."builtin:openai@0.0.0".grant.bundles.default.network]
  mode        = "proxy"
  allow_hosts = ["litellm.thepromisedlan.club"]

  # Pass the deployment's host env var verbatim. The plugin reads its
  # name from RFL_OPENAI_API_KEY_ENV (set below) and looks up the value.
  [plugin."builtin:openai@0.0.0".grant.bundles.default.env]
  pass = ["LITELLM_API_KEY"]

  [plugin."builtin:openai@0.0.0".grant.bundles.default.env.set]
  RFL_OPENAI_API_KEY_ENV  = "LITELLM_API_KEY"
  RFL_OPENAI_ENDPOINT_URL = "https://litellm.thepromisedlan.club/v1"
  RFL_OPENAI_MODEL        = "vllm/qwen3.6-27b"
  ```
  The plugin's source code reads exactly three env vars:
  - `RFL_OPENAI_ENDPOINT_URL` — the OpenAI-compatible
    endpoint URL (m5a-reserved);
  - `RFL_OPENAI_MODEL` — the model name to request
    (m5a-reserved);
  - **the env var *named* by `RFL_OPENAI_API_KEY_ENV`** —
    the API key value. So in the dev deployment the
    plugin does `std::env::var(std::env::var("RFL_OPENAI_API_KEY_ENV")?)?`,
    which resolves through `LITELLM_API_KEY`. In a
    different deployment, the lock would set
    `RFL_OPENAI_API_KEY_ENV = "OPENAI_API_KEY"` and
    `pass = ["OPENAI_API_KEY"]`.

  m1's scrubber's `SECRET_PATTERNS` strips
  `*_KEY`-pattern names from `env.pass` unless
  `i_know_what_im_doing == true`. `LITELLM_API_KEY`
  matches the pattern. Two sub-options:
  1. **(Selected)** The `rfl-openai` lock entry sets
     `flags.i_know_what_im_doing = true` so its
     `env.pass` is honoured verbatim. This is loud in
     `rfl status` (red marker) and is honest: the
     bundled provider is a deliberate exception to the
     scrubber's "all `*_KEY` look like secrets and
     should not flow" default. Alternative for users
     who reject the marker: rename their host env var
     to `LITELLM_PROXY_TOKEN` (no `*_KEY` suffix) and
     adjust the lock; the plugin's source is unchanged.
  2. (Rejected) Add a `LITELLM_API_KEY` exception to
     `SECRET_PATTERNS`. Too narrow.

  Tests:
  - `openai_lock_with_litellm_api_key_pass_requires_iknow_flag.rs`
    (without the flag, `validate::lock` strips the
    pass entry → plugin starts without the key →
    auth fails on first call; documented behaviour).
  - `openai_endpoint_url_taken_from_env_var.rs`
  - `openai_model_taken_from_env_var.rs`
  - `openai_api_key_resolved_via_indirection_env_var.rs`
- **OP6.** **(Section deleted in round 2.)** The
  rename-syntax extension (round-1 §OP6 / §A3) is
  withdrawn per pi-1 B-5 + the round-2 prompt. The
  indirection-via-`RFL_OPENAI_API_KEY_ENV` approach in
  OP5 above is the m5a path; no schema extension.
- **OP7.** Tests in `rafaello-openai/tests/`:
  - `openai_manifest_compiles.rs`
  - `openai_calls_tools_list_after_handshake.rs`
  - `openai_emits_assistant_message_for_user_message.rs`
    (against the stub server — §W2)
  - `openai_emits_tool_request_when_model_returns_tool_call.rs`
  - `openai_request_carries_tool_schemas.rs`
  - `openai_in_reply_to_populated_for_assistant_message.rs`
  - `openai_in_reply_to_populated_for_tool_request.rs`
  - `openai_endpoint_url_taken_from_env_var.rs`
  - `openai_api_key_resolved_via_indirection_env_var.rs`
  - `openai_handles_tool_call_followed_by_assistant_message.rs`
    (multi-turn).
  - **Negatives** (pi-1 M-2):
    - `openai_http_401_emits_auth_failed_assistant_message.rs`
    - `openai_http_500_emits_server_error_assistant_message.rs`
    - `openai_malformed_response_body_emits_diagnostic.rs`
    - `openai_empty_choices_emits_no_response_assistant_message.rs`
    - `openai_multiple_choices_uses_first_logs_warning.rs`
    - `openai_invalid_tool_arguments_string_emits_error_assistant_only.rs`
    - `openai_unknown_tool_name_from_model_emits_error_assistant.rs`
    - `openai_multiple_tool_calls_one_response_emits_each_with_shared_in_reply_to.rs`
    - `openai_mixed_content_and_tool_calls_emits_assistant_then_tool_requests.rs`
    - `openai_post_handshake_failure_propagates_through_supervisor.rs`

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
- **TP2.** `[provides.tool.send-mail].grant_match`
  schema referencing
  `crates/rafaello-mailcat/schemas/send-mail-grant.json`:
  ```json
  {
    "type": "object",
    "properties": { "to": {"type": "string"} },
    "required": ["to"]
  }
  ```
  Per §UG2 (revised round 2): core validates the
  user-supplied **template object** (from
  `/grant send-mail to=alice@example.com`) against this
  schema at `/grant` time using the `jsonschema` crate.
  Runtime matching against incoming tool args is
  structural-subset (cheap; no per-call schema compile).
  m5a does **not** run JSON-Schema validation on every
  tool invocation — see §"Out of scope".
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
      kind         TEXT NOT NULL,           -- see kind list below
      request_id   TEXT,                    -- nullable for events with no correlation id
      payload      TEXT NOT NULL            -- JSON
  );
  ```
  `kind` values (m5a):
  - **gate**: `gate_passthrough`, `gate_grant_match`,
    `gate_grant_match_short_circuit`, `confirm_request`,
    `confirm_allowed`, `confirm_denied`,
    `confirm_allowed_with_session_grant`,
    `confirm_timeout`, `confirm_late`,
    `confirm_duplicate`, `confirm_unknown`;
  - **slash command** (core-side handler): `grant_added`,
    `grant_revoked`, `grant_list`, `slash_unknown`;
  - **install**: `install_refused`, `install_accepted`,
    `trifecta_overridden`, `credential_paths_overridden`.
- **AL2.** `AuditWriter` consumer (a single in-process
  writer holding the SQLite connection from m3's session
  store) wired into:
  - the gate (confirm_* events, gate_passthrough,
    gate_grant_match, gate_grant_match_short_circuit);
  - the core-side slash-command handler
    (grant_added / grant_revoked / grant_list /
    slash_unknown); pi-1 B-1 — slash commands are
    bus-mediated, so the audit hook fires inside core's
    handler, not the TUI;
  - the `rfl install` subcommand (install_refused /
    install_accepted / trifecta_overridden /
    credential_paths_overridden).
- **AL3.** No bus-side `audit.*` topic; the audit log is
  a **passive sink** read only via SQLite (a future
  `rfl audit` subcommand; not in m5a). Rationale: a bus
  topic would invite plugin subscribers and complicate
  the trust model.
- **AL4.** Tests:
  - `audit_records_confirm_request_event.rs`
  - `audit_records_confirm_reply_event.rs`
  - `audit_records_confirm_timeout_with_reason.rs`
  - `audit_records_confirm_late_after_timeout.rs`
  - `audit_records_confirm_duplicate.rs`
  - `audit_records_grant_addition_with_plugin_pin.rs`
  - `audit_records_grant_revocation.rs`
  - `audit_records_slash_unknown.rs`
  - `audit_records_trifecta_override_at_install.rs`
  - `audit_records_install_refused_with_three_booleans.rs`
  - `audit_seq_monotonic_per_session.rs`

### M1 — m1 lock-side carryovers

- **M1.1.** Extend m1's `RESERVED_ENV_VARS`
  (`crates/rafaello-core/src/scrubber.rs:23-31` —
  **currently seven** per pi-1 N-5; the round-1
  count of "six per row 40" was stale, the live list is
  `RFL_BUS_FD`, `RFL_PLUGIN`, `RFL_HELPER_FD`,
  `RFL_TOPIC_ID`, `RFL_PROJECT_ROOT`,
  `RFL_PRIVATE_STATE_DIR`, `RFL_PROVIDER_ID`) to **nine**
  by adding `RFL_OPENAI_ENDPOINT_URL` and
  `RFL_OPENAI_MODEL`. Per row 40's pattern: rejected at
  compile / V3 time when present in `env.set` or
  `env.pass` of any plugin's lock entry. The
  `RFL_OPENAI_API_KEY_ENV` indirection name (§OP5) is
  intentionally **not** reserved — it is a user-set
  string whose *value* is interpreted as the name of
  another env var. Round 1's `RFL_OPENAI_API_KEY`
  reservation and the `<host>:<canonical>` rename
  syntax are both withdrawn (pi-1 B-5).
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
  - construct an `AuditWriter` against the SQLite path
    (the connection is shared with m3's session store
    via the existing `Arc<SessionController>` pool);
  - register the `core.tools_list` fittings RPC method
    on the broker's fittings server with the compiled
    tool-routing table (§OP2 — replaces round 1's
    bus-event approach per pi-1 B-4);
  - register the core-side slash-command handler
    (§SL3) as an internal subscriber on
    `frontend.tui.slash_command`;
  - construct the `ConfirmationGate` (§CG1) wired to the
    broker, the `UserGrants`, the audit writer, and the
    session controller; spawn its task;
  - then proceed with m4's existing supervisor + plugin
    spawn + agent loop construction. The agent loop's
    direct dispatch path is removed (§CG6); the gate
    is now between agent loop and the
    `plugin.<topic-id>.tool_request` publish.
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
  `confirm_request` and a `confirm_allowed`.
- **`deny` arm:** assert `entries` contains `text`
  (user), `tool_call` (status `Denied`),
  `tool_result` (`{ok: false, error: "user_denied"}`,
  taint `[{source: "system", detail: "user_denied"}]`),
  `text` (assistant — the model's response to the
  denial); mailcat.log is empty; audit log records
  `confirm_denied`.

### Negative 1 — confirmation timeout denies

`rafaello/tests/rfl_chat_demo_bar_send_mail_timeout.rs`
— same setup but `RFL_TUI_TEST_CONFIRM_ANSWER=timeout`
(the TUI does not publish an answer at all). The test
uses tokio paused time advanced past 60 s. Assert: the
gate publishes a synthetic `core.session.tool_result`
with the §CG4a shape (`taint = [{source: "system",
detail: "confirm_timeout"}]`, `in_reply_to =
[held_id]`); the entries / mailcat state matches the
deny arm above; the audit log records a
`confirm_timeout` event.

### Negative 2 — `always_allow_session` clears on `rfl chat` restart

`rafaello/tests/rfl_chat_always_allow_session_clears_on_restart.rs`
— first invocation with
`RFL_TUI_TEST_CONFIRM_ANSWER=always_allow_session`;
assert mailcat.log gains one entry, audit log records
`confirm_allowed_with_session_grant` and `grant_added`.
Second invocation in the same tempdir (same SQLite, same
lock — but a fresh `rfl chat` process, so a fresh
empty `UserGrants`) drives the same user message; the
TUI is configured with no pre-existing grant; automated
TUI answers `deny` after 10 ms via
`RFL_TUI_TEST_CONFIRM_ANSWER=deny` +
`RFL_TUI_TEST_CONFIRM_DELAY_MS=10` (pi-1 N-6 — the
round-1 wording said "unset" while also setting
the env vars; clarified). Assert: the second run
**prompts again** (a fresh `confirm_request` audit
entry appears) and the deny holds (mailcat.log
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

### Bonus negatives implied by the security RFC / m4 retro / pi-1 M-7

- `rafaello/tests/rfl_chat_always_confirm_true_holds_non_sink_tool.rs`
  — a fixture tool with `sinks = []` and
  `always_confirm = true`. Assert the gate fires the
  prompt even though no sinks are declared.
- `rafaello/tests/rfl_install_status_shows_red_for_override.rs`
  — install a trifecta plugin with
  `--i-know-what-im-doing`; assert `rfl status`
  prints the entry with the red ANSI marker.
- `rafaello/tests/rfl_chat_grant_revoked_blocks_next_call_but_not_in_flight.rs`
  (pi-1 M-7) — grant `send-mail to=alice@…`; observe
  one allowed call; revoke; observe the next call
  prompts again. The in-flight call (mid-dispatch, not
  yet `tool_result`) is **not** retroactively un-allowed.
- `rafaello/tests/rfl_chat_grant_for_one_plugin_does_not_authorise_another.rs`
  (pi-1 M-7) — install two `send-mail`-providing
  plugins (one as `mailcat`, one as a second sink-fixture
  with the same tool name); `lock.session.tool_owner`
  pins the canonical for `send-mail`. `/grant send-mail
  to=...` pins to the owning canonical (per §UG1). A
  later `rfl provider tool send-mail <other-plugin>`
  re-pins; the next call to `send-mail` prompts again
  because the grant doesn't match the new plugin.
  (This negative is reachable only via manual
  validation in m5a — `rfl provider tool` is post-v1
  per overview §8 — but the unit-level `UserGrants`
  test `user_grants_plugin_pinned_does_not_match_other_plugin.rs`
  covers the data-structure side.)
- `rafaello-core/tests/broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`
  — closes m4 §5.1 / pi-3 M-2.

---

## Out of scope

The following are explicitly NOT in m5a and not allowed to
sneak in via implementation drift. **m5a is not the full m5
roadmap row; m5b remains required before m5 is closed**
(pi-1 M-1 / round-2 prompt). m5b's carve-out is sketched in
Appendix A.

1. **Taint matching against recently-emitted tool_result
   payloads** (security RFC §7.2.1–§7.2.2) — m5b. The gate
   is taint-independent in m5a per `decisions.md` row 9.
2. **Plugin-supplied taint superset enforcement on
   re-emission** (security RFC §7.2.6 superset rule) — m5b.
3. **Verbatim tool-result-to-sink exfil demo (the roadmap
   row's fourth negative)** — m5b. m5a alone cannot show
   the verbatim status in the prompt; m5b layers
   taint-influenced prompt details on top of m5a's stable
   gate. m5 is not closed until m5b ships this negative.
4. **Provider-extracted user_grants proposals** (security
   RFC §7.2.4 item 3) — deferred to m6 / v2. The
   `GrantSource::ProviderProposal` arm is reserved but
   never constructed in m5a.
5. **Per-tool-call JSON-Schema validation of incoming
   args against the manifest's `grant_match` schema**
   (round 1's wording was over-broad — pi-1 M-5 caught
   it). m5a *does* use the schema at `/grant` time to
   validate the matcher template (§UG2); m5a does *not*
   re-run schema validation on every tool invocation
   (that would cost a per-call schema compile). The
   runtime check is structural-subset against the
   stored template. Deferred to m6 if profiling justifies
   it.
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
   frontend principal; `frontend.tui.confirm_answer` and
   `frontend.tui.slash_command` are the only m5a-added
   frontend publishers.
10. **Subprocess plugin renderers** — `decisions.md` row 29.
    The TUI overlay is TUI-internal UI (pi-1 M-4); the
    round-1 `RenderNode::Confirm` is withdrawn.
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
cost. Round-1 §A1 (matcher), §A3 (env rename), §A4
(`RenderNode::Confirm`) **resolved in round 2** per pi-1
M-5 / B-5 / M-4 + the round-2 prompt's owner-judgment
guidance; deleted from this list. §A2 also resolved (now a
fittings RPC, not a bus event) and removed.

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

6. **`core.tools_list` RPC race with provider startup**
   (revised round 2 — round-1 risk re tools_advertised
   event obviated by pi-1 B-4's switch to RPC). The
   provider calls `peer.call("core.tools_list",
   json!({}))` after fittings handshake completes. If
   core's broker has not yet registered the
   `core.tools_list` method when the call arrives, the
   call fails with `MethodNotFound`. Mitigation: core
   registers the method **before** spawning any plugin
   (CHAT1 ordering). Test:
   `core_tools_list_registered_before_provider_spawn.rs`.

7. **Slash-command parsing collides with future user
   intent.** A user typing `/path/to/file` as part of a
   real conversation triggers `unknown command`.
   Mitigation: documented in §SL — the user sees a
   clear "unknown command: <input>" message in the
   `core.session.command_result` and can re-type with
   a leading space. Acceptable for v1; a richer parser
   (e.g. require `/` *and* a known verb) is m6.

8. **TUI overlay does not pass through entry-persistence.**
   The `core.session.confirm_request` is transient; the
   TUI consumes it directly without finalising any
   entry (pi-1 M-4 — round-1 §RC withdrawn). Mitigation:
   the gate publishes confirm events via
   `Broker::publish_core_with_taint` but the agent loop
   does **not** subscribe to `core.session.confirm_*`
   (its `handle_event` match arms in
   `agent/mod.rs:106-116` only cover `user_message` /
   `assistant_message` / `tool_request` / `tool_result`
   — confirm topics are explicitly outside the persistence
   path). Test:
   `agent_loop_does_not_persist_confirm_request_event.rs`.

9. **Audit-log writes contend with session-store
   writes.** Both share the SQLite database. m3's
   session store uses connection-per-task with WAL.
   m5a's audit writer reuses the same connection pool.
   No new locking contracts; risk is bounded.

10. **(Deleted in round 2 — env-rename schema extension
    withdrawn per pi-1 B-5.)**

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

13. **`rfl install --fixture` boundary.** Bounded per
    pi-1 B-6: reads a local manifest + package, computes
    digests, snapshots a candidate `PluginEntry`, runs
    validate + trifecta, writes the lock. Network fetch /
    update / review-UI explicitly excluded — those are
    m6's `rfl init` territory. The risk is implementation
    drift towards "a real installer"; mitigation in
    commits.md drafting is to inline the §Tr1 algorithm
    verbatim into the per-commit prompts so the agent
    cannot accidentally invent a download path.

14. **Stream A drift carryover patches.** §10 banner
    fix and any `confirm_*` schema additions to Stream A's
    body land as anticipated retro drift, **not in this
    branch**. Pi may catch a missing patch; m5a retro is
    the natural place.

15. **`jsonschema` is a new workspace dep** (§W1 / §UG2).
    m5a uses it only at `/grant` time (one schema compile
    per slash command, then drop). Mitigation: feature-gate
    the `jsonschema` crate behind a `slash-grants` feature
    on `rafaello-core` if the dep weight surprises CI;
    the gate's runtime structural-subset matcher does
    not need it.

16. **`flags.i_know_what_im_doing` on the bundled
    `rfl-openai` lock entry** (§OP5). The dev deployment
    requires this flag because `LITELLM_API_KEY` matches
    the scrubber's `*_KEY` pattern. The risk is operator
    confusion ("why does the bundled provider need this
    scary flag?"). Mitigation: `rfl status` rendering
    distinguishes bundled-plugin overrides from
    third-party overrides (a future `--bundled-ok`
    affordance is m6); the manual-validation script
    documents the flag with rationale.

---

## Manual validation

The companion `manual-validation.md` (Phase 3) records:

1. **Real-network demo.** Run `rfl chat` against the
   dev LiteLLM proxy with a fixture lock setting
   `RFL_OPENAI_ENDPOINT_URL = "https://litellm.thepromisedlan.club/v1"`,
   `RFL_OPENAI_API_KEY_ENV = "LITELLM_API_KEY"`, and
   `env.pass = ["LITELLM_API_KEY"]` (with
   `flags.i_know_what_im_doing = true` per §OP5 to bypass
   the `*_KEY` scrubber). Type "please email
   alice@example.com that I'll be late" — the model
   proposes `send-mail`; the overlay fires; the operator
   presses `y`; the mailcat plugin's on-disk log gains an
   entry. Same flow with `n` shows the deny path.
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
5. **(Deleted in round 2 — `RenderNode::Confirm` /
   §RC withdrawn per pi-1 M-4. The TUI overlay is
   transient UI on the bus event; no renderer-tree
   work.)**
6. **Confirmation gate (CG1-CG8)** — ~4-5 commits. The
   gate's decision logic is the largest single module;
   passthrough vs hold vs reply paths each merit their
   own commit. The agent-loop pivot (CG6) is its own
   commit because it removes the m4 dispatch path; pi
   will want this commit isolated.
7. **`user_grants` (UG1-UG5)** — ~2-3 commits. Matcher,
   API surface, and the `jsonschema`-template-validation
   path each warrant their own commit.
8. **Slash commands (SL1-SL5) — bus-mediated path
   (pi-1 B-1)** — ~3 commits. The TUI parser publishes
   typed events; core's handler subscribes and mutates
   `UserGrants`; `core.session.command_result` payload
   shape and audit hook are the third commit.
9. **TUI confirmation overlay (TUI1-TUI5)** — ~2 commits.
   Input mode + queue + key handling + tests. **No
   `RenderNode::Confirm`** (round-1 §RC withdrawn per
   pi-1 M-4).
10. **Audit log (AL1-AL4)** — ~2 commits. Schema migration
    + writer.
11. **Install-time trifecta refusal (Tr1-Tr4) bound to
    `rfl install --fixture` (pi-1 B-6)** —
    ~3 commits. The `rfl install` subcommand + the
    `rfl status` subcommand + the four tests. The
    transitive-not-chased test is its own commit
    because it asserts a deliberate non-feature.
12. **`rafaello-mailcat` fixture (TP1-TP3)** —
    ~2 commits.
13. **`rfl-openai` provider plugin (OP1-OP7) including
    `core.tools_list` RPC (pi-1 B-4) and the negative
    matrix (pi-1 M-2)** — ~5-6 commits. Wire client +
    error mapping; bus adapter; `core.tools_list` RPC
    method on core + provider-side caller; integration
    tests; negative matrix.
14. **`rfl chat` orchestration extension (CHAT1-CHAT3)** —
    ~2 commits. Wiring + test-mode env hooks.
15. **Demo-bar headline + manual validation** —
    ~2 commits. The two `rfl_chat_demo_bar_send_mail*`
    tests + `manual-validation.md` skeleton.

Forced-monolithic commits called out explicitly:

- The agent-loop pivot (CG6) is a m0-c08-class API change
  inside the agent loop; it is the *only* place where
  m4 behaviour changes shape in m5a. Commit body must
  call this out.
- The §OM broker outstanding-dispatched map +
  `handle_plugin_publish` validation lands as a single
  commit (the populator and the consumer are coupled at
  the broker-state level — splitting them across two
  commits leaves a window where the populator is dead
  code).

Realistic total: **~30-38 commits sequential** (round-2
estimate revised slightly upward — round-1 underestimated
the slash-command bus path and the OpenAI negative matrix).
m4 took 28 plan-row commits at comparable surface; m5a's
surface is similar (gate + ACL + provider + two
fixtures + slash commands + overlay + audit + install).
Pi round budget: **plan for 6-8 scope rounds** (m4 took
6). No m5a-i / m5a-ii split anticipated.

---

## Acceptance summary

> **m5a is not the full m5 roadmap row.** The roadmap row
> in `milestones/README.md` defines m5 as "OpenAI-compatible
> provider + sinks + confirmation protocol + user_grants +
> exfil demo" with a four-negative demo. m5a covers the
> positive demo + three of four negatives; **m5b (Appendix A)
> remains required before m5 is closed**, specifically for
> the verbatim-tool-result-to-sink-flow-blocked-at-the-broker
> negative, taint propagation, and the plugin-supplied
> taint superset enforcement. Owner ratification of m5a
> implicitly accepts the split per the roadmap row's
> pre-authorisation language ("May split into m5a … and m5b
> … if scoping finds it too big") — see pi-1 M-1.

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
  - **`decisions.md` row for the `audit_events` table** —
    optional; recording-only would suffice.
  - **`decisions.md` row for the `core.tools_list`
    fittings RPC** — required (it's a new core surface
    the security model depends on for provider tool-schema
    discovery).
  - **`overview.md` §4.6 reserved env-vars table** —
    add `RFL_OPENAI_ENDPOINT_URL` and `RFL_OPENAI_MODEL`
    (two, not three — `RFL_OPENAI_API_KEY` is
    intentionally not reserved per §OP5).
  - **`glossary.md`** — add an `Audit log` entry
    (table-passive, append-only) and adjust the
    `Confirmation protocol` entry to point at m5a's
    `gate/` module.

  **Pushed back (pi-1 M-6):** m5a does **not** promise
  to remove the m4 §5.5
  `#[allow(dead_code)]` sites on `bus.rs:101`
  (`ProviderConn.peer`) and `supervisor.rs:176`
  (`SpawnRegistration::Provider`). The gate publishes
  through `Broker::publish_for_tool_dispatch` and does
  not need direct provider peer access; `SpawnRegistration`
  is RAII-by-design. Manufacturing a read-side just to
  satisfy the allow-removal is exactly the "fake work"
  pi-1 M-6 flagged. The allows stay open as m4 retro
  follow-ups.

- No follow-up Stream RFC drift is owed by m5a beyond the
  items above. m5a does NOT modify Stream A's body in
  this branch (banner-only, m1 / m3 / m4 precedent).

m5a ships the **first sink-confirmation gate**: a real
model proposes a sink-declaring tool call, core holds it
behind a TUI overlay, the user answers, and the audit log
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

*End of m5a scope round 2.*
