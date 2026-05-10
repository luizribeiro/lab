# commits.md round-2 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `f1e4bcf`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 2 fixes the six round-1 blockers, but I still see **3 blockers**, **3 high-priority findings**, and **3 medium-priority findings**. The remaining issues are mostly about the per-commit green-workspace invariant: several acceptance tests still require helper code, fixture behavior, or lifecycle machinery that has not landed by the commit that claims the test.

## Blockers

### B1 — c20/c21/c23 split leaves `FrontendSupervisor::spawn` in impossible partial states

- **Where:** c20, c21, c23.
- **Problem:** c20 acceptance says it spawns `rfl-bus-fixture` in `signal_ready` mode and asserts child PID, child stderr, and env application. But c20 only implements Phase B steps 1–8: socketpair, command construction, env application, private state dir, stderr piping, spawn, and stderr take. It does **not** yet implement readiness watch, reaper watch, reaper task, parent fittings server, broker registration, serve loop, or a complete handle-return path. A `signal_ready` fixture also needs a live parent fittings server to receive and answer the `frontend.ready` RPC, which does not exist until later.

  c21 has a similar partial-state problem: its acceptance tests assert reaper outcomes while register/serve/return are deferred to c23. That can work only if c21 also introduces a test-only partial spawn helper or a temporary handle shape, neither of which is specified.
- **Why it blocks:** A per-commit agent cannot keep the workspace green while satisfying c20/c21 acceptance exactly as written unless it invents unplanned temporary APIs or implements future c23 behavior early. That violates the one-logical-idea and sequential-dependency contract.
- **Fix:** Either merge c20–c23 into one full valid-spawn commit, or explicitly introduce cfg/test-only phase helpers in c20/c21 and make their acceptance tests use those helpers. Move real `signal_ready` spawn/handle tests to c23, after the parent server, registration, serve loop, and complete `FrontendHandle` exist.

### B2 — c21 uses an unowned `inject_reaper_panic` hook

- **Where:** c21 acceptance.
- **Problem:** `frontend_reaper_publishes_reaper_panicked_on_panic.rs` requires inducing a reaper-task panic via a cfg-gated `inject_reaper_panic` hook. No commit owns that hook, and the scope later explicitly says m3 does **not** add frontend-side `TestHooks` in this milestone.
- **Why it blocks:** The acceptance test cannot be implemented from the planned public surface. A per-commit agent would have to invent a frontend test-hook API that the plan does not define and that the scope appears to reject.
- **Fix:** Either add the hook explicitly to c21's What section, including its API, cfg gate, and relationship to the no-frontend-`TestHooks` statement, or remove/defer this test and cover `ReaperPanicked` only through the pure `shutdown_with_outcome` seam.

### B3 — c32/c33 CLI tests need `workspace_bin_path`, but it lands in c34

- **Where:** c32, c33, c34.
- **Problem:** c32/c33 acceptance tests under `rafaello/tests/` need to locate `rfl-tui` and `rfl-bus-fixture`. Scope says those tests use the shared `workspace_bin_path` helper because `env!("CARGO_BIN_EXE_*")` is not sound across crates. But c34 is the first commit that adds `rafaello/tests/common/workspace_bin_path.rs`.
- **Why it blocks:** c32/c33 acceptance contains a forward reference to a helper not yet landed. Implementers either duplicate ad-hoc binary resolution in c32/c33 or cannot satisfy the tests.
- **Fix:** Move `workspace_bin_path` into c32, or add a dedicated earlier test-harness helper commit before c32. c34 should consume the helper for the headline test, not introduce it after earlier CLI tests already need it.

## High-priority findings

### H1 — c32 declares c27 but needs c28 TUI test-mode behavior

- **Where:** c32 Depends on / acceptance.
- **Problem:** c32 tests use the real `rfl-tui` in headless test mode and rely on behavior introduced in c28: `RFL_TUI_TEST_MODE=1`, stderr sentinels such as `project-root=<abs-path>`, and `RFL_TUI_MAX_LIFETIME` self-timeout. c27 only wires env parsing, fd adoption, client setup, and `frontend.ready`.
- **Risk:** c32 can compile against c27, but several acceptance tests cannot pass until c28 lands.
- **Fix:** Change c32 dependency from c27 to **c28**. c33 can keep depending on c32 transitively.

### H2 — c22 shutdown seam dependency/signature is underdeclared

- **Where:** c22.
- **Problem:** `shutdown_with_outcome` is described as the pure extraction of the full shutdown algorithm. The algorithm needs `ReaperOutcome` and a live reaper outcome receiver/watch for the live-watch branch and for `ShutdownReport.exit_status` population. c22 depends only on c18, while the reaper-outcome watch and lifecycle plumbing are introduced in c21. The c22 signature also abbreviates the receiver as `cached, child_pid, config, signal_fn, probe_fn, serve_handle, register_guard`, omitting the reaper outcome receiver/watch that scope says the extraction takes.
- **Risk:** A c22 implementer may create a simplified dead-watch-only seam that passes the two listed tests but does not match the production shutdown algorithm that c23 is supposed to call.
- **Fix:** Make c22 depend on c21 and spell the full signature, including the reaper outcome receiver/watch. If c22 is intentionally dead-watch-only, rename/scope it that way and have c23 own the full extraction.

### H3 — c33 likely lacks owned dependency additions for fixture-entry harness

- **Where:** c33 / `rafaello` crate dependencies.
- **Problem:** The `RFL_HARNESS_FIXTURES=1` harness in c33 constructs real `Entry` values for eight built-in kinds plus one unknown kind. That likely requires direct `rafaello` dependencies on types/crates such as `serde_json`, `ulid`, and `chrono` unless `rafaello-core` exposes helper constructors that hide those details. No c31–c33 commit owns these `crates/rafaello/Cargo.toml` additions or such core-side constructors.
- **Risk:** The CLI crate may fail to compile when the harness is implemented, or agents may add dependencies opportunistically outside the plan.
- **Fix:** In c33, explicitly add any needed direct dependencies to `crates/rafaello/Cargo.toml`, or specify that c08/c25/c26 expose fixture-entry constructors that c33 uses without new deps.

## Medium-priority findings

### M1 — c34 mixes commit acceptance with driver/manual gates

- **Where:** c34.
- **Problem:** c34 bundles `rfl_chat_demo_bar.rs`, the `workspace_bin_path` helper, and `manual-validation.md`. Its acceptance requires Linux test pass, macOS CI green, and a manual-validation artifact with an interactive recording and CI URLs. The macOS CI result and manual recording are external ratification artifacts rather than normal per-commit local-green acceptance.
- **Why it matters:** This is not impossible, but it blurs the boundary between per-commit implementation and driver-owned milestone validation. It can make c34 appear red locally even if the code/test change is correct.
- **Fix:** Keep c34 to code/tests and, if desired, a placeholder/manual-validation skeleton. Treat real CI URLs and interactive recording as driver-owned phase gates, not per-commit acceptance.

### M2 — scope summary still says H6 has two inject points

- **Where:** `scope.md` Goal item 5 vs commits c06/c07.
- **Problem:** Detailed scope §H6 and commits c06/c07 use three inject points: pre-spawn, post-spawn-pre-register, and post-register. The high-level Goal item still says the mechanism has two inject points.
- **Why it matters:** The detailed section is clearly newer, but this stale summary can still cause ratification or implementer confusion when checking `commits.md` against scope.
- **Fix:** Add a note in `commits.md` that the detailed §H6 three-point model supersedes the stale Goal summary, or patch `scope.md` before ratification.

### M3 — c17 acceptance omits self-timeout coverage for existing modes

- **Where:** c17.
- **Problem:** Scope L1 requires `RFL_FIXTURE_MAX_LIFETIME` support for existing long-running fixture modes (`respond_peer_call`, `observer`) as well as the five new m3 modes. c17 acceptance lists five tests for the five new modes, but no regression test that an existing mode honors the self-timeout.
- **Why it matters:** The highest-value part of the leak mitigation is protecting pre-existing long-running fixture modes from orphaning. Without at least one existing-mode regression, an implementation could add the new modes and still leave the m2 leak class unfixed.
- **Fix:** Add at least one c17 acceptance test for an existing long-running mode honoring `RFL_FIXTURE_MAX_LIFETIME`, or explicitly state where that behavior is covered.

## Summary

Round 2 is much closer than round 1 and resolves the previously identified ordering problems around workspace membership, fixture modes, EBADF, renderer tests, and the painter dependency. The remaining hard failures are still forward-reference/per-commit-green issues:

1. the frontend supervisor is split into partial commits whose acceptance tests require later lifecycle pieces;
2. c21 references an unowned frontend reaper panic hook;
3. c32/c33 need a CLI binary-resolution helper that is not introduced until c34.

Fix those before handing this plan to per-commit implementers.
