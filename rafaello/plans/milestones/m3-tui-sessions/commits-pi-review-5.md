# commits.md round-5 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `8e5c742`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 5 addresses the round-4 semantic blockers: c08 now owns `RenderNode`, and the `rafaello` crate explicitly accepts a direct `serde_json` dependency. The remaining issues are mostly cleanup from the c08+c09 collapse, but one of them is still a per-commit sequencing blocker: c09 declares a self-dependency.

## Blockers

### B1 — c09 depends on itself

- **Where:** c09 `Depends on` line.
- **Problem:** `c09 — feat(rafaello-core::renderer): registry + Renderer trait + Capabilities` says `Depends on. c09.`
- **Why it blocks:** The plan's branch/agent sequencing model requires every commit to land only after its declared dependencies. A self-dependency is impossible to satisfy and makes the first renderer commit non-executable as written.
- **Fix:** Change c09's dependency to c08. After the round-5 collapse, c08 is the owner of `Entry`, `RenderNode`, `RawFormat`, and payload types that the renderer surface builds on.

## Medium-priority findings

### M1 — c08 acceptance count says three tests but lists five

- **Where:** c08 acceptance.
- **Problem:** The row says `Three new tests:` but lists five test files: `entry_serde_round_trip.rs`, `entry_stream_state_rejects_open.rs`, `render_node_serde_round_trip.rs`, `render_node_unknown_carries_entry_fallback.rs`, and `entry_constructors_smoke.rs`.
- **Fix:** Change the count to five, or remove the numeric count.

### M2 — Group 6 heading still says five m3 fixture modes

- **Where:** `## Group 6 — rfl-bus-fixture extensions (fixture self-timeout + 5 m3 modes)`.
- **Problem:** c16 now owns six new modes after adding `frontend_bus_publish` for the c20 wiring test.
- **Fix:** Change the heading to `+ 6 m3 modes` or make it count-free.

### M3 — c26 still attributes `RenderNode` to c09 after the collapse

- **Where:** c26 `Depends on` line.
- **Problem:** c26 says `Depends on. c09 (RenderNode), c17 (PaintError type)`, but round 5 moved `RenderNode` into c08.
- **Why it matters:** This is not greenability-blocking once c09's own dependency is fixed, but it violates the file's dependency convention and is another stale c08+c09-collapse reference.
- **Fix:** Change the dependency note to `c08 (RenderNode), c17 (PaintError type)` unless c26 actually references renderer-registry code from c09.

### M4 — Phase 4 still says “After all 32 commits land”

- **Where:** `Phase 4 — driver-owned artifacts` intro.
- **Problem:** The status banner and rows now define 31 commits, but Phase 4 still says `After all 32 commits land`.
- **Fix:** Change `32` to `31`.

## Summary

Round 5 is close, but not ratifiable while c09 has an impossible self-dependency. Fix B1 first, then run a small collapse-cleanup pass over counts and stale c09/32-commit references.
