# Pi review round 3 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial round 3, focused on the targeted fixes against `pi-review-2.md`.

## Verdict

**Not ratifiable yet.**

Round 3 is much smaller than round 2: most of the named pi-review-2 blockers were addressed in substance. The remaining problems are now concentrated in stale cross-references and a few API-coordinate errors. Unfortunately several of those are still build-breaking or leave public contracts contradictory, so this should get one more cleanup pass before `commits.md`.

## Blocking findings

### 1. `SpawnPaths` was added but the public spawn contract was not normalized

Sections: Goal, lock-correspondence claim, SP1, SP4 step 11, H3.

The document now correctly rejects the old private-state inference and adds explicit `SpawnPaths` in SP4/H3, but the earlier public API still says the only entry point is one-argument spawn:

- Goal: supervisor's only entry point is `spawn(plan: &CompiledPlugin)`.
- Lock-correspondence claim: `PluginSupervisor::spawn` accepts only `&CompiledPlugin`.
- SP1 code block: `pub async fn spawn(&self, plan: &CompiledPlugin) -> Result<SpawnHandle, SpawnError>;`
- SP4 step 11 later changes this to `spawn(&self, plan: &CompiledPlugin, paths: &SpawnPaths)`.
- H3 calls `supervisor.spawn(plan, &paths).await`.

This is a direct public-signature contradiction. Update SP1/Goal/lock-correspondence/tests to the two-argument API, and define `SpawnPaths` in the SP1 public surface rather than as a later patch note. Also state the `SpawnPaths` validation rules in Phase A: at minimum both paths should be absolute, and the scope should say whether `private_state_dir` must be under `project_root` / match the compiled private-state grant or is purely caller-trusted.

### 2. Boot-event behavior is still contradictory

Sections: B1, B9, positive tests.

The round-2 fix chose explicit `Broker::publish_boot()`, but stale text still says construction emits boot:

- B1 says **no boot-event emission on construction** and boot is explicit.
- The `publish_core` bullet in B1 says m2 emits a single `core.lifecycle.boot` event after `Broker::new` completes.
- B9 says `core.lifecycle.boot` is emitted by `Broker::new`.
- The `broker_publish_boot_event.rs` row says explicit `publish_boot()` is the only entry point.

There is also no clear public signature for `Broker::publish_boot()`, despite tests calling it, and B1 says `PluginSupervisor::shutdown` invokes `publish_boot()`, which looks like a stale/incorrect lifecycle statement.

Pick exactly one model. The current test matrix implies: `Broker::new(acl)?` emits nothing; `pub fn publish_boot(&self) -> Result<(), BrokerError>` is explicit; supervisor construction may call it and ignore/log the result if desired, but that automatic call is no substitute for test-visible explicit re-emission.

### 3. `BrokerError` derives are still contradictory

Sections: B2, E.

B2 now correctly says `BrokerError` is not `Clone` and not `PartialEq`, because invalid-topic reasons are stored as strings and tests use `matches!`. E still says:

```text
BrokerError ... thiserror-derived, Debug + Clone + PartialEq
```

This reintroduces pi-review-2 blocker #4 at the acceptance-summary level. E must match B2: `Debug`/`Error`, no `Clone`, no `PartialEq`.

### 4. Fittings transport/connector coordinates are still wrong

Sections: Inputs, SP4 step 14, F3.

The fixture connector fix is conceptually right, but two concrete paths are not repo-real:

- The `Connector` trait is `fittings_core::transport::Connector`, not `fittings_transport::Connector`.
- `StdioTransport` is exported at `fittings_transport::stdio::StdioTransport`, not `fittings_transport::StdioTransport` from the crate root.

Current repo references:

- `fittings/crates/core/src/transport.rs` defines `Connector`.
- `fittings/crates/transport/src/lib.rs` only declares `pub mod stdio;`.
- `fittings/crates/client/src/subprocess.rs` imports `fittings_transport::stdio::StdioTransport`.

As written, SP4/F3 will not compile unless m2 also adds new re-exports, which the scope does not state.

### 5. Provider-refusal lookup names a non-existent API and contradicts `plugin_acl`'s return type

Sections: B1, SP4 step 7.

B1 defines:

```rust
Broker::plugin_acl(&self, canonical: &CanonicalId) -> Option<PluginAcl>
```

SP4 step 7 instead says to inspect:

```text
broker.acl().plugin_acl(&plan.canonical)
```

and then says the public API is:

```text
Broker::plugin_acl(&CanonicalId) -> Option<&PluginAcl>
```

The current `BrokerAcl` type has a public `plugins: BTreeMap<CanonicalId, PluginAcl>` but no `plugin_acl` method. Normalize this to one buildable call, probably `broker.plugin_acl(&plan.canonical)` returning a cloned `PluginAcl` as B1 says, or `broker.acl().plugins.get(&plan.canonical)` if the scope really wants direct ACL access.

### 6. The `wait()` test row still uses the old fallible API

Sections: SP1, positive test matrix.

SP1 now defines:

```rust
pub async fn wait(&self) -> ExitStatus;
pub fn try_wait(&self) -> Option<ExitStatus>;
```

but `supervisor_peer_call_plugin_to_core.rs` still asserts:

```rust
wait().await?.code() == Some(0)
```

The `?` cannot compile against the scoped API. Update the row to `wait().await.code() == Some(0)` or change SP1 back to a fallible result. The text around SP1 says wait is infallible, so the test should change.

### 7. Duplicate-spawn/resource-hook text still contradicts the new Phase A precheck

Sections: B1 `try_reserve_registration`, SP4 step 1, negative tests, H6.

The sequential duplicate case is now intended to fail before socketpair/proxy/sandbox allocation. However the test row still says the second attempt's allocated socketpair/proxy/child are torn down and verified via `harness::TestHooks::*_drop_count()`. H6 exposes only start/create/spawn counters, not drop counters.

Additionally, `try_reserve_registration` explicitly does not reserve a slot. The text says the supervisor registry is serialized, but does not specify an in-progress reservation. With public `spawn(&self)` on a multi-thread Tokio runtime, two concurrent spawns of the same canonical can both pass `try_reserve_registration` before either reaches `register_plugin`, causing exactly the launch-side effect pi-review-2 asked to avoid.

Fix by either:

- adding a real supervisor-side in-flight reservation set/token acquired before awaits and released on failure; or
- explicitly scoping duplicate-no-allocation to sequential calls and updating the tests/claims accordingly.

The better fix is the reservation set, because `spawn(&self)` is explicitly multi-thread friendly.

### 8. The lockin private-tmp/env-clear contradiction remains in Inputs

Sections: Inputs, SP4 step 12.

The Inputs section still says `env_clear()` removes `TMPDIR`/`TMP`/`TEMP` and m2 explicitly re-injects them. SP4 step 12 now correctly says current lockin does not expose the private tmp path from `SandboxedCommand`, so m2 cannot re-inject those vars and accepts the behavior change.

This was the core of pi-review-2 blocker #5. The implementation direction is now honest in SP4, but the authoritative Inputs bullet still states the impossible behavior. Delete or rewrite the Inputs sentence so it matches SP4.

## Non-blocking but should fix before ratification

1. **H2 references non-existent `digest::recompute_*` helpers.** Current m1 exposes `digest::content_digest`, `digest::manifest_digest`, and `RecomputedDigests`, not `recompute_*` helpers. The harness can still be implemented, but the scope should name the real functions.

2. **H6 hook counters are underspecified for parallel tests.** The suite says most tests are multi-thread and only some are `#[serial(proc)]`. If `TestHooks` are global atomics and tests assert exact deltas, parallel spawn tests can race the counters. Either make hooks per-supervisor or require every hook-counting test to run under the same serial gate.

3. **`contains_plugin` semantics are unclear.** The unregister test expects `contains_plugin` to remain true after the RAII guard drops, so it apparently means "in ACL", not "currently registered". B1 should state that explicitly or rename it; otherwise it is easy to implement it as live-registry membership and fail the matrix.

4. **`Broker::publish_boot` needs a first-class bullet.** Tests call it, so B1 should list its signature and error behavior instead of only mentioning it inside the `Broker::new` prose.

## Summary of required edits

Before ratification, make these concrete edits:

- Normalize `PluginSupervisor::spawn` everywhere to include `SpawnPaths`, and define/validate `SpawnPaths` in SP1/Phase A.
- Delete all stale `Broker::new` boot-emission text; add an explicit `Broker::publish_boot` signature.
- Make E match B2 for `BrokerError` derives.
- Fix fittings paths to `fittings_core::transport::Connector` and `fittings_transport::stdio::StdioTransport` (or explicitly add re-exports).
- Normalize provider ACL lookup to one real API.
- Fix the `wait().await?` test row.
- Reconcile duplicate-spawn tests/hooks with the no-allocation precheck and add an in-flight reservation if concurrent `spawn(&self)` is supported.
- Remove the stale private-tmp re-injection claim from Inputs.
