# Pi review round 7 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial round 7, focused on literal implementability after the round-6 fixes and on whether the remaining issues are ratification blockers.

## Verdict

**Ratifiable.**

Round 7 closes the three round-6 blockers. I do not see a remaining issue that should block owner ratification. The remaining findings are wording / precision cleanups: one stale `in_flight` sentence, one underspecified error-mapping sentence, and a few local examples that should be tightened so implementers do not cargo-cult awkward code.

## Things newly right in round 7

1. **The lockin tokio path is now consistent.**
   - Inputs chooses `SandboxBuilder::tokio_command(...)`.
   - SP4 step 12 now also calls `builder.tokio_command(&plan.entry_absolute)`.
   - The nearby note explicitly warns not to use sync `command(...)`.

2. **Shutdown wait-error reporting is now compatible with cached reaper outcomes.**
   - `SpawnHandle::wait()` / `try_wait()` return `Arc<ReaperOutcome>`.
   - `ReaperOutcome::WaitFailed(std::io::Error)` remains the single owner of the non-cloneable error.
   - `ShutdownFailure::WaitFailed { kind, message }` is now a shareable projection that shutdown can build from the cached `Arc<ReaperOutcome>`.

3. **Supervisor ACL lookup now uses the broker API.**
   - SP4 Phase A step 2 uses `broker.plugin_acl(&plan.canonical)` instead of reaching for `broker_acl.plugins`.
   - This matches B1's public `Broker::plugin_acl` contract.

4. **The round-6 boot-event contradiction is fixed.**
   - The test row now says `broker.publish_boot()` is explicit.
   - It no longer claims `PluginSupervisor::new` auto-emits boot.

5. **Private-state directory creation wording is now coherent.**
   - H3 now says the harness does not need to pre-create `.rafaello-plugin-data` because the supervisor's `create_dir_all(private_state_dir)` creates intermediates.

6. **Drop/reap wording is safer.**
   - `PluginSupervisor::Drop` no longer says it lets the OS reap while the parent is alive.
   - The scope now points at the already-running reaper task for actual wait/reap.

7. **The post-spawn registration unwind repeats the zombie fix.**
   - SP4 step 17 now explicitly says to SIGKILL and `child.wait().await` to reap on register failure.

## Round-6 closure check

### Blocking #1 — SP4 called sync `command(...)` while claiming a tokio child

**Closed.** SP4 step 12 now calls:

```text
builder.tokio_command(&plan.entry_absolute)
```

and the note directly below says `command` returns the sync variant and must not be used.

### Blocking #2 — `ShutdownFailure::WaitFailed(std::io::Error)` was not producible from `Arc<ReaperOutcome>`

**Closed.** `ShutdownFailure::WaitFailed` now stores:

```rust
WaitFailed { kind: std::io::ErrorKind, message: String }
```

which can be derived without moving the `std::io::Error` out of the cached `Arc<ReaperOutcome>`.

### Blocking #3 — SP4 Phase A reached for private `broker_acl.plugins`

**Closed.** SP4 step 2 now uses `broker.plugin_acl(&plan.canonical)` and maps `None` to `SpawnError::NotInAcl` defensively.

### Non-blocking #1 — stale `broadcast channel` wording

**Closed.** The remaining wait-channel wording uses `tokio::sync::watch`. The only remaining occurrence of the word `broadcast` is a parenthetical note saying the stale line was removed.

### Non-blocking #2 — boot-event test contradicted `PluginSupervisor::new`

**Closed.** The test row now states that boot is emitted only via explicit `Broker::publish_boot()` and that `PluginSupervisor::new` does not auto-emit it.

### Non-blocking #3 — `SpawnPaths` parent-dir wording conflicted with `create_dir_all`

**Closed.** H3 now correctly says the harness does not need to pre-create the parent directory.

### Non-blocking #4 — Drop should not say it lets the OS reap

**Closed.** The Drop wording now relies on the per-spawn reaper task.

### Non-blocking #5 — confusing `in_flight` success wording

**Partially closed.** SP1 and SP4 step 17 now state the intended rule: hold the local RAII reservation through Phase B and drop it immediately after successful `broker.register_plugin`. But SP4 step 1a still says the success path "moves the guard into the `SpawnHandle`'s shared state," which contradicts the simpler rule. This is no longer a blocker because the later authoritative step is clear, but it should still be cleaned up.

### Non-blocking #6 — step 17 should repeat reap wording

**Closed.** SP4 step 17 now explicitly says `child.wait().await` is part of the unwind.

## New blocking findings

**None.**

## New non-blocking findings

### 1. SP4 step 1a still has stale `in_flight` guard ownership wording

Section: SP4 Phase A step 1a.

SP1 and SP4 step 17 now say the clean rule:

- acquire a local RAII `in_flight` guard before Phase A;
- drop it on every failure;
- drop it immediately after successful `broker.register_plugin` because broker registration is now the source of truth.

But SP4 step 1a still says:

```text
success path moves the guard into the `SpawnHandle`'s shared state
and only releases it after broker `register_plugin` succeeds
```

There is no need to move this guard into `SpawnHandle` shared state, and doing so conflicts with step 17. Replace that sentence with the simpler SP1/step-17 rule.

### 2. SP4 step 17 should spell out the `BrokerError` → `SpawnError` mapping

Section: SP4 step 17; SP3 `SpawnError`.

Step 17 uses `broker.register_plugin(...)?` and says `BrokerError` maps to `SpawnError`, but `SpawnError` only has explicit registration variants for:

- `NotInAcl(CanonicalId)`
- `AlreadyRegistered(CanonicalId)`

B1 says `register_plugin` only returns those two errors, so this is implementable. Still, the spawn sequence should say the exact mapping instead of implying a blanket `From<BrokerError>`:

- `BrokerError::NotInAcl(c)` → `SpawnError::NotInAcl(c)`
- `BrokerError::AlreadyRegistered(c)` → `SpawnError::AlreadyRegistered(c)`

and any other broker error is unreachable for `register_plugin` under B1.

### 3. One wait-status assertion example should borrow the `Arc<ReaperOutcome>`

Section: positive test row `supervisor_peer_call_plugin_to_core.rs`.

The row currently uses:

```rust
matches!(*spawn.wait().await, ReaperOutcome::Exited(s) if s.code() == Some(0))
```

Because `wait()` returns `Arc<ReaperOutcome>` and `ReaperOutcome` has a non-`Copy` `WaitFailed(std::io::Error)` variant, implementers should avoid matching by value out of the `Arc`. Prefer:

```rust
matches!(spawn.wait().await.as_ref(), ReaperOutcome::Exited(s) if s.code() == Some(0))
```

This is a test-row polish item, not a design blocker.

### 4. Local unwind wording after transport setup should also say "reap"

Sections: SP4 step 14 and the post-spawn unwind contract.

The global post-spawn contract correctly says every failure after `cmd.spawn()` must kill **and reap**. Step 17 repeats that. Step 14's local sentence still only says to SIGKILL, drop proxy, and drop `core_fd`.

The global contract is clear enough for ratification, but step 14 should mirror step 17 to avoid future regressions: "SIGKILL the child, `child.wait().await` to reap, then drop proxy/core fd."

### 5. `SpawnHandle`/`PluginSupervisor` drop ownership semantics should be made one sentence clearer

Sections: SP1 `SpawnHandle`, SP5 lifecycle.

SP1 says the process is killed when the last caller handle plus the supervisor's internal handle are dropped, while SP5 says `PluginSupervisor::Drop` sends SIGKILL to every handle in the registry synchronously. The latter is the safer and more concrete rule: dropping the supervisor kills managed children even if external `SpawnHandle` clones still exist; those clones then observe the cached reaper outcome.

If that is the intended semantics, adjust SP1's RAII sentence so it does not imply external clones can keep a process alive after supervisor drop.

### 6. `FittingsBuild` appears to be a stale or future-proofed `SpawnError` variant

Sections: SP3 `SpawnError`, SP4 steps 14–16.

SP3 includes:

```rust
FittingsBuild { canonical: CanonicalId, source: FittingsError }
```

but SP4's fittings setup is currently infallible except for `TransportSetup`:

- `StdioTransport::new(...)` is infallible;
- `Server::new(service, transport)` is infallible;
- service-router construction is described as infallible.

Keeping the variant is harmless, especially if it is intentional future-proofing, but the scope should either identify the exact fallible call that maps to it or drop the variant to reduce dead error surface.

## Summary

Round 7 is ready for owner ratification. The remaining edits are small textual cleanups and test-example polish; none changes the broker/supervisor architecture or the acceptance matrix.
