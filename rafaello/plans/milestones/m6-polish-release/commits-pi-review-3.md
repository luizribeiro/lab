# m6 commits.md round-3 pi review

> Verdict: blocking.
> Counts: B/5 M/4 N/3

Reviewed round-3 `commits.md` at `7f4d415`, `commits-pi-review-2.md`, `commits-pi-review-1.md`, ratified `scope.md`, owner ratification commit `a0764b3`, and the live closure targets named in the prompt. Round 3 fixes the obvious forward dependency and the `LoadPolicy` field-name mismatch, but it is not converged: several rewritten rows still describe behavior the live APIs do not provide.

The remaining failures are mechanical rather than philosophical. c10 still shows tests using a private field on `SandboxedCommand`; c14 assumes tokio spawned-task panics terminate the HTTP stub process, which the live serve loop does not do; c18's Linux/macOS shell step uses GNU `find -printf`; and the c24a/c24b lazy-load split still does not have enough live state/process-boundary plumbing to implement or observe spawn-on-demand.

## Round-2 follow-up

| prior id | status | round-3 result |
|---|---|---|
| B-1 c04 forward dependency on c06 | closed | c04 is self-contained again; in-tree bundled-openai smoke moved to c07 after c06 |
| B-2 c09/c10 lockin command-building mismatch | partially open | c09 option-A resolve/store shape matches live constructors, but c10's test snippet still uses a private `SandboxedCommand` field |
| B-3 c23 wrong crate/process for ordering hook | closed | test moved to `rafaello/crates/rafaello/tests/` and uses `RFL_STARTUP_ORDERING_LOG` file mode |
| B-4 c24 nonexistent `bindings.load.triggers` | partially open | field shape fixed to `LoadPolicy::Lazy { command }`, but spawn-on-demand API lacks plan/path state and test observability is still wrong |
| B-5 c14/c15 HTTP 400 vs panic | partially open | text restores panic semantics, but the live tokio spawn model will not make that panic exit the process as claimed |
| M-1 stale appendix | mostly closed | Phase C/E/I appendix entries updated; c28 row-number prose still stale |
| M-2 c24a vague supervisor owner/types | partially open | supervisor path pinned, but `ToolSpawnError`/`spawn_on_demand(canonical)` shape does not match live state |
| M-3 c24a wrong row-60 decision ref | closed | row 60 removed; row 42 + new lazy-load placeholder used |
| M-4 c17 nix-build-in-cargo-test | closed-but-related | Cargo test removed; new CI shell step has a macOS portability bug |
| N-1 c10 record-env arg-loop wording | closed | now uses top-level `RFL_BUS_FIXTURE_RECORD_ENV` env var |
| N-2 c15 fixture lifetime env | closed | override wording dropped |
| N-3 retro numbering note | partially open | c29 wording removed, but c28 still says rows 59–68 while listing 69 |

## Blockers

### B-1. c10 still uses a private `SandboxedCommand` field in its public-API test

**Anchor:** commits.md c10 / live `lockin/crates/sandbox/src/lib.rs:655-770`

**Issue:** c10 correctly switches from private `SandboxBuilder::build()` to public `.command(...)`, but the sample test then does `cmd.command.status()?`. Live `SandboxedCommand` has a private `command: Command` field and no public `status()` method. The public execution path is `spawn()` followed by `SandboxedChild::wait()`.

**Recommendation:** Rewrite the c10 acceptance snippet to use the actual public API, e.g. `let mut child = cmd.spawn()?; child.wait()?;`. Keep `as_command()` only for read-only inspection, not process execution.

### B-2. c14/c15's restored panic semantic still does not work with the live HTTP serve loop

**Anchor:** commits.md c14/c15 / live `rafaello-openai-stub/src/bin/rfl_openai_stub.rs:86-102`

**Issue:** c14 says a per-request `panic!()` “propagates to the main `select!`” and the process exits non-zero. Live `serve()` accepts a TCP stream and calls `tokio::spawn(async move { if let Err(e) = handle(...).await { ... } });` without awaiting the `JoinHandle`. A panic inside `handle()` is contained in that spawned task; it does not make `serve()` return or the main `select!` exit non-zero. c15's `exhaustion_panics_deterministically` test will therefore hang until the 5s self-timeout and likely observe a normal process exit path, not the requested non-zero panic.

**Recommendation:** Make c14 explicitly change the serve model so scripted-turn fatal errors terminate the process (for example `std::process::exit(1)`, a shared fatal channel selected by `main`, or awaiting/propagating connection `JoinHandle`s). Then c15 can assert non-zero exit honestly.

### B-3. c18's layout shell step is not macOS-portable

**Anchor:** commits.md c18 / scope.md §F3 macOS CI hard gate

**Issue:** c18's Linux+macOS matrix runs `find ./result/bin -maxdepth 1 -type f -printf '%f\n'`. BSD `find` on `macos-latest` does not support GNU `-printf`, so the hard-gate macOS job will fail before it checks the Nix layout.

**Recommendation:** Use a POSIX/macOS-safe command (`find ... -exec basename {} \; | sort`, `ls -1`, or a small shell loop) or install/use GNU find explicitly. Apply the same fix to c16's manual/bin-list acceptance if it is expected to run on macOS.

### B-4. c24a's `spawn_on_demand(canonical)` API lacks the state needed to spawn

**Anchor:** commits.md c24a / live `rafaello-core/src/supervisor.rs:321-399`, `rafaello/src/lib.rs:423-465`

**Issue:** Round 3 pins the live supervisor file, but the proposed method is still not implementable as written. `PluginSupervisor::spawn(...)` requires a `CompiledPlugin` plan and `SpawnPaths`; the live supervisor stores `managed` children but does not store all compiled plans or per-plugin paths. c24a proposes `spawn_on_demand(&CanonicalId) -> Result<(), ToolSpawnError>` and says it idempotently inserts into `managed`, but no plan/path source is available from that signature. It also says `ToolSpawnError` already exists in the live supervisor module; it does not (`RflChatError` has `ToolSpawnFailed`, and `supervisor.rs` exposes `SpawnError`).

**Recommendation:** Define the real API and state: either register lazy candidates with `(CompiledPlugin, SpawnPaths)` in the supervisor, or make `spawn_on_demand` accept the plan/paths (and use `SpawnError` or introduce a new error in the files-touched list). Do not claim `ToolSpawnError` exists unless the row adds it explicitly.

### B-5. c24b cannot observe supervisor internals across the spawned-`rfl` process boundary

**Anchor:** commits.md c24b / live supervisor test hooks / scope.md §I2

**Issue:** c24b says the integration test “spawns `rfl chat`” and then verifies “not spawned at session startup” by polling `PluginSupervisor::is_in_flight` / a new `is_managed` helper. Those helpers are in the child process's supervisor instance; a parent integration test that spawns the `rfl` binary cannot access them. This repeats the process-boundary class fixed for c23, just in the lazy-load row.

**Recommendation:** Pick an observable boundary: run `run_chat` in-process with test hooks, or instrument a file-log/env-log event from the child process, or use the lazy plugin's own startup sentinel to prove it did not start until the tool call. The row must not rely on parent access to child-process supervisor internals.

## Major

### M-1. The c24a dispatch hook is still too imprecise for a cross-crate cutover

**Anchor:** commits.md c24a / live `gate/mod.rs:130-318`, `reemit/mod.rs:280-505`

**Issue:** c24a says the dispatch site is “determined by the per-commit agent” and is “likely `gate/mod.rs` or `reemit/mod.rs`.” Live tool dispatch from `core.session.tool_request` happens in `gate/mod.rs`, and that handler is currently synchronous; adding an async `spawn_on_demand(...).await` requires changing the gate task/handler shape and passing the supervisor/lazy registry into `ConfirmationGate::new`. Leaving this discovery to the implementer risks another row-local design change.

**Recommendation:** Name the exact dispatch owner (`ConfirmationGate` / `gate::handle_tool_request` unless the plan deliberately moves dispatch) and list the required signature/constructor changes in c24a.

### M-2. c10's fake-syd tests need explicit Linux gating

**Anchor:** commits.md c10 / scope.md acceptance-summary Linux exception

**Issue:** c10 explicitly gates only the rafaello-side smoke test. The three fake-syd lockin tests rely on Linux `syd` command construction; on macOS `Sandbox::new` takes the Darwin path and will not execute fake-syd or set `CARGO_BIN_EXE_syd-pty` on a syd child. If anyone runs the lockin test suite on macOS, these tests are nonsensical.

**Recommendation:** State that all three fake-syd tests are `#[cfg(target_os = "linux")]`, matching scope's Linux-only exception for syd-dependent tests.

### M-3. c28 decisions-row numbering contradicts itself and ratified scope placeholders

**Anchor:** commits.md c28 / scope.md §J3

**Issue:** c28 says “decisions.md row appends — placeholders 59–68 per scope §J3” and then lists rows 59–69 after adding a lazy-load row 68 and moving m6 ratification to 69. Adding a new lazy-load decision may be reasonable, but the row must not simultaneously claim the ratified 59–68 placeholder range is unchanged.

**Recommendation:** Either fold lazy-load into an existing row/cross-reference, or explicitly say round 3 expands the retrospective decision placeholders to 59–69 and update the acceptance row consistently.

### M-4. c14 has stale “exhaustion-as-400” size text

**Anchor:** commits.md c14 size section

**Issue:** c14's `Size` line still says the row is justified by “exhaustion-as-400 dispatcher,” even though round 3 restored panic semantics.

**Recommendation:** Replace with “exhaustion-panic dispatcher” after fixing B-2's actual process-exit mechanism.

## Nits

### N-1. c09 says “six coordinated edits” but enumerates five

**Anchor:** commits.md c09

The text says six coordinated edits and then lists items 1–5. Either count the `Sandbox` field/signature changes separately or say five.

### N-2. c04's synthetic binary uses `/usr/bin/true`

**Anchor:** commits.md c04

`/usr/bin/true` is probably fine on the target platforms, but a test-created shell script with `exit 0` (or using `env!("CARGO_BIN_EXE_rfl-openai")` in the later c07 smoke) would avoid a hard-coded system path in Phase A tests.

### N-3. c28 acceptance still says “round 1” and “28-slot budget closes”

**Anchor:** commits.md c28 acceptance

The retrospective row still says “Out of scope for round 1” and “the slot is reserved here so the 28-slot budget closes.” Round 3 is 28 implementation + 1 retro = 29 slots.

## What's working

- c04's forward dependency is fixed cleanly by moving the in-tree bundled-openai smoke to c07.
- c09's resolve-in-`Sandbox::new` approach is the right live lockin layer; only the c10 test snippet needs public-API cleanup.
- c23's file-log mode fixes the process-boundary problem from round 2.
- The `LoadPolicy::Lazy { command }` mapping is the right live parser shape for I2; the remaining work is making the spawn-on-demand state and test observability concrete.
