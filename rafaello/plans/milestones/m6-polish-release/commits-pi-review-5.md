# m6 commits.md round-5 pi review

> Verdict: blocking.
> Counts: B/5 M/2 N/2

Reviewed round-5 `commits.md` at `37c10bd`, `commits-pi-review-4.md`, ratified `scope.md` §I2, owner ratification commit `a0764b3`, and live code for the restored lazy-load runtime (`PluginSupervisor`, `ConfirmationGate`, `run_chat`, `LoadPolicy`, and the existing file-log pattern).

Round 5 correctly withdraws the parser-only pivot: the restored c24a/c24b shape is the right scope direction. However the concrete c24a mechanics still are not implementation-ready against live code. The main issues are all in the lazy-load cutover: cross-crate visibility for `LazyCandidate`, `Arc<PluginSupervisor>` vs consuming `shutdown(self)`, calling `spawn_on_demand` for every tool including eager tools, idempotency without a tool→canonical mapping, and trace semantics that make c24b fail.

## Round-4 follow-up

| prior id | status | round-5 result |
|---|---|---|
| B-1 parser-only c24 violates scope §I2 | direction closed, mechanics open | runtime c24a/c24b restored with scoped test path, but c24a has new implementation blockers below |
| B-2 c18 `find -printf` | closed | c18 workflow block uses `find ... -exec basename {} \;` |
| M-1 c28 row range inconsistent | closed | c28 restored rows 59–68 and removed lazy-load v2 deferral row |
| M-2 c14/c15 panic-vs-exit wording | mostly closed | c15 test renamed to `exhaustion_exits_deterministically`; one stale “task panics” phrase remains |
| M-3 c18 acceptance missing layout check | closed | c18 acceptance now explicitly names F2 layout shell-step |
| N-1 c24 fixture table shorthand | closed | c24b uses full `[plugin."local:readfile@0.0.0".bindings.load]` table path |
| N-2 c18 appendix find text | closed | appendix names portable `find ... -exec basename` form |

## Lazy-load restoration check

**Scope direction: yes. Implementation mechanics: no.**

Round 5 restores the required end-to-end `lazy_load_tool_trigger_spawns_on_first_call.rs` coverage and removes the invalid parser-only pivot. That closes the round-4 scope objection in principle. But the restored c24a/c24b plan still needs another round because its APIs do not line up with live ownership, visibility, and dispatch semantics.

## Blockers

### B-1. `register_lazy` exposes a private `LazyCandidate` across crate boundaries

**Anchor:** commits.md c24a / live `rafaello/src/lib.rs` + `rafaello-core/src/supervisor.rs`

**Issue:** c24a defines `struct LazyCandidate { plan, paths, triggers }` inside `rafaello-core/src/supervisor.rs`, then has `run_chat` in the separate `rafaello` crate construct `LazyCandidate { ... }` and pass it to `plugin_supervisor.register_lazy(...)`. As written the struct is private, and even `pub fn register_lazy(..., candidate: LazyCandidate)` would trigger a private-interface problem and be unusable from `rafaello`.

**Recommendation:** Make the API cross-crate-safe: either expose `pub struct LazyCandidate` with public fields/constructor, or better, define `pub fn register_lazy(&self, canonical, plan, paths, triggers)` so `LazyCandidate` stays internal to `rafaello-core`.

### B-2. `Arc<PluginSupervisor>` cutover does not address consuming `shutdown(self)`

**Anchor:** commits.md c24a / live `rafaello-core/src/supervisor.rs:933`, `rafaello/src/lib.rs:546`

**Issue:** c24a says `run_chat` wraps `plugin_supervisor` in `Arc::new(...)` and passes clones to `ConfirmationGate`. Live shutdown is `pub async fn shutdown(self) -> ShutdownReport`, and `run_chat` currently calls `plugin_supervisor.shutdown().await`. Once the supervisor is in an `Arc`, that consuming shutdown cannot be called unless the row also changes shutdown's signature or uses `Arc::try_unwrap` after aborting/dropping all gate/router holders. The row does not mention this compile-breaking ownership change.

**Recommendation:** Add an explicit shutdown plan: change `shutdown` to take `&self` with internal draining, or prove `Arc::try_unwrap` after `gate_join.abort()` and all clones are dropped. Include the needed file edits and acceptance.

### B-3. Gate calls `spawn_on_demand` for every tool, so eager tools fail dispatch

**Anchor:** commits.md c24a / live `gate/mod.rs:248-318`

**Issue:** c24a says the first action after parsing `tool` is `let _ = supervisor.spawn_on_demand(&tool).await;`, and on `Err(SpawnError)` it logs and returns early. For normal eager-spawned tools there will be no lazy candidate, so `spawn_on_demand` returns the “unknown tool/no candidate” error and the gate drops a valid request. This regresses all existing eager tool dispatch.

**Recommendation:** Make “no lazy candidate for this tool” a non-error/no-op in the gate path, or call a method like `ensure_lazy_spawned_if_registered(tool) -> Result<bool, SpawnError>` where `Ok(false)` means proceed normally. Reserve hard errors for candidate spawn failures.

### B-4. `spawn_on_demand(&tool_name)` cannot implement idempotent redispatch after removing the candidate

**Anchor:** commits.md c24a unit tests / live supervisor managed state

**Issue:** The algorithm removes the lazy candidate before spawning. On a second call with the same tool name, there is no candidate left. The method takes only `tool_name`, while `managed` is keyed by `CanonicalId` and does not remember which tools triggered that canonical. The row nevertheless requires `spawn_on_demand_idempotent_after_first_dispatch` to no-op and keep managed size unchanged. There is no tool→canonical mapping left to make that decision.

**Recommendation:** Retain a `tool_to_canonical` map after first dispatch, or have gate validate `dispatch_target` first and call `spawn_on_demand(canonical, tool)`. Then idempotency can check `managed.contains_key(canonical)`.

### B-5. Trace emission makes c24b's “exactly once / not eager” assertions fail

**Anchor:** commits.md c24a/c24b / `RFL_SPAWN_TRACE_LOG`

**Issue:** c24a emits `spawn_on_demand <canonical>` in `spawn_on_demand`, then calls `self.spawn(...)`. It also says `spawn` emits `eager_spawn <canonical>` for every spawn. Therefore a lazy plugin spawned on demand will produce both `spawn_on_demand local:readfile@0.0.0` and `eager_spawn local:readfile@0.0.0`. c24b asserts the lazy plugin appears exactly once and not on an `eager_spawn` line. The planned trace semantics contradict the planned test.

**Recommendation:** Do not emit `eager_spawn` from the generic `spawn` method, or pass a spawn reason into `spawn`/`record_spawn_event`. Emit `eager_spawn` only from startup eager paths and `spawn_on_demand` only from the lazy path.

## Major

### M-1. c24a spawns before validating `dispatch_target`

**Anchor:** commits.md c24a / live `gate/mod.rs:260-276`

**Issue:** The row says to call `spawn_on_demand(&tool).await` immediately after parsing `tool`, before parsing and validating `dispatch_target`. A malformed or malicious tool request could cause a lazy plugin to spawn based only on a tool name even if the request has no valid target.

**Recommendation:** Parse and validate `dispatch_target` first, then ensure the lazy candidate for that target/tool is spawned. This also fixes the idempotency mapping problem.

### M-2. c15 still contains stale panic wording

**Anchor:** commits.md c15

**Issue:** The test is renamed `exhaustion_exits_deterministically`, but the body still says “the in-process tokio task panics” and “connection is dropped when the task panics.” c14 now uses `std::process::exit(1)`, not a panic.

**Recommendation:** Replace the stale phrases with deterministic process exit / connection closed by process termination.

## Nits

### N-1. c24a's `SpawnError::AlreadyRegistered` sentinel wording is confusing

**Anchor:** commits.md c24a

The row says an already-spawned path returns an `AlreadyRegistered` sentinel that callers treat as proceed, while the signature returns `Result<SpawnHandle, SpawnError>`. Prefer an explicit `Ok(AlreadySpawned)`-style enum or `Ok(false)` helper so errors remain errors.

### N-2. c24a acceptance relies on `rg` counts for core behavior

**Anchor:** commits.md c24a acceptance

`rg "lazy_candidates"` / `rg "spawn_on_demand"` checks are useful hygiene, but the real acceptance is the supervisor unit tests and c24b integration test. Consider demoting the `rg` checks to review notes or making them non-primary.

## What's working

- The round-5 restoration correctly accepts that scope §I2 requires end-to-end spawn-on-first-call coverage.
- c18's portable `find -exec basename` fix and explicit F2 layout acceptance are good.
- c10's public lockin API usage and Linux gating are now aligned with live code.
- The c24a file list and size justification properly recognize this as a workspace cutover.
