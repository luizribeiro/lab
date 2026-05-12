# m5b retrospective.md — pi review round 2

Reviewed round-2 `retrospective.md` at the current `agents/m5b/retro-pi`
HEAD (`5b165c1`; same tree/content as the `agents/m5b/retro-claude` commit
`5c742b5` named in the prompt), the diff from round 1, and the live
source/history behind the round-1 findings.

## Blocking findings

### B1. Round-1 B2 is still not resolved: §8 manual-validation additions still diverge from ratified scope

Round 2 correctly fixes the live-file history: `manual-validation.md` landed at
c15 (`6bea5ba`) as a 9-line §3 wire-shape note, not as a c28 skeleton. But the
actual §8 additions list still does not match ratified `scope.md` §"Manual
validation".

Scope's six manual-validation bullets are:

1. verbatim-exfil walkthrough against the m5b fixture, file-backed fetch,
   provenance on the send-mail modal, deny, empty mailcat log, and
   `confirm_request_taint_attached`;
2. allow-arm audit trail with mailcat receiving and the
   `confirm_request_taint_attached` row containing the fetch entry;
3. overlay rendering plus terminal-clipping/ellipsis;
4. macOS CI URL;
5. audit-log inspection;
6. no-match/provider-only path with no `_taint_attached` row.

Round-2 §8 still lists instead:

- `§1 Real-network demo` against LiteLLM;
- `§2 Verbatim exfil walkthrough`;
- `§3 §PT1 violation demo`;
- macOS CI;
- audit-log inspection.

That keeps three problems from round 1:

- The scoped allow-arm audit-trail manual run is still missing as its own item
  (especially important now that c24's end-to-end test deviated on the audit
  row).
- The scoped overlay/clipping walk and no-match/provider-only smoke are still
  missing.
- The PT1 violation demo may be useful, but it is an extra check, not one of
  the scoped manual-validation bullets and not a substitute for the omitted
  bullets.

The `Real-network demo` label is still risky. Scope explicitly says manual
validation does **not** exercise real-network `web-fetch`; the tool half stays
file-backed via `RFL_FETCH_TEST_BODY_PATH`. If the intent is "real provider via
LiteLLM proxy, file-backed fetch tool", say that explicitly and do not name it
"real-network".

### B2. Round-1 B3 is fixed in one direction but now falsely says Stream A has no `rpc_reply` row

Round 1 flagged that §6 misidentified Stream A §7.2.6 row 4 as
`confirm_reply`. Round 2 drops the `confirm_reply` row-4 claim, but replaces it
with a new false claim:

> No separate §7.2.6 table row in the live RFC; banner anchors on the §A9
> fallback list rather than on an invented row number.

Live Stream A §7.2.6 **does** have a separate row for `plugin.<a>.rpc_reply`:

```text
| `plugin.<a>.rpc_reply` | required, exactly one entry, must reference the matching `plugin.<b>.rpc_call.<a>` |
```

The right correction is:

- row 3 = `provider.<id>.assistant_message` known v1 limitation;
- row 4 = `plugin.<a>.rpc_reply` known v1 limitation;
- row 5 = `frontend.<id>.confirm_answer` known v1 limitation;
- `core.session.confirm_reply` is a core output / symmetric confirm-path prose
  item from the scope's §TR5 reserve discussion, not Stream A §7.2.6 row 4.

This also needs cleanup in §5 item 2 and §10.9, which still mix row numbers and
names inconsistently (e.g. "rows 3 / 5" while listing `confirm_reply` and
`plugin.<a>.rpc_reply`, then later "rows 3 / 4 / 5" without naming row 4
correctly). Since these bullets drive the pending drift commit, the row-number
archaeology must be correct before ratification.

## Major findings

### M1. c24 deviation disposition is internally contradictory and overstates what c23b proves for the allow-arm audit trail

Round 2 does the important thing by documenting c24 as a deviation. But the new
§3 table says c24 "selects path 2 implicitly" because the audit-row anchor lives
on c23b. That contradicts both live c24 and the retrospective's own §3.2:

- c24 commit body heading: `Deviation (path 3 per c24 row text)`.
- c24 commit body: "c24 therefore lands on **path 3 (accept the deviation)**".
- Round-2 §3.2: "Path 3 ... is what landed."

Fix the table to say path 3 landed, with c23b providing the harness-level
coverage for the missing audit-row primitive.

Also be careful with the sentence that c23b "covers the allow-arm semantics".
C23b proves the §AL1 predicate, `details.taint`, and
`confirm_request_taint_attached` audit row at modal-open time. C24 proves the
allow-arm `confirm_allowed` / mailcat / entries shape end-to-end. No single
live test proves `confirm_request_taint_attached` and `confirm_allowed` joined
on the same real `rfl chat` allow-arm request. The retrospective can argue this
split coverage is acceptable (and §10.2 asks owner to confirm), but it should
not imply the full allow-arm audit trail is covered end-to-end.

### M2. §2.5 slightly overclaims the publish-test hook's production absence

The corrected hook signature is good. One remaining phrasing is too strong:
§2.5 says production builds compile a no-op `check_publish_test_hook` arm "so
the dispatch site vanishes in release builds", and then says the seam adds
"zero production code path".

Live `bus.rs` still has unconditional `check_publish_test_hook(&event)` call
sites; in non-test/non-`test-fixture` builds those call a cfg-selected no-op
function returning `None`, with the hook storage/method absent. Optimisation may
inline it away, but the source-level production shape is "cfg no-op at the call
site", not literally no dispatch site. Prefer the exact wording: storage and
installer are cfg-gated; production uses a no-op checker and pays no hook
storage / dynamic dispatch.

## Non-blocking notes / polish

### N1. Round-2 fix-list M-3 has a self-contradictory filename parenthetical

The status banner says §9 was corrected to
`referenced_taint_index.rs` "(not the round-1 `referenced_taint_index.rs`)".
Round 1's wrong path was `referenced_taint.rs`. The body sections are corrected;
this is just banner polish.

### N2. §5 item 2's surface column should include the RPC-reply surface if the text names it

The item text names `plugin.<a>.rpc_reply`, but the surface column lists only
`handle_assistant_message`, `handle_confirm_answer`, and `handle_confirm_reply`.
If the row continues to name the RPC-reply limitation, include the broker/RPC
reply surface in that column or split it into a separate sentence. This is
mostly a traceability polish item after B2 fixes the row-number/name mismatch.

## Round-1 closure check

| Round-1 finding | Status |
|---|---|
| B1 c24 deviation omitted | Mostly fixed: c24 is now in §3/§8/§10. Remaining issues are the path-2/path-3 contradiction and over-broad "c23b covers allow-arm semantics" wording (M1). |
| B2 manual-validation live-file/scope mismatch | Partially fixed: live-file history corrected, but the additions list still does not match ratified scope (B1). |
| B3 Stream A row-4 drift mistake | Partially fixed: `confirm_reply` is no longer called row 4, but the replacement falsely says there is no live `rpc_reply` row (B2). |
| M1 c23b filename / overlay overclaim | Mostly fixed: filename corrected and c23b described as payload/audit harness coverage; no remaining blocker. |
| M2 §TM4 hook signature | Fixed, with one production-no-op wording nit promoted to M2 above. |
| M3 `referenced_taint.rs` path | Fixed in body sections; banner typo noted as N1. |
| M4 other-streams/TUI wording | Fixed. §6.5 now explicitly checks Stream B/C/E and explains why TUI-visible changes do not need stream patches. |
| N1 "bonus negatives" label | Fixed. |
| N2 stats pinned to c23b tip | Fixed. |
| N3 conditional decisions row numbering | Fixed. |

## Confirmation table

| Check | Result |
|---|---|
| Reviewed commit | Current HEAD `5b165c1`; same tree as prompt commit `5c742b5`. |
| `git diff --shortstat rafaello-v0.1..e533361` | 190 files, 20,181 insertions, 69 deletions — matches §1. |
| `git diff --shortstat rafaello-v0.1..HEAD` | 192 files, 21,964 insertions, 69 deletions — confirms why §1 must stay pinned to `e533361`. |
| Design drift files in implementation range | No `overview.md`, `decisions.md`, `glossary.md`, or `streams/*` changes in `rafaello-v0.1..e533361`; drift remains pending. |
| Stream A §7.2.6 row 4 | Live row is `plugin.<a>.rpc_reply`, so round-2 §6.1's "no separate row" wording is false. |
| `manual-validation.md` | Still a 9-line c15 wire-shape note; round-2 §8 additions still need scope alignment. |

## Verdict

Not ready for owner ratification yet. Issues raised: **2 blocking, 2 major, 2 non-blocking**. The critical fixes are to align §8 with the ratified manual-validation bullets and correct the Stream A §7.2.6 `rpc_reply` row before the drift commit is written.
