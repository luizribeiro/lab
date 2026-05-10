# m3 retrospective.md — pi review round 3

Review target: `rafaello/plans/milestones/m3-tui-sessions/retrospective.md` at `400426e`.

Verdict: **not ratifiable yet**. Round 3 is materially better: it stops over-greening the manual-validation and drift rows, and it correctly recognizes the macOS TUI-test harness issue as blocking. However, there are still factual mismatches that would mislead the follow-up work.

## Blocking findings

### B1. The positive matrix still has stale/wrong mappings for `tui_subscribes_to_core_session_events.rs`

The reconciliation row at `retrospective.md:76` is mostly honest: the scoped TUI-subscription behavior is split across broker fan-out, TUI stderr logging, TUI test-done exit, and CLI end-to-end `core.session.entry.finalized` tests.

But the positive matrix row at `retrospective.md:108` still says:

> `tui_subscribes_to_core_session_events.rs` | `frontend_subscribes_to_core_session_events.rs` | c14

That is both incomplete and factually wrong:

- `frontend_subscribes_to_core_session_events.rs` landed in c15 (`dbfeebc`), not c14.
- It covers broker-side fan-out only, not the full scoped TUI-process behavior.
- The row drops the other files that round 3 itself says are necessary (`tui_test_mode_logs_bus_events_to_stderr.rs`, `tui_test_mode_exits_on_test_done.rs`, `rfl_chat_demo_bar.rs`, `rfl_chat_replay_withheld_until_frontend_ready.rs`).

Fix the matrix row to match the reconciliation row, including correct commits, or add the exact scope-named test.

### B2. The macOS TUI-harness follow-up root cause is inaccurate

`retrospective.md:584-619` says the TUI test harness is Linux-gated because it calls `socketpair` with `SockFlag::SOCK_CLOEXEC`, and proposes applying the m2 `SOCK_CLOEXEC -> fcntl` fallback.

But the current landed harness already calls:

```rust
nix::sys::socket::socketpair(..., nix::sys::socket::SockFlag::empty())
...
nix::fcntl::fcntl(parent_fd, F_SETFD(FD_CLOEXEC))
```

in `crates/rafaello-tui/tests/common/mod.rs`. It does **not** use `SockFlag::SOCK_CLOEXEC`. The blanket Linux gates are real and blocking, but the proposed fix is not the actual fix as written. At minimum, the follow-up should be rephrased as: remove the direct `#![cfg(target_os = "linux")]` gates from the five TUI integration tests and `common/mod.rs`, run/repair the harness on macOS, and only add platform-specific fd handling if the macOS build/run proves it is needed.

As written, the retrospective sends the driver to apply a non-existent SOCK_CLOEXEC patch and may still leave the real macOS test gating unresolved.

### B3. Final ratification sentence omits the new sixth blocking follow-up

Round 3 adds a required code follow-up item 6 for §5.9, but the final paragraph still says:

> m3 ratifies once macOS CI captures green and the five pre-named follow-up commits land.

That is now incomplete. Ratification also requires the §5.9 TUI-harness macOS-un-gating code follow-up and an ensuing macOS CI run that actually executes those tests.

Fix the closing ratification condition and any similar prose to say five docs/drift commits **plus** the §5.9 code commit, followed by green Linux/macOS evidence and manual smoke capture.

## Non-blocking notes / polish

- `retrospective.md:86` says `frontend_register_with_broker.rs` is covered by “c14 setup paths”; the setup coverage spans c14/c15, and the direct frontend-supervisor unknown-attach test is c18. Consider saying “setup paths across c14/c15” rather than c14 only.
- `manual-validation.md` and the retrospective banner still call the companion “round 1” even though the commit message says manual-validation round 2. Pure bookkeeping, but update before final archive.
- Section numbering has §5.9 before §5.8. Harmless, but easy to clean while editing.
