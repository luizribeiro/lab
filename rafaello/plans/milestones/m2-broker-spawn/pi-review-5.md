# Pi review round 5 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial round 5, focused on whether the round-4 cleanup made the scope ratifiable and whether the remaining implementation-shape contradictions are closed.

## Verdict

**Not ratifiable yet.**

Round 5 closes most of pi-review-4's concrete blockers: `SpawnPaths` now has path kinds and Phase A validation, control-character path validation is scoped, the lockin async path is consistently chosen, fittings parameter examples are mostly normalized to `serde_json::Value`, fixture readiness and top-level taint controls are specified, and the wait API is now fallible rather than pretending an `ExitStatus` can encode reaper failure.

However, the draft still has several correctness issues that would mislead implementers or produce flaky tests. The new blockers are fewer than previous rounds, but they are still real: the cached wait/reaper design is not type-correct as written, the post-publish flush claim is false if implemented with `PeerHandle`, the core publish namespace error mapping is contradictory, duplicate-spawn protection is described outside the authoritative spawn sequence, post-spawn unwind can leak zombies, and one proxy-env test cannot observe the lowercase vars it asserts.

## Things newly right in round 5

These are the major round-4 findings that now appear resolved and should be preserved:

1. **`SpawnPaths` validation is now part of Phase A.**
   - SP4.3 now explicitly validates `paths.project_root` and `paths.private_state_dir` along with `plan.filesystem` and `plan.entry_absolute`.
   - `PathKind` now contains `ProjectRoot` and `PrivateStateDir`.

2. **Control-character path validation is now scoped.**
   - The lock-correspondence claim now names ASCII control characters as part of the supervisor's `InvalidPlan` spot-check set.
   - `InvalidPlanReason::ControlCharsInPath` exists.
   - SP4.3 defines the byte-level check as `< 0x20 || == 0x7f`.

3. **The lockin child model is now consistently async.**
   - Inputs chooses lockin's `--features tokio` path.
   - SP4 step 12 uses `tokio_command(...)` rather than sync `command(...)`.
   - SP1 accounts for `tokio::process::Child::id() -> Option<u32>`.
   - The reaper is specified as awaiting `child.wait().await` directly.

4. **Most fittings parameter-type examples are now corrected.**
   - B7 serializes `BusEvent` to a `serde_json::Value` once, then clones per fan-out.
   - H5 uses `serde_json::Value::Null` for `core.fixture.start`.
   - H4 explains that `Client::with_notification_handler` is synchronous and must spawn/queue async forwarding work.

5. **Fixture readiness is now explicit.**
   - F2 universal init now calls `core.fixture.ready` after service/notification handler installation.
   - H5 waits for fixture-ready before issuing `core.fixture.start` or other core-to-plugin calls.

6. **Publish-and-exit modes now have a post-publish ack concept.**
   - F2 adds `core.fixture.after_publish` before exit.
   - This is the right direction, although one ordering bug remains as blocking finding #2 below.

7. **Top-level `taint` can now be emitted by the fixture.**
   - `publish_with_taint` and `RFL_FIXTURE_TAINT_JSON` are now scoped.
   - `publish_full_params` gives tests an escape hatch for malformed publish-param shapes.

8. **The wait API now has a type-level error path.**
   - `wait()` and `try_wait()` now return `Result<ExitStatus, ReaperError>` rather than pretending reaper failure is an `ExitStatus`.
   - This resolves the round-4 type mismatch in principle, although the storage model still needs fixing.

9. **Round-4 non-blockers were mostly cleaned up.**
   - `TransportSetup` is now in the `SpawnError` enum block.
   - Socketpair pseudocode now uses nix 0.29's Rust-shaped call.
   - The redundant nested reserved-env reason was removed.
   - Private-state creation has a dedicated `PrivateStateDirCreate` variant.

## Blocking findings

### 1. Reaper / `wait()` storage semantics are still not implementable as written

Sections: SP1, SP4 step 18, Risks, positive test matrix.

Round 5 correctly changes the public API to:

```rust
pub async fn wait(&self) -> Result<ExitStatus, ReaperError>;
pub fn try_wait(&self) -> Option<Result<ExitStatus, ReaperError>>;
```

and SP1 says the cached status uses `tokio::sync::watch` initialized to `None` and later set to `Some(result)`.

But multiple contradictions remain:

1. **The document still says "broadcast channel" in authoritative places.**
   - SP1 says: "The reaper publishes the `ExitStatus` to a broadcast channel."
   - SP4 step 18 says it publishes to the "broadcast channel inside the `SpawnHandle`'s shared state".
   - Risks still says the reaper publishes to a broadcast channel and that late waiters see the latest value.

   Tokio `broadcast` still does not provide the described late-subscriber cached-value semantics. If the intended mechanism is `watch`, all remaining broadcast wording must be replaced.

2. **The cached result is not cloneable.**
   `std::io::Error` is not `Clone`, so this enum is not cloneable:

   ```rust
   pub enum ReaperError {
       Wait(std::io::Error),
       ReaperPanicked,
   }
   ```

   Yet `wait()` and `try_wait()` are specified as returning the cached result by value to multiple callers, including late callers. A `watch::Receiver<Option<Result<ExitStatus, ReaperError>>>` cannot hand out owned copies of a non-`Clone` value. Nor can a simple cached `Option<Result<...>>` be moved out repeatedly.

   Fix options:
   - store `Arc<ReaperError>` in the cached result;
   - make `ReaperError::Wait` store a cloneable projection such as `{ kind: std::io::ErrorKind, message: String }`;
   - expose `wait()` as single-consumer only, but that would contradict the current "multiple awaits return same status" contract.

3. **`ReaperPanicked` is promised but no task observes the reaper `JoinHandle`.**
   SP4 step 18 says:

   ```text
   tokio::spawn(async move { child.wait().await })
   ```

   and says that task publishes `Result<ExitStatus, ReaperError>` to the shared status. That catches `child.wait()` I/O errors, but it does not catch a panic in the task itself; a panicked task's `JoinError` is only visible to a separate task that awaits the `JoinHandle`.

   If `ReaperPanicked` remains a public variant, the scope must specify the join-handler task or wrapper that converts `JoinError::is_panic()` into the cached result. Otherwise remove `ReaperPanicked` from the public contract.

4. **One positive test row still treats `wait()` as infallible.**
   `supervisor_peer_call_plugin_to_core.rs` still says:

   ```text
   wait().await.code() == Some(0) (pi-3 §6 — wait is infallible per SP1)
   ```

   That no longer compiles with the SP1 API.

Fix: choose and document a concrete cached-status representation, make its value repeatably returnable, replace every remaining `broadcast` reference with that mechanism, define how reaper panics are observed or remove that variant, and update the test row to unwrap/match the `Result`.

### 2. `after_publish` is not a reliable flush if publish uses `PeerHandle::notify`

Sections: F2, F3, H5.

F2 says publish-and-exit modes:

1. perform the configured `bus.publish` notification;
2. call `peer.call("core.fixture.after_publish", Value::Null).await`;
3. exit 0.

It then claims this request roundtrip guarantees the prior `bus.publish` notification has been written to the socket because notifications and requests share a single ordered transport stream.

That claim is false if the fixture sends the publish via `PeerHandle::notify` and sends the flush via `PeerHandle::call`.

Current fittings has separate outbound queues for those two `PeerHandle` paths:

- `PeerHandle::notify` sends into the peer's bounded notification channel;
- `PeerHandle::call` sends into the peer's outbound-request unbounded channel;
- the client loop selects between `notify_rx` and `outbound_request_rx`.

Because those are separate queues and `tokio::select!` chooses among ready branches, the later `peer.call("core.fixture.after_publish", ...)` may be written before the earlier `peer.notify("bus.publish", ...)`. In that case the ack returning does **not** prove the publish was written or processed. The publish can still be lost when the fixture exits.

Fix: require the fixture publish-and-exit modes to use the `Client`'s FIFO command path for both operations:

```rust
client.notify("bus.publish", publish_params).await?;
client.call("core.fixture.after_publish", Value::Null).await?;
```

`Client::notify` and `Client::call` enqueue into the same `ClientCommand` queue, preserving order. Alternatively, define a different flush mechanism that explicitly observes the publish at core, but the current `PeerHandle` wording is not sufficient.

### 3. `publish_core("evil.foo")` has contradictory expected errors

Sections: B2, B5, positive test `broker_publish_core_invalid_topic_rejected.rs`.

B2 defines `PublishOnReservedNamespace` as covering:

```text
plugin published core.* / provider.* / frontend.* / foreign plugin.*,
OR core published a non-core topic
```

Read literally, `publish_core("evil.foo", ...)` is "core published a non-core topic", so an implementer would return `PublishOnReservedNamespace`.

But the positive test matrix says:

```text
publish_core("evil.foo", ...) → UnknownNamespace
```

That is the better structural rule: grammar-valid top-level segments should be classified first:

- segment not in `{core, provider, plugin, frontend}` → `UnknownNamespace`;
- segment is known-but-not-allowed for this publisher → `PublishOnReservedNamespace`.

Fix B2 so it says core publishing on `plugin.*` / `provider.*` / `frontend.*` is `PublishOnReservedNamespace`, while unknown top-level namespaces remain `UnknownNamespace` for both core and plugins.

### 4. Duplicate-spawn in-flight protection is described but missing from the numbered spawn sequence

Sections: SP1, SP4 Phase A.

SP1 says the supervisor has a separate `in_flight` set and that Phase A inserts the canonical into that set to prevent concurrent duplicate spawns.

But the authoritative numbered SP4 Phase A sequence starts with:

```text
1. broker.try_reserve_registration(&plan.canonical) → ...
```

and never says to acquire or release the supervisor `in_flight` reservation.

This matters because B1 explicitly says `try_reserve_registration` does **not** reserve a slot. If an implementer follows the numbered SP4 sequence, two concurrent `spawn(&self, same_plan, ...)` calls can both pass the broker precheck and both allocate a socketpair/proxy/child before one loses at `register_plugin`.

Fix: add an explicit first Phase A step such as:

```text
0. Acquire a supervisor-local in-flight reservation guard for `plan.canonical`.
   If already present, return `SpawnError::AlreadyRegistered`.
   The guard is dropped/removed on every failure path and after successful
   `broker.register_plugin`.
```

Then run `broker.try_reserve_registration` while the in-flight guard is held. Also state the scope of this protection: it prevents duplicates within one supervisor. If multiple supervisors share one broker, `try_reserve_registration` is still only a precheck unless the broker grows a real reservation token.

### 5. Post-spawn / pre-reaper unwind kills the child but does not reap it

Sections: SP4 steps 14 and 17.

The spawn sequence has fallible work after `cmd.spawn()` and before the reaper task is created:

- transport setup can fail at step 14;
- server/service/fittings setup can fail;
- `broker.register_plugin` can fail at step 17.

For those paths the scope says to SIGKILL the child and drop other resources. It does not say to `wait()`/reap the child.

With `lockin::tokio::SandboxedChild`, the supervisor still owns the child object on these paths. If it sends raw `nix::kill` and then drops the child without awaiting `child.wait().await`, the process can become a zombie until some other reaper handles it. In tests and long-running dev sessions, this is observable process leakage.

Fix: every post-spawn/pre-reaper unwind path must either:

- send SIGKILL and then `let _ = child.wait().await;` before returning the spawn error; or
- install a reaper handoff immediately after spawn before any other fallible post-spawn step; or
- set and document a reliable `kill_on_drop`/reap mechanism that actually waits, not merely signals.

The simplest scope edit is to say: "On any failure after step 13 and before step 18, SIGKILL the child and await `child.wait().await` best-effort before returning."

### 6. The proxy-env test asserts lowercase variables it did not ask the fixture to dump

Section: positive test `supervisor_proxy_starts_and_env_injected.rs`.

The fixture's `dump_env` mode returns only keys allow-listed via `RFL_FIXTURE_ENV_KEYS`.

The test row says it requests:

```text
RFL_FIXTURE_ENV_KEYS=HTTP_PROXY,HTTPS_PROXY,NO_PROXY,ALL_PROXY,RFL_BUS_FD
```

but then asserts:

```text
same for HTTPS_PROXY/ALL_PROXY + lowercase; NO_PROXY = ""
```

The lowercase variables (`http_proxy`, `https_proxy`, `all_proxy`, `no_proxy`) are not in the allowlist, so the fixture will not return them. The assertions are therefore unobservable or will fail if written literally.

Fix: include the lowercase names in `RFL_FIXTURE_ENV_KEYS`, or remove the lowercase assertions from the test row.

## Non-blocking but should fix before ratification

1. **B1 still overstates `RegisteredPlugin::drop` in the bullet list.**
   - The preface correctly says dropping the guard only drops the broker-held `PeerHandle` clone and stops fan-out.
   - But B1's bullet still says drop "closes the in-flight notification channel for that plugin."
   - Reword that bullet too: it removes the broker's registration entry and drops the broker's clone; other `PeerHandle` clones may keep the channel alive.

2. **Add explicit tests for new Phase A path checks.**
   - The scope now requires non-absolute `SpawnPaths` rejection and control-character path rejection.
   - The matrix still only has `supervisor_spawn_relative_path_refused` for `entry_absolute`.
   - Add tests for `SpawnPaths.project_root`, `SpawnPaths.private_state_dir`, and at least one control-character path to prevent regressions.

3. **`PluginSupervisor::new` boot emission is probably unobservable.**
   - `PluginSupervisor::new` calls `broker.publish_boot()` immediately after construction.
   - At that point no plugin managed by that supervisor has been spawned/registered yet, so no supervised observer can receive it.
   - This is not a correctness blocker because explicit `Broker::publish_boot()` exists for tests, but the "production observability" rationale should be softened or the emission should move to a point where subscribers can exist.

4. **Clarify `watch` send semantics if using `watch`.**
   - `watch::Sender::send(value)` returns an error when there are no receivers.
   - If the reaper uses `watch`, it should use `send_replace` or ensure a receiver is always retained in shared state. Otherwise an early child exit before any caller awaits could fail to cache the status.

5. **Manual validation fd-inode claim may be brittle.**
   - The manual validation says the parent has no duplicate of the child-side socket inode.
   - With split streams, Tokio internals, and momentary fd lifetimes, this is fine as an observational note, but should remain non-normative and not become an exact test.

## Closure check vs pi-review-4

### Round-4 blocking #1 — `SpawnPaths` validation promised but not specified

**Closed.** SP4 now validates `paths.project_root` and `paths.private_state_dir`, and `PathKind` has matching variants.

### Round-4 blocking #2 — control-character paths could panic in lockin

**Closed.** Phase A now checks ASCII control characters for plan paths and `SpawnPaths`, and `InvalidPlanReason::ControlCharsInPath` exists.

### Round-4 blocking #3 — lockin sync/async API inconsistent

**Closed.** The scope now consistently chooses lockin's `tokio` feature and `tokio_command(...)`.

### Round-4 blocking #4 — fittings parameter types not real

**Mostly closed.** `peer.notify` and `peer.call` examples now mostly use `serde_json::Value`, and observer forwarding accounts for the synchronous handler. A new fittings-ordering issue remains for the post-publish ack when `PeerHandle` is used; see blocking finding #2.

### Round-4 blocking #5 — fixture readiness handshake nondeterministic

**Closed.** F2/H5 now include `core.fixture.ready` and harness-side waiting.

### Round-4 blocking #6 — publish-and-exit modes may drop notifications

**Partially closed.** The draft adds `core.fixture.after_publish`, which is the right shape, but the ordering guarantee is false if the publish is sent through `PeerHandle::notify` and the ack through `PeerHandle::call`. See blocking finding #2.

### Round-4 blocking #7 — taint round-trip not implementable

**Closed.** `publish_with_taint`, `RFL_FIXTURE_TAINT_JSON`, and `publish_full_params` now provide top-level publish-param control.

### Round-4 blocking #8 — wait storage semantics wrong

**Partially closed.** The public API now has `Result<ExitStatus, ReaperError>` and mentions `watch`, but stale broadcast wording remains and the cached result is not cloneable. See blocking finding #1.

### Round-4 non-blocking #1 — `TransportSetup` omitted from enum block

**Closed.** `TransportSetup` is now in the `SpawnError` enum block.

### Round-4 non-blocking #2 — `RegisteredPlugin::drop` cannot close all channels

**Partially closed.** The new B1 preface is correct, but the bullet list still says it closes the notification channel. See non-blocking finding #1.

### Round-4 non-blocking #3 — socketpair pseudocode C-shaped

**Closed.** SP4 now uses the nix 0.29 Rust shape:

```rust
socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::SOCK_CLOEXEC)
```

### Round-4 non-blocking #4 — redundant reserved-env error variants

**Closed.** The redundant nested `InvalidPlanReason::ReservedEnvVar` was removed; `SpawnError::ReservedEnvInPlan` is the single path.

### Round-4 non-blocking #5 — `create_dir_all` mapped to `TransportSetup`

**Closed.** `SpawnError::PrivateStateDirCreate` now exists and SP4 maps to it.

### Round-4 non-blocking #6 — rejection-event recursion/serialization wording

**Mostly closed / no longer material.** B9 now describes an internal path that bypasses rejection emission recursion. Serialization is still implicit, but given the event schema is serializable JSON values, this is not a blocker.

## Summary of required edits before ratification

- Replace all remaining broadcast-channel wait wording with a concrete cached-status mechanism.
- Make cached wait results repeatably returnable despite `std::io::Error` being non-`Clone`.
- Specify how `ReaperPanicked` is actually produced, or remove it.
- Update the `wait()` test row to handle `Result`.
- Make post-publish fixture flush ordered: use `Client::notify(...).await` then `Client::call(...).await`, or define an equivalent ordered flush.
- Clarify `publish_core` error precedence for unknown vs known-reserved namespaces.
- Put supervisor `in_flight` reservation acquisition/release directly into SP4 Phase A.
- On any post-spawn/pre-reaper failure, kill **and reap** the child before returning.
- Fix the proxy-env test's allowlist for lowercase proxy variables.
- Clean up the remaining non-blocking wording/tests above.
