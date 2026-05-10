# commits.md round-7 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `42d4bc1`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 7 fixes the substantive round-6 items: c29 now has a bounded success tail, c21 exposes `lock_fd_for_test()` under `#[cfg(any(test, feature = "test-fixture"))]`, c09 owns the full `Capabilities` shape plus `tui_default()`, c02 is idempotent against the live `chrono` dependency, and the duplicate Round-5 banner label is gone. I did not find a dependency cycle. The remaining issues are mostly prompt/acceptance precision gaps near the final `rfl chat` harness.

## High-priority findings

### H1 — c30 owns the fixture harness, but the row does not fully specify or test the c31 contract

- **Where:** c30 `What` / `Acceptance`; c31 `What`.
- **Problem:** c31 is explicitly test-only, but its headline test requires code that must already exist in c30: when `RFL_HARNESS_FIXTURES=1`, `rfl chat` must finalize exactly nine entries (eight built-in kinds + one unknown) and then publish `core.lifecycle.test_done` so headless `rfl-tui` exits promptly. c30 currently says only "in-test fixture-entry harness" and that it uses constructors / `serde_json::json!`; it does not spell out the nine-entry inventory or the required `core.lifecycle.test_done` publish. c30 acceptance also does not exercise the harness path.
- **Why it matters:** Per the milestone convention, per-commit agents get the row text verbatim. A c30 implementation can pass its two listed tests while omitting `test_done` or only partially implementing the fixture entries. Then c31, being test-only, cannot remain green without changing production/test-harness code that belonged in c30.
- **Fix:** Expand c30 `What` to enumerate the fixture harness contract: one finalized entry for each built-in constructor plus one unknown-kind entry, using `Capabilities::tui_default()`, then publish `core.lifecycle.test_done`. Add a c30 acceptance test for the harness path, or move the headline `rfl_chat_demo_bar.rs` implementation into c30 and leave c31 for documentation/manual artifacts only.

## Medium-priority findings

### M1 — c30 does not tell its implementer to use `Capabilities::tui_default()`

- **Where:** c09 says c30 calls `Capabilities::tui_default()`; c30 `What` / `Depends on` omit it.
- **Problem:** Round 7 added the default TUI capabilities in c09 specifically so c23/c30 do not invent local caps values. But the c30 row, which is what the implementer receives, never says to construct `let caps = Capabilities::tui_default()` for replay/harness calls, and its `Depends on` omits c09.
- **Fix:** Add `Capabilities::tui_default()` explicitly to c30 `What`, and add c09 to c30 `Depends on` if following the stated dependency convention literally.

### M2 — current-draft metadata still says round 6, and the split checkpoint names the wrong commit

- **Where:** top status banner; `m3a / m3b checkpoint` section.
- **Problem:** The banner still says `Status: round-6 draft` even though this is the round-7 draft at `42d4bc1`. The checkpoint says re-evaluate after **c13** "(renderer pipeline complete)", but renderer work completes at c12; c13 is the first broker-frontend commit.
- **Fix:** Mark the banner as round 7 / round-7 cleanup of `commits-pi-review-6.md`, and change the checkpoint text to c12 if the intended checkpoint is renderer completion (or rename the c13 checkpoint rationale).

### M3 — c31 should explicitly set `RFL_TUI_PATH` via `workspace_bin_path("rfl-tui")`

- **Where:** c31 `What`; c28 dependency implies the helper but c31 does not say to use it.
- **Problem:** The headline test spawns `rfl chat` with `RFL_HARNESS_FIXTURES=1` and `RFL_TUI_TEST_MODE=1`, but the row does not explicitly set `RFL_TUI_PATH`. In a test environment there is usually no installed sibling `rfl-tui`; scope.md says the path should be provided via `workspace_bin_path("rfl-tui")`.
- **Fix:** Add that env setup to c31 `What` so the test is deterministic outside developer machines with a coincidental sibling binary.

## Summary

Round 7 resolves the prior review's technical blockers. I would do one more precision pass before ratification: make c30's harness contract complete and covered before the test-only c31 commit, wire c30 explicitly to `Capabilities::tui_default()`, and clean up the small status/checkpoint/test-env drift.
