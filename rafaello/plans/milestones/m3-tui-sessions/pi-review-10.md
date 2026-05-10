Round-10 review findings for `rafaello/plans/milestones/m3-tui-sessions/scope.md` @ `0828b19`:

1. **Blocker — impossible frontend spawn ordering for stderr/reaper setup**  
   §F3 spawns the child at line 708, then only configures `.stderr(Stdio::piped())` at lines 730–733. That must happen before `spawn()`. The Phase-B unwind text also references a reaper-spawn step at lines 750–759, but the main Phase-B sequence never actually includes that step. Fix by making the sequence explicit: configure stderr before spawn → spawn child → take stderr → create reaper watch → spawn reaper → continue server/register setup.

2. **High — stderr forwarder is not awaited, so CLI tests can lose TUI log lines**  
   §C2 spawns a child-stderr forwarding task at lines 1693–1712, and replay-order tests rely on forwarded `"rfl-tui: bus.event"` lines at lines 2274–2285. But §C2 step 10 only waits/shuts down the frontend and drops store/controller (lines 1819–1833); it never awaits or drains the stderr-forwarder task. On child exit, `rfl chat` can terminate the runtime before the forwarder flushes final lines, making the combined-stderr assertions flaky. Store the forwarder `JoinHandle` and await it after `frontend_handle.wait()`/`shutdown()` or before returning from error paths.

3. **High — deprecated `nix::fcntl::flock` conflicts with warning-free acceptance**  
   §S5 specifies `nix::fcntl::flock(fd, FlockArg::LockExclusiveNonblock)` at line 1209 and says `Flock` is “a different helper” at lines 1205–1206. In nix 0.29, `fcntl::flock` is deprecated in favor of `fcntl::Flock`, so this will produce deprecation warnings and conflicts with the warning-free doc/build acceptance gate. Use `nix::fcntl::Flock` or call `libc::flock` deliberately with a local wrapper.

4. **Medium — CLI SenderDropped path is specified but not covered at CLI level**  
   §C2 maps `FrontendReadyError::SenderDropped` to `RflChatError::FrontendExitedBeforeReady` and bounded `handle.shutdown()` at lines 1742–1756. The test matrix only has the timeout path (`rfl_chat_frontend_ready_timeout_errors.rs`, lines 2286–2297). Add an `rfl_chat_frontend_exits_before_ready_errors.rs` using the `exit_immediately` fixture mode from lines 2422–2424.

5. **Medium — abnormal reaper outcomes are specified but untested**  
   §C2 requires `WaitFailed(io::Error)` and `ReaperPanicked` to map to abnormal frontend exit at lines 1804–1818, but the matrix has no test seam or test for those branches. Add injected reaper-outcome tests or explicitly mark these branches intentionally untested with rationale.

6. **Low/consistency — W5 says fixture binary is unchanged, but §L1a extends it**  
   W5 says the `rfl-bus-fixture` binary is “unchanged” at lines 429–431, while §L1/L1a adds self-timeout plus four new modes at lines 2409–2443. Reword W5 to mean only the feature gate/required-features setup is unchanged.
