Round-11 review result: **needs another small revision pass**.

### Findings

1. **Blocking — `FrontendHandle::shutdown()` uses config that the handle does not own**
   - Refs: §F1, §F3 step 14, §F4 shutdown steps 4–5.
   - `shutdown(self)` references `config.shutdown_grace` / `config.shutdown_kill_grace`, but `FrontendHandle`’s specified fields do not include `FrontendConfig` or copied durations.
   - Fix: store `config: FrontendConfig` or the two durations on `FrontendHandle`, and include it in §F3 step 14.

2. **High — `SessionStore` does not explicitly retain the `Flock<File>` guard**
   - Refs: §S1, §S5.
   - §S5 correctly switches to `nix::fcntl::Flock`, but §S1 still says `SessionStore` owns only a rusqlite connection behind a mutex. If the `Flock<File>` is not a `SessionStore` field, the lock can drop at the end of `open()`.
   - Fix: specify e.g. `lock_guard: Flock<File>` as a `SessionStore` field and state it is retained until `SessionStore` drop.

3. **High — round-11 stderr-forwarder drain is incomplete for pre-ready error paths**
   - Refs: §C2 step 6, §C2 step 7, §C2 step 10.
   - The forwarder `JoinHandle` is only awaited in step 10, but `SenderDropped` and timeout exits return from step 7 after `handle.shutdown().await` and never reach step 10.
   - Fix: in both step-7 error arms, drain `stderr_forwarder.await` after shutdown and before returning the CLI error.

4. **High — abnormal-exit coverage claim is false**
   - Refs: §C2 step 10 “Test coverage”, §I `rfl_chat_frontend_exits_before_ready_errors.rs`, §L1a `exit_immediately`.
   - The cited test uses `exit_immediately`, which exits `0` before `frontend.ready`. That exercises the step-7 `SenderDropped` path, not the step-10 `Exited(!success)` branch.
   - Fix: either remove the coverage claim or add a dedicated post-ready nonzero-exit test/mode, e.g. `signal_ready_then_exit_nonzero`.

5. **Medium — replay-withheld assertion text is truncated / stale**
   - Ref: §I `rfl_chat_replay_withheld_until_frontend_ready.rs`.
   - The final bullet ends with “exactly three `rfl-tui: bus.event` lines follow within sentinel.” That is incomplete.
   - Same paragraph still says ordering is produced “from a single forwarding task that runs after `wait_ready` resolves”, which conflicts with §C2 step 6’s updated mutex-based parent/forwarder scheme.
   - Fix: say precisely: no `rfl-tui: bus.event` before the sentinel; exactly three after it before process exit.

6. **Low — stale `Flock` wording remains**
   - Ref: §S5.
   - The parenthetical still says “`Flock` is a different helper” immediately before requiring `nix::fcntl::Flock`. This is now confusing.
   - Fix: delete/update that old aside.

### Verdict

Architecture is still sound, and the round-11 fixes mostly land correctly, but the handle config omission and incomplete forwarder-drain path should be fixed before ratifying round 11.
