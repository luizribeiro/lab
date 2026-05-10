# commits.md round-6 adversarial review

Review target: `rafaello/plans/milestones/m3-tui-sessions/commits.md`  
Commit under review: `18c1e32`  
Reviewer: pi  
Verdict: **not ratified yet**

Round 6 fixes the round-5 collapse-cleanup items: c09 no longer self-depends, c08's test count matches its list, Group 6 says six modes, c26 points at c08 for `RenderNode`, and Phase 4 says 31 commits. I did not find a new dependency-cycle blocker. The remaining issues are execution-spec gaps that can still make per-commit agents guess or produce flaky intermediate commits.

## High-priority findings

### H1 — c29 has no specified successful-ready tail, but its acceptance tests need `rfl chat` to terminate cleanly

- **Where:** c29 `What` / `Acceptance`.
- **Problem:** c29 owns only C2 steps 1-7 (`open/spawn/wait_ready`). On the `Ok(Ok(()))` readiness path it prints the parent-side sentinel and, in the full scope, would proceed to step 8. But step 8-10 do not land until c30. c29's acceptance includes successful CLI paths using the real `rfl-tui` in headless mode (`rfl_chat_resolves_tui_via_env_override.rs`, and likely the project-root canonicalisation path), so the command must have some c29-only tail behavior after readiness.
- **Why it matters:** Without an explicit c29 tail, implementers must invent whether to return immediately, wait for the child self-timeout, or call `handle.shutdown()` and drain the stderr forwarder. Those choices affect test runtime and can create flaky missing-stderr assertions because c29 already owns the stderr forwarder join handle.
- **Fix:** Add a c29-only note: after successful readiness, print `rfl-chat: frontend-ready-observed`, then perform bounded `handle.shutdown().await` + `stderr_forwarder.await`, and return success. c30 then replaces that temporary tail with replay/harness/wait plus the cleanup guard.

### H2 — c21's `lock_fd_for_test()` cfg gate is too vague for an integration test

- **Where:** c21 `What`: ``session_id()`, `lock_fd_for_test()` (cfg-gated).``
- **Problem:** The acceptance test `rafaello-core/tests/session_store_lock_fd_not_inherited_by_child.rs` is an integration test. A plain `#[cfg(test)]` accessor is not visible to integration tests because the library is compiled as a normal dependency.
- **Why it matters:** The row only says "cfg-gated" and per-commit agents receive the row verbatim. The green implementation needs the scope's exact gate: `#[cfg(any(test, feature = "test-fixture"))]`, with the mandated `rafaello-core/test-fixture` feature enabled for the test run.
- **Fix:** Spell the cfg out in c21: `#[cfg(any(test, feature = "test-fixture"))] pub fn lock_fd_for_test(&self) -> RawFd`.

## Medium-priority findings

### M1 — c09 under-specifies `Capabilities` for c10/c23/c30 consumers

- **Where:** c09 `What`; downstream c10 capability tests and c23/c30 `caps` parameters.
- **Problem:** c09 says `Capabilities (with raw_formats)`, but c10's acceptance includes `renderer_capabilities_downgrade_unsupported_node.rs`, which requires a `nodes` capability set, and later session/replay code needs a concrete m3 TUI capability value to pass into `finalize_entry` / `replay_history`.
- **Fix:** Expand c09 to own the full m3 `Capabilities` shape from scope §R4, including `nodes` and `raw_formats`, and preferably a helper/constant for the baked-in m3 TUI capabilities (`nodes = all supported nodes`, `raw_formats = {"ansi", "plain"}`) so c23/c30 do not invent it locally.

### M2 — c02 says to add `chrono` to `rafaello-core` even though it is already present in the live baseline

- **Where:** c02 `What`.
- **Problem:** Current `rafaello/crates/rafaello-core/Cargo.toml` already has `chrono = { workspace = true }`. c02 says `[dependencies] adds rusqlite, ulid, chrono`, which is not a literal green edit against the live tree.
- **Fix:** Change c02 to "add `rusqlite` and `ulid`; keep the existing `chrono = { workspace = true }` entry" (or "ensure chrono remains wired").

### M3 — duplicate "Round-5 highlights" header in the status banner

- **Where:** top status/trajectory block.
- **Problem:** There are two consecutive `Round-5 highlights` labels. This is only metadata drift, but it makes the already-long banner harder to trust.
- **Fix:** Collapse the two labels into one, or rename the first to `Round-6 cleanup` and keep a single `Round-5 highlights (kept for trajectory)` block.

## Summary

Round 6 resolves the explicit round-5 blockers, but I would do one more cleanup pass before ratification: specify c29's temporary success-path teardown, make c21's integration-test cfg exact, complete c09's `Capabilities` contract, and make c02 idempotent against the current `rafaello-core` manifest.
