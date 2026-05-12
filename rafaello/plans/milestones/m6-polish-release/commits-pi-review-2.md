# m6 commits.md round-2 pi review

> Verdict: blocking.
> Counts: B/5 M/4 N/3

Reviewed round-2 `commits.md` at `064418d`, prior `commits-pi-review-1.md`, ratified `scope.md`, owner ratification commit `a0764b3`, and the live source called out by the round-2 prompt (`lockin/crates/sandbox`, `rafaello-openai-stub`, `test_ordering_hook`, `supervisor/core_service`, `run_chat`, `compile.rs`, `digest.rs`, and `rfl_bus_fixture.rs`).

Round 2 closes several round-1 path/name mistakes, but it still has row-local implementation blockers. The main regressions are new: c04 now depends on a later c06 row; c09/c10 still do not match lockin's command-building API; c23 places an in-process hook test in the wrong crate/process; c24a/c24b target a nonexistent `bindings.load.triggers` API; and c14/c15 silently weaken scope's deterministic-panic stub requirement to HTTP 400.

## Round-1 follow-up

| prior id | status | round-2 result |
|---|---|---|
| B-1 c09 fake-syd source missing | closed-but-related | fake-syd source moved into c09; new lockin API issues remain under B-2 below |
| B-2 c09/c10 private lockin internals / nonexistent typed error | partially open | typed `SandboxError` removed and public `syd_pty_path` planned, but c09/c10 still mismatch live `build_command` / private `build()` APIs |
| B-3 c14/c15 bus dispatcher vs HTTP stub | partially open | HTTP reshape is right, but c14/c15 now contradict scope's exhaustion-panic acceptance |
| B-4 stub release feature gate/source tree | closed | c14 drops `required-features`, c06 adds stub manifest/openrpc work, c16 depends on c14 |
| B-5 c23 nonexistent `Broker::register_rpc` seam | partially open | live `ToolSchemaCatalogBuilt` instrumentation is plausible, but the test location/process cannot use `drain()` as written |
| B-6 c24 test-only lazy-load row | open | c24 split adds implementation, but against nonexistent `bindings.load.triggers` surface |
| M-1 private `resolve_entry` / nonexistent digest helper | closed | c02 makes `resolve_entry` public and uses `content_digest` / `manifest_digest` |
| M-2 dependency lines | partially open | c07/c11/c27 improved, but c04 now depends on future c06 |
| M-3 decisions-row placeholders | mostly closed | main rows cite placeholders; c24a wrongly cites row 60 for lazy-load |
| M-4 live paths | mostly closed | bus-fixture/test/Homebrew paths mostly fixed; c24a still says `rafaello/src/supervisor.rs` “or” core |
| M-5 sizing/inventory | mostly closed | c25 inventory fixed; sizing summary still self-corrects noisily and has bucket-label inconsistencies |
| N-1 c04 fixture mirror | closed | mirror dropped |
| N-2 c10 test count | closed | c10 now says four tests |
| N-3 c16 `nix-store --query` | closed | replaced with `find ./result/bin` |
| N-4 long subjects | closed | c05/c09/c17 subjects tightened |

## Blockers

### B-1. c04 depends on future c06, breaking the commit order

**Anchor:** commits.md c04 / c06 / scope.md §A4 / phase ordering rationale

**Issue:** c04 is a Phase A row, but its `Depends on.` line now includes c06 because `rfl_init_against_in_tree_bundled_openai.rs` consumes the in-tree `rafaello-openai/rafaello.toml` + `openrpc.json` promotion. c06 lands later in Phase B and itself depends on c05. This violates the mechanical dependency rule: a row cannot depend on a later row in the table, and Phase A's advertised ladder is A1→A2→A3→A4 before Phase B.

**Recommendation:** Either remove c04's c06 dependency by returning c04 to a self-contained fixture-source-tree test, or move the in-tree bundled-openai smoke to a later row that actually lands after c06 (for example c07 or F2). Do not leave a forward dependency in the commit table.

### B-2. c09/c10 still do not match lockin's live command-building API

**Anchor:** commits.md c09/c10 / live `lockin/crates/sandbox/src/lib.rs:193-199`, `:613-624`, `linux.rs:13-32`

**Issue:** c09 says to call `resolve_syd_pty_path(&self.spec, &resolved_syd)?` in the Linux child-command construction site, but live `Sandbox::build_command` and `linux::build_sandbox_command` return `Command`, not `Result<Command>`, and the Linux helper receives only `spec`, `private_tmp`, `syd`, and `program`. There is no error channel at that layer unless c09 changes signatures or resolves/stores the syd-pty path earlier in `Sandbox::new`. c10 also shows tests calling `SandboxBuilder::new().syd_path(...).build()?`, but `build` is a private helper; the public API is `command(program)`.

**Recommendation:** Make c09 explicit about the real shape: either resolve `syd_pty` in `Sandbox::new`, store it on `Sandbox`, and pass it into `linux::build_sandbox_command`, or change `build_command`/Linux helper signatures to return `Result`. Rewrite c10 tests to drive `SandboxBuilder::command(<absolute program>)`, not private `build()`.

### B-3. c23's test cannot use the rafaello hook from a `rafaello-core` integration test as written

**Anchor:** commits.md c23 / scope.md §I1 / live `rafaello/crates/rafaello/src/chat/test_ordering_hook.rs`

**Issue:** c23 adds instrumentation in the `rafaello` crate, then puts the regression test under `rafaello/crates/rafaello-core/tests/core_tools_list_registered_before_provider_spawn.rs`. The row says the test “spawns `rfl chat`” and uses the live in-process `drain()` helper. A `rafaello-core` integration test is a separate crate/process from the spawned `rfl` binary, so `drain()` will not see child-process events; it also cannot naturally import the `rafaello` crate without creating awkward/cyclic dev-dependency shape. The live hook already has an inter-process file log via `RFL_STARTUP_ORDERING_LOG`, but c23 does not use it.

**Recommendation:** Put this regression under `rafaello/crates/rafaello/tests/` and use the in-process hook, or keep the scope-named file under core but drive the child with `RFL_STARTUP_ORDERING_LOG` and assert the log file contents. The row must pick an observable process boundary.

### B-4. c24a/c24b target nonexistent `bindings.load.triggers` and invalid lock syntax

**Anchor:** commits.md c24a/c24b / scope.md §I2 / live `rafaello-core/src/lock/bindings.rs`, `lock/load_policy.rs`, `manifest/load.rs`, `rafaello/src/lib.rs:455-465`

**Issue:** Round 2 correctly notices live `run_chat` eagerly spawns all tool plugins, but the proposed fix reads `entry.bindings.load.triggers` and the fixture uses `bindings.load.triggers = [{ kind = "tool", tool = "<name>" }]`. Live `Bindings` has `load: LoadPolicy`; `LoadPolicy` is either a string or `Lazy { event: Vec<String>, command: Vec<String>, kind: Vec<String> }`. There is no `triggers` field and no `{ kind="tool", tool=... }` table form for lock TOML. A per-commit agent following c24a/c24b will write tests against a surface the live parser rejects.

**Recommendation:** Rebase I2 on the actual live load model, or explicitly add the missing lock/manifest schema change in a commit before using it. If the intended “tool trigger” maps to `LoadPolicy::Lazy { command: [<tool>], .. }`, say that and update c24a/c24b acceptance accordingly.

### B-5. c14/c15 silently replace scope's exhaustion panic with HTTP 400

**Anchor:** commits.md c14/c15 + appendix Phase E / scope.md §E1–§E2

**Issue:** Ratified scope says scripted-turn exhaustion is a **hard panic** and E2 includes an exhaustion-panic test. Round 2 changes c14 to “exhaustion is a hard 400 response” and c15 tests `exhaustion_returns_400_with_deterministic_body`. That may be a reasonable HTTP design, but it is not the ratified scope and it also leaves the traceability appendix contradictory (`Exhaustion panics deterministically`).

**Recommendation:** Either keep the ratified hard-panic/process-exit behavior and test non-zero child exit, or take this back through scope/owner as an explicit semantic change. Align the appendix and decisions-row placeholder with whichever semantic wins.

## Major

### M-1. The acceptance appendix is stale in load-bearing places

**Anchor:** commits.md acceptance traceability appendix Phase C / Phase E / Phase I

**Issue:** The appendix still says `SandboxError::SydPtyNotFound` even though c09 intentionally keeps `anyhow`; Phase E still says “emits `tool_request` then `assistant_message`”, “exhaustion panics”, and `match_in_reply_to` even though c14/c15 use HTTP responses and no bus correlation IDs; Phase I repeats `load.triggers.kind = "tool"` even though that is not a live field.

**Recommendation:** Update the appendix after fixing the blockers. The appendix is used for spot-checking and will mislead per-commit prompt construction if it contradicts the row bodies.

### M-2. c24a's implementation row is still too vague about the real dispatch owner

**Anchor:** commits.md c24a / live `rafaello-core/src/supervisor.rs:321-399`, `rafaello/src/lib.rs:455-465`

**Issue:** c24a lists `rafaello/crates/rafaello/src/supervisor.rs (or rafaello-core/src/supervisor.rs — whichever owns PluginSupervisor)`. The live owner is `rafaello-core/src/supervisor.rs`; the row should not leave that discovery/design choice to the per-commit agent, especially for a cross-crate supervisor API cutover.

**Recommendation:** Name the exact live file(s), return type, and call path. For example, if `SpawnHandle` is cloneable, plan `spawn_on_demand(...) -> Result<SpawnHandle, SpawnError>` rather than `Result<&SpawnHandle, ToolSpawnError>` unless those types are deliberately introduced.

### M-3. c24a cites the wrong decisions row placeholder

**Anchor:** commits.md c24a / decisions placeholders in scope.md §J3

**Issue:** c24a says decisions row placeholder **60** captures lazy-load ratification. Row 60 is reserved for `rfl init`; lazy-load was already tied to existing decisions row 42 / owner item 6 and no row-60 update should describe it.

**Recommendation:** Remove row 60 from c24a. If m6 needs a new decision row for lazy-load runtime semantics, add a new explicit placeholder in the retrospective list; otherwise cross-reference row 42 only.

### M-4. c17 still puts `nix build` inside a Cargo integration test without CI-shape justification

**Anchor:** commits.md c17/c18

**Issue:** `nix_build_layout.rs` runs `nix build .#rafaello` from `cargo test`. Then c18's CI runs `cargo test --workspace --features test-fixture`, which will run that Rust test and recursively launch Nix. This may be acceptable, but the row gives no timeout/ignore/feature-gate story beyond `#[cfg(target_os = "linux")]`, so the failure domain is muddy and macOS layout coverage is only manual/CI-job level.

**Recommendation:** Prefer a dedicated CI/script check for F2 layout, or mark the Cargo test ignored/manual with a precise CI invocation. If kept as a normal test, justify the nesting and timeout explicitly.

## Nits

### N-1. c10's `--record-env` wording points at the wrong arg loop

**Anchor:** commits.md c10 / live `rfl_bus_fixture.rs:154-174`, `:325-333`

The live lines 326-333 parse `--probe-fd` only inside `probe_fd_closed` mode. A general `--record-env <path>` mode would need top-level mode/arg handling before the real-bus mode dispatch, not “one more arm” in that loop.

### N-2. c15 mentions `RFL_FIXTURE_MAX_LIFETIME` override, but the live stub has a fixed const timeout

**Anchor:** commits.md c15 / live `rfl_openai_stub.rs:16`, `:59-66`

The live stub uses `const SELF_TIMEOUT: Duration = Duration::from_secs(5)` and does not read `RFL_FIXTURE_MAX_LIFETIME`. Either add that support in c14 or drop the “overridden if needed” wording.

### N-3. The retro numbering note is more confusing than useful

**Anchor:** commits.md sizing summary / c28 heading

The summary says “c29 retro” while the actual row remains headed c28. The c24a/c24b convention is enough to count 28 implementation rows + c28 retro; avoid introducing a second retro number that agents might cite.

## What's working

- The c14/c16/c17 chain now acknowledges the live stub feature gate and release inclusion requirement.
- Public `resolve_entry` + live digest helper names close the PP1 API mismatch.
- Owner defaults for G.β, no `pty:off`, bus-fixture exclusion, stub inclusion, manual-only Homebrew validation, and Ctrl-C quit remain visible.
- The c24 split keeps the budget within scope's 30-max, once the live load-policy surface is corrected.
