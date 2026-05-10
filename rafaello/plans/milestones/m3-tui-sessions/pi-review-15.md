# Pi review 15 — m3 TUI sessions scope

Review target: `plans/milestones/m3-tui-sessions/scope.md`  
Commit: `86733f2265346ee1bffe9119c9eb64b708e12fde`  
Scope size: 3169 lines  
Result: **1 blocker / 1 high / 3 medium / 0 low**

## Blocker

### 1. Dead-watch shutdown test seam cannot model the specified probes

- Lines: **1010-1014**, **1037-1043**, **2115-2126**
- Round 15 changes the dead-watch branch to use a no-op liveness probe, described as `kill(pid, 0)`.
- The proposed unit seam is documented as accepting a mock `kill_fn` of type:
  ```rust
  Fn(Pid, Signal) -> Result<(), Errno>
  ```
- That type cannot express the no-op probe. With nix 0.29, the public API is `kill(pid, None)` for signal 0, not `kill(pid, Signal::...)`.
- The test expectation also says the mock `kill_fn` is called twice, but the specified dead-watch path includes more operations when the child remains alive:
  1. SIGTERM
  2. no-op liveness probe
  3. SIGKILL
  4. no-op liveness probe

This makes the round-15 claimed coverage seam under-specified / unimplementable as written. Implementers could either omit the probes from tests, invent a fake signal value, or alter the algorithm to fit the seam.

**Recommendation:** change the seam to one of:

```rust
Fn(Pid, Option<Signal>) -> Result<(), Errno>
```

matching nix's `kill(pid, Option<Signal>)`, or split it into:

```rust
signal_fn: Fn(Pid, Signal) -> Result<(), Errno>
probe_fn: Fn(Pid) -> Result<(), Errno>
```

Then update the expected call sequence/counts explicitly for both dead-watch cases, including probe outcomes (`ESRCH` vs alive / `EPERM`).

## High

### 2. Round-15 coverage text directly contradicts itself

- Lines: **2127-2138**
- The document first says round 15 adds strict blocking coverage for `WaitFailed` and `ReaperPanicked`, and that the prior intentionally-untested framing is retracted:
  > this is the strict-blocking-coverage standard pi-14 asked for; the prior "intentionally untested" framing is now retracted
- The next sentence immediately resumes the old text, saying triggering those paths requires reaper-task fault injection, the branches are defensive, m4 may add fault injection, and the intentional-untested status is filed in retrospective drift.

These statements require opposite implementation/test expectations. One says coverage is mandatory in m3; the other says the branches remain deferred.

**Recommendation:** delete the stale paragraph beginning with “triggering them requires...” or rewrite it narrowly: full end-to-end reaper-task fault injection is deferred, but the shutdown branch behavior itself is covered in m3 by `frontend_shutdown_dead_watch_paths.rs`.

## Medium

### 3. Drop semantics still contain stale always-SIGKILL wording

- Lines: **973-977** vs **1067-1082**
- The early Drop summary says:
  > if `child_pid.is_some()`, send best-effort SIGKILL
- Round 15 later correctly specifies Exited-only skip-kill semantics:
  - `Exited(_)`: skip SIGKILL
  - `WaitFailed(_)` / `ReaperPanicked`: best-effort SIGKILL
  - `None`: best-effort SIGKILL

The early summary contradicts the new round-15 rule and could cause an implementation to kill after a cached `Exited(_)`, reintroducing the PID-recycle hazard round 15 is trying to close.

**Recommendation:** replace the early summary with a pointer to the detailed cached-outcome Drop rules, e.g. “Drop aborts the serve task and releases registration; child signaling follows the Exited-only cached-outcome rules below.”

### 4. `holder_pid: Option<u32>` is not reflected in CLI UX/test coverage

- Lines: **1396-1401**, **1872-1875**, **2555-2559**, **2678-2684**
- Round 15 changes `SessionError::Locked` to:
  ```rust
  Locked { holder_pid: Option<u32> }
  ```
  where `None` means the lock is held by an unknown process.
- `rfl chat` still says it prints a friendly error “citing the holder pid,” which is impossible for `None`.
- The CLI matrix only covers `rfl_chat_locked_session_errors_with_holder_pid.rs`; the new unknown-holder path is covered only at the core store level.

This leaves the user-facing behavior for an empty/corrupt lockfile ambiguous and risks either misleading output or raw debug formatting.

**Recommendation:** specify both CLI messages:

- `Some(pid)`: “session is locked by pid <pid>”
- `None`: “session is locked by another process; holder pid unavailable”

Add a CLI-level negative test, e.g. `rfl_chat_locked_session_errors_unknown_holder.rs`, that creates/holds an empty or corrupt lockfile and asserts the friendly unknown-holder message.

### 5. CLI fixture-backed tests need an explicit cross-crate binary resolver

- Lines: **2343-2365**, **2605-2633**
- The test-placement section correctly notes that `CARGO_BIN_EXE_<name>` is only reliable for binaries in the package whose integration test is being built.
- It then documents how `rafaello/tests/` locates `rfl-tui` via `RFL_TUI_PATH` and a workspace-target helper.
- However, several `rafaello/tests/` negative tests also set `RFL_TUI_PATH` to `rfl-bus-fixture`, which is a binary from the `rafaello-core` package:
  - `rfl_chat_frontend_exits_before_ready_errors.rs`
  - `rfl_chat_frontend_post_ready_nonzero_exit_errors.rs`
  - `rfl_chat_frontend_ready_timeout_errors.rs`

Those tests cannot use `env!("CARGO_BIN_EXE_rfl-bus-fixture")` from the `rafaello` crate. The plan does not explicitly define a resolver for this second cross-crate binary.

**Recommendation:** extend the documented workspace-target helper to resolve both `rfl-tui` and `rfl-bus-fixture`, or add a shared CLI-test helper that computes both paths from `CARGO_TARGET_DIR` / workspace `target/debug` with platform suffix handling.

## Verdict

Round 15 is substantially closer to convergence, especially on shutdown ordering and Exited-only skip-kill semantics. The main remaining ratification blocker is the dead-watch shutdown test seam: as written it cannot represent the newly specified no-op liveness probes and has an incorrect call-count expectation.
