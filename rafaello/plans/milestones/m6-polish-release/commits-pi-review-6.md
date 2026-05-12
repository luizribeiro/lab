# m6 commits.md round-6 pi review

> Verdict: non-blocking.
> Counts: B/0 M/0 N/2

Reviewed round-6 `commits.md` at `b83093b`, round-5 review, ratified scope §I2, and the live lazy-load-adjacent code in `rafaello-core/src/supervisor.rs`, `rafaello-core/src/gate/mod.rs`, and `rafaello/src/lib.rs`.

Round 6 closes the round-5 lazy-load mechanics. The redesigned supervisor API is coherent against live code: `register_lazy` takes primitives with private `LazyCandidate`, `ensure_spawned(canonical)` is called after `dispatch_target` validation and no-ops for eager/no-candidate cases, trace emission is caller-side, and the `shutdown(&self)` switch is genuinely mechanical because the live body already drains `managed` with `std::mem::take(&mut *guard)`.

## Round-5 follow-up

| prior id | status | round-6 result |
|---|---|---|
| B-1 `LazyCandidate` crosses crate boundary | closed | c24a makes `LazyCandidate` private inside `rafaello-core` and exposes `register_lazy(canonical, plan, paths, triggers)` primitives only. |
| B-2 `Arc<PluginSupervisor>` vs `shutdown(self)` | closed | live `shutdown` currently takes `self`, but its body already uses `std::mem::take(&mut *self.managed.lock())`; changing to `&self` is mechanical and works through `Arc` deref. |
| B-3 eager tools fail on unconditional on-demand spawn | closed | `ensure_spawned(canonical) -> Result<bool, SpawnError>` returns `Ok(false)` for already-managed or no-candidate cases, so eager tools proceed normally. |
| B-4 idempotent redispatch after candidate removal | closed | c24a checks `managed.contains_key(canonical)` before removing candidates; `tool_to_canonical` entries are specified as persistent, and gate passes the validated canonical. |
| B-5 trace emission contradicts c24b | closed | c24a moves `eager_spawn` trace writes to the three `run_chat` eager call sites; generic `spawn` writes no spawn trace; `ensure_spawned` writes `spawn_on_demand` for lazy transition. c24b assertions match this scheme. |
| M-1 spawn before `dispatch_target` validation | closed | c24a's gate snippet parses and validates `dispatch_target`, checks `compiled`, then awaits `ensure_spawned(&dispatch_target)`. |
| M-2 stale c15 panic wording | closed | c15 now says the connection task terminates the process via `std::process::exit(1)` and tolerates connection reset from `reqwest::send`. |
| N-1 `AlreadyRegistered` sentinel | closed | c24a does not add or rely on a new sentinel; already-managed/no-candidate is `Ok(false)`. Live `SpawnError` already has `AlreadyRegistered(CanonicalId)`, but not for this control path. |
| N-2 `rg` checks as primary acceptance | closed | c24a demotes the `rg` checks to review-time hygiene below unit/build acceptance. |

## Live-code spot checks

- `PluginSupervisor.managed` is live as `Mutex<BTreeMap<CanonicalId, ManagedSpawn>>`.
- Live `SpawnError` already exists in `error.rs` with variants: `NotInAcl`, `AlreadyRegistered`, `InvalidPlan`, `EntryNotExecutable`, `SandboxBuild`, `Spawn`, `ProxyStart`, `Socketpair`, `FittingsBuild`, `ReservedEnvInPlan`, `TransportSetup`, and `PrivateStateDirCreate`.
- Live gate parsing validates `dispatch_target` before consulting the compiled map; c24a places `ensure_spawned` after that validation.
- Live `run_chat` has the three eager spawn sites c24a cites: active provider, inactive providers, and eager tool plugins.

## Nits

### N-1. Round-6 changelog says `record_spawn_event` stays module-private, but c24a correctly makes it `pub`

The c24a body needs a public helper so `rafaello/src/lib.rs` can emit `eager_spawn` from the caller side. The row body is correct (`pub fn record_spawn_event(...)`), but the top-level round-6 changelog still says the helper "stays module-private." Fold by changing that changelog sentence to say the helper is exposed for the `run_chat` eager-spawn call sites while `spawn` itself emits no trace.

### N-2. Cross-check appendix still contains stale round-4 lazy-load pivot text

The "Two-stage tests called out explicitly" appendix still has a preserved stale bullet saying the c24a/c24b ladder was dropped and spawn-on-demand runtime deferred to v2/parser-only. That directly contradicts round 6's restored c24a/c24b runtime plan. The same appendix area also says only c05/c09/c16 are the explicit workspace cutovers, while round 6 elsewhere includes c24a. Fold by removing the stale pivot bullet and updating the cutover summary to include c24a.

## Verdict

No blocking or major findings. The two nits are documentation consistency issues and can be folded by Claude/owner without another mechanics redesign.
