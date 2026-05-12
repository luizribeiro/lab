# m5b — taint matching + propagation + verbatim exfil demo — retrospective

> **Status: round 3 — folds `retrospective-pi-review-2.md`
> (2 B / 2 M / 2 N).** Pi-2 closed pi-1's blockers and
> surfaced two new carries: (B-1) the round-2 §8 additions
> list still diverged from the six ratified scope §"Manual
> validation" bullets; (B-2) the Stream A §7.2.6 row 3/4/5
> disposition still misnamed the rows. Two majors: (M-1)
> the c24 deviation table contradicted §3.2 on whether c24
> took path 2 or path 3; (M-2) §2.5 + §9 overclaimed "zero
> production code path" / "dispatch site vanishes in
> release builds" for §TM4 — the live cfg-gate is more
> precise. Two nits: (N-1) banner M-3 self-contradiction on
> the round-1 wrong path; (N-2) §5 item 2 surface column
> missing `handle_rpc_reply`.
>
> Round-3 fixes by pi-2 finding (one line each):
>
> - **B-1** §8 manual-validation additions list rewritten
>   verbatim to the six ratified scope bullets:
>   (1) verbatim-exfil walkthrough with file-backed fetch,
>   (2) allow-arm audit trail, (3) overlay rendering +
>   clipping, (4) macOS CI URL, (5) audit-log inspection,
>   (6) no-match / provider-only path with no
>   `_taint_attached` row. The "Real-network demo" framing
>   dropped. The §PT1 violation demo is preserved as an
>   *extra-not-scoped* note, clearly flagged. Any LiteLLM-
>   driven run is labelled "Real-provider walkthrough
>   (file-backed fetch via `RFL_FETCH_TEST_BODY_PATH`)" —
>   explicit no-real-network.
> - **B-2** §6.1 + §5 item 2 + §8 "what's not tested" +
>   §10.9 rewritten with the live row mapping: row 3 =
>   `provider.<id>.assistant_message`; row 4 =
>   `plugin.<a>.rpc_reply`; row 5 =
>   `frontend.<id>.confirm_answer`.
>   `core.session.confirm_reply` is a core-output topic
>   under the §TR5 reserve framing, not §7.2.6 row 4.
> - **M-1** §3 c24 row corrected to state c24 **took path
>   3** (accepted the deviation; routed the audit-row
>   *primitive* to c23b). §3.2 narrative reworded: c23b
>   proves the §AL1 predicate at modal-open; c24 proves
>   `confirm_allowed` + mailcat + entries; **no single test
>   joins them**. The "operator-disposition-independent"
>   framing is replaced with the explicit split-coverage
>   statement. §10.2 owner re-look mirrored.
> - **M-2** §2.5 §TM4 wording softened: "storage and
>   installer are cfg-gated; production uses a cfg-selected
>   no-op checker and pays no hook-storage allocation or
>   dynamic dispatch." §9 inheritance bullet mirrored.
>   "Zero production code path" / "vanishes in release
>   builds" claims dropped.
> - **N-1** Banner M-3 fixed: round-1's wrong path was
>   `referenced_taint.rs` (not `referenced_taint_index.rs`,
>   which is the correct live path).
> - **N-2** §5 item 2 surface column updated:
>   `reemit/mod.rs::handle_assistant_message`,
>   `handle_rpc_reply`, `handle_confirm_answer` (was
>   missing `handle_rpc_reply`).
>
> Drift commits (§6 patches + §7 `decisions.md` row
> appends) remain deferred to a follow-up commit after
> retrospective ratification per the m5a `816b273`
> precedent.
>
> ---
>
> **(History — round 2 draft, kept for trajectory.)**
>
> Round 2 folded `retrospective-pi-review-1.md`
> (3 B / 4 M / 3 N).
>
> Round-2 fixes by pi-1 finding (one line each):
>
> - **B-1** §3 now lists **c24 as a deviation peer to c23**
>   (same single-completion-stub limitation). c24's commit
>   body explicitly carries a "Deviation (path 3 per c24
>   row text)" header. §3 deviation table expands to two
>   rows; §3.2 adds the c24 narrative. §8 coverage report
>   re-routes the audit-row *primitive* to harness coverage
>   (c23b proves the §AL1 predicate at modal-open under a
>   synthetic event sequence; c24 proves the end-to-end
>   allow-arm shape; no single test joins them). §10
>   owner-judgment item 2 status moves from "honoured" to
>   "honoured with deviation".
> - **B-2** §8 manual-validation framing corrected: the file
>   landed at **c15** (`6bea5ba`) as a 9-line wire-shape
>   note (§3 "Wire shapes" anchoring the `details.taint`
>   `[]`-vs-`null` rendering contract), not a c28 skeleton.
>   The c28 row's manual-validation work was the §C38c
>   acceptance test, not the doc. §8's "bullets to fill"
>   list is reframed as "additions the existing 9-line
>   file needs before merge."
> - **B-3** §6.1 Stream A drift plan: §7.2.6 row 4 reference
>   dropped (no such row in the live RFC table per pi-1
>   B-3). The actual narrowing is the **`rpc_reply`
>   superset arm** which sits under §A9's §"Out of scope"
>   item 2 framing (not taken in m5b; v2 candidate).
>   Reframed §6.1 bullets so rows 3 + 5 (assistant_message
>   + confirm_answer) and the `rpc_reply` arm are listed
>   explicitly rather than via the invented row-4 anchor.
> - **M-1** c23b file path corrected throughout (§3 / §5 /
>   §8): live file lives at
>   `rafaello-core/tests/m5b_value_match_exfil_chain_fires_harness.rs`
>   (not the round-1
>   `rfl_chat_value_match_taint_unioned_in_canonical_tool_request.rs`
>   guess).
> - **M-2** §2.5 §TM4 hook signature corrected to the live
>   shape: `pub type PublishTestHook = Arc<dyn Fn(&BusEvent)
>   -> Option<BrokerError> + Send + Sync>` (the hook
>   *may inject a `BrokerError`*, which is how the TR1/TR3
>   stale-entry assertions exercise the rejection paths
>   without forcing a separate fault-injection seam); stored
>   in `Mutex<Option<PublishTestHook>>` with last-writer-
>   wins semantics. The round-1 `Box<dyn Fn(&PublishMsg) +
>   Send + Sync>` was wrong on three counts (Arc not Box;
>   `BusEvent` not `PublishMsg`; hook returns
>   `Option<BrokerError>` not unit).
> - **M-3** §9 cache path corrected:
>   `crates/rafaello-core/src/reemit/referenced_taint_index.rs`
>   (not the round-1 `referenced_taint.rs`).
> - **M-4** §6.5 explicit Stream B (fittings) unaffected
>   rationale added — c16's TUI overlay extension is
>   `rafaello-tui`-internal and does not touch the fittings
>   RPC surface; c18/c19's multi-answer hook is test-only
>   plumbing also outside the fittings surface. Stream E
>   (renderer) coverage anchored on `decisions.md` row 29
>   (m5a-internal TUI overlay rendering) — m5b honours
>   unchanged. No §6.6 needed.
> - **N-1** §1 "bonus negatives" phrasing dropped — c24
>   (§EXFIL2 allow-arm) and c25 (§EXFIL3 provider-only
>   negative) are *scope-ratified* tests, not bonuses.
> - **N-2** §1 LoC + test-count stats explicitly pinned to
>   the c23b tip `e533361` (was implicit `HEAD`).
> - **N-3** §7 row numbering reworded "expected to land as
>   rows 50-58" (not "lands as rows 50-58") — the rows are
>   sketches; the editor commit adding them to
>   `decisions.md` lands after retro convergence.
>
> Drift commits (§6 patches + §7 `decisions.md` row
> appends) land in a follow-up commit on this retro branch
> **after retrospective ratification**, per the m5a
> precedent (m5a's drift landed at `816b273` after retro
> convergence). This commit folds pi-1 only.
>
> `scope.md` converged in **7 pi rounds** (m5a was 6, m4 was
> 6, m3 was 22, m2 was 8, m1 was 4, m0 was 3); `commits.md`
> converged in **6 pi rounds** (m5a was 6, m4 was 3, m3 was
> 9, m2 was 4, m1 was 3, m0 was 3). The seventh scope round
> was a short editorial pass folding pi-6's 0 blockers / 2
> majors / 5 nits — pi-6 itself called for "one short round
> before convergence." Round 7's load-bearing addition was
> §TM4 (the broker publish-test hook), which surfaced from
> pi-6 M-1 and unlocked the TR1/TR3 stale-entry test seam
> without leaking a test-only path into the production
> `Broker::publish_async` body. See §2.5.
>
> Companion: `manual-validation.md` — landed at **c15**
> (`6bea5ba`) as a 9-line **§3 "Wire shapes"** note that
> anchors the `details.taint` rendering contract
> (`Vec<TaintEntry>` JSON; `[]` for empty, never `null`;
> §CD1 / §CD3). The c28 row's manual-validation work was
> the §C38c positive gate-through-orchestration test, not
> a doc skeleton. §8 below enumerates the *additions* the
> existing 9-line file needs before merge (manual-run
> transcripts, audit-log dumps, etc.), framed as
> additions-to-an-existing-file rather than skeleton-fill.
>
> This document is the milestone-level review against
> `scope.md` round 7 and `commits.md` round 6, following the
> `plans/README.md` Phase-3 contract and the m5a retrospective
> shape (the same ten-section layout — m5b inherits m5a's
> "introduces enough new surface, plus one in-flight
> orchestrator carveout, that the five-section shape is too
> coarse" framing, though m5b's deviation count is smaller).

---

## 1. Summary

m5b ships **value-driven taint matching + taint propagation
through the canonical re-emit pipeline + broker-intake
plugin-supplied-taint superset enforcement + the
`rafaello-fetch` sink-declaring fixture + the verbatim exfil
demo test (roadmap row's fourth negative) + the §AL1 /
§AL2 / §AL3 audit-row enrichments + the multi-answer TUI
scripted hook + the three m5a c38 acceptance-test
follow-ups**. m5b is the **last v1 milestone with security
primitives**; m6 is polish (`rfl init`, docs, Homebrew,
`rfl audit` read CLI). The roadmap's "May split…" pre-
authorisation for m5 is honoured and discharged.

**Commit count.** 28 plan-row commits on the m5b branch
(`e10f9e8..0261962` covering c01–c28 in 1:1 correspondence
with `commits.md` round-6 rows c01–c28) + **one ratified-
deviation sibling test commit** (`e533361`, c23b) added per
the OWNER-RATIFIED option-C disposition at `86d6124` (see
§3.1). Total Phase-3 implementation commits: **29**. No
mid-Phase-3 bundling, no docs-only commits inserted between
plan-row commits other than the c23 deviation-ratification
note (`86d6124`) which carries no code. Phase-2 docs commits
(scope rounds 1–7 + 6 pi-review files + 2 ratification
commits; commits rounds 1–6 + 6 pi-review files + 3
ratification commits; driver-preflight) land before c01 and
are not counted in the plan-row total.

**LoC.** `git diff rafaello-v0.1..e533361 --shortstat` (the
c23b tip — pinned explicitly per the m5a §1 N-3 trap of
"shortstat-against-`HEAD`-includes-retro-commits") reports
**190 files changed, 20,181 insertions, 69 deletions**
across the 29 plan-row commits + the docs-iteration commits.
The same diff restricted to `rafaello/crates`,
`rafaello/tests`, and `rafaello/fixtures` (the
implementation surface, excluding `rafaello/plans/`) reports
**172 files changed, 11,675 insertions, 68 deletions**. The
implementation half nets **106 new top-level `tests/*.rs`
files** (106 A, 19 M, 0 D — no Phase-3 test renames or
deletions). The `rafaello-v0.1` baseline (workspace-wide
`rafaello.*/tests/[^/]+\.rs$` files via `git ls-tree`)
carried **577** test files; the live tree at `e533361`
carries **683**.

**Demo bar status.** The m5b-scoped demo-bar arm green:
- **Negative 4 — verbatim tool-result-to-sink exfil flow
  blocked at the broker** —
  `rafaello/tests/rfl_chat_demo_bar_verbatim_exfil_blocked.rs`
  (c23, `9503912`). The headline integration test drives
  `rfl chat` end-to-end against the m5b lock (`rfl-openai`
  + `rafaello-mailcat` + `rafaello-fetch` + `rafaello-
  readfile` + `rafaello-mockprovider`) with an in-process
  OpenAI stub scripting `web-fetch` → `send-mail` and
  asserts the persisted entries, audit kinds, fetch log
  shape, and empty `mailcat.log`. The broker-block end-to-
  end (the headline security guarantee) is asserted by the
  integration test; the value-match chain end-to-end is
  covered by the c23b harness sibling (`e533361`,
  `rafaello-core/tests/m5b_value_match_exfil_chain_fires_harness.rs`
  — see §3.1).

Other scope-ratified tests landed:
- §EXFIL2 allow-arm audit-trail variant (`cac7ae5`, c24)
  — landed with a documented deviation (path 3:
  `confirm_request_taint_attached` audit row not asserted
  in the integration test; same single-completion-stub
  limitation as c23, harness-covered by c23b's audit-row
  assertion path). See §3.2.
- §EXFIL3 provider-only-taint negative — no value match, no
  referenced union (`6bb626b`, c25).
- §C38a five-tree spawn + clean shutdown smoke (`ca28de3`,
  c26) — closes m5a §5 item 12.
- §C38b inactive-provider re-emit ignored (`92db0a8`, c27)
  — closes m5a §5 item 13.
- §C38c positive gate-through-orchestration (`0261962`,
  c28) — closes m5a §5 item 15.

m5a §5 item 14 (`core_tools_list_registered_before_provider_spawn.rs`)
remains routed to m6 — m5b did not pick it up because the
structural guarantee (`CorePluginService::new` runs ahead
of the supervisor's spawn loop by construction) is m5a-
territory regression coverage rather than m5b-shaped work.

**Pi convergence trajectory.**

- `scope.md`: **7 rounds** (round 1 → round 7 ratified
  `50c5ae8`). Blocker trajectory: **6B → 3B → 3B → 3B →
  2B → 0B → CONVERGED**. m5a was 6, m4 was 6, m3 was 22.
- `commits.md`: **6 rounds** (round 1 → round 6 ratified
  `b301a39`). Blocker trajectory: **5B → 3B → 4B → 3B →
  1B → CONVERGED**. m5a was 6, m4 was 3, m3 was 9.

The scope bracket exceeded m4 / m5a by one round, attributable
to two cumulative pressures across rounds 5–7: (a) the
§TR4b construct-the-superset vs synthetic-deny choice took
two rounds to converge (pi-1 B-6 surfaced the split; pi-2
B-1 ripple split `ReferencedTaintIndex` into `by_request_id`
+ `by_result_id` arms); (b) the §TM4 broker publish-test
hook was added in round 7 as a pi-6 M-1 fold to keep the
TR1/TR3 stale-entry tests honest. The commits bracket
matched m4-baseline (6 rounds against m5a's 6) and shrank
relative to the surface count: m5a's 41-row plan needed 6
commits rounds, m5b's narrower 28-row plan needed the
same 6 to land the unsplittable cutovers (c04, c14) and the
internal-split moves (row 13 vacate → row 1'' add for the
`AuditKind` ordering fix).

---

## 2. Implementation surprises / non-obvious decisions

### 2.1 c04 `OutstandingDispatch.tool_request_taint` unsplittable cutover

c04 (`af62ab5`) is the m5b equivalent of m0 c08 / m4 c07 / m5a
c06: a `bus.rs` struct extension (adding the `tool_request_taint:
Vec<TaintEntry>` field to `OutstandingDispatch`) that ripples
to every test constructor and every call site of
`publish_for_tool_dispatch`. Declared `medium-to-large` in
`commits.md` §"Sizing summary" with body justification (scope
§"Risks" #17 / m0 c08 / m4 c07 precedent). Landed self-
contained — no agent-side pressure to split during Phase 3.
The cutover commit body cites the precedent and names the
single critical-section invariant: at the moment
`publish_for_tool_dispatch` populates `outstanding_dispatched`,
the gate's canonical taint (provider-identity ∪ value-match
∪ referenced-request-taint) is recorded alongside the
`request_id` / `dispatched_at` pair so that §PT1's superset
check at intake can compare against an authoritative ground
truth without a second lookup against the re-emit cache.

### 2.2 c14 §PT1 broker-intake superset critical-section cutover

c14 (`75cc375`) is the second m5b unsplittable cutover.
Declared in `commits.md` round 5 as "forced-monolithic" per
internal-split row 10: the superset check, the drain order,
the synthetic-deny `core.session.tool_result` publish, the
`plugin_publish_rejected_taint_superset` audit row, the
`core.lifecycle.publish_rejected` emission with
`code = "taint_superset_violated"`, and the new
`BrokerError::TaintSupersetViolated` consumer are coupled at
the critical-section level of `handle_plugin_publish`. A
split (e.g., "check first, audit second, deny third") would
leave the tree in a state where intake rejects but neither
audits nor synthesises a deny, hanging the provider's
in-flight tool turn. The cutover lands self-contained with
m0 c08 / m4 c07 precedent cited in the commit body. The
~340-LoC bundle fits within the declared `medium-to-large`
bucket and required no Phase-3 split pressure.

### 2.3 `Broker::set_audit_writer` interior-mutable plumbing

The §PT1 critical section needs an `AuditWriter` handle for
`plugin_publish_rejected_taint_superset`, but the
`AuditWriter` is constructed by `rfl chat` *after*
`Broker::new` because the audit-DB path resolves from the
`SessionStore` which is opened from the resolved project
root. Wiring the audit writer as a constructor argument
would have forced the broker construction order to flip —
unacceptable for `rfl-openai`'s startup latency budget and
incompatible with the m5a-shipped `Broker::new` call site.

The chosen shape (pi-2 B-2 / pi-3 B-2 ratified at scope
§A2 / internal-split row 1') is:

- `BrokerInner.audit: parking_lot::Mutex<Option<Arc<AuditWriter>>>`
  — interior-mutable, defaults to `None`, replaced at most
  once via `Broker::set_audit_writer(&self,
  Arc<AuditWriter>)`.
- `rfl chat` calls `set_audit_writer` between `Broker::new`
  and the first plugin spawn (`run_chat` line right after
  `AuditWriter` open, ahead of `supervisor.spawn` loop).
- Acceptance test
  `rfl_chat_sets_audit_writer_before_first_plugin_spawn.rs`
  (c02) asserts the order via a supervisor-internal test
  seam.

The same pattern serves §AL1 (`confirm_request_taint_attached`)
and §AL3 (`tool_request_taint_unioned_from_in_reply_to`) —
the gate and re-emit pipelines all read
`BrokerInner.audit.lock().as_ref().cloned()` at the moment
they need to write a row, accepting `None` as "no audit
configured" (fail-open for tests that don't wire one).
m5a-style audit kinds that flow through `gate/mod.rs`
already inherit this shape via the m5a-landed `AuditWriter`
plumbing; m5b's broker-side rows are the new consumers.

### 2.4 §TR4b construct-the-superset vs synthetic-deny (pi-1 B-6 ripple)

Round-1 scope.md drafted §TR4b as a "if the inbound
provider envelope's `in_reply_to` references events whose
union taint is not a subset of the canonical re-emit's
taint, synthesise a `core.session.tool_result` deny" — i.e.,
a re-emit-side rejection mirror of §PT1's broker-intake
rejection. Pi-1 B-6 pushed back: the re-emit pipeline runs
*before* the canonical envelope is published, so a
"rejection" there means dropping the provider's request on
the floor without ever giving the operator a chance to
allow/deny via the gate. That semantics conflates two
different failure modes — *intake* (a plugin contradicts
itself; reject the publish) vs *propagation* (the canonical
envelope inherits ancestry; construct it correctly).

The ratified shape (scope round 2+, internal-split rows
8 + 9):

- **Re-emit side (§TR4b)**: *construct the superset*. The
  `ReferencedTaintIndex.lookup_result` (§TR4a) returns the
  union of referenced-event taints; the canonical envelope's
  `taint` field is built as
  `provider_identity ∪ value_match ∪ referenced_union` by
  construction. There is no rejection path; the gate sees
  the union and the audit row
  `tool_request_taint_unioned_from_in_reply_to` (§AL3)
  records the non-redundant union pickup.
- **Intake side (§PT1)**: rejection still lives here. When
  a *plugin* publishes `plugin.<id>.tool_result` with a
  non-empty `taint` claim, the broker compares the claim
  against the *originating tool_request*'s canonical taint
  (cached on `OutstandingDispatch.tool_request_taint`). On
  violation: synthetic deny + audit + lifecycle publish.

The c12 commit (`48229d2`) lands §TR3+§TR4b together as
the value-match + referenced-union arm; c13 (`b81c3a4`)
lands the §TR1 `handle_tool_result` ancestry union (the
read-side that §PT1 then compares against). Together they
close Stream A §7.2.6 rows 1 + 2.

### 2.5 §TM4 `Broker::install_publish_test_hook` test seam (scope round-7 fold)

The TR1/TR3 stale-entry tests need to assert "the
`TaintMatchMap` reflects the canonical `tool_result` *before*
the canonical envelope is fanned out to subscribers" — i.e.,
the record-before-publish ordering. The natural assertion
shape is: install a hook on the broker that fires *after*
the handler records but *before* `fan_out` to subscribers,
inspect the map's state at that point, then resume.

Round-5 scope.md proposed asserting the invariant via the
public subscriber API (subscribe to `core.session.tool_result`,
inspect the map on the receive side). Pi-6 M-1 flagged that
this is racy: the map refresh and the subscriber publish are
both on the broker's internal pump task, but the *test's*
subscriber runs on a different tokio task, so the
"record before publish" check becomes "record before
subscriber observes" which is a weaker invariant (the
broker's internal task could publish-then-fan-out-then-record
in some interleaving and still pass the subscriber-side
check).

Round-7 fold (pi-6 M-1) added §TM4 as a dedicated test
seam. Live shape (verified in `crates/rafaello-core/src/bus.rs`):

- `pub type PublishTestHook = Arc<dyn Fn(&BusEvent) ->
  Option<BrokerError> + Send + Sync>` — the hook receives
  the `BusEvent` (not a `PublishMsg`) and *may inject a
  `BrokerError`* by returning `Some(_)`, returning `None`
  for the pass-through case. The injection arm is what
  lets the TR1/TR3 stale-entry assertions exercise the
  rejection paths without forcing a separate
  fault-injection seam.
- `pub fn install_publish_test_hook(&self, hook:
  PublishTestHook)` — interior-mutable, stores into
  `BrokerInner.publish_test_hook:
  Mutex<Option<PublishTestHook>>`.
- Storage (`publish_test_hook` field on `BrokerInner`) and
  installer (`install_publish_test_hook`) are cfg-gated
  behind `#[cfg(any(test, feature = "test-fixture"))]`.
  Production builds select a no-op
  `check_publish_test_hook` via the corresponding
  `#[cfg(not(any(test, feature = "test-fixture")))]` arm,
  so the production pump pays neither the hook-storage
  allocation nor a dynamic-dispatch call at each
  `check_publish_test_hook` call site.
- Fires inside the broker pump *after* the handler records
  into the re-emit caches but *before* `fan_out` to
  subscribers — the exact ordering point the TR1/TR3 tests
  need. Three `check_publish_test_hook` call sites in
  `bus.rs` (around line 1128 / 1196 / 1234) cover the
  re-emit paths exercised by the tests.
- Last-writer-wins on second install (the `Mutex<Option<_>>`
  replacement); no explicit clear method (install a no-op
  hook to "remove"). Acceptance test
  `broker_publish_test_hook_replaces_on_second_install.rs`
  pins the semantics.

Landed at c08 (`f4b9421`) as the round-7-added internal-
split row 4'. The seam adds **no production code path**:
the cfg-gate fences both the storage field and the dispatch
call. m6 should keep this pattern in mind for any other
"this happens before that" assertion where the ordering is
broker-internal.

### 2.6 c23 deviation: m5a `rfl-openai-stub` single-completion shape (in-flight carveout)

**What happened.** c23 (§EXFIL1) is the headline integration
test driving the verbatim exfil flow end-to-end through the
m5a `rfl-openai-stub` binary. The stub (inherited from m5a
c39 / c32-c36) emits a *single* chat-completion response
per stubbed turn; the test scripts two `tool_calls` in that
one response (`web-fetch` + `send-mail`). The m5b value-
match chain (`TaintMatchMap` records on the fetch
tool_result; the send-mail tool_request's value-walk picks
up the fetch taint; `details.taint` carries the union; the
`confirm_request_taint_attached` audit row fires) requires
the *fetch* tool_result to be observed by the canonical
re-emit pipeline *before* the *send-mail* tool_request
arrives. With the single-completion stub, both tool_calls
land on the bus inside the same provider message and the
canonical synthesis order is fan-out-determined, not turn-
determined — the value-match arm cannot fire end-to-end
because by the time `handle_tool_request` evaluates
send-mail, the fetch result hasn't completed the
`handle_tool_result` → `TaintMatchMap.record` round-trip.

The broker-block end-to-end (the *headline* security
guarantee — "verbatim tool-result-to-sink flow blocked at
the broker") *is* asserted by the integration test: the
operator denies send-mail at the gate, the `mailcat.log`
stays empty, the persisted entries / audit kinds match.
But the value-match → canonical-taint-union → audit-row →
provenance-overlay chain (the *informative* arm of m5b's
work) is not exercised end-to-end via the stub.

**How it was authorised.** Owner ratified option C at commit
`86d6124` (`docs(rafaello-m5b): c23 deviation OWNER-RATIFIED
— proceed with option C`):

- §EXFIL1's *demo test* stays as landed at `9503912`
  (broker-block end-to-end). The test name maps to the
  m5 roadmap row's wording ("verbatim tool-result-to-sink
  flow blocked at the broker"); the security guarantee
  promised by that sentence is proven.
- The value-match / audit-row / provenance coverage closes
  via a **harness-level sibling integration test** that
  drives `ReemitRouter` + `ConfirmationGate` +
  `TaintMatchMap` + `ReferencedTaintIndex` + `AuditWriter`
  directly with a synthetic event sequence that puts the
  fetch tool_result into the map *before* the send-mail
  tool_request evaluates. This is more rigorous than a
  stub-driven test for the same primitives — no LLM round-
  trip variance, deterministic event ordering.
- Routed to retrospective §3 (commit deviations) + §5
  (m6 follow-up: multi-turn stub shape).

**How it landed.** c23b at commit `e533361`
(`test(rafaello-core): §EXFIL1 value-match chain harness
sibling — closes c23 e2e gap (option C)`). The synthetic
sequence: (1) seeds an outstanding fetch dispatch + publishes
a synthesised `plugin.<fetch>.tool_result` whose canonical
re-emit records the exfil payload into `TaintMatchMap` with
`[{tool, <fetch>}]`; (2) publishes a
`provider.<openai>.tool_request` for `send_mail` whose args
contain the exfil strings; (3) asserts the gate's
`core.session.confirm_request` carries the value-match entry
in `details.taint`; (4) asserts the
`confirm_request_taint_attached` audit row is written for
the confirm correlation id.

**m5b equivalent of m5a's c38 acceptance-test substitution.**
This is the same class of deviation pattern: a `commits.md`-
ratified test whose acceptance shape collides with a Phase-3-
discovered subtlety, where the security guarantee promised
by the roadmap row is still met but the *exact* test-file
shape needs adjustment. m5a c38 landed three substitute
tests for four ratified names (m5a retro §3.1); m5b c23
keeps the ratified test and adds a sibling closing the
end-to-end gap. The substitution decision was the
orchestrator's, not the per-commit agent's, and the
deviation was ratified before c23b landed.

### 2.7 §AL1 predicate: non-provider canonical taint

The m5a gate already populates `details.taint =
event.taint.clone().unwrap_or_default()` (m5a §CD1 / live at
`gate/mod.rs:386-402`). m5b's §CD1 is *normalisation +
regression coverage* — pinning the wire shape (`[]` for
empty, `[entries...]` for non-empty) and asserting it via
the c15 (`6bea5ba`) helper extraction +
`details.taint`-regression tests.

The §AL1 audit row + the §CD2 TUI provenance overlay are
gated by the **non-provider predicate**: a canonical
`tool_request`'s taint vector contains at least one entry
whose `source` is *not* `"provider"`. Every m5a / m5b
canonical tool_request carries the provider-identity entry
(`{source: "provider", detail: "<provider_id>"}`) by
construction; the predicate fires only when the value-walk
or the referenced-union *added* something. This is the
"value-driven ancestry beyond the bare provider marker"
shape (scope item 6); §EXFIL3 (c25) is the negative anchor
locking in the no-fire behaviour for a tool_request whose
args happen to be provider-only.

c17 (`f6abfa2`) wires the predicate inside the gate's
`build_confirm_request_payload` path; c16 (`188a779`)
wires the same predicate in the TUI overlay render so the
overlay renders the `provenance:` block only when the audit
row would have fired.

### 2.8 Internal-split row 13 vacate → row 1'' add (`AuditKind` ordering, pi-5 M-1)

Round-5 scope.md / round-5 commits.md placed the
`AuditKind` enum + `as_str()` table extension at internal-
split row 13 — between §CD2 (TUI overlay) and §AL1 (audit
writer). Pi-5 M-1 flagged the ordering: the §AL1 writer
(row 14) consumes the new variant
`confirm_request_taint_attached` and the §PT1 enforcement
(row 10) consumes `plugin_publish_rejected_taint_superset`,
both of which would be unreferenced (`#[allow(unused)]`-
shim territory) on a per-commit green bar if the enum
extension landed at row 13. Round 6 vacated row 13 and
added row 1'' (`AuditKind` extension lands ahead of all
consumers, between row 1' set_audit_writer and row 2
OutstandingDispatch). The commits.md round-6 ratification
landed it as c03 (`50e01b4`), three commits before c04's
unsplittable cutover, six before c14's §PT1 enforcement.
No `#[allow(unused)]` shims needed in any plan-row commit.

---

## 3. What deviated from commits.md

Of the 28 plan rows, **26 landed exactly as written** and
**two landed with documented deviations** (c23 and c24 —
both rooted in the same m5a `rfl-openai-stub` single-
completion limitation; see §3.1 / §3.2). **One additional
commit** (c23b, `e533361`) landed as a Phase-3 closure per
the owner ratification at `86d6124`, raising the Phase-3
implementation total to 29 commits. The c23b sibling closes
the audit-row anchor that *both* c23 and c24 leave
unasserted at the integration-test level.

| Row(s) | Deviation | Rationale | Routed forward to |
|--------|-----------|-----------|-------------------|
| c23 | Headline integration test (§EXFIL1) landed at `9503912` but the value-match / audit-row / provenance-overlay chain cannot fire end-to-end through the m5a `rfl-openai-stub` single-completion shape (§2.6 / §3.1). Owner ratified option C: keep c23 as the broker-block end-to-end demo, add a harness-level sibling closing the coverage gap. | Single-completion stub puts both tool_calls into the canonical pipeline before the first `tool_result` round-trips through `handle_tool_result`. The end-to-end value-match chain needs the fetch result to be observed by the canonical re-emit pipeline before the send-mail tool_request evaluates. Stub-shape change would touch m5a's `rfl-openai-stub` binary and add 1-2 commits — better deferred to m6 alongside `rfl init` / interactive-demo polish. | §5 item 1 (m6 follow-up: multi-turn `rfl-openai-stub` shape); c23b harness sibling closed the coverage gap mechanically at `e533361`. |
| c24 | Allow-arm audit-trail variant (§EXFIL2) landed at `cac7ae5` with an in-commit deviation note (commit body §"Deviation (path 3 per c24 row text)"): the scope §EXFIL2 acceptance bullet cites `confirm_request_taint_attached` as the regression anchor for "the operator inspecting `audit_events` afterward can see the operator allowed a verbatim flow," but the audit row does not fire under the same single-completion-stub fixture c23 uses — the §AL1 non-provider predicate cannot fire for send-mail's modal because no fetch tool_result has been recorded into `TaintMatchMap` by the time send-mail's args reach the broker. c24 lands the **end-to-end allow-arm shape** (entries / mailcat.log / fetch.log / audit-kind sequence) without the audit-row anchor. | Same root cause as c23 (m5a stub single-completion shape). c24's commit body enumerates the same three paths as c23's deviation note and **takes path 3** (accept the integration test without the audit-row anchor; route the anchor to the c23b harness sibling, which proves the §AL1 predicate at modal-open under a synthetic event sequence). c23b covers the audit-row *primitive*; c24 covers the allow-arm *end-to-end shape* (`confirm_allowed` + `mailcat.log` + `entries`). No single test joins all three — the coverage is split across c23b (audit-row primitive) and c24 (allow-arm end-to-end). | §5 item 1 (multi-turn stub shape); audit-row primitive closed by c23b at `e533361`. |

No mid-Phase-3 file renames, no test relocations, no row
reorderings. Two rows (c04, c14) are unsplittable cutovers
declared in `commits.md` round 6 — neither needed further
pi pressure on the size declaration during Phase 3.

### 3.1 c23 deviation in detail (option-C ratification)

The deviation note at `86d6124` lays out the three
alternatives the orchestrator considered:

- **Option A** (accept c23 as-is, no harness test) — leaves
  the value-match end-to-end path uncovered. Unacceptable
  per pi review precedent (the `plans/README.md` "Patterns"
  rule: "two-stage tests are the right way to ladder API-
  surface dependencies"; the m0 / m1 examples cited there).
- **Option B** (extend `rfl-openai-stub` to emit two
  separate chat-completion responses sequenced across two
  HTTP turns) — would touch the m5a stub binary, add 1-2
  commits, and conflate m5b's security-completion work with
  m6's developer-ergonomics polish. The interactive-demo
  recording for `manual-validation.md` §1 needs a multi-turn
  stub shape anyway; better landed there.
- **Option C** (keep c23 as the broker-block end-to-end
  demo, add a harness-level sibling) — c23 retains the
  roadmap-row mapping ("verbatim tool-result-to-sink flow
  blocked at the broker"); c23b drives the value-match
  chain directly with a synthetic event sequence at the
  `ReemitRouter` + `Broker` + `Gate` + `AuditWriter` seam.
  Deterministic, no LLM round-trip variance, stronger
  coverage than a stub-driven end-to-end would have been.

Owner ratified C. c23 landed at `9503912`; the deviation
note landed at `86d6124`; c23b landed at `e533361`.

**Why this is acceptable as a deviation rather than a
round-7 commits.md round.** The headline test *name* and
*roadmap-row mapping* match the ratified commits.md row
exactly; the test *body* covers what the row promised. The
value-match chain is m5b in-scope work (scope items 1-4)
which c23b explicitly closes at the harness seam — same
primitives, deterministic shape. Re-opening commits.md for
this would have rewritten c23 to "scripted-two-turn stub +
harness sibling" — strictly larger than the ratified row,
and orthogonal to the security guarantee the row was meant
to assert. The orchestrator's call (record the deviation,
add the sibling commit, route the multi-turn-stub work to
m6) is the m5a c38 / m4 pattern for deviations whose security
guarantee is still met by the ratified test.

**Recorded for future drivers.** When a `commits.md`-
ratified integration test discovers a Phase-3-only subtlety
that doesn't change the roadmap-row security guarantee but
*does* leave a coverage gap, the option-C pattern (keep the
ratified test + add a harness sibling) is cheaper than
reopening commits.md and stronger than ignoring the gap.

### 3.2 c24 deviation in detail (same-root cause as c23)

c24 (§EXFIL2, `cac7ae5`) is the allow-arm audit-trail
variant: same fixture as c23, same scripted stub response,
but `RFL_TUI_TEST_CONFIRM_ANSWERS = "allow,allow"` instead
of `"allow,deny"`. The operator allows fetch, then allows
the verbatim send-mail; the integration test asserts
`mailcat.log` gains one entry whose `args.to` / `args.body`
capture the verbatim exfil values, `entries` carries both
`tool_call`s + both `tool_result`s with turn-2
`tool_result.ok = true`, `fetch.log` records the turn-1 URL
once, and `audit_events` carries 2× `confirm_request` +
2× `confirm_allowed` rows with zero `confirm_denied`.

The deviation, recorded verbatim in c24's commit body under
`## Deviation (path 3 per c24 row text)`: the scope §EXFIL2
acceptance bullet cites the
`confirm_request_taint_attached` audit row as the
regression anchor for "the operator inspecting
`audit_events` afterward can see they allowed a verbatim
flow." Under the c23 stub fixture
(`exfil-stub-response.json` — single chat completion
carrying both tool_calls) and m5a's `rfl-openai` adapter
(which appends tool_results to history but does not
currently issue a follow-up chat completion), both
`provider.openai.tool_request` events publish before either
`core.session.tool_result` lands. The `TaintMatchMap`
therefore has no fetch tool_result recorded by the time
send-mail's args reach the broker, and the §AL1
non-provider predicate cannot fire for send-mail's modal.

The same three paths from c23's deviation are enumerated in
c24's commit body. Path 1 (extend `rfl-openai` to issue a
follow-up chat completion when in-flight tool_results land)
is non-trivial production work with ripple risk into m5a
tests that script single-turn completions; routed to m6
alongside `rfl init` polish (§5 item 1). Path 2 (synthesise
the bus events in-tree via `ReemitRouter` +
`ConfirmationGate` directly — the rafaello-core unit-test
idiom) is what c23b implements at the harness seam for the
audit-row *primitive*. **Path 3 — accept the integration
test without the audit-row anchor and route the audit-row
anchor to the harness sibling — is what c24 took.** The
commit body's "Deviation (path 3 per c24 row text)" header
states it explicitly.

**What c23b proves vs what c24 proves.** c23b's harness
sequence asserts the §AL1 predicate fires at modal *open*
(the gate writes `confirm_request_taint_attached` when it
builds the confirm-request payload, before any answer
arrives) for a canonical taint vector containing a
non-provider entry. c24's integration test asserts the
allow-arm trajectory — `confirm_allowed` rows × 2,
`mailcat.log` receives the verbatim send-mail entry,
`entries` carries both `tool_call`s + both `tool_result`s
with `ok = true` on the turn-2 row. **No single test
joins the §AL1 audit row to the `confirm_allowed`-followed-
by-mailcat-receive trajectory under one process.** The
coverage is split: c23b proves the primitive; c24 proves
the end-to-end allow-arm shape. The owner ratification at
`86d6124` accepted this split, with the multi-turn-stub
work (which would let a single end-to-end test join them)
routed to m6 per §5 item 1.

**Why the c23 + c24 deviations cluster under one
owner-ratification.** Both rows hit the same single-
completion-stub limitation; c24 was authored after c23's
option-C ratification (`86d6124`) and inlined the
analysis into its commit body rather than spinning out a
second deviation note. The c23b harness sibling
(`e533361`) was scoped to cover the audit-row primitive
that *both* rows leave unasserted at the integration-test
level; no separate c24b sibling was needed.

---

## 4. Sizing signal

### Pi rounds

- `scope.md`: **7** (m5a: 6, m4: 6, m3: 22, m2: 8, m1: 4,
  m0: 3). One round above the m4 / m5a baseline.
- `commits.md`: **6** (m5a: 6, m4: 3, m3: 9, m2: 4, m1: 3,
  m0: 3). Matches m5a's bracket on a narrower row count.

The scope bracket exceeded m4 / m5a by one round because
of two cumulative pressures:

1. **§TR4b construct-the-superset vs synthetic-deny.**
   Round 1's "re-emit-side rejection" framing collided with
   the gate's allow/deny contract (pi-1 B-6). Round 2's
   `ReferencedTaintIndex.by_request_id`-only shape collided
   with the §PT2 closure requirement that the canonical
   `tool_result` *also* records its own taint for downstream
   consumers (pi-2 B-1). The two arms (`by_request_id` +
   `by_result_id`) ratified at round 3.
2. **§TM4 broker publish-test hook (round 7 fold).** Added
   in pi-6 M-1 to keep the TR1/TR3 stale-entry assertions
   honest. The seam adds zero production code (cfg-gated
   both at the storage field and the dispatch call) but
   needed a round to ratify the shape (last-writer-wins,
   no explicit clear, fresh `Broker` per test).

The commits bracket landed at m5a-parity (6 rounds) despite
the narrower row count because the round-by-round folds had
to thread the row-13 vacate → row 1'' add ordering fix
(pi-5 M-1, the `AuditKind` consumer-ordering issue) and the
row 4' add (pi-6 M-1, the §TM4 hook). Both folds preserved
the 28-row total by vacating or compressing other rows.

### Phase-3 walltime

Phase 3 ran roughly one driver day for the 28 plan-row
commits + the c23b deviation sibling (29 commits total).
Per-commit walltimes (orchestrator log spot-checks):

- c04 (`OutstandingDispatch` cutover, medium-to-large):
  ~25 min.
- c14 (§PT1 critical-section cutover, medium-to-large):
  ~32 min.
- c23 (§EXFIL1 headline, large): ~38 min — within the
  driver brief's budget for a large body-justified row.
  The deviation-ratification follow-up (`86d6124` + c23b
  at `e533361`) added ~25 min on top.
- Median across the remaining 25 rows: ~9-12 min,
  matching m5a / m4 per-commit profile.

No disk-full restarts during Phase 3. No `Cargo.lock` ff-
merge aborts (m2 / m4 §4.5 stash mitigation held). No
`.pre-commit-config.yaml` symlink misses (the m2 §4.6
worktree-symlink-at-creation mitigation held across all 29
plan-row commits).

### Mis-budgeted rows

None observed. The 6 small / 5 small-medium / 14 medium /
2 medium-large / 1 large declaration in `commits.md`
§"Sizing summary" landed without agent-side splitting
requests on any of the 28 rows. The c23b deviation sibling
landed at ~150 LoC (the synthetic-sequence harness), within
the medium bucket implied by the option-C ratification
note's framing.

---

## 5. Follow-ups routed to m6 (or later)

| # | Item | Surface | Routed to |
|---|------|---------|-----------|
| 1 | **Multi-turn `rfl-openai-stub` shape** — the m5a stub emits a single chat-completion response per stubbed turn. A two-turn (or N-turn) shape — where the stub emits one `tool_call`, awaits the canonical `tool_result`, then emits the next response — would let a single end-to-end integration test cover the value-match chain (the gap c23b closes at the harness seam). Useful for the m6 interactive-demo recording (`manual-validation.md` §1 pattern) but **not load-bearing for m5b security**. | `crates/rafaello-openai/src/bin/rfl_openai_stub.rs` + an `rfl-openai-stub.scripted-turns` env-var conventions extension | → m6 |
| 2 | **§A9 fallback — `assistant_message` / `rpc_reply` / `confirm_answer` superset narrowing.** Scope §"Out of scope" item 2 + owner-judgment item 9 ratified the v1 narrowing (m5b enforces superset only on the `tool_request ↔ tool_result` flow; Stream A §7.2.6 row 3 = `provider.<id>.assistant_message`, row 4 = `plugin.<a>.rpc_reply`, row 5 = `frontend.<id>.confirm_answer` — all descriptive but unenforced in v1). Default position is "known v1 limitation; v2 candidate." | `reemit/mod.rs::handle_assistant_message`, `handle_rpc_reply`, `handle_confirm_answer` + matching tests | → v2 (NOT m6; m6 has no security primitives per scope §"m5b → m6 boundary") |
| 3 | **Real-network `rafaello-fetch`.** §TF2 ships the file-backed handler via `RFL_FETCH_TEST_BODY_PATH`. A real-network handler (HTTP client, host allowlist, timeout) is post-v1; the `network` sink declaration is the load-bearing fact for m5b's exfil demo. | `crates/rafaello-fetch/src/lib.rs` (real-HTTP arm) | → post-v1 / v2 |
| 4 | **Substring-containment threshold tuning** (scope §A3 / owner-judgment item 5). m5b ships single threshold = 16 bytes. v2 candidate: per-source-class table (e.g., user-source: 8 bytes; tool-source: 16 bytes; provider-source: 24 bytes). | `crates/rafaello-core/src/reemit/taint_match.rs::TaintMatchMap::lookup` | → v2 (data needed: false-positive / false-negative rates from dogfooding) |
| 5 | **TaintMatchMap hard cap (max-entries-per-session).** Scope §"Risks" #2 reserved this for v2. m5b's per-router map is dropped on `ReemitRouter` shutdown; lazy TTL expiry on `record` / `lookup` keeps memory bounded for normal session lengths but pathological scripts could grow the map without bound within the TTL window. | `TaintMatchMap` add bounded-LRU eviction | → v2 |
| 6 | **Aho-Corasick substring scan path.** Scope §"Risks" #3 noted the m5b linear-scan cost is fine for v1 dogfooding. v2 path is `aho-corasick`. Not pulled in m5b. | `TaintMatchMap::lookup` substring arm | → v2 |
| 7 | **Laundered-flow taint** (scope §"Out of scope" item 1). Explicit non-coverage per security RFC §7.2.1; CaMeL v2 territory. Model summarises a tool result, then proposes a sink with the summary — the value-walk does not catch this because the summary's bytes don't match the original tool_result's bytes. | full re-architecture | → v2 / CaMeL territory |
| 8 | **`rfl audit` read CLI** (scope §"Out of scope" item 4). m6 polish. | new CLI subcommand reading `audit_events` | → m6 |
| 9 | **macOS CI green hard gate** (m3 / m4 / m5a carryover ratification gate). | CI run URL in `manual-validation.md` §4 | → driver post-merge sweep |
| 10 | **Interactive `rfl chat` recording for `manual-validation.md` §1** (LiteLLM proxy + `send-mail` walkthrough). m5a §5 item 10 carryover; m5b adds the verbatim-exfil walkthrough on top. | recorded asciinema/transcript | → driver post-merge sweep (m4 §5.3 / m5a §5 item 10 pattern) |
| 11 | **`manual-validation.md` additions** (§8 below enumerates the additions). The file exists as a 9-line §3 wire-shape note landed at c15 (`6bea5ba`); m5b additions extend it with manual-run transcripts, audit-log dumps, and the macOS-CI URL. | the c15 9-line file | → driver post-merge sweep |
| 12 | **`core_tools_list_registered_before_provider_spawn.rs`** — m5a §5 item 14 carryover. Structural guarantee (the supervisor's `CorePluginService::new` runs ahead of the spawn loop by construction); the missing test is a defence-in-depth regression anchor. m5b did not pick this up because it is m5a-territory regression coverage, not m5b-shaped work. | `rafaello-core/tests/` defence-in-depth | → m6 (unchanged) |
| 13 | **Production `#[allow(clippy::result_large_err)]` sweep** — m4 / m5a carryover. m5b's new production code (broker §PT1 enforcement, re-emit `TaintMatchMap`, `ReferencedTaintIndex`) does not introduce new `result_large_err` allows beyond the m4 / m5a baseline. Recorded for completeness; scope unchanged. | workspace-wide error-shape choice | → m6 (deferred per m4 retro §5.5, unchanged scope) |

Items 1, 9-11 are known driver-sweep follow-ups (item 1 is
m5b's c23-deviation-ripple). Items 2 / 4-7 are explicit
out-of-scope-in-m5b v2 candidates ratified at scope.md
§"Out of scope" + §"Architectural choices to ratify"
+ §"Owner-judgment items"; the v2 routing is **not** an
m5b retro decision but a record of the scope-ratified
boundary. Items 3 / 8 / 12-13 are m6-or-later carryovers
that survived m5b without scope creep.

There are **no load-bearing m5b decisions routed forward**.
The §A9 narrowing is the only m5b-scoped surface explicitly
deferred (to v2, not m6), and the scope §"Out of scope"
item 2 + owner-judgment item 9 ratified that disposition
before Phase 3 began.

---

## 6. Stream RFC drift

`git diff rafaello-v0.1..HEAD --name-only | grep streams/`
returns empty: **no `streams/` RFC was modified during m5b
Phase 3.** `git diff rafaello-v0.1..HEAD --name-only | grep
-E '^rafaello/(overview|decisions|glossary)\.md$'` also
returns empty: **no `overview.md` / `decisions.md` /
`glossary.md` patches landed during Phase 3.**

This is the **same shape m5a closed Phase 3 with** (m5a
retro §6 records the same empty-during-Phase-3 grep result;
drift commits land separately on the retro branch before
merge). m5b follows the same pattern: §6 patches land as
separate follow-up commits on this retro branch before
merge to `rafaello-v0.1` (m4 §6 / m5a §6 drift-commit
precedent).

**Planned drift commits** (to land on this retro branch
after retrospective ratification, before merge):

### 6.1 Stream A (security) — value-driven matching + ancestry-union surface

m5b adds load-bearing surface to Stream A's §5 status banner
and closes two rows of the §7.2.6 mandatory-`in_reply_to`
table. Patches:

- **§5 status banner** — extend with an m5b paragraph per
  the m5a precedent: name `TaintMatchMap` + `ReferencedTaintIndex`
  + `OutstandingDispatch.tool_request_taint` + the §PT1
  superset-violation rejection + the three new audit kinds
  + the `BrokerError::TaintSupersetViolated` variant + the
  `Broker::set_audit_writer` interior-mutable plumbing. Cite
  implementing commits (c05-c14).
- **§7.2.1 taint matching algorithm** — Stream A round-1
  ratified the literal-hash + substring-containment shape;
  the live `taint_match.rs` implements it verbatim. Banner
  update naming `siphasher::sip::SipHasher13` with the
  `RFL_TAINT_MATCH_HASH_KEY = (0xc0ffee_d00d_f00d_b002,
  0xa11ce_b0b_face_b00c)` constant pair as the load-bearing
  determinism choice (m5b implementation detail; recorded
  here so v2's `aho-corasick` migration knows what to
  preserve).
- **§7.2.2 taint sources** — banner clarifying that the
  illustrative `{source: "web", detail: "<host>"}` form for
  `web.fetch` results is **not** what live canonical synthesis
  produces. The live form (m4 / m5a / m5b) is `{source:
  "tool", detail: "<canonical>"}` per `handle_tool_result`.
  Recorded in scope.md round 1 as a Stream A drift candidate;
  banner update lands on the drift commit.
- **§7.2.6 row 1 — `plugin.<id>.tool_result` superset
  check.** m5a closed the routed-to-this-plugin half via the
  broker's `outstanding_dispatched` atomic intake check; m5b
  closes the superset half via §PT1 (`b81c3a4` + `75cc375`)
  + §PT2 closure at c13. Banner update referencing both
  halves with implementing commits.
- **§7.2.6 row 2 — `provider.<id>.tool_request` superset.**
  m5b closes by construction via §TR4a + §TR4b (the
  `ReferencedTaintIndex` cache + the construct-the-superset
  re-emit step). Banner update.
- **§7.2.6 row 3 — `provider.<id>.assistant_message`
  superset** — v1 known limitation; v2 candidate. Banner
  records the narrowing rationale (the load-bearing path
  is `tool_request ↔ tool_result`; row 3 is descriptive
  but unenforced in v1).
- **§7.2.6 row 4 — `plugin.<a>.rpc_reply` superset** — v1
  known limitation; v2 candidate. Same v1-narrowing
  banner. (Round-1 draft mistakenly framed row 4 as
  `confirm_reply`; the live RFC table row 4 is the fittings
  RPC-reply arm. pi-1 B-3 / pi-2 B-2 caught the
  misattribution.) `core.session.confirm_reply` is a
  *core-output* topic and lives under the §TR5 reserve
  framing, **not** under §7.2.6 row 4.
- **§7.2.6 row 5 — `frontend.<id>.confirm_answer`
  superset** — v1 known limitation; v2 candidate. Same
  v1-narrowing banner.

All three rows (3, 4, 5) are covered by scope §"Out of
scope" item 2 + owner-judgment item 9 ratification.

### 6.2 `overview.md` patches

- **§4.5 bus event envelopes** — already documents `taint:
  Option<Vec<TaintEntry>>` on `PublishMsg` and `BusEvent`;
  m5b populates the field but does not change the shape. No
  patch needed beyond a one-line banner pointing at the
  m5b implementation (`handle_tool_request` value-walk;
  `handle_tool_result` ancestry-union; §PT1 intake check).
- **§6.6 confirmation protocol** — m5a-banner already
  documents the topic family; m5b extends with the
  `details.taint` value-driven population (m5a populated
  it `[]`-or-clone; m5b unions value-match + referenced
  entries). One-line banner addition.
- **§7 tool dispatch** — banner addition naming the
  `TaintMatchMap` refresh ordering (record-before-publish)
  and the `ReferencedTaintIndex` lookup in `handle_tool_request`.

### 6.3 `glossary.md` patches

- **`Taint`** — current entry says "populated by core,
  never trusted from plugins" which is still correct. m5b
  adds two clarifying lines: (a) value-driven matching via
  the per-router `TaintMatchMap`; (b) ancestry union via
  `ReferencedTaintIndex.lookup_request` /
  `lookup_result`. Authoritative implementation cite:
  `crates/rafaello-core/src/reemit/taint_match.rs` +
  `reemit/referenced_taint_index.rs`.
- **`Audit log`** — m5a entry already cites
  `AuditKind::as_str()` as authoritative. m5b adds the
  three new kinds (`confirm_request_taint_attached`,
  `plugin_publish_rejected_taint_superset`,
  `tool_request_taint_unioned_from_in_reply_to`) to the
  example family list — same authoritative-pointer pattern
  m5a established.

### 6.4 Stream F (manifest) — no drift

The `rafaello-fetch` fixture uses the existing m1 schema
verbatim (`sinks = ["network"]`,
`env.pass = ["RFL_FETCH_TEST_BODY_PATH",
"RFL_FETCH_TEST_LOG_PATH", "RFL_FETCH_TEST_TAINT_OVERRIDE"]`,
the `grant_match` schema). No Stream F changes needed.

### 6.5 Other streams (B / C / E)

Live `streams/` tree contains `a-security`, `b-fittings`,
`c-scripting`, `e-renderer`, `f-manifest` (no `d-*`
directory — TUI work lives under Stream E renderer +
`decisions.md` row 29). m5b's load-bearing additions touch
none of B / C / E:

- **Stream B (fittings)** — **unaffected.** The TUI overlay
  extension (c16 `provenance:` block render) is
  `rafaello-tui`-internal; the multi-answer scripted hook
  (c18 + c19) is test-plumbing-only env var conventions.
  Neither touches the fittings RPC surface
  (`core.tools_list`, `plugin.<id>.rpc_request` /
  `rpc_reply`, the fittings handshake). The §PT1 broker-
  intake superset check fires on bus traffic, not on
  fittings RPC traffic — fittings `rpc_reply` arm is
  explicitly *not* covered by the §A9 / §"Out of scope"
  item 2 narrowing per §6.1 above (descriptive but
  unenforced in v1). No Stream B patch needed; pi-1 M-4
  asked for the explicit confirmation.
- **Stream C (scripting)** — unaffected. m5b does not touch
  the slash-command surface, the `frontend.tui.slash_command`
  / `core.session.command_result` topic pair, or the
  `SlashHandler`. m5a's c38 RwLock migration holds
  unchanged.
- **Stream E (renderer)** — `decisions.md` row 29 already
  pins TUI overlay rendering as m5a-internal; m5b honours
  unchanged. The c16 `provenance:` block is a render-shape
  *content* extension internal to the overlay component;
  the Stream E RFC documents the rendering pipeline and
  the overlay's *existence* but not its internal block
  list, so no patch needed. The
  `RFL_TUI_TEST_CONFIRM_ANSWERS` multi-answer hook
  (§TUI-MA / c18 + c19) is test-only surface; not Stream-
  E-documented.

---

## 7. `decisions.md` additions

m5b lands **nine** load-bearing design choices that warrant
new `decisions.md` rows. Each row sketch below carries an
explicit `Refines/Reverses` anchor in the decision-table
style (m5a precedent). An editor commit during the retro-
branch sweep adds these as **expected rows 50-58** (after
m5a's 46-49; the exact row numbers land at editor-commit
time pending no intervening `decisions.md` appends)
to `decisions.md` proper.

### 7.1 Row candidate: Taint matching algorithm — literal hash + substring containment + `RFL_TAINT_MATCH_HASH_KEY`

**Refines/Reverses.** No prior `decisions.md` row anchor —
the matching primitive is net-new in m5b. Anchored instead
in scope §TM1 + §TM2 + §A3 (the round-1 introduction; the
substring threshold; the hash-key constant) and Stream A
§7.2.1 (the design source). Reverses nothing.

**Choice.** The per-router `TaintMatchMap` exposes two lookup
arms: (a) literal hash via `siphasher::sip::SipHasher13`
keyed by the fixed constant pair `RFL_TAINT_MATCH_HASH_KEY =
(0xc0ffee_d00d_f00d_b002, 0xa11ce_b0b_face_b00c)`; (b)
substring containment over a 16-byte minimum threshold. The
fixed hash key is required so process restarts produce
identical hashes within the same `rfl chat` session
boundary and so test reproducibility holds; the map is
in-process only and never persisted, so cross-session
determinism is the only consumer.

**Rationale.** Default `DefaultHasher` randomises per-process,
which would break determinism in test suites that script
event sequences. The substring threshold is single-valued
(16 bytes) per scope §A3 / owner-judgment item 5; per-class
tables are v2 territory pending dogfooding signal.

### 7.2 Row candidate: Plugin-supplied taint discard policy + superset check as additional rejection signal

**Refines/Reverses.** Refines `decisions.md` row 7 (canonical
taint is core-supplied; plugins do not contribute) by
pinning the precise interpretation: a plugin *may* include
a `taint` field on `plugin.<id>.tool_result`, but the field
is (a) discarded at canonical synthesis (row 7 unchanged);
(b) checked against the originating tool_request's canonical
taint at intake — a *contradiction* (plugin claims fewer
entries than the canonical superset) triggers rejection via
§PT1.

**Choice.** Plugin-supplied taint is discarded at canonical
synthesis (m4 / Stream A §7.2.2 / `decisions.md` row 7 —
unchanged). m5b adds an **additional rejection signal**:
before the discard, the broker verifies the plugin's
`taint` claim is a *superset* of the canonical
tool_request's taint (cached on
`OutstandingDispatch.tool_request_taint`). On violation:
audit `plugin_publish_rejected_taint_superset` + publish
`core.lifecycle.publish_rejected` with `code =
"taint_superset_violated"` + synthesise a deny-shaped
`core.session.tool_result`. After the check, the plugin's
field is discarded as before.

**Rationale.** A plugin that *narrows* taint relative to the
canonical ancestry is making a self-contradicting claim —
either lying about its ancestry or buggy. Rejecting the
publish is defence-in-depth: even though the canonical
synthesis would discard the field anyway, the rejection
signal lets the operator see the contradiction at audit-
log time rather than silently dropping a contradictory
claim. §PT1 is the single rejection site; §TR4b is
*construct-the-superset* with no rejection (the canonical
envelope is computed correctly by construction).

### 7.3 Row candidate: TTL on per-router value→taint map + `ReferencedTaintIndex` — 5 minutes, lazy expiry, shared

**Refines/Reverses.** No prior anchor — net-new in m5b.
Anchored in scope §A4 + owner-judgment item 4.

**Choice.** Both `TaintMatchMap` and `ReferencedTaintIndex`
use a default TTL of **5 minutes**, expired lazily on
`record` / `lookup`. No background sweep task. The two
indexes share the TTL value (single `Duration` constant
per `ReemitRouter`).

**Rationale.** Lazy expiry keeps the modules dep-free (no
tokio-task ownership at the cache layer); the symmetry
between the two indexes keeps the per-router resource shape
predictable. 5 minutes is the m5b default per scope §A4;
owner may push smaller or background-sweep at v2 if
dogfooding surfaces a need. The per-router scoping (not
per-session) means cache entries survive across confirm
modals within a single `rfl chat` process but are dropped
on shutdown.

### 7.4 Row candidate: `BrokerError::TaintSupersetViolated` distinct variant

**Refines/Reverses.** No prior anchor — net-new. Anchored
in scope §A1 + owner-judgment item 6.

**Choice.** `BrokerError::TaintSupersetViolated { publisher,
topic, missing: Vec<TaintEntry> }` is a distinct
`BrokerError` variant rather than an arm under an existing
variant (e.g., `TaintReason`).

**Rationale.** The superset violation is a content-level
contradiction (the plugin's published `taint` field claims
fewer entries than the canonical), not a structural
malformation of the `taint` field (which `TaintReason`
covers — invalid `source`, missing `detail`, etc.). The
shape mirrors `BrokerError::StaleRequestId` being its own
variant rather than an `InReplyToReason` arm: distinct
failure modes get distinct variants so the audit table's
`reason` column can distinguish them without parsing
nested detail strings.

### 7.5 Row candidate: §TR4b construct-the-superset — no re-emit-side rejection

**Refines/Reverses.** No prior anchor — net-new. Anchored
in scope §A11 + owner-judgment item 11 + pi-1 B-6.

**Choice.** The re-emit pipeline's handling of
`provider.<id>.tool_request` with `in_reply_to` references
*constructs the superset* of referenced-event taints into
the canonical envelope; it **never rejects on the re-emit
side**. The synthetic-deny path lives only at §PT1
(broker-intake side, where a *plugin claim* can be
contradicted). Alternative (re-emit-side synthetic deny if
the provider's `in_reply_to` declares ancestry beyond what
the value-walk catches) was considered and rejected.

**Rationale.** Re-emit runs *before* the canonical envelope
is published to subscribers, including the gate. A
"rejection" at re-emit would drop the provider's request on
the floor without ever giving the operator a chance to
allow/deny via the modal — conflating *propagation*
(construct the canonical envelope correctly) with *intake*
(reject contradictory publishes). The asymmetry between
re-emit (always-construct) and intake (can-reject) keeps the
two concerns separate: the canonical envelope reflects the
union of all known ancestry; the broker rejects only when a
plugin's claim is internally inconsistent with what core
already knows.

### 7.6 Row candidate: Broker audit plumbing — `Mutex<Option<Arc<AuditWriter>>>` + `set_audit_writer`

**Refines/Reverses.** No prior anchor — net-new. Anchored
in scope §A2 + owner-judgment item 10 + pi-2 B-2 + pi-3
B-2.

**Choice.** `BrokerInner.audit:
parking_lot::Mutex<Option<Arc<AuditWriter>>>` is interior-
mutable, default `None`, replaced at most once via
`Broker::set_audit_writer(&self, Arc<AuditWriter>)`. `rfl
chat` calls `set_audit_writer` between `Broker::new` and
the first plugin spawn. Consumers (`handle_plugin_publish`,
`gate::build_confirm_request_payload`, re-emit handlers)
read `audit.lock().as_ref().cloned()` and treat `None` as
"no audit configured."

**Rationale.** The `AuditWriter` is constructed from the
resolved `SessionStore` path, which resolves after
`Broker::new` in `rfl chat`'s startup order. Threading the
audit writer through `Broker::new` would force an unrelated
reorder of `rfl chat`'s construction. Interior mutability
with a `set_audit_writer` seam keeps the existing
construction order and lets tests opt in to audit coverage
without forcing every test to wire an `AuditWriter`.

### 7.7 Row candidate: Multi-answer TUI scripted hook — `RFL_TUI_TEST_CONFIRM_ANSWERS` comma-list

**Refines/Reverses.** Refines the m4 / m5a-introduced
`RFL_TUI_TEST_CONFIRM_ANSWER` single-answer hook (scope §A12
+ owner-judgment item 12).

**Choice.** New env var `RFL_TUI_TEST_CONFIRM_ANSWERS`
carries a comma-separated list of answers consumed one-per-
confirm-modal in order. Mutually exclusive with the
singular hook (setting both is a TUI startup error);
exhaustion (more modals than scripted answers) is a hard
panic that fails the test deterministically.

**Rationale.** §EXFIL1's scripted flow requires two distinct
answers (allow for fetch, deny for send-mail) within a
single `rfl chat` process. The singular hook (one answer
applied to every modal) cannot drive this. Comma-separated
list matches the parser shape of existing rfl envs
(`network.allow_hosts`). Mutual exclusion with the singular
hook keeps m5a tests untouched. Deterministic panic on
exhaustion makes the test failure mode obvious — a silent
fall-through to "no scripted answer" would block the test
on a modal that no key press resolves.

### 7.8 Row candidate: Canonical `tool_result` ancestry — tool-source ∪ referenced-tool_request-taint

**Refines/Reverses.** Refines `decisions.md` row 7
(canonical taint is core-supplied) by pinning the precise
ancestry composition: `handle_tool_result` synthesises
canonical `core.session.tool_result.taint` as the union of
the m5a `[{source: "tool", detail: "<canonical>"}]` entry
*and* the taint of the `core.session.tool_request` event
the result cites in `in_reply_to`. Closes Stream A §7.2.6
row 1 (the superset half). Anchored in scope §A8 + owner-
judgment item 1 + pi-1 B-5.

**Choice.** Canonical `tool_result.taint` = tool-source ∪
referenced-tool_request-taint. The
`ReferencedTaintIndex.lookup_request(request_id)` returns
the cached canonical `tool_request.taint`; the union is
computed pre-publish by `handle_tool_result` and recorded
into `by_result_id` before fan-out. The alternative
(record deliberate Stream A drift; v1 canonical
tool_results are fresh tool-origin sources only) was
considered and rejected — it would have left §PT1's claim
narrowed from "prevents stripping" to "rejects self-
contradictory plugin claims before discard," weakening
the inheritance guarantee.

**Rationale.** Without the union, a plugin could publish a
tool_result whose canonical envelope drops the originating
tool_request's ancestry — undermining the value-match chain
for any downstream sink the result feeds into. The union
preserves the full ancestry chain on the canonical
envelope, which is what the value-walk reads on subsequent
`tool_request` syntheses.

### 7.9 Row candidate: §AL1 predicate — non-provider canonical taint

**Refines/Reverses.** No prior anchor — net-new. Anchored
in scope item 6 + §AL1 + pi-2 (the round-2 introduction of
the predicate framing).

**Choice.** The `confirm_request_taint_attached` audit row
and the TUI overlay's `provenance:` block fire only when
the canonical `tool_request.taint` vector contains at
least one entry whose `source` is not `"provider"`.
Provider-only taint (`[{source: "provider", detail:
"<provider_id>"}]` — every canonical tool_request carries
it) does not trigger the audit row or the overlay block.

**Rationale.** The provider-identity entry is structurally
present on every canonical tool_request and carries no
information beyond "this came from <provider>." Surfacing
it in the audit row or overlay would generate spam for
every modal. The predicate ("ancestry beyond the bare
provider marker") matches the operator's mental model:
"why does this prompt say something different from the
last one?"

---

## 8. Coverage report

### What's tested

- **All scope §"In scope" items 1-11** — landed across c01-c25:
  - **Taint matching primitive** (§TM1-§TM4, item 1):
    c05 (literal-hash arm + module skeleton), c06
    (substring arm + bounded walk), c07 (`with_taint_match_map`
    builder), c08 (broker publish-test hook).
  - **Re-emit propagation** (§TR1-§TR4b, items 2-4):
    c09 (`ReferencedTaintIndex` cache), c10
    (`handle_tool_result` + `handle_user_message`
    refresh), c11 (`handle_tool_request` records canonical
    request taint), c12 (`handle_tool_request` value-walk
    + referenced-union + §AL3 audit row), c13
    (`handle_tool_result` ancestry union + §PT2 closure
    via `by_result_id`).
  - **Plugin-supplied taint superset enforcement** (§PT1,
    item 5): c14 (unsplittable cutover — check + drain +
    synthetic-deny + audit + lifecycle).
  - **Confirmation prompt `details.taint`** (§CD1 +
    §CD2, item 6): c15 (helper extraction + regression
    tests), c16 (TUI overlay `provenance:` block render).
  - **Audit-log enrichment** (§AL1-§AL3, item 7): c03
    (enum + table extension landing ahead of all
    consumers), c12 (§AL3 consumer), c14 (§AL2
    consumer), c17 (§AL1 writer + non-provider
    predicate).
  - **`rafaello-fetch` fixture** (§TF1-§TF3, item 8):
    c20 (scaffold + manifest), c21 (file-backed handler
    + bus-client bin + fixture env vars), c22 (five-
    plugin lock chaining `rfl-openai` + `rafaello-mailcat`
    + `rafaello-fetch` + `rafaello-readfile` +
    `rafaello-mockprovider`).
  - **Multi-answer TUI hook** (§TUI-MA1 + §TUI-MA2,
    item 9): c18 (parser + queue + exhaustion panic +
    mutual-exclusion error), c19 (rfl env allowlist +
    passthrough test).
  - **Verbatim exfil demo** (§EXFIL1-§EXFIL3, item 10):
    c23 (headline integration test — broker-block end-
    to-end), c24 (§EXFIL2 allow-arm audit-trail
    variant), c25 (§EXFIL3 provider-only-taint
    negative). **§EXFIL1 value-match-chain end-to-end
    closed at the harness seam by c23b** (`e533361`,
    deviation per option C — §3.1).
  - **c38 acceptance-test follow-ups** (§C38a-§C38c,
    item 11): c26 (five-tree spawn + clean shutdown),
    c27 (inactive-provider re-emit ignored), c28
    (positive gate-through-orchestration).

- **All scope §"Demo bar" negative 4** rows green: §EXFIL1
  via c23, §EXFIL2 via c24, §EXFIL3 via c25, plus the
  c23b harness sibling closing the c23 end-to-end gap.

- **All m5a §5 follow-up items 12, 13, 15** (the c38
  acceptance-test carryovers) — closed by c26 / c27 /
  c28 respectively. Item 14 remains routed to m6 by
  m5a's own §5 routing — m5b did not pick it up
  (§5 item 12 above).

- **Stream A §7.2.6 row 1** — closed in full. m5a closed
  the routed-to-this-plugin half via
  `outstanding_dispatched`; m5b closes the superset half
  via §PT1 (c14) + §PT2 (c13).

- **Stream A §7.2.6 row 2** — closed by construction via
  §TR4a (c09) + §TR4b (c12).

- **`Broker::set_audit_writer` ordering** —
  `rfl_chat_sets_audit_writer_before_first_plugin_spawn.rs`
  (c02) asserts the order via a supervisor-internal seam.

### What's not tested (load-bearing follow-ups)

- **Stream A §7.2.6 rows 3 / 4 / 5** — row 3
  (`provider.<id>.assistant_message`), row 4
  (`plugin.<a>.rpc_reply`), row 5
  (`frontend.<id>.confirm_answer`) superset narrowing —
  scope §"Out of scope" item 2 ratifies as v1 known
  limitation. §5 item 2 above.
- **Laundered-flow taint** — scope §"Out of scope" item
  1. §5 item 7.
- **Real-network `rafaello-fetch`** — §5 item 3. The
  fixture's `network` sink declaration is load-bearing;
  the actual HTTP arm is post-v1.

### What's not tested (manual-validation surface)

- **macOS CI green** — §5 item 9.
- **Interactive `rfl chat` recording** — §5 item 10. m5a's
  pattern (mechanical-green-as-substitute, or recorded
  asciinema if owner accepts manual-validation walltime) is
  the m5b default.
- **`manual-validation.md` additions** — §5 item 11;
  enumerated below.

### `manual-validation.md` additions the existing 9-line file needs

The live `manual-validation.md` (landed at c15 / `6bea5ba`)
contains one §3 "Wire shapes" note pinning the
`details.taint` `Vec<TaintEntry>` JSON rendering (`[]` for
empty, never `null`; §CD1 / §CD3). To close before merge,
the file needs the following additions (m4 §5.3 / m5a §5.3
pattern — manual-run transcripts as new sections appended
to the existing §3):

The six ratified scope §"Manual validation" bullets (verbatim
from `scope.md` round 7, listed in scope order; the existing
§3 wire-shape note stays as-is and the additions append as
§4 onward):

1. **Verbatim-exfil walkthrough** against the m5b fixture
   with the file-backed `rafaello-fetch` (via
   `RFL_FETCH_TEST_BODY_PATH`); demonstrate the
   `provenance:` block rendering on the send-mail modal,
   operator denies, `mailcat.log` confirmed empty,
   `audit_events` carries the
   `confirm_request_taint_attached` row for the send-mail
   correlation id (the c23 deny-arm trajectory recorded
   manually).
2. **Allow-arm audit trail** — same fixture, operator
   allows both modals; `mailcat.log` receives the verbatim
   send-mail entry; `audit_events` carries the
   `confirm_request_taint_attached` row containing the
   fetch `{source: "tool", detail: "<rafaello-fetch
   canonical>"}` entry alongside `confirm_allowed` rows
   for both modals (the c24 allow-arm trajectory recorded
   manually).
3. **Overlay rendering plus terminal-clipping/ellipsis** —
   the c16 `provenance:` block render exercised at multiple
   terminal widths; capture a screenshot or text dump at
   80×24 demonstrating the ellipsis behaviour for long
   taint vectors (scope §"Risks" #7 ratifies the audit-row
   carries the full vector; the overlay clips).
4. **macOS CI URL** — the run URL after branch push (m3 /
   m4 / m5a carryover hard gate).
5. **Audit-log inspection** — dump `audit_events` from
   `<project_root>/.rafaello/state/session.sqlite` (m5a
   §2.4 pinned the path); assert the three new m5b kinds
   surface alongside m5a's `confirm_request` /
   `confirm_allowed` / `confirm_denied` etc.
6. **No-match / provider-only path** — drive a turn whose
   `tool_request` args don't match any prior tool_result
   substring; the modal fires with provider-only canonical
   taint; observe **no** `confirm_request_taint_attached`
   row in `audit_events` (the §AL1 predicate fails for
   provider-only taint; §EXFIL3 / c25 is the mechanical
   anchor).

If a LiteLLM-proxy-driven run is included alongside the
above, label it explicitly **"Real-provider walkthrough
(file-backed fetch via `RFL_FETCH_TEST_BODY_PATH`)"** — the
provider is real (LiteLLM proxy) but the fetch is still
file-backed per scope §A6 / owner-judgment item 3. No
real-network claim. (Round-1 / round-2 drafts framed bullet
1 as "Real-network demo" — pi-1 M-6 / pi-2 B-1 caught the
misframing.)

**Extras (not scope bullets):** the §PT1 violation demo —
drive a plugin that publishes `plugin.<id>.tool_result`
with a deliberately narrowed `taint` claim, observe the
`core.lifecycle.publish_rejected` emission with
`code = "taint_superset_violated"`, the synthetic-deny
`core.session.tool_result`, and the
`plugin_publish_rejected_taint_superset` audit row — is a
useful integration check but is **not** one of the six
ratified scope §"Manual validation" bullets. Recorded here
as extra-not-scoped; owner may include or skip at the
post-merge sweep.

Acceptable substitute coverage (m4 retro §5.3 / m5a §8
precedent): the mechanical green on c23 + c24 + c25 +
c23b suffices for bullets 1 + 2 + 6 if owner accepts
mechanical-green-as-substitute. Default expectation per
m5a / m4 pattern is a recorded run.

---

## 9. Inheritance — what m6 inherits

m6 is the final v1 polish milestone: `rfl init` materialising
the lock, documentation pass (`rafaello/README.md`,
`CONTRIBUTING.md`), Homebrew formula, `rfl audit` read CLI.
Per scope §"m5b → m6 boundary", **no further security
primitives in m6**. m6 inherits m5b's full security surface:

- **Taint matching primitive** — `crates/rafaello-core/src/reemit/taint_match.rs`
  including the `RFL_TAINT_MATCH_HASH_KEY` constant, the
  literal-hash + substring-containment arms, the bounded
  value-walk, the lazy-TTL `record` / `lookup` shape, and
  the per-router ownership via
  `ReemitRouter::with_taint_match_map`.
- **`ReferencedTaintIndex`** — `crates/rafaello-core/src/reemit/referenced_taint_index.rs`
  with both arms (`by_request_id` populated by
  `handle_tool_request`; `by_result_id` populated by
  `handle_tool_result`), the `record_request` /
  `record_result` / `lookup_request` / `lookup_result` /
  `clear` API, and the unknown-id fail-open semantics
  (owner-judgment item 8).
- **`OutstandingDispatch.tool_request_taint` field** —
  c04's cutover; the field is the §PT1 superset-check
  ground truth.
- **§PT1 broker-intake enforcement** — `handle_plugin_publish`
  superset check critical section; the
  `BrokerError::TaintSupersetViolated` variant; the
  `core.lifecycle.publish_rejected` emission with
  `code = "taint_superset_violated"`; the synthetic-deny
  `core.session.tool_result` shape; the
  `plugin_publish_rejected_taint_superset` audit kind.
- **Construct-the-superset re-emit semantics** — §TR4b's
  `handle_tool_request` value-walk + referenced-union; the
  §AL3 `tool_request_taint_unioned_from_in_reply_to` audit
  row.
- **Canonical `tool_result` ancestry union** —
  `handle_tool_result` synthesises `taint = tool-source ∪
  referenced-tool_request-taint`; the §PT2 closure via
  `ReferencedTaintIndex.by_result_id` pre-publish.
- **Gate `details.taint` normalised wire shape** — c15's
  regression coverage pins `[]` for empty / `[entries...]`
  for non-empty; the helper extraction
  `build_confirm_request_payload` is the canonical
  publish-shape source.
- **TUI overlay `provenance:` block** — c16's render arm
  fires on non-provider canonical taint; gated by the §AL1
  predicate.
- **§AL1 audit row** — `confirm_request_taint_attached`
  writer in `gate/mod.rs`; fires on the same predicate
  as the overlay block.
- **`AuditKind` enum + `as_str()` table** — extended with
  the three new variants; lands ahead of all consumers in
  c03 so consumer rows compile clean per-commit.
- **`Broker::set_audit_writer` interior-mutable plumbing**
  — `BrokerInner.audit: Mutex<Option<Arc<AuditWriter>>>`;
  `rfl chat` calls `set_audit_writer` between `Broker::new`
  and the first plugin spawn.
- **`Broker::install_publish_test_hook`** — cfg-gated
  test seam for record-before-publish ordering assertions;
  last-writer-wins semantics. Storage + installer are
  cfg-gated; production selects a no-op checker via
  `#[cfg(not(any(test, feature = "test-fixture")))]` and
  pays no hook-storage allocation or dynamic dispatch.
- **Multi-answer TUI scripted hook** —
  `RFL_TUI_TEST_CONFIRM_ANSWERS` comma-list; mutual
  exclusion with the singular hook; deterministic panic
  on exhaustion.
- **`rafaello-fetch` sink-declaring fixture** —
  `rafaello/fixtures/m5b-locks/rafaello-fetch/` with
  `sinks = ["network"]`; file-backed handler via
  `RFL_FETCH_TEST_BODY_PATH` / `RFL_FETCH_TEST_LOG_PATH` /
  `RFL_FETCH_TEST_TAINT_OVERRIDE`.
- **Five-plugin m5b lock** —
  `rafaello/fixtures/m5b-locks/rafaello.lock` chaining
  `rafaello-openai` + `rafaello-mailcat` + `rafaello-fetch`
  + `rafaello-readfile` + `rafaello-mockprovider`. m6's
  `rfl init` materialiser should use this lock shape as
  one of its reference templates.

m6's `rfl init` work consumes the m5b lock shape; m6's
`rfl audit` read CLI consumes the `audit_events` SQLite
table that m5a + m5b have collectively extended with eleven
audit-kind families (m5a §6.4 + m5b's three additions); m6's
documentation pass should reference the value-driven
matching layer (`Taint` glossary entry updated per §6.3
drift). No m6 work should require re-opening m5b's security
primitives — the v1 security story is complete.

---

## 10. Owner-judgment items still standing

The twelve items scope.md §"Owner-judgment items" surfaced
for explicit owner sign-off at scope-round ratification.
Status:

### 10.1 Canonical `tool_result` ancestry policy (item 1, §A8)

**Status: honoured.** Default (tool-source ∪ referenced-
tool_request-taint) ratified at scope round 1; implemented
at c13 (`b81c3a4`). Closes Stream A §7.2.6 row 1 in full.

**Owner re-look before merge:** confirm c13's union shape
matches the owner's mental model. Specifically: the
`rafaello-core/tests/m5b_value_match_exfil_chain_fires_harness.rs`
harness sibling (c23b) and the c13 unit tests both encode
the union semantics; either failing under m6 polish would
signal drift.

### 10.2 §EXFIL2 allow-arm audit-trail variant inclusion (item 2, §A5)

**Status: honoured with deviation.** Default (include)
ratified at scope round 1; landed as c24 (`cac7ae5`) with
the path-3 deviation documented in §3.2 — the §EXFIL2
end-to-end shape (entries / mailcat.log / fetch.log /
audit-kind sequence) is asserted against the same m5b lock
as c23, but the `confirm_request_taint_attached` regression
anchor cited in the scope §EXFIL2 acceptance bullet does
not fire under the c23/c24 shared single-completion-stub
fixture. c24 took option-C path 3 (§3.2): accept the
integration test without the audit-row anchor; route the
audit-row primitive to the c23b harness sibling
(`e533361`), which proves the §AL1 predicate at modal-open
under a synthetic event sequence. No single test joins the
§AL1 row to the allow-arm `confirm_allowed` + mailcat-
receive trajectory under one process.

**Owner re-look before merge:** confirm that "audit-row
anchor covered at the harness seam rather than at the
integration-test seam" is acceptable as the §EXFIL2
acceptance shape. Reopening would require the m6 multi-
turn-stub work to land in m5b instead (§5 item 1) — out of
scope per the m5b → m6 boundary.

### 10.3 `rafaello-fetch` semantics — file-backed (item 3, §A6)

**Status: honoured.** Default (file-backed via
`RFL_FETCH_TEST_BODY_PATH`) ratified at scope round 1;
landed as c21 (`4d43269`). Real-network arm routed to
post-v1 per §5 item 3. Manual validation §1 uses the
file-backed path per pi-1 M-6.

### 10.4 TTL on per-router value→taint map + `ReferencedTaintIndex` (item 4, §A4)

**Status: honoured.** Default (5 minutes, lazy expiry,
shared TTL) ratified at scope round 1; implemented at c05
+ c06 (`TaintMatchMap`) + c09 (`ReferencedTaintIndex`).
Per §7.3 the default lands as a `decisions.md` row in
the drift commit. Owner may revisit at v2 if dogfooding
surfaces a smaller-TTL or background-sweep need.

### 10.5 Substring-containment minimum threshold (item 5, §A3)

**Status: honoured.** Default (16 bytes, single-valued)
ratified at scope round 1; implemented at c06. Per-source
table reserved for v2 (§5 item 4).

**Owner re-look before merge:** the 16-byte threshold is
the value the §EXFIL1 / §EXFIL2 / §EXFIL3 tests assume; a
post-merge tuning round would re-validate. No m5b agent-
side pressure to change the value.

### 10.6 `BrokerError` variant vs `TaintReason` extension (item 6, §A1)

**Status: honoured.** Default (new `TaintSupersetViolated`
variant) ratified at scope round 1; implemented at c01
(`e10f9e8`). Mirrors `StaleRequestId` shape.

### 10.7 Audit-row split — two rows joined on `request_id` (item 7, §A7)

**Status: honoured.** Default (two rows: m5a's
`confirm_request` keeps its shape; m5b adds
`confirm_request_taint_attached` joined on `request_id`)
ratified at scope round 1; implemented at c17 (`f6abfa2`).
m5a-era audit-query shape preserved.

### 10.8 `ReferencedTaintIndex` unknown-id semantics — fail-open (item 8, §A10)

**Status: honoured.** Default (fail-open — unknown id at
lookup returns `None`) ratified at scope round 2 (pi-2
M-3 ripple); implemented at c09 (`1cd274f`). A long-ago
reference whose entry expired returns `None` and the
canonical envelope falls back to the value-walk-only
union. Fabricated ids are upstream-rejected and not in
scope for this choice.

**Owner re-look before merge:** the fail-open semantics
mean a sufficiently-long-running `rfl chat` session could
have entries TTL-expire between the originating
tool_request and a much-later tool_result referencing it.
The §7.3 5-minute default is well above realistic
turn-to-turn latencies; v2 may revisit with dogfooding
data.

### 10.9 `assistant_message` / `confirm_*` superset narrowing (item 9, §A9)

**Status: honoured (narrowing accepted).** Default (accept
as v1 known limitation; v2 candidate) ratified at scope
round 2 (pi-1 M-1 + pi-2 M-2). Stream A §7.2.6 row 3
(`provider.<id>.assistant_message`), row 4
(`plugin.<a>.rpc_reply`), row 5
(`frontend.<id>.confirm_answer`) are descriptive but
unenforced in v1. The +2-4 commit reserve in `commits.md`
§"Internal split" was not consumed; the 28-commit total
holds. Routed to v2 per §5 item 2.

**Owner re-look before merge:** confirm the v1-known-
limitation framing is acceptable. The Stream A drift
patch (§6.1) records the rationale (load-bearing path is
`tool_request ↔ tool_result`); rows 3 + 4 + 5 are
descriptive but unenforced. Reopening would add ~4
commits + ~6 tests
in a v2 milestone.

### 10.10 Map / cache / outstanding-taint / audit-writer location split (item 10, §A2)

**Status: honoured.** Default (map + cache in
`ReemitRouter`; outstanding-taint + audit writer in
`Broker`) ratified at scope round 2 (pi-2 B-2 confirmation).
Implemented at c02 (`Broker::set_audit_writer` plumbing)
+ c04 (`OutstandingDispatch.tool_request_taint`) + c05-c09
(re-emit cache surface). Split-by-responsibility shape
holds.

### 10.11 §TR4b construct-the-superset vs synthetic-deny (item 11, §A11)

**Status: honoured.** Default (construct the superset; no
re-emit-side rejection) ratified at scope round 2 (pi-1
B-6 fold). Implemented at c12 (`48229d2`). Synthetic-deny
path is §PT1-only at c14.

### 10.12 Multi-answer hook env-var format (item 12, §A12)

**Status: honoured.** Default
(`RFL_TUI_TEST_CONFIRM_ANSWERS` comma-list; mutually
exclusive with singular hook; deterministic panic on
exhaustion) ratified at scope round 1; implemented at c18
(`a97da60`). The c18 acceptance tests pin parser symmetry,
mutual exclusion error, exhaustion panic, and the queue
dequeue semantics.

---

*End of m5b retrospective round 3. Folds pi-2's 2 blockers
/ 2 majors / 2 nits per the inline fix list at the top of
the file. Pi-3 review expected per `plans/README.md`
"Retrospective drafts deserve the same adversarial review
as scope and commits" rule + the m1 / m5a precedent (m1
needed 4 rounds; m5a needed 6 — though m5a's bracket
reflects the drift-commit ratification trajectory rather
than pi-finding density). Drift commits (§6) + the
`decisions.md` row appends (§7) land in a separate
follow-up commit on this retro branch **after retrospective
ratification**, per the m5a `816b273` precedent.*
