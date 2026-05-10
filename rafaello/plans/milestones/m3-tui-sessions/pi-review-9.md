# pi review 9 — scope.md round-9 adversarial review

Verdict: Not ready to proceed as-is; address the blockers before implementation planning.

Round-9 review findings for `rafaello/plans/milestones/m3-tui-sessions/scope.md` @ `403cf16`:

- **Blocker — `FrontendHandle::shutdown` is specified twice with conflicting behavior.**  
  Locations: lines 769-809 vs 810-832.  
  The first contract is the newer disarmable/reaper-outcome-checked/config-duration flow; the second is stale, hardcodes `2s/1s`, uses old `_register_guard` naming, and omits PID-recycle protection. Delete the stale “Cooperative shutdown” block or fold it into the canonical contract.

- **Blocker — `rfl chat` calls consuming `shutdown(self)` twice.**  
  Location: lines 1740-1749.  
  The abnormal-exit path says to run `frontend_handle.shutdown().await`, then immediately says “Then call” it again. Since `shutdown` consumes the handle, this cannot compile. Specify one shutdown call per control path.

- **Blocker — stderr forwarding has no API surface.**  
  Locations: lines 508-560 and 1635-1644.  
  `rfl chat` must read the child’s piped stderr, but `FrontendHandle` exposes only lifecycle/readiness fields and no `ChildStderr`/forwarder handle/accessor. Add a field/API such as `take_stderr()` or have `FrontendSupervisor::spawn` accept a stderr policy/callback.

- **High — post-spawn unwind conflicts with reaper ownership.**  
  Locations: lines 706-720 and 735-741.  
  The unwind rules say to `child.start_kill()` and `child.wait().await`, but F4 says the reaper task owns the `Child`. Once moved into the reaper, spawn failure cleanup can only signal by pid and await the reaper outcome; it cannot wait on `child` directly. Pin the exact order: either start the reaper only after all fallible setup, or make unwind go through the reaper channel.

- **High — `SenderDropped` handling can hang indefinitely.**  
  Location: lines 1674-1680.  
  The plan treats sender-drop as “child exited / connection closed” and then does unbounded `handle.wait().await`. If the bus connection closes before ready but the process stays alive, `rfl chat` hangs instead of exiting. Use bounded wait + `shutdown()`, or shutdown first and then await the reaper.

- **Medium — session lock fd test lacks a way to obtain the fd.**  
  Locations: lines 1005-1052 and 1157-1169.  
  The test must pass the lock fd number to `probe_fd_closed`, but `SessionStore`’s public/test surface does not expose it. Add a cfg-gated `lock_fd_for_test()`/`AsRawFd` accessor or revise the test mechanism.

- **Medium — combined stderr ordering guarantee is underspecified/overstated.**  
  Locations: lines 1635-1642 and 2180-2188.  
  The plan claims line ordering “by construction,” but parent writes and forwarded child writes need an explicit single serialized writer/lock/channel. Also TUI-side lines already include `rfl-tui:` (lines 1499-1502) and the forwarder adds `rfl-tui:` again, producing double prefixes unless changed.

- **Medium — stale `ReaperOutcome` shapes remain.**  
  Locations: lines 840-841, 1728, 2037-2039.  
  The plan elsewhere says `ReaperOutcome::Exited(ExitStatus)`, but tests still say `Exited(0)`, and C2 says `WaitFailed { errno }` while current m2 shape is `WaitFailed(std::io::Error)`. Update assertions to `Exited(status) if status.success()` and describe `WaitFailed(e)` / `raw_os_error()` handling.

- **Low — fixture lifetime section contradicts test matrix.**  
  Locations: lines 838-840, 2210-2212, 2360-2364.  
  L2 says tests don’t override the default, but several tests intentionally set `RFL_FIXTURE_MAX_LIFETIME`. Rewrite L2 as “default unless a test needs a deterministic shorter bound.”

- **Low — fixture mode count is stale.**  
  Locations: lines 2329-2359.  
  It says “Three new modes” / “All three modes” but lists four (`signal_ready`, `exit_immediately`, `hold_silent`, `probe_fd_closed`).
