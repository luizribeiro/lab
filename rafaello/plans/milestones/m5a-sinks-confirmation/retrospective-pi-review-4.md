# m5a retrospective.md — pi review round 4

Reviewed round-4 `retrospective.md` at `d3dda74`, the diff from
`3334a5e`, and the live source for the round-3 findings.

## Blocking findings

None. **retrospective.md is ready for owner ratification.**

## Major findings

### M1. One c38 gap-accounting sentence still omits item 15

Round 4 fixed the c38 deviation table and §8 §CHAT caveat, but the summary
paragraph after the §5 follow-up table still says:

> items 12-14 are the c38 acceptance-test deviation (§3.1).

Now that item 15 exists for the missing positive gate-through-orchestration
assertion, this should read "items 12-15". This is non-blocking because the
actual follow-up table and coverage caveat are now accurate.

## Non-blocking notes / polish

### N1. Current round hash is still a placeholder

The banner says `at hash TBD-round-4`. If the document keeps per-round hashes,
replace it with `d3dda74` (or the final folded commit hash) before ratification.

## Round-3 closure check

| Round-3 finding | Status |
|---|---|
| B1 `confirm_resolved` over-emission | Fixed. §6.1 and §8 now say normal answers do not publish `confirm_resolved`; it is only for grant-short-circuit queue pruning. Verified against `gate/mod.rs` / `reemit/mod.rs`. |
| M1 c38 partial substitute accounting | Mostly fixed; one summary sentence still says items 12-14 instead of 12-15 (M1). |
| M2 audit-kind glossary completeness | Fixed. §6.4 now frames families/examples and points at `AuditKind::as_str()` as authoritative. |
| M3 yellow unused-`allow_secrets` warning | Fixed. §7.1 now says the install warning is plain stderr and separates the yellow `rfl status` marker. |
| N1 round/status boilerplate | Partially fixed; round-3 history hash filled, current round hash still TBD (N1). |
| N2 monotonic helper wording | Fixed. §2.3 now names `supervisor::monotonic_nanos()` and its `OnceLock<Instant>` epoch shape. |

## Verdict

No blockers. **retrospective.md is ready for owner ratification.** Issues raised:
**0 blocking, 1 major, 1 non-blocking**.
