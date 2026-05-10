# m4 commits.md round-3 pi review

> Verdict: clean — no blockers/highs; pi-2 b/1 h/1 l/1 are closed.
> Counts: b/0 h/0 m/0 l/0

## Coverage

| scope.md section | commit(s) | review |
|---|---:|---|
| W1-W5 | c01-c04 | Covered; c01 now describes a workspace-member placeholder cutover with both placeholder crates in c01 and full deps/bin targets later (`commits.md:326`, `commits.md:331`, `commits.md:344`). |
| B0 | c07, c10, c12, c18 | Covered across the cutover, cross-handler request-id checks, core taint-aware publishing, and frontend re-emit (`commits.md:721`, `commits.md:781`, `commits.md:1019`, `commits.md:1435`). |
| B1-B3 | c07 | Covered by the monolithic workspace cutover (`commits.md:497`). |
| B4 | c08 | Covered by the provider/MissingRequestId/InvalidTaint/StaleRequestId error surface (`commits.md:574`). |
| B5 | c09 | Covered by provider registration and RAII guard (`commits.md:614`). |
| B6/B7b | c10 | Covered; observed-result and observed-user-message seed seams make c10 self-sufficient before c12 fan-out (`commits.md:672`, `commits.md:758`, `commits.md:766`). |
| B7 | c11 | Covered by `subscribe_internal` and internal-intake fan-out (`commits.md:896`, `commits.md:948`). |
| B8/B10 | c12 | Covered by `publish_core_with_taint`, origin-provider exclusion, and observed-id fan-out side effects (`commits.md:999`, `commits.md:1004`, `commits.md:1045`, `commits.md:1048`). |
| B11 | c13 | Covered by defence-in-depth provider publish-id check (`commits.md:1087`). |
| F + T | c15-c16 | Covered; frontend ACL grant lands before TUI test-message hook (`commits.md:1210`, `commits.md:1249`). |
| PS + M2 | c14 | Covered by row-39 removal and provider broker registration (`commits.md:1121`, `commits.md:1168`). |
| CR | c17-c18 | Covered; c17 owns router/fault seam and c18 owns per-direction re-emit plus failure tests (`commits.md:1292`, `commits.md:1326`, `commits.md:1390`, `commits.md:1502`). |
| AL + TD | c19 | Covered by AgentLoop and tool dispatch/persistence (`commits.md:1540`). |
| PR | c20-c21 | Covered by mockprovider fixture/implementation commits (`commits.md:1657`, `commits.md:1710`). |
| TP | c22-c23 | Covered by readfile fixture/implementation commits (`commits.md:1773`, `commits.md:1811`). |
| M1 | c05-c06 | Covered by reserved env-var extension and compiler-inserted tool_result publish grant (`commits.md:422`, `commits.md:449`). |
| H6 | c14 / no-op | Covered by explicit no-new-hooks statement in coverage check (`commits.md:2161`). |
| I/H | distributed | Covered; tests and harness helpers land with the consuming commits (`commits.md:1447`, `commits.md:1731`, `commits.md:1846`). |
| C | c24-c26 | Covered by lock-load/orchestration/spawn/reemit commits (`commits.md:1877`, `commits.md:1970`, `commits.md:2016`). |

## r2 fix verification

| r2 id | status | verification |
|---|---|---|
| B1 | closed | r2 reported c10 observed-user-message tests had no same-commit fan-out or seed seam (`commits-pi-review-2.md:55`). Round 3 adds both seed accessors in c10, including `seed_provider_observed_user_message_for_test` (`commits.md:758`, `commits.md:763`, `commits.md:766`), states they avoid any dependency on c12 fan-out (`commits.md:771`, `commits.md:773`), and updates both affected c10 tests to call the user-message seed seam (`commits.md:836`, `commits.md:838`, `commits.md:849`, `commits.md:850`). |
| H1 | closed | r2 reported the invalid-taint test could bypass `ReemitRouter` by directly calling `publish_core` (`commits-pi-review-2.md:59`). Round 3 adds `ReemitRouter::with_test_fault_injector` in c17 (`commits.md:1326`), requires injector errors to run before real re-emit through the CR7 failure path (`commits.md:1337`, `commits.md:1341`), and rewrites `reemit_invalid_taint_emits_reemit_rejected_event.rs` to drive `handle_provider_publish` through the real router body with no canonical tool_request fan-out (`commits.md:1502`, `commits.md:1505`, `commits.md:1517`, `commits.md:1521`, `commits.md:1526`). The direct `publish_core` alternative is explicitly removed (`commits.md:1527`). |
| L1 | closed | r2 reported contradictory c01 wording about “members edit only” and crate dirs landing later (`commits-pi-review-2.md:67`). Round 3 rewrites c01 as a placeholder cutover, saying the members edit and both minimal crate placeholders land together (`commits.md:326`, `commits.md:331`), with full deps/bin targets landing later (`commits.md:344`). |

## Blockers

None.

## High

None.

## Medium

None.

## Low

None.

## Notes

- Clean/non-blocking-fixes verdict: no blockers or highs remain.
- Review was limited to `commits.md` and `commits-pi-review-2.md` as requested.
