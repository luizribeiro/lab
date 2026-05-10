# commits.md round-8 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `15e483f`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 8 closes the round-7 precision gaps: c30 now owns the nine-entry harness, uses `Capabilities::tui_default()`, publishes `core.lifecycle.test_done`, and has its own harness-path acceptance test; the checkpoint and c31 `RFL_TUI_PATH` setup are corrected. I did not find a dependency cycle across the 31 commits. One remaining acceptance mismatch can still break the headline test, plus one dependency-precision gap should be cleaned before ratification.

## High-priority findings

### H1 — c31 counts nine generic `bus.event` lines, but the harness also publishes `core.lifecycle.test_done`

- **Where:** c30 harness contract; c31 `What`; c25 headless-mode stderr contract.
- **Problem:** c30 now correctly publishes `core.lifecycle.test_done` after the ninth fixture entry. The m3 `rfl chat` ACL subscribes the TUI to both `core.session.**` and `core.lifecycle.**`, and c25 says headless mode writes one `"bus.event topic=<topic> seq=<n>"` line per received `bus.event`, plus `"test-done"` on the lifecycle event. Therefore the combined stderr can contain ten generic `"rfl-tui: bus.event"` lines: nine `core.session.entry.finalized` events and one `core.lifecycle.test_done` event.
- **Why it matters:** c31 currently says to assert “nine `"rfl-tui: bus.event"` lines”. An implementer following that literally can write a flaky/impossible assertion depending on whether the test-done event is logged before exit. The converged scope’s headline test counts nine `core.session.entry.finalized` bus-event lines, not nine generic bus events.
- **Fix:** Change c31 to assert exactly nine `"rfl-tui: bus.event topic=core.session.entry.finalized"` lines (seq/order as desired) and separately allow/assert the `"rfl-tui: test-done"` sentinel. If the intended implementation suppresses logging for `core.lifecycle.test_done`, say so explicitly in c25/c31; otherwise keep the topic-qualified assertion.

## Medium-priority findings

### M1 — c25 acceptance references frontend service/test harness types but omits their dependencies

- **Where:** c25 `Depends on` and acceptance tests, especially `tui_handler_calls_frontend_ready.rs` / `tui_sends_frontend_ready_after_handler_registration.rs`.
- **Problem:** c25 depends only on c24, but its tests are described in terms of a parent-side `FrontendReadyService` mock and the deterministic callback ordering from the frontend extra-service harness. Those public types are introduced in c17, and the practical spawn/server composition used by the TUI integration path lands in c20.
- **Why it matters:** The workspace is sequential so this is not a DAG blocker, but it violates the plan’s own convention that `Depends on` cites the lowest commit numbers whose code or types the commit references. Per-commit prompts built from the row may under-specify which earlier frontend test harness pieces the c25 agent should use.
- **Fix:** Add c17 to c25 `Depends on` if the tests instantiate the service types directly, and c20 as well if they use `FrontendSupervisor`/the composed frontend server path. Alternatively, reword the acceptance tests to say they use a raw fittings-server test harness and do not reference frontend supervisor services.

## Summary

Round 8 is close, but I would do one more precision edit before ratification: topic-qualify the c31 bus-event line-count assertion so `core.lifecycle.test_done` does not make the headline test contradictory, and tighten c25’s dependency row around the frontend readiness test harness.
