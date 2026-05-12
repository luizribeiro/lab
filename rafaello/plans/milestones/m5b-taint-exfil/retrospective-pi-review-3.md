# m5b retrospective.md — pi review round 3

Reviewed round-3 `retrospective.md` at `a69be4d`, the diff from round 2,
`retrospective-pi-review-2.md`, and the live source/history behind the round-2
findings.

## Blocking findings

None. **retrospective.md is ready for owner ratification / CONVERGED.**

## Major findings

### M1. §2.5 / §4 still contain the old "zero production code path" overclaim

Round 3 fixed the detailed §TM4 bullet by saying production builds select a
cfg-no-op `check_publish_test_hook` and pay no hook storage / dynamic dispatch.
That is the correct live-source shape.

Two nearby sentences still carry the old wording:

- §2.5 final paragraph: "The seam adds **no production code path**: the cfg-gate
  fences both the storage field and the dispatch call."
- §4 sizing: "The seam adds zero production code (cfg-gated both at the storage
  field and the dispatch call) ..."

Live `bus.rs` still has unconditional `check_publish_test_hook(&event)` call
sites; production chooses the no-op checker. The storage and installer are
cfg-gated, but the source-level dispatch call sites are not literally fenced
away. This is non-blocking because the corrected live wording appears in the
main §2.5 bullet and §9, but these two residual sentences should be aligned
before the drift/ratification polish commit.

### M2. §5 item 2 invents `handle_rpc_reply`, and §6.5 phrases the rpc-reply limitation backwards

Round 3 correctly maps Stream A §7.2.6 row 4 to `plugin.<a>.rpc_reply`, but the
follow-up table now names a non-live surface:

```text
reemit/mod.rs::handle_assistant_message, handle_rpc_reply, handle_confirm_answer
```

There is no `handle_rpc_reply` function in live `rafaello-core` (`rg
handle_rpc_reply` returns no function; the rpc-reply handling lives in
`bus.rs::handle_plugin_publish`'s `last == "tool_result" || last == "rpc_reply"`
validation / result-routing branch). Use a live anchor such as
`bus.rs::handle_plugin_publish`'s `rpc_reply` branch, or keep the row at the
conceptual surface level without a function name.

Related polish: §6.5 says the fittings `rpc_reply` arm is "explicitly *not*
covered by the §A9 / §\"Out of scope\" item 2 narrowing". Per §6.1 / §10.9 it
*is* covered by that narrowing as a known v1 limitation; it is just not enforced
by §PT1. Reword to "covered only as a v1-known-limitation / not enforced".

## Non-blocking notes / polish

### N1. §8 coverage summary should carry the c24 split-coverage caveat locally

The c24 deviation is now documented well in §3 and §10.2. §8 still says "All
scope §\"Demo bar\" negative 4 rows green: §EXFIL1 via c23, §EXFIL2 via c24,
§EXFIL3 via c25, plus c23b ... closing the c23 end-to-end gap." Since c23b also
covers the audit-row primitive that c24 lacks, the local coverage summary should
say that explicitly (e.g. "§EXFIL2 via c24 for allow-arm end-to-end shape, with
its audit-row primitive covered by c23b"). This avoids reintroducing the round-1
B1 overclaim in the coverage section.

## Round-2 closure check

| Round-2 finding | Status |
|---|---|
| B1 manual-validation additions diverged from scope | Fixed. §8 now lists the six ratified scope bullets, labels LiteLLM as real-provider/file-backed-fetch, and marks PT1 as extra-not-scoped. |
| B2 Stream A rpc_reply row mapping | Fixed for §6.1 / §8 / §10.9 row mapping. Remaining live-surface wording issues are non-blocking M2. |
| M1 c24 path-2/path-3 contradiction + allow-arm overclaim | Fixed. §3 now says c24 took path 3 and explicitly records split coverage. |
| M2 §TM4 production no-op wording | Mostly fixed. Main §2.5 bullet and §9 are accurate; two residual "zero production code path" sentences remain (M1). |
| N1 wrong-path parenthetical | Fixed. |
| N2 rpc-reply surface column | Partially fixed; row now mentions rpc_reply but names nonexistent `handle_rpc_reply` (M2). |

## Fresh-pass confirmation table

| Check | Result |
|---|---|
| Reviewed commit | `a69be4d` (`docs(rafaello-m5b): retrospective.md round 3 — fold pi-review-2`). |
| `git diff --shortstat rafaello-v0.1..e533361` | 190 files, 20,181 insertions, 69 deletions — still matches §1. |
| `git diff --shortstat rafaello-v0.1..HEAD` | 193 files, 22,253 insertions, 69 deletions — confirms §1 must remain pinned to implementation tip. |
| Design drift files in implementation range | No `overview.md`, `decisions.md`, `glossary.md`, or `streams/*` changes in `rafaello-v0.1..e533361`; drift remains pending as stated. |
| Stream A §7.2.6 rows | §6.1 now maps row 3 = assistant_message, row 4 = rpc_reply, row 5 = confirm_answer correctly. |
| Manual-validation file | Still the c15 9-line wire-shape note; §8 now describes scoped additions rather than a nonexistent skeleton. |

## Verdict

**CONVERGED: no blockers.** Issues raised: **0 blocking, 2 major, 1 non-blocking**. The remaining items are wording / live-anchor polish and can be folded with ratification/drift-prep; they do not block owner sign-off.
