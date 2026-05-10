# commits.md round-3 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `9ccb812`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 3 fixes the round-2 structural blockers: the frontend supervisor lifecycle is now consolidated into a greenable cutover commit, the unowned `inject_reaper_panic` test is removed, and `workspace_bin_path` lands before the CLI tests that need it. The remaining issues are narrower, but one is still a per-commit execution blocker: c31 depends on constructors that no prior commit owns. I also see three high-priority issues and three medium-priority cleanup items.

## Blocker

### B1 — c31 depends on `Entry::new_*` constructors that c08 does not introduce

- **Where:** c08, c31.
- **Problem:** c31 says the `RFL_HARNESS_FIXTURES=1` harness uses c08's `Entry::new_*` constructors to avoid adding direct `serde_json` / `ulid` / `chrono` dependencies to the `rafaello` CLI crate. But c08's What section only introduces `Entry`, `EntryMetadata`, helper enums/types, and the built-in payload structs. It does not say that any `Entry::new_*` constructors are added, and `scope.md`'s Entry section likewise defines the data types but not constructor helpers.
- **Why it blocks:** A c31 implementer cannot satisfy the c31 harness exactly as written from the public surface that exists after c08/c30. They must either invent new `rafaello-core` API outside the planned owning commit, or add direct dependencies to `crates/rafaello/Cargo.toml`, contradicting c31's explicit rationale.
- **Fix:** Pick one ownership model before handing this to per-commit implementers:
  1. Add the `Entry::new_*` constructors explicitly to c08's What section and acceptance tests; or
  2. Move constructor/API introduction to c31 and state the exact helpers added there; or
  3. Have c31 explicitly add the direct `rafaello` dependencies it needs and remove the claim that c08 constructors avoid them.

## High-priority findings

### H1 — c25 acceptance forward-references c26 headless test mode

- **Where:** c25, c26.
- **Problem:** c25's acceptance test, `rafaello-tui/tests/tui_handler_calls_frontend_ready.rs`, says to spawn `rfl-tui` in headless mode. However, `RFL_TUI_TEST_MODE=1`, terminal-init skipping, stderr sentinels, and test-mode lifetime behavior are not introduced until c26. c25 only owns env parsing, fd adoption, client setup, handler registration, and the `frontend.ready` RPC.
- **Risk:** A c25 implementer either cannot run the acceptance test without a real terminal, or must implement part of c26 early, breaking the per-commit ownership boundary.
- **Fix:** Either make c25 explicitly own a minimal non-terminal startup path sufficient for the ready-RPC test, or move `tui_handler_calls_frontend_ready.rs` to c26 after headless mode exists. If the test remains in c25, spell out the exact non-terminal mechanism c25 provides.

### H2 — H6 private-state-dir unwind contract is still ambiguous between c07 and scope.md

- **Where:** c06, c07, `scope.md` §H6.2/§H6.3.
- **Problem:** `scope.md` §H6.2 says the pre-spawn hook fires after private-state-dir creation and that the private-state dir remains on disk; m3 does not remove it on unwind. But `scope.md` §H6.3 still contains stale wording that `supervisor_spawn_unwinds_after_socketpair.rs` verifies proxy and private-state dirs are cleaned up. c07 lists `supervisor_spawn_unwinds_after_socketpair.rs` and the fd-baseline test without explicitly resolving this assertion mismatch.
- **Risk:** Implementers may write the old private-state-dir cleanup assertion, producing a false red, or may change production unwind behavior to remove a user-scoped state directory contrary to the newer contract.
- **Fix:** In c07, explicitly constrain the socketpair/pre-spawn acceptance to fd-count/proxy/in-flight cleanup and state that private-state-dir cleanup is **not** asserted. Also patch the stale `scope.md` §H6.3 sentence before ratification so scope and commits agree.

### H3 — c17 acceptance under-covers `RFL_FIXTURE_MAX_LIFETIME` for existing long-running modes

- **Where:** c17.
- **Problem:** c17's What section correctly says all long-running modes read `RFL_FIXTURE_MAX_LIFETIME`, but acceptance lists only five tests for the five new m3 modes. `scope.md` §L1 requires the self-timeout mitigation for existing long-running fixture modes such as `respond_peer_call` and `observer` too.
- **Risk:** The highest-value leak mitigation can regress while c17 still appears green: an implementer could add the five new modes and their timeout behavior while leaving an m2 long-running mode able to orphan indefinitely.
- **Fix:** Add at least one existing-mode regression to c17 acceptance, for example `fixture_mode_respond_peer_call_honors_max_lifetime.rs` with `RFL_FIXTURE_MAX_LIFETIME=1`. Add an `observer` regression too if that mode is cheap to exercise.

## Medium-priority findings

### M1 — c21 does not prove `FrontendBusPublishService` is wired into the spawned parent server

- **Where:** c21.
- **Problem:** c16 tests broker-level frontend publish authority and fan-out. c21 then claims the parent fittings server is built with both `FrontendBusPublishService` and `FrontendReadyService`, but its acceptance tests only exercise lifecycle/readiness/shutdown paths. A spawn implementation could wire `FrontendReadyService` correctly but omit or miswire `FrontendBusPublishService` and still satisfy the listed c21 tests.
- **Why it matters:** This is the cutover commit where the frontend connection's service composition becomes real. Missing this wiring would break m4-style frontend-originated RPC/publish flows and any m3 smoke that relies on an attached frontend publishing through the connection.
- **Fix:** Add one c21 test proving an attached frontend can invoke the frontend publish path through the parent server. The assertion may be either a successful allowed publish with a test ACL grant or a deterministic broker denial for a known disallowed topic, as long as it proves the `bus.publish` request reached `handle_frontend_publish` as `Publisher::Frontend`.

### M2 — top matter still has stale status/count/headline references

- **Where:** preamble, canonical test names.
- **Problem:** The file still identifies itself as a "round-2 draft" and says m3 has **34 commits in 12 groups**, while the current plan has c01 through c32: **32 commits in 12 groups**. The canonical test names section says `rfl_chat_demo_bar.rs` lands at c31, but the test now lands at c32.
- **Why it matters:** These are not implementation blockers, but stale counts and landing references are exactly the sort of drift that causes driver prompts and per-commit branches to target the wrong row.
- **Fix:** Update the status/count text to round 3 / 32 commits, and update the headline test landing reference to c32.

### M3 — checkpoint text says the session controller has landed by c23, but it lands in c24

- **Where:** `m3a / m3b checkpoint` section.
- **Problem:** The checkpoint says the driver re-evaluates after c23, described as "session store + controller landed". In the current plan, c22 opens the store, c23 adds append/load, and c24 introduces `SessionController`.
- **Why it matters:** This can cause a premature checkpoint with the controller still absent, or a driver prompt that incorrectly summarizes the landed surface.
- **Fix:** Either move the checkpoint to after c24, or change the c23 checkpoint description to "session store append/load landed" and add a second/updated checkpoint after c24 for the controller.

## Summary

Round 3 is close: the serious round-2 forward-reference issues around frontend lifecycle and CLI binary resolution are resolved. Before ratification, fix the c31 constructor ownership blocker, then tighten c25/c26 test ownership, H6 unwind wording, fixture lifetime coverage, and the stale metadata/checkpoint references.
