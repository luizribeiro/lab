Round-3 review of `rafaello/plans/milestones/m3-tui-sessions/scope.md` @ `348baa3`.

Verdict: **not ratifiable yet**, but close. Round 3 fixes the prior round-2 blockers. I found a few remaining blockers around the readiness handshake/test boundary and one likely compile/dependency issue.

## Blocking findings

1. **`frontend.ready` has no exposed wait handle, and the server serve loop is omitted.**

   §F1 defines `FrontendReadyService { tx: oneshot::Sender<()> }`, and §C2 step 7 says `rfl chat` waits on the receiver. But `FrontendSupervisor::spawn(...)` returns only `FrontendHandle`; no API exposes the receiver/future to the caller.

   Also §F3 says “Build a `fittings_server::Server`” and then register/return, but does not say to spawn `server.serve().await` or store/clean up its join handle. Without that, neither `bus.publish` nor `frontend.ready` is processed.

   Refs: `scope.md:285-309`, `358-366`, `1009-1029`.

   Fix: make readiness part of the public spawn contract, e.g. `FrontendHandle::wait_ready()`, `FrontendHandle::ready_rx`, or `spawn -> (FrontendHandle, FrontendReady)`, and explicitly add the frontend serve task + lifecycle cleanup.

2. **Readiness tests contradict the `SessionController` contract / are in the wrong layer.**

   `frontend_ready_replay_withheld_until_signal.rs` says call `controller.replay_history()` and assert no events are emitted before ready. But §S1 says `SessionController::replay_history()` directly renders/publishes entries; readiness gating lives in `rfl chat` orchestration (§C2), not in the controller.

   Similarly, `frontend_ready_timeout_errors.rs` is listed under `rafaello-core/tests/` but asserts `RflChatError::FrontendReadyTimeout`, which belongs to the `rafaello` binary/crate layer, not core.

   Refs: `scope.md:1274-1287`, `1009-1030`.

   Fix: either introduce a core-level orchestration helper that owns the ready gate and is testable from core, or move these tests to `rafaello/tests/` and assert CLI behavior.

3. **`ulid = "1"` likely lacks serde support for `Entry` JSON round-trips.**

   §E1 uses `Ulid` directly in serializable `Entry`, and §E4 requires exact JSON serialization. The `ulid` crate usually needs the `serde` feature for deriving serde on `Ulid`.

   Ref: `scope.md:196`, `707-708`.

   Fix: specify `ulid = { version = "1", features = ["serde"] }`, unless the plan explicitly requires custom string serializers for ULIDs.

## Medium findings

4. **`CARGO_BIN_EXE_rfl-tui` resolution is still overstated.**

   §C3 says `option_env!("CARGO_BIN_EXE_rfl-tui")` may support `cargo run -- chat`. Cargo does not generally set `CARGO_BIN_EXE_*` for normal binary builds, especially cross-package. The sibling lookup only works if `rfl-tui` was built separately.

   Ref: `scope.md:1059-1065`.

   Fix: document dev flow as `cargo build --workspace --bins` or require `RFL_TUI_PATH` for cargo-run workflows.

5. **Risk section still uses the old bus-topic readiness name.**

   Risk #9 says §C2 waits on `core.lifecycle.frontend_ready`; round 3 renamed this to RPC method `frontend.ready`.

   Ref: `scope.md:1597-1602`.

6. **`rfl` depending on `rafaello-tui` is probably unnecessary.**

   §W4 adds `rafaello-tui` as a dependency of the `rafaello` crate. Since the TUI is a separate binary found by path, linking the TUI library into `rfl` looks contrary to the process separation and adds ratatui/crossterm to the CLI unnecessarily.

   Ref: `scope.md:225-227`.

## Summary

Round 3 successfully addresses the prior review’s main issues. The main remaining blocker is that the readiness handshake is now conceptually right but not API-complete/testable as written. Fix the ready-wait surface, the serve-loop lifecycle, and the readiness tests’ ownership layer, then this should be close to ratifiable.
