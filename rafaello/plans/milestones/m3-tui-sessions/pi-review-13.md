Reviewed `rafaello/plans/milestones/m3-tui-sessions/scope.md` at `e9b1b6e`.

Findings:

1. **Blocker — `WaitFailed` / `ReaperPanicked` are treated as proof the child exited.**  
   §F4 says `shutdown()` skips SIGTERM/SIGKILL on any `Some(_)` reaper outcome, claiming “the child has already exited” (`scope.md:942-954`). §C2 also drains stderr before teardown because “the reaper has already observed” child exit (`scope.md:1961-1967`) and says shutdown is a no-kill drain for all four outcomes, including `WaitFailed` and `ReaperPanicked` (`scope.md:2006-2018`).  
   But round 13 adds a watcher that can publish `ReaperPanicked` when the reaper task panics/cancels (`scope.md:795-805`), which does **not** guarantee the child exited. This can hang on stderr drain and/or leak the TUI child. Fix: only `Exited(_)` should imply no-kill; abnormal reaper outcomes need a bounded cleanup policy before stderr drain.

2. **High — SIGKILL path still has a PID-recycle race.**  
   `shutdown()` checks the reaper outcome before SIGTERM, but after the SIGTERM grace timeout it immediately sends SIGKILL (`scope.md:957-962`). If the child exits and the reaper publishes between timeout return and SIGKILL, this reopens the same “signal recycled PID” hazard the section is trying to close. Re-check the cached outcome immediately before SIGKILL, or state why the remaining race is accepted.

3. **Medium — stale `self.config` in `rfl chat` orchestration.**  
   §C2 step 7 now says the SenderDropped shutdown is bounded by `self.config.shutdown_grace + self.config.shutdown_kill_grace` (`scope.md:1887-1890`), but this is not inside `FrontendHandle`; it is `rfl chat` flow. Use “the handle’s `FrontendConfig`” or a named local config, not `self.config`.

4. **Medium — env-override E2E test can run for the 60s TUI self-timeout.**  
   `rfl_chat_resolves_tui_via_env_override.rs` only specifies `RFL_TUI_PATH` + `RFL_TUI_TEST_MODE=1` (`scope.md:2066-2077`, `2418-2419`). With no harness fixtures / `test_done`, headless TUI exits only via default `RFL_TUI_MAX_LIFETIME=60` (`scope.md:1720-1723`). Set a short `RFL_TUI_MAX_LIFETIME` or enable a deterministic `test_done` path.

5. **Low — fixture-mode count is stale.**  
   §L1a says “Four new modes” and “All four modes” (`scope.md:2617`, `2654`), but it lists five after adding `signal_ready_then_exit_n` (`scope.md:2620-2650`).
