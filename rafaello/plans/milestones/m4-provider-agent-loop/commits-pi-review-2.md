# m4 commits.md round-2 pi review

> Verdict: blocking
> Counts: b/1 h/1 m/0 l/1

## Coverage

| scope.md section | commit(s) | review |
|---|---:|---|
| W1-W5 | c01-c04, c02 | Covered; c01 wording still contradictory (low). |
| B0 | c07, c10, c12, c18 | Covered for plugin/frontend/provider/core `request_id`; one c10 test-order blocker remains. |
| B1-B3 | c07 | Covered by explicit workspace cutover waiver. |
| B4 | c08 | Covered. |
| B5 | c09 | Covered. |
| B6/B7b | c10 | Covered in intent, but two c10 observed-user-message tests need c12 fan-out or a same-commit seed seam. |
| B7 | c11 | Covered; moved internal-subscriber positive now belongs here. |
| B8/B10 | c12 | Covered. |
| B11 | c13 | Covered. |
| F | c15 | Grant-only test in c15; re-emit test correctly moved to c18. |
| PS + M2 | c14 | Covered; provider fixture publish shape pinned. |
| CR | c17-c18 | Mostly covered; re-emit failure-path test remains underspecified. |
| AL + TD | c19 | Covered; `Capabilities` is wired through c19/c26. |
| PR | c20-c21 | Covered; fixture `bin/` shim is created before compile-test. |
| TP | c22-c23 | Covered; fixture `bin/` shim is created before compile-test. |
| M1 | c05-c06 | Covered; c05 picks one scrubber test file. |
| H6 | c14 / no-op | Covered by explicit no-new-hooks statement. |
| T | c16 | Covered. |
| C | c24-c26 | Covered; m3 tests migrate in c24, c26 has same-commit smoke. |
| I | distributed | Covered, including the scope-named plugin `in_reply_to` regression. |
| H | c18, c21, c23 | Harness placements explicit. |

## Round-1 fix verification

| r1 id | status | verification |
|---|---|---|
| B1 | closed | c20/c22 create executable fixture entry shims before their manifest compile tests (`commits.md:1559`, `commits.md:1578`, `commits.md:1673`, `commits.md:1688`); live validator requires the entry path to exist and be a file (`validate_with_package.rs:18`, `validate_with_package.rs:99`-`114`). |
| B2 | closed for original evidence | `broker_publish_provider_topic_to_internal_subscriber.rs` is moved to c11 after `subscribe_internal` lands (`commits.md:812`, `commits.md:897`-`915`). New blocker below is a different c10 ordering problem. |
| B3 | closed | c10 explicitly patches `handle_plugin_publish` and `handle_frontend_publish` with shared `MissingRequestId` enforcement (`commits.md:710`-`730`) and lands plugin/frontend/provider tests plus the scope-named `broker_plugin_tool_result_missing_in_reply_to_rejected.rs` (`commits.md:786`-`806`). |
| B4 | closed | c15 now has grant-only `frontend_publish_user_message_accepted_by_broker.rs` (`commits.md:1152`-`1168`); c18 owns `frontend_publish_user_message_reemitted_as_core_session_user_message.rs` after re-emit exists (`commits.md:1285`, `commits.md:1384`-`1396`). |
| B5 | closed | c19 `AgentLoop` stores `caps: Capabilities` and constructor arg (`commits.md:1426`-`1444`); c26 passes `Capabilities::tui_default()` (`commits.md:1903`-`1906`); live controller requires `&Capabilities` (`session/mod.rs:275`-`279`). |
| B6 | closed | c26 now includes same-commit smoke test `rfl_chat_eager_spawns_provider_and_tool_then_shuts_down_cleanly.rs` (`commits.md:1930`-`1949`). |
| B7 | closed | c24 migrates every m3 `rfl chat` test to a stub lock / updated failure expectation in the same commit that introduces lock loading (`commits.md:1794`-`1847`). |
| H1 | partly open | Unknown-tool lifecycle test is added (`commits.md:1411`-`1419`), but the re-emit-rejected test still allows a non-router direct `publish_core` path (`commits.md:1397`-`1408`); see High. |
| H2 | closed | c14 pins `provider_bus_publish` payload with fresh `request_id` and `in_reply_to: []` (`commits.md:1119`-`1128`). |
| H3 | closed | c10 sizing waiver now covers handler, maps/bookkeeping, cross-handler enforcement, and test bulk (`commits.md:626`-`632`). |
| H4 | closed | c25 adds `rfl_chat_tool_spawn_failure_propagates.rs` and requires provider spawn to succeed first (`commits.md:1887`-`1895`). |
| M1 | closed | c05 chooses exactly `env_scrubber_rejects_rfl_provider_id.rs` (`commits.md:390`-`393`). |
| M2 | closed | c17 resolves public `provider_id` from `acl.plugins[active_provider].provider_id` before subscribing (`commits.md:1239`-`1257`). |
| M3 | closed | Shared helpers are assigned to c18/c21/c23 (`commits.md:1341`-`1346`, `commits.md:1610`-`1617`, `commits.md:1724`-`1731`). |
| L1 | not closed | c01 still says “members edit only” / crate dirs land in c03/c04 while also saying placeholders land in c01 (`commits.md:278`-`286`). |
| L2 | closed | c10 enumerates all eighteen test filenames and removes optional collapsing language (`commits.md:733`-`806`). |

## Blockers

- **B1 — c10 still has observed-user-message tests before any provider fan-out or seed seam exists.** c10 acceptance requires `provider_tool_request_in_reply_to_user_message_id_rejected.rs` to “fan out a `user_message`” to populate `provider_observed_user_messages`, and the sibling positive uses the same setup (`commits.md:765`-`773`). But provider recipient fan-out and the side-effect that inserts `core.session.user_message` ids into `provider_observed_user_messages` do not land until c12 (`commits.md:924`, `commits.md:963`-`979`). c11 noticed the analogous result-id problem and adds a seed accessor for `provider_observed_results` (`commits.md:897`-`915`), but c10 has no `seed_provider_observed_user_message_for_test` seam. Move these two tests to c12, or add the c10 seed seam explicitly.

## High

- **H1 — `reemit_invalid_taint_emits_reemit_rejected_event.rs` still permits a false-positive test.** The test is supposed to prove the router catches a re-emit error and emits `core.lifecycle.reemit_rejected`, but its allowed setup includes “directly calling `publish_core("core.session.tool_request", _)`” (`commits.md:1397`-`1408`). That bypasses `ReemitRouter`, so it does not close the original CR7 failure-path coverage gap. Require a same-commit router fault-injection seam, or mark the route untestable with an explicit waiver.

## Medium

None.

## Low

- **L1 — c01 wording still contradicts the placeholder cutover.** The row says c01 lands “the `members` edit only” and crate directories land in c03/c04, then immediately says c01 also lands two placeholder crates (`commits.md:278`-`286`). Drop the “only” / “directories land in c03/c04” sentence; keep “workspace-member placeholder cutover.”

## Notes

- The requested spot checks pass: plugin/frontend handler `request_id` enforcement is assigned in c10 (`commits.md:710`-`730`), `broker_plugin_tool_result_missing_in_reply_to_rejected.rs` lands in c10 (`commits.md:801`-`806`), c20/c22 fixture-bin files precede compile-tests (`commits.md:1559`-`1586`, `commits.md:1673`-`1689`), and c26 has a same-commit orchestration smoke (`commits.md:1930`-`1949`).
