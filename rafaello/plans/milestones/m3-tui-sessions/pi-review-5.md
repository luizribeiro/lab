# pi review 5 — m3 TUI sessions scope

Reviewed: `rafaello/plans/milestones/m3-tui-sessions/scope.md` at commit `a060c3a86d7a5b68563a5ec3668502421c7c0b6a`.

## Verdict

**NOT RATIFIED.** Round 5 fixes several round-4 issues, but the scope still has blocking API/test-seam contradictions around frontend lifecycle ownership and the TUI paint-panic test. A short round-6 edit pass should be enough; most remaining items are consistency or implementation-guidance fixes.

## Blocking findings

### B1 — Frontend lifecycle ownership and shutdown API are internally inconsistent

References: §F1 lines 300-327, §F3 lines 472-485, §F4 lines 496-504, §C2 lines 1166-1194.

The document assigns the same lifecycle resources to two different owners:

- §F1 says `FrontendSupervisor` “owns ... the spawned frontend's lifecycle” (lines 300-302).
- The returned `FrontendHandle` then carries drop-time SIGKILL, the `Server::serve` join handle, `_register_guard`, readiness receiver, and reaper outcome (lines 322-327, 472-485).
- §F4 says `FrontendHandle::Drop` kills the child (lines 499-502), but also says `FrontendSupervisor::shutdown(self)` sends SIGTERM/SIGKILL and awaits the reaper watch (lines 503-506).
- §C2 timeout handling calls `handle.shutdown()` (lines 1166-1168), but no `FrontendHandle::shutdown` API is specified anywhere.
- §C2 step 10 then calls `frontend_supervisor.shutdown().await` after waiting on `frontend_handle.wait()` (lines 1192-1194), but if the handle owns the guard/join/reaper state, the supervisor has no specified managed record to drain.

This is not just wording: it determines where `RegisteredFrontend` lives, who aborts/awaits `server.serve()`, who sends TERM/KILL on timeout, and what object tests should hold. Pick one model and make every section match it:

1. **Handle-owned model:** `FrontendHandle` owns guard, serve join, reaper watch, and exposes `shutdown()`; `FrontendSupervisor` is only a factory and has no cooperative shutdown state.
2. **Supervisor-owned model:** mirror `PluginSupervisor`: supervisor keeps a managed map; handle is observational; `RegisteredFrontend` lives in supervisor-managed state; timeout path calls supervisor shutdown or a per-frontend supervisor method, not `handle.shutdown()`.

Until this is resolved, the timeout path, drop semantics, and shutdown tests are underspecified/unimplementable.

### B2 — `tui_paint_panic_isolation` is specified against a seam that cannot panic in the described way

References: §T3-§T5 lines 1068-1086, §I lines 1476-1490.

Round 5 correctly moves the test out of headless integration mode, but the new unit-test design is still incoherent. The TUI paint path consumes an already-rendered `RenderNode` from core (§T3/§T4); it does not invoke renderer implementations. The test nevertheless says to feed a “panicking render-tree” and wire it “with a renderer that panics on call” (lines 1483-1486).

There is no renderer call in `paint.rs`, and `RenderNode` has a closed listed variant set. A synthetic renderer panic belongs to `renderer_pipeline_panic_falls_through_to_path_a`, not to TUI painting. To make this test implementable, define a paint-level panic seam, for example:

- a test-only painter adapter/trait whose `paint_node` panics for a chosen node;
- a test-only `RenderNode` fixture that triggers a deliberate panic branch in `paint.rs` under `#[cfg(test)]`; or
- a redraw-loop unit test that wraps a closure known to panic and verifies the fallback line plus continued painting of the next entry.

As written, this named acceptance test cannot be implemented faithfully.

## Medium findings

### M1 — Round-5 readiness uses `watch`, but stale `oneshot` text remains in load-bearing sections

References: header lines 10-15, §F1 lines 337-399, §C2 lines 1145-1149, Risks lines 1822-1826.

The main F1 design now uses `tokio::sync::watch::{Sender, Receiver}` and documents the correct idempotent wait pattern. However §C2 still says `FrontendReadyService` “drains a `oneshot::Receiver`” (lines 1147-1149), and Risk #9 says the RPC “drains a oneshot” backing `wait_ready` (lines 1822-1826). These should be updated to watch-channel wording so implementers do not resurrect the round-4 design.

### M2 — `frontend_handle_wait_resolves_on_child_exit` names two different child binaries

References: §F4 lines 510-514, §I lines 1432-1435.

§F4 says the test spawns `rfl-tui` in `RFL_TUI_TEST_MODE` and exits via `core.lifecycle.test_done`. The integration matrix later explicitly corrects this to `rfl-bus-fixture` because the test lives in `rafaello-core/tests/` and cannot rely on `CARGO_BIN_EXE_rfl-tui`. The matrix is the better plan; update §F4 to match it.

### M3 — Env-override end-to-end test says “stub binary” but the stub must speak the readiness protocol

References: §C3 lines 1239-1242, §C2 lines 1135-1177.

`rfl_chat_resolves_tui_via_env_override.rs` sets `RFL_TUI_PATH` to a “stub binary” and expects a clean exit. After round 5, a child that merely starts and exits will produce `FrontendExitedBeforeReady`, and a child that sleeps will hit `FrontendReadyTimeout`. The test stub must adopt `RFL_BUS_FD`, run enough fittings protocol to call `frontend.ready`, and then exit after replay/harness conditions.

Either specify that the “stub” is a protocol-capable fixture, or use the real `rfl-tui` binary for this end-to-end and keep pure `resolve_tui_path_*` tests for synthetic stubs.

### M4 — Frontend-ready timeout test lacks a concrete way to spawn the non-ready fixture

References: §C3 lines 1196-1242, §I lines 1549-1553.

`rfl_chat_frontend_ready_timeout_errors.rs` says to spawn `rfl chat` “against a fixture that never sends `frontend.ready`”. But `rfl chat` only has `RFL_TUI_PATH` for replacing the TUI binary; the scope does not define argv/env hooks for selecting an `rfl-bus-fixture` mode or any protocol-capable non-ready stub. A binary that never adopts the bus may test transport failure or early exit rather than readiness timeout.

Specify the fixture binary and launch contract for this test: e.g. a tiny test helper executable that adopts `RFL_BUS_FD` and holds the connection open without calling `frontend.ready`, or a supported `rfl-bus-fixture` mode that can be selected through env alone.

### M5 — Frontend publish namespace error classification erases `UnknownNamespace`

References: §B2 lines 583-586, §B4 lines 612-618, negative matrix lines 1560-1564.

§B2 keeps `UnknownNamespace { publisher, topic }`, but §B4 says every top-level segment other than `frontend.<own-attach-id>` maps to `PublishOnReservedNamespace`. That would classify `evil.foo` as reserved for frontends, unlike plugin publish handling and unlike the m1 manifest-tightening distinction between forbidden known namespaces and unknown namespaces.

Recommended rule: for frontend publishers, `core`, `provider`, `plugin`, and `frontend.<other/malformed>` are reserved/forbidden; truly unknown first segments remain `UnknownNamespace`. Add a frontend unknown-namespace test if this distinction is intentional.

### M6 — Frontend ACL validation mentions patterns only, not frontend publish topic validation

References: §B1 lines 542-546, §B6 lines 627-631.

`FrontendAcl` includes `publish_topics`, and §B4 makes exact grants authoritative, but §B6 only extends pattern revalidation to frontend subscribe/auto-subscribe patterns. Existing plugin ACL construction validates both publish topics and subscribe patterns. The frontend map should get symmetric validation of `publish_topics`, even though m3's built-in TUI grant is empty. Add or name an invalid frontend publish-topic test.

### M7 — `EntryFallback` and some entry/render helper types are used but not defined in the m3 API surface

References: §E1 lines 822-832, §E2 lines 867-872, §R3 lines 920-971.

The plan uses `EntryFallback` in `Entry` and `RenderNode::Unknown`, and §R3 depends on fields `text`, `markdown`, and `summary`, but §E never defines the struct, serde shape, optionality, or defaulting rules. Similar smaller gaps exist for helper enums/structs such as `EntryAuthor`, `RawFormat` serde names, and render-node payload structs.

The RFC gives enough hints, but the milestone scope should pin the Rust/wire shape because SQLite rows and bus payloads depend on it.

### M8 — Session-lock implementation guidance uses the wrong nix symbol and an undeclared `libc` path

References: §S5 lines 776-781, workspace deps in `rafaello/Cargo.toml`.

The scope says:

```rust
.custom_flags(libc::O_CLOEXEC)
nix::fcntl::flock(fd, Flock::LockExclusiveNonblock)
```

With the current `nix = 0.29`, the enum is `nix::fcntl::FlockArg::LockExclusiveNonblock`; `Flock` is a different helper type. Also, the workspace does not add a direct `libc` dependency. Use `nix::libc::O_CLOEXEC` or add `libc` explicitly, and write the flock call with `FlockArg`.

### M9 — Markdown fallback path is underspecified and contradicts the listed built-ins

References: §R3 lines 920-931, §E3 built-ins lines 875-897.

§R3 says fallback markdown “re-enters Path A” and that “the markdown path is itself a built-in kind in m3”, but §E3 lists `text` with `{ markdown: bool }`, not a separate markdown kind or markdown parser dependency. If m3 is not adding a markdown-to-render-tree parser, the fallback rule should probably be the simpler text fallback. If `text(markdown=true)` is supposed to parse markdown, specify the parser/dependency and expected render tree.

### M10 — Replay-withheld timing assertion lacks a deterministic start marker

References: §T2 lines 1044-1050, §I lines 1534-1548.

The test asserts no stderr `bus.event` lines in the first 100 ms “after rfl-tui starts”, then three lines within 50 ms after the 200 ms delay. There is no specified startup sentinel from `rfl-tui`, so the harness cannot reliably know when that 100 ms window begins. CI scheduling can also make the 50 ms post-delay bound flaky.

Add a deterministic headless startup marker on stderr, or assert ordering using the readiness delay plus eventual output without tight wall-clock windows.

### M11 — Acceptance allows macOS failures to be gated and deferred despite a cross-platform user-facing milestone

References: Acceptance summary lines 1926-1929 and Risks lines 1768-1781.

The plan requires Linux devshell green, but says macOS CI failures get per-test gates and retrospective follow-up rather than blocking ratification. That may be acceptable for tests that are inherently Linux-only, but m3's deliverable is a user-facing TUI/session path and the risks explicitly call out macOS flock/terminal behavior. If macOS is a supported target for v0.1, the acceptance gate should require the core non-Linux-specific suite and at least the manual interactive smoke to pass on macOS before close.

## Low findings

### L1 — TUI startup step reference typo

Reference: §T2 line 1023.

`RFL_TUI_TEST_MODE` says “if `=1`, see step 5”, but headless test mode is step 4. Step 5 is production mode.

### L2 — §C2 has a formatting break in the timeout bullet

Reference: §C2 lines 1166-1169.

The timeout bullet runs `handle.shutdown()` and the bold “The gate is enforced...” sentence onto the same physical line. This is harmless but makes the load-bearing readiness paragraph harder to read.

### L3 — `CARGO_TARGET_DIR` fallback remains fragile for cross-package `rfl-tui` lookup

References: §I lines 1514-1519.

The plan acknowledges the env-override approach, but the fallback to `target/debug/rfl-tui` from the workspace root can still fail under custom target dirs, profile differences, or `nextest`-style runners. This is not a blocker because `RFL_TUI_PATH` is the real contract, but a tiny helper using cargo metadata or a test-built helper binary would be less brittle.

## Summary

Round 5 converges on the right architecture: core-side rendering, a trusted local TUI process, SQLite session persistence, and a readiness gate before replay. The main remaining problems are not architectural; they are precision issues that would make commit planning painful. Resolve the frontend lifecycle ownership model, replace the impossible paint-panic test seam, and clean up the stale readiness/test fixture text. After that, the medium items are straightforward spec-tightening edits rather than design resets.
