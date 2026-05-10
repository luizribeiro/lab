Round-4 review for `rafaello/plans/milestones/m3-tui-sessions/scope.md` at `538ba6b`.

Verdict: **needs another revision**. Round 4 resolves the big readiness/supervisor direction, but a few spec holes can still lead to unimplementable or subtly broken commits.

## Findings

### Blocking

1. **Frontend broker registration RAII guard is not owned anywhere**
   - `RegisteredFrontend` is specified as an RAII guard mirroring `RegisteredPlugin` (`scope.md:520-524`), but `FrontendHandle`’s carried fields omit it (`scope.md:293-314`), and spawn just says “Register … Return handle” (`scope.md:414-417`).
   - If implemented literally, the guard drops at the end of `spawn`, immediately unregistering the frontend; replay fan-out to the TUI then fails.
   - Fix: explicitly store `RegisteredFrontend` in `FrontendHandle` or in supervisor-owned managed state.

2. **`wait_ready()` idempotency is not implementable with a bare oneshot receiver**
   - The plan says `FrontendHandle` carries `ready: oneshot::Receiver<()>` and second `wait_ready()` returns `Ok(())` because the receiver “retains the observed value” (`scope.md:300-312`).
   - Tokio oneshot is not a replayable latch. Also `has_signalled_ready(&self)` cannot peek a plain receiver cleanly (`scope.md:313-314`).
   - Fix: specify explicit cached readiness state, e.g. `ready_seen: AtomicBool`/`Mutex<Option<Receiver<_>>>` or a `watch` channel, and add a test for second-call behavior.

3. **Readiness error naming is inconsistent**
   - Spec says `FrontendReadyError::Sender Dropped` (`scope.md:305`), test expects `FrontendReadyError::SenderDropped` (`scope.md:1346-1350`).
   - Fix to one canonical variant name.

### High / Medium

4. **Early child exit before readiness is not mapped at the CLI layer**
   - C2 only describes timeout handling (`scope.md:1079-1083`), while the core test expects `SenderDropped` when the child exits before signaling (`scope.md:1346-1350`).
   - `rfl chat` should specify immediate failure on `SenderDropped` / connection close, distinct from `FrontendReadyTimeout`.

5. **The replay-withheld-until-ready CLI test introduces an unspecified test seam**
   - `rfl_chat_replay_withheld_until_frontend_ready.rs` relies on `RFL_CHAT_TEST_OBSERVER_FD` and “a test-side broker subscriber injected as an extra service into the spawned `rfl chat` process” (`scope.md:1408-1421`), but no implementation surface for that env var/seam is defined elsewhere.
   - Fix: either fully specify the test-only observer protocol/feature gate, or make the fake TUI/fixture log received events and assert via stderr.

6. **Paint-panic test conflicts with headless test-mode behavior**
   - Headless mode explicitly skips terminal init and only logs received `bus.event`s (`scope.md:976-983`), while the named test claims it exercises painter panic isolation (`scope.md:1364-1369`) and risk mitigation says all TUI integration tests run headless (`scope.md:1623-1626`).
   - If headless mode never paints, this test does not exercise §T5 (`scope.md:1018-1022`).
   - Fix: define a pure library-level “paint one frame into a test backend” path, or clarify that headless mode still runs the painter against an in-memory backend.

### Low

7. **`RFL_TUI_PATH` negative test may be flaky**
   - The negative test “unsets both” lookups (`scope.md:1129-1134`), but sibling lookup uses `current_exe().parent().join("rfl-tui")` (`scope.md:1125-1128`). A previous workspace build may leave `target/debug/rfl-tui` beside `rfl`, making the negative unexpectedly pass resolution.
   - Fix: factor path resolution into a testable function with injectable `current_exe`/sibling dir.

Overall: strong convergence, but fix the RAII ownership and readiness state semantics before turning this into `commits.md`.
