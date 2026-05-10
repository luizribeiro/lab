# commits.md round-9 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `62f8fc1`  
Reviewer: pi  
Verdict: **ratified**

Round 9 integrates the remaining round-8 precision fixes:

- c31 now counts exactly nine topic-qualified
  `"rfl-tui: bus.event topic=core.session.entry.finalized"`
  lines, so the allowed `core.lifecycle.test_done` bus event no
  longer makes the headline assertion contradictory.
- c25 now declares c17 and c20 dependencies for the frontend
  readiness/supervisor test harness types it references.

I rechecked the 31-commit dependency graph, the c30/c31 harness
handoff, the frontend/TUI readiness path, and the driver-owned Phase 4
boundary. I did not find a blocker, high-priority issue, dependency
cycle, or test/code ownership contradiction that should prevent
ratification.

## Findings

None.

## Non-blocking polish

These are not ratification blockers, but may be worth carrying into
per-commit prompts if the driver wants extra precision:

- c30's `rfl_chat_replay_withheld_until_frontend_ready.rs` row could
  remind the implementer to set a short `RFL_TUI_MAX_LIFETIME`; the
  scope already requires this, and otherwise the default headless TUI
  self-timeout could make a failure path slow.
- c26 could explicitly say it creates `rafaello-tui/src/paint.rs` and
  exports the module from `src/lib.rs`; this is implied by the current
  `What`/acceptance text and is straightforward for the per-commit
  agent.

## Summary

The round-9 draft is executable as a sequential 31-commit plan. The
remaining notes are prompt-polish only; I would move this plan forward
to implementation/driver ratification.
