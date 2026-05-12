# m5b retrospective.md — pi review round 1

Reviewed `retrospective.md` at `fa179a1` against ratified `scope.md`
(round 7, `50c5ae8`), ratified `commits.md` (round 6, `b301a39`), the
c23 deviation ratification (`86d6124`), the c23b sibling test (`e533361`),
the live `rafaello-v0.1..HEAD` history/source, m5a retrospective-review
precedent, and `plans/README.md`'s retrospective-review rule.

## Blocking findings

### B1. §3/§8/§10 omit the real c24 §EXFIL2 deviation and over-mark the allow-arm audit trail as green

The retrospective records only one Phase-3 deviation: c23 plus the owner-
ratified c23b sibling. That is incomplete. c24 itself landed with an explicit
commit-message and test-file deviation: the allow-arm test does **not** assert
the `confirm_request_taint_attached` audit row that ratified `commits.md` c24
made the regression anchor.

Ratified c24 says the allow-arm test must assert this audit shape:

- two `confirm_request` rows;
- `confirm_request_taint_attached` for the mail/send-mail turn;
- the `confirm_request_taint_attached` payload reconstructs the
  rafaello-fetch provenance vector;
- then `confirm_allowed` for the mail turn.

Live c24 (`cac7ae5`) says the opposite in its commit body:

> Deviation (path 3 per c24 row text) — `confirm_request_taint_attached` not asserted

and the test file
`crates/rafaello/tests/rfl_chat_verbatim_exfil_audit_trail_visible_when_allowed.rs`
repeats that the row is omitted because the single-completion stub publishes
both tool calls before the fetch `tool_result` populates `TaintMatchMap`.
The actual c24 assertions are mailcat log, entries table, fetch log,
2× `confirm_request`, 2× `confirm_allowed`, and zero `confirm_denied`.

This affects multiple retrospective sections:

- §1 calls §EXFIL2 an "allow-arm audit-trail variant" and lists it as green
  without caveat.
- §3 says the only deviation is c23/c23b and that 28 rows landed exactly as
  written.
- §5 item 1 routes only the c23 multi-turn-stub gap; it should also say this
  is the same missing end-to-end shape that weakened c24's allow-arm audit
  trail.
- §8 says all §EXFIL1-§EXFIL3 coverage is green and does not list the c24
  audit-row omission under gaps.
- §10.2 says owner-judgment item 2 (§EXFIL2 inclusion) was simply honoured;
  the row did land, but its load-bearing audit assertion did not.

This is the m5b analogue of m5a's c38 acceptance-substitution problem. It must
be documented in §3 with a per-row disposition and reflected in §8 coverage.
Either add the missing end-to-end allow-arm audit assertion before ratification
or route it explicitly (probably to the same m6 multi-turn stub follow-up that
would let c23/c24 run as true two-turn end-to-end tests).

### B2. §8/manual-validation framing does not match either the live file or the ratified scope

The retrospective repeatedly says `manual-validation.md` landed as a c28
skeleton/scaffold. Live history disagrees:

- `git log -- rafaello/plans/milestones/m5b-taint-exfil/manual-validation.md`
  shows the file was created in c15 (`6bea5ba`), not c28.
- The file is only a 9-line §3 wire-shape note for `details.taint`; it is not a
  section scaffold for the milestone manual-validation run.

The retrospective's proposed fill list also diverges from ratified `scope.md`
§"Manual validation". Scope requires six bullets:

1. verbatim-exfil walkthrough with provenance and deny;
2. allow-arm audit trail with `confirm_request_taint_attached`;
3. overlay rendering + clipping;
4. macOS CI URL;
5. audit-log inspection;
6. no-match/provider-only path.

Retrospective §8 instead lists a "Real-network demo", a verbatim walkthrough,
a PT1 violation demo, macOS CI, and audit-log inspection. It omits the scoped
allow-arm audit trail, overlay/clipping walk, and no-match smoke; it adds a PT1
manual demo that may be useful but is not one of the scoped manual-validation
bullets. The "Real-network demo" label is also dangerous because scope says
manual validation **does not** exercise real-network `web-fetch`; only the
provider side may use the dev LiteLLM proxy while the fetch tool remains
file-backed.

Fix §1/§5/§8 to describe the actual live state: the manual-validation document
is not a c28 skeleton and still needs the scoped scaffold/content. If PT1 is
kept as an extra manual check, label it as extra, not as a substitute for the
ratified bullets.

### B3. §6 Stream-A drift plan misidentifies §7.2.6 row 4 and drops the real `rpc_reply` narrowing

Retrospective §6.1 says:

> §7.2.6 row 4 — `confirm_reply` narrowing follows the same v1-known-limitation banner as rows 3 / 5.

But live Stream A §7.2.6 row 4 is **`plugin.<a>.rpc_reply`**, not
`core.session.confirm_reply`. The table rows are:

1. `plugin.<id>.tool_result`
2. `provider.<id>.tool_request`
3. `provider.<id>.assistant_message`
4. `plugin.<a>.rpc_reply`
5. `frontend.<id>.confirm_answer`

This matters because §5 item 2 correctly mentions `plugin.<a>.rpc_reply` as
part of the v1 narrowing, while §6's planned drift patch would update the wrong
row and omit the actual RPC-reply limitation. If the retrospective wants to
mention core's `confirm_reply` output shape, it needs a separate anchor; it is
not Stream A §7.2.6 row 4. The drift plan should explicitly record rows 3, 4,
and 5 as `assistant_message`, `plugin.rpc_reply`, and `frontend.confirm_answer`
known limitations (with any `confirm_reply` prose tied to the confirm-answer
re-emit path rather than to row 4).

## Major findings

### M1. c23b is named incorrectly, and the c23/c23b coverage prose overstates what the sibling directly proves

The retrospective names the c23b sibling twice as
`tests/rfl_chat_value_match_taint_unioned_in_canonical_tool_request.rs` (§1 and
§10.1). No such file exists. The landed file is:

`rafaello/crates/rafaello-core/tests/m5b_value_match_exfil_chain_fires_harness.rs`

The coverage prose should also be a little more precise. c23b directly drives
`ReemitRouter` + `ConfirmationGate` + `TaintMatchMap` +
`ReferencedTaintIndex` + `AuditWriter` and asserts `details.taint` plus the
`confirm_request_taint_attached` audit row. It does **not** render the TUI
`provenance:` overlay in that harness; the overlay render is covered separately
by c16 TUI tests. So avoid saying c23b alone closes the provenance-overlay arm.
The safe wording is: c23b closes the value-match → canonical-taint → confirm
payload/audit-row gap at the harness seam; c16 covers the overlay rendering of
that payload.

### M2. §2.5 documents the §TM4 hook with the wrong live signature and semantics

§2.5 says the hook is:

```rust
Broker::install_publish_test_hook(&self,
  hook: Box<dyn Fn(&PublishMsg) + Send + Sync>)
```

Live `bus.rs` exposes:

```rust
pub type PublishTestHook = Arc<dyn Fn(&BusEvent) -> Option<BrokerError> + Send + Sync>;
pub fn install_publish_test_hook(&self, hook: PublishTestHook)
```

The `Option<BrokerError>` return is load-bearing: `Some(err)` short-circuits
before fan-out; `None` permits delivery. The argument is the constructed
`BusEvent`, not `PublishMsg`, and the storage type is an `Arc` alias, not a
`Box`. Since §2.5 is documenting reusable test-seam precedent, it should match
the live surface exactly.

### M3. §6/§9 cite a non-existent `referenced_taint.rs` path

The live file is
`crates/rafaello-core/src/reemit/referenced_taint_index.rs`. The retrospective
cites `reemit/referenced_taint.rs` in §6.3 and §9. This is minor, but drift
commits and m6 readers should not be pointed at a path that never existed.

### M4. §6.5's "other streams unaffected" wording should account for the TUI-facing render extension

It may be correct that Stream D does not need a formal RFC patch, but §6.5's
current wording is too absolute: m5b did change TUI-visible behaviour via the
`ConfirmOverlay` `provenance:` block and added the plural scripted-answer hook.
The section should say explicitly that these are intentionally covered by the
`decisions.md` rows / overview confirmation-protocol banner and do not require a
Stream-D RFC edit because Stream D does not enumerate overlay block internals.
That keeps the "no patch" decision from reading like the TUI surface was not
checked.

## Non-blocking notes / polish

### N1. §1's c24/c25 "bonus negatives" label is confusing

§EXFIL2 is an allow-arm companion, not a negative. Calling c24 a "bonus
negative" hides the fact that it is a positive/allow audit-trail variant and
makes the c24 audit-row omission easier to miss.

### N2. §1 should pin the stats to `e533361`, not generic `HEAD`

The numbers in §1 are correct for `git diff rafaello-v0.1..e533361`, but not
for current `HEAD` after the retrospective draft (`HEAD` is 191 files / 21,530
insertions). The paragraph says "at the c23b tip" nearby; make the command
itself reproducible like m5a did after pi caught the same trap.

### N3. §7 row numbering should be conditional until drift rows land

The sketches say rows 50-58 will be added. That is likely true after m5a's rows
46-49, but phrase as "expected rows 50-58" or "next rows" unless the drift
commit has already landed. This avoids creating false archaeology if an earlier
retro-polish commit inserts an extra decision row.

## Confirmation table

| Check | Result |
|---|---|
| `git log rafaello-v0.1..HEAD --oneline` | Shows 28 plan-row commits c01-c28, the docs-only c23 deviation ratification (`86d6124`), c23b (`e533361`), and the round-1 retrospective draft (`fa179a1`). |
| `git diff --shortstat rafaello-v0.1..e533361` | Matches retrospective's implementation-tip stat: 190 files, 20,181 insertions, 69 deletions. Current `HEAD` includes the retrospective and is 191 files / 21,530 insertions. |
| Implementation-surface shortstat | `git diff --shortstat rafaello-v0.1..e533361 -- rafaello/crates rafaello/tests rafaello/fixtures` = 172 files, 11,675 insertions, 68 deletions. |
| Top-level test count | Baseline 577, c23b tip 683; net +106 top-level `tests/*.rs` files. |
| Design drift files changed during Phase 3 | No `overview.md`, `decisions.md`, `glossary.md`, or `streams/*` files changed in `rafaello-v0.1..e533361`; drift patches are still pending. |
| c23b file | Landed as `rafaello-core/tests/m5b_value_match_exfil_chain_fires_harness.rs`, not the filename cited in the retrospective. |
| c24 allow-arm audit trail | Live c24 explicitly omits `confirm_request_taint_attached`; must be recorded as a deviation/gap. |
| `manual-validation.md` | Created in c15 as a 9-line wire-shape note; not a c28 scaffold. |

## Verdict

Not ready for owner ratification yet. Issues raised: **3 blocking, 4 major, 3 non-blocking**. The most important fixes are to document the c24 deviation, align manual-validation with the ratified scope/live file, and correct the Stream A row-4 drift plan before those patches land.
