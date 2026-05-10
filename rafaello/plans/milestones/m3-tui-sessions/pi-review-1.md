Round-1 adversarial review of `rafaello/plans/milestones/m3-tui-sessions/scope.md`.

Verdict: **not ratifiable yet**. The shape is mostly coherent, but there are several blocking contract gaps around session persistence, replay ordering, subprocess lifecycle, and test feasibility.

## Blocking findings

1. **Replay is published before the TUI exists.**

   `rfl chat` publishes replay events in step 5, then spawns/registers the TUI in step 6 (`scope.md:685-694`). The current broker fan-out is to registered peers only; there is no queue. Result: the TUI misses all replay history.

   Fix: spawn/register the TUI first, then replay, or define an explicit durable replay request path. Also update T3: it says the TUI subscribes to `core.session.entry.finalized` only (`642-643`), but replay uses `core.session.entry.replay` (`687`).

2. **Fixture-entry harness bypasses persistence, but the demo expects SQLite rows.**

   The draft says fixture entries are injected directly through `broker.publish_core` (`65-69`, `695-700`), while `SessionStore::append_entry` is only an isolated API (`389-391`). Nothing says “on finalized entry, append to SQLite, render, then publish.” Yet the headline test expects nine persisted rows (`838-843`).

   Fix: introduce an explicit `SessionController` / `EntryPublisher` path: `finalize_entry(entry)` must append, render, and publish atomically-ish. The fixture harness should call that, not raw `publish_core`.

3. **Session lock fd inheritance is incorrectly dismissed.**

   The draft claims “fork inheritance is not an issue because m3 never forks with the lock held” (`449-452`), but `rfl chat` opens the store/lock before spawning `rfl-tui` (`675-694`). That is a fork+exec with the lock held. If `session.lock` is not opened with `O_CLOEXEC` / `FD_CLOEXEC`, the child can inherit the lock fd and keep the lock alive after parent failure, with a stale holder pid.

   Fix: require `O_CLOEXEC` on the lock fd and test that the TUI process does not inherit it.

4. **Frontend process lifecycle lacks reaping semantics.**

   F4 says `FrontendHandle::Drop` SIGKILLs if alive and shutdown does SIGTERM/SIGKILL (`296-299`), but it does not require `wait()` / a reaper task. That is a zombie leak in tests and long-running parents. m2 already learned this lesson.

   Fix: frontend supervisor needs the same managed child/reaper model as plugin supervisor, or a clearly specified `kill + wait` path, with a test.

5. **`CARGO_BIN_EXE_rfl-tui` in `rafaello-core` tests is probably invalid.**

   The harness under `rafaello-core/tests/common` calls `env!("CARGO_BIN_EXE_rfl-tui")` (`891-904`), but `rfl-tui` is a binary of a different package (`rafaello-tui`). Cargo only reliably exposes `CARGO_BIN_EXE_*` for binaries of the package whose integration test is being built.

   Fix: move those tests to the `rafaello-tui` package, make the bin a target of the package under test, or add a robust workspace test binary resolver.

6. **Installed `rfl` has no specified way to find `rfl-tui`.**

   C2 says spawn the TUI via `CompiledFrontend` (`693-694`), but the scope never defines how `rfl` resolves the bundled `rfl-tui` path in dev, cargo tests, nix package, and installed distributions. “Compile-time constants” is only mentioned abstractly.

   Fix: specify resolution order, e.g. explicit env override for tests, `current_exe` sibling lookup for installed package, and nix wrapper behavior.

7. **Headless TUI tests have no deterministic exit condition.**

   The headline test spawns `rfl chat` with `RFL_HARNESS_FIXTURES=1` and expects shutdown + SQLite assertions (`838-843`). But TUI loop exits only on keyboard quit (`634-640`), and the mitigation is a 30s max lifetime (`1107-1109`). That makes the headline test slow/flaky.

   Fix: in `RFL_TUI_TEST_MODE`, exit after receiving/rendering an expected event count or after an explicit `core.lifecycle.test_done` event.

8. **Workspace dependency plan misses required changes.**

   Frontend supervisor uses `tokio::process::Command` (`274`), but current workspace tokio features do not include `"process"` (`rafaello/Cargo.toml:16`). W3 also forgets to explicitly add `crates/rafaello-tui` to workspace members. W1 says add `chrono` with different feature flags (`163-165`) even though chrono already exists in workspace with a different declaration.

   Fix W1/W3: update existing `chrono`, add `tokio` `process`, and add the new workspace member.

9. **Renderer fallback semantics contradict Stream E.**

   R3 says unknown entry kind emits `Unknown { kind, payload, fallback }` (`562-568`). Stream E §6 says unavailable renderer with author fallback emits a `Block/Text`, and no fallback emits a default `Callout`. The test later expects no-fallback → `Callout` (`824-827`). Also downgrade says `Unknown { ..., fallback }` where fallback may be “a default Block” (`580-584`), but E2 defines `Unknown.fallback` as `EntryFallback`, not a `RenderNode`.

   Fix: separate:
   - unknown entry kind fallback rendering: `Block/Text` or `Callout`;
   - unsupported render-tree node downgrade: `Unknown { fallback: EntryFallback }`.

10. **Broker error/API symmetry is under-specified.**

   B2 says frontend registration errors are `NotInAcl` / `AlreadyRegistered` (`335-339`), and B5 says existing publisher-bearing variants accept frontend “without API breakage” (`351-356`). But current `BrokerError::{NotInAcl, AlreadyRegistered, PublishOutsideGrant, InvalidInReplyTo}` are plugin/`CanonicalId`-shaped. Frontend support is not just adding `Publisher::Frontend`.

   Fix: explicitly define new/generic error variants, e.g. `FrontendNotInAcl`, `FrontendAlreadyRegistered`, `PublishOutsideGrant { publisher, topic }`.

## Smaller required cleanups

11. **M1 namespace tightening wording contradicts itself.**

   M1.1 says first segment must be one of `{plugin, frontend}` (`773-775`), then says `frontend.*` is rejected (`779-781`), and M1.2 expects `frontend.foo` rejected (`782-784`). Say “must be `plugin`” for plugin manifests.

12. **`seq` ownership is ambiguous.**

   DB `seq` is server-assigned (`419-420`), but `EntryMetadata` also has `seq: Option<u64>` (`496`). Define whether append mutates/populates metadata on load, rejects mismatched seq, or removes metadata seq from finalized v1 entries.

13. **Acceptance build command is wrong for fixture bin.**

   The acceptance command is `cargo build --workspace --bins` (`1172-1175`) but says it verifies `rfl-bus-fixture` “with `--features test-fixture`”. The command lacks that feature, so the required-feature fixture bin will not be built.

Overall: good m3 direction, but the draft needs one more pass to make the event/session pipeline and subprocess/test contracts precise enough for `commits.md`.
