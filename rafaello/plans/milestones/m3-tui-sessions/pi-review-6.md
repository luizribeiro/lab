# pi review 6 — `scope.md` round-6 adversarial review

Reviewed: `plans/milestones/m3-tui-sessions/scope.md` at `d18a5c4`.

## Verdict

**Not ready to ratify.** Round 6 fixed several round-5 issues, but the draft still contains multiple implementation-blocking contradictions and a few required tests that are either flaky by construction or impossible to implement literally. The biggest remaining risks are: test-only environment variables are scrubbed before the TUI/fixture child can see them, `SessionController` cannot type its broker-publish failures, and two required renderer/TUI tests lack a compilable public/test seam.

## Blocking findings

### B1 — `rfl chat` scrubs the exact env vars its required CLI tests depend on

`FrontendSupervisor::spawn` is specified to `env_clear` and then re-inject only reserved runtime variables plus `env.pass` / `env.set` (lines 497–502). The `rfl chat` flow never specifies that its `CompiledFrontend.env` passes through the test-mode knobs consumed by the child:

- `RFL_TUI_TEST_MODE` (TUI headless mode; lines 1164, 1192, 1700, 1730)
- `RFL_TUI_READY_DELAY_MS` (readiness-delay test; lines 1185–1191, 1730–1735)
- `RFL_FIXTURE_MODE=hold_silent` and `RFL_FIXTURE_MAX_LIFETIME` when `RFL_TUI_PATH` points at `rfl-bus-fixture` (lines 1748–1755)

As written, the headline `rfl_chat_demo_bar` can spawn a production-mode TUI instead of headless mode, and the ready-timeout test can spawn a fixture without the requested mode/lifetime. This is not just underspecified; it invalidates required acceptance tests.

Required fix: specify the exact `EnvPlan` used by `rfl chat` for the bundled frontend, including an allowlist of test-only pass-through variables, and cover it in the CLI tests.

### B2 — `SessionController` publishes broker events but `SessionError` has no broker-publish variant

`finalize_entry` and `replay_history` publish through `broker.publish_core(...)` and say step 3 errors are surfaced as `SessionError` (lines 794–819). But `SessionError` is defined only as `Io`, `Sqlite`, `Serde`, `Locked`, and `SchemaMismatch` (lines 837–839).

That public API cannot typecheck without either dropping broker errors, panicking, or inventing an unlisted conversion. Since the controller is the canonical append → render → publish path, publish failure handling must be explicit.

Required fix: add a `Broker { source: BrokerError }` / `Publish { source: BrokerError }` variant (or change the controller return type) and update the tests to assert it.

### B3 — the `ready-sent` stderr sentinel does not establish the ordering asserted by the replay-withheld test

The TUI prints `"rfl-tui: ready-sent"` **after** the `frontend.ready` RPC call returns (lines 1201–1206). The parent readiness service flips the watch before returning the RPC response (lines 1298–1304). Therefore `rfl chat` can observe readiness, replay history, and enqueue `bus.event` notifications before the TUI has received the RPC response and printed `ready-sent`.

The required test then asserts that no `bus.event` lines appear before `ready-sent` (lines 1737–1744). That ordering is not guaranteed by the specified protocol and can fail under a valid implementation, especially because the RPC response and subsequent notifications share the same connection and scheduling is concurrent.

Required fix: use a parent-side deterministic sentinel after `handle.wait_ready()` returns and before replay starts, or change the TUI sentinel semantics so it is emitted before the parent can possibly observe readiness. As written, the test is flaky by construction.

### B4 — required renderer panic/error tests need a public registry insertion seam that is not specified

`RendererRegistry` is only specified as a `BTreeMap` with `RendererRegistry::with_builtins()` (lines 1097–1101). The positive matrix requires integration tests that “register a test renderer that panics” and one that returns `Err(_)` (lines 1567–1574). Those tests live under `rafaello-core/tests/`, i.e. outside the crate, so they cannot mutate a private map.

Required fix: specify a public/test-supported API such as `RendererRegistry::new()`, `register(kind, Arc<dyn Renderer>)`, and/or `with_renderer_for_test(...)`. Otherwise two named tests cannot be implemented as integration tests.

### B5 — the TUI paint-panic seam uses a `RawFormat` value that cannot exist

`RawFormat` is an enum with only `Ansi`, `Html`, and `Plain` (lines 988–994), and `RenderNode::Raw` uses that enum (lines 1010–1012). The paint-panic unit-test seam says to inject a test closure through `RenderNode::Raw { format: "test", body: <token> }` (lines 1672–1676).

That literal shape is not type-correct Rust and is not valid wire data for the enum. The required `tui_paint_panic_isolation` test is therefore unimplementable as written.

Required fix: add a `#[cfg(test)] RawFormat::Test` variant, use a valid existing raw format plus a test-only magic body token, or define a separate test-only `PaintNode`/hook that does not pretend to be production `RenderNode` wire data.

### B6 — `frontend_handle_wait_resolves_on_child_exit.rs` has two conflicting fixture modes

§F4 says the test spawns `rfl-bus-fixture` in new `signal_ready` mode with `RFL_FIXTURE_MAX_LIFETIME=1` so it exits after one second (lines 576–584). The positive matrix says the same test uses existing `respond_peer_call` mode (lines 1601–1604), which is long-running by design and will not naturally exit without the newly specified lifetime behavior being made explicit there.

This is a test-plan contradiction and can turn the wait test into a hang.

Required fix: choose one mode. Prefer `signal_ready` + explicit `RFL_FIXTURE_MAX_LIFETIME=1`, because it exercises the new readiness-capable frontend path and has a deterministic exit.

## Medium findings

### M1 — lifecycle ownership is still contradicted in `FrontendSupervisor`’s public-surface text

Round 6 states the handle-owned model unambiguously: `FrontendHandle` owns lifecycle state and `FrontendSupervisor` is a pure factory with no per-frontend state (lines 323–331). But F1 still says `FrontendSupervisor` “owns the broker handle and the spawned frontend’s lifecycle” (lines 346–348).

This is exactly the model conflict round 6 claims to resolve. It should be corrected to “owns/configures the broker handle used to construct `FrontendHandle`s; the returned handle owns lifecycle resources.”

### M2 — macOS policy is internally inconsistent

Out of scope still says macOS is “verified post-hoc via origin CI per m2’s §5.7 precedent” (lines 1968–1972). Acceptance says macOS CI is a hard ratification gate and failures must be fixed in m3 (lines 2150–2167).

The acceptance section is probably the intended final stance, but the out-of-scope bullet should be rewritten or removed so implementers do not defer macOS failures.

### M3 — `rfl chat` does not define post-ready abnormal TUI exit semantics

C2 handles “exited before ready” and ready timeout (lines 1313–1324), then later says after replay/harness it waits for TUI exit “clean or otherwise” and calls `shutdown()` (lines 1350–1356). It does not say whether a non-zero TUI exit after readiness makes `rfl chat` exit non-zero, whether the exit status is reported, or whether persisted entries remain success.

For a user-facing command, this should be explicit and tested. Otherwise a TUI crash after replay can be silently treated as a clean `rfl chat` run.

### M4 — public lifecycle/error types are underspecified

Several public types are used but not fully defined:

- `ShutdownReport` is returned by `FrontendHandle::shutdown(self)` but only shown with `exit_status`, `used_sigkill`, `serve_aborted`, and `...` (lines 554–571).
- `FrontendReadyError` is matched by tests/CLI as `SenderDropped`, but no typed enum definition is given alongside `FrontendSpawnError` (lines 399–417, 1313–1323).

These do not have to be large, but the scope should pin their fields/variants because they are part of the test and CLI error surface.

### M5 — `rfl-tui` self-timeout is referenced but not specified as concretely as the fixture timeout

T2 and Risks say the 60-second self-timeout pattern extends to `rfl-tui` test mode (lines 1212–1216, 2071–2074). L1, however, only specifies `RFL_FIXTURE_MAX_LIFETIME` for `rfl-bus-fixture` modes (lines 1858–1862). There is no equivalent normative variable/name/behavior for `rfl-tui` itself.

If the two-level subprocess-chain leak mitigation depends on the TUI self-exiting in headless mode, specify the TUI timeout in T2 directly, including whether it is configurable and what exit code it uses.

## Low findings

### L1 — internal split references `B1-B7`, but the broker section only has `B1` through `B6`

The internal split says “Broker frontend extension (B1-B7)” (line 2114), but the broker section ends at B6 (lines 604–724). This is harmless but stale.

### L2 — the Goal’s integration-test placement omits the final `rafaello/tests/` layer

Goal item 7 says integration tests live under `rafaello-core/tests/` and `rafaello-tui/tests/` (lines 122–124), while the later test-placement section correctly adds `rafaello/tests/` for the headline `rfl chat` tests (lines 1528–1534). The Goal should mention the CLI crate too.

### L3 — `/proc/<pid>/status returns ENOENT` is stronger than “not zombie” and can be PID-reuse sensitive

The Linux zombie test says to assert `/proc/<pid>/status` returns `ENOENT` after 500 ms (lines 585–590). That proves the process disappeared, not merely that it is not zombie, and it is theoretically vulnerable to rapid PID reuse. A more robust assertion is: if `/proc/<pid>/status` exists, its `State:` is not `Z`; otherwise success.

## Summary

Round 6 substantially improved the draft: the handle-owned lifecycle direction, watch-based readiness, real `rfl-tui` env override, reserved-namespace classification, and macOS hard gate are all good moves. The remaining problems are mostly precise spec seams rather than architectural objections.

Before ratification, fix the environment-pass-through contract, add the missing `SessionError` broker variant, repair the readiness sentinel/test ordering, provide the renderer/TUI test seams in type-correct form, and normalize the conflicting fixture mode for `frontend_handle_wait_resolves_on_child_exit.rs`. After those are patched, the remaining medium/low items are straightforward cleanup.
