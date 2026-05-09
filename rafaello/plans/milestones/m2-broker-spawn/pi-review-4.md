# Pi review round 4 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial round 4, focused on whether the round-3 cleanup made the scope ratifiable and whether any remaining implementation-shape contradictions would still block `commits.md`.

## Verdict

**Not ratifiable yet.**

Round 4 fixed the round-3 stale-reference blockers: the public `SpawnPaths` signature is now mostly normalized, boot emission is no longer contradictory, the fittings connector/transport paths are corrected, the provider lookup is now expressed through `Broker::plugin_acl`, the wait-test row no longer uses `?`, duplicate-spawn protection now has an in-flight set, and the lockin private-tmp deviation is stated honestly.

The remaining blockers are fewer and more localized, but they are still real implementation blockers. They concentrate around supervisor path validation, sync-vs-async lockin child handling, fittings parameter types, fixture readiness/flush semantics, and the cached wait-status model.

## Things newly right in round 4

These are the major round-3 blockers that appear resolved in the current draft and should be preserved:

1. **The supervisor spawn signature is now consistently two-argument at the public surface.**
   - Goal, lock-correspondence claim, SP1, SP4, and H3 all now use `spawn(plan: &CompiledPlugin, paths: &SpawnPaths)`.
   - `SpawnPaths` is defined in SP1 rather than only appearing as a later patch note.

2. **Boot-event semantics are now coherent.**
   - `Broker::new(acl)?` emits nothing.
   - `Broker::publish_boot(&self) -> Result<(), BrokerError>` is first-class.
   - Tests explicitly call `publish_boot()` after registering observers.
   - `PluginSupervisor::new` may call it once for production observability and logs/ignores failure.

3. **`BrokerError` derives are no longer contradictory.**
   - B2 and E agree that `BrokerError` is `thiserror`-derived but not `Clone` and not `PartialEq`.
   - Tests are specified to use `matches!` and substring checks for reasons.

4. **Fittings coordinates are corrected.**
   - `Connector` is now named as `fittings_core::transport::Connector`.
   - `StdioTransport` is now named as `fittings_transport::stdio::StdioTransport`.
   - The fixture one-shot connector is explicitly scoped.

5. **Provider refusal is now routed through a scoped broker API.**
   - `Broker::plugin_acl(&CanonicalId) -> Option<PluginAcl>` is the canonical lookup path.
   - The stale `broker.acl().plugin_acl(...)` / `Option<&PluginAcl>` wording is gone.

6. **Duplicate spawn is no longer only a late `register_plugin` failure.**
   - The supervisor now has an `in_flight` set acquired during Phase A.
   - The duplicate-spawn test now asserts no socketpair/proxy/child allocation via per-supervisor counters.

7. **The lockin private tmp/env-clear deviation is honestly scoped.**
   - The Inputs and SP4 sections now agree that current lockin does not expose the private tmp path before spawn, so m2 cannot re-inject `TMPDIR`/`TMP`/`TEMP` after `env_clear()`.

8. **Digest helper references were corrected.**
   - H2 now names m1's real `digest::content_digest`, `digest::manifest_digest`, and `RecomputedDigests` surface rather than non-existent `recompute_*` helpers.

9. **Test hook counters are now per-supervisor.**
   - H6 avoids global atomic interference and exposes only create/start/spawn counts used by the negative tests.

10. **`contains_plugin` semantics are now explicit.**
    - The scope clarifies that `contains_plugin` means ACL membership, not live registration.

## Blocking findings

### 1. `SpawnPaths` validation is promised but not actually specified

Sections: SP1, SP3, SP4 Phase A.

SP1 says both `SpawnPaths` fields must be absolute and that Phase A verifies this:

```text
Both fields must be absolute (Phase A SP4.3 verifies).
```

But SP4.3 only checks paths in `plan.filesystem` and `plan.entry_absolute`:

```text
For each path field in plan.filesystem and plan.entry_absolute: assert absolute.
```

It does not mention `paths.project_root` or `paths.private_state_dir`.

The error surface is also incomplete for this promised validation. `InvalidPlanReason::NonAbsolutePath` carries a `PathKind`, but `PathKind` only contains:

```rust
ReadPath, ReadDir, WritePath, WriteDir, ExecPath, ExecDir, EntryAbsolute
```

There is no `ProjectRoot` or `PrivateStateDir`, so the scoped API cannot report the two `SpawnPaths` failures SP1 promises.

Fix: add an explicit Phase A step validating `paths.project_root.is_absolute()` and `paths.private_state_dir.is_absolute()`, and extend `PathKind` with `ProjectRoot` and `PrivateStateDir` (or add a separate `InvalidPlanReason` for invalid spawn paths).

### 2. Plan path validation still allows lockin builder panics

Sections: lock-correspondence claim, SP4 Phase A/B.

The lock-correspondence section says the supervisor performs `InvalidPlan` spot-checks for cases that would otherwise crash the lockin builder:

```text
The supervisor performs InvalidPlan spot-checks for the cases that would otherwise crash the lockin builder (non-absolute paths, reserved env vars, topic-id ↔ canonical mismatch) ...
```

However, current lockin builder methods assert not only absoluteness but also no control characters. For example, `read_path`, `write_dir`, `exec_path`, `exec_dir`, and `command` call `assert_no_control_chars(...)`.

A hand-mutated `CompiledPlugin` with an absolute path containing a newline will therefore pass Phase A and panic in Phase B when passed into lockin. That contradicts the scope's API-level defensive claim that these cases become typed `InvalidPlan` errors rather than runtime builder crashes.

Fix either:

- validate all plan paths and `SpawnPaths` for control characters in Phase A and return `InvalidPlan`, adding a reason variant if needed; or
- narrow the lock-correspondence claim to admit that public-field mutation can still panic on control-character paths.

The better fix is to validate, because m2 already chose to spot-check mutated plans before resource allocation.

### 3. Lockin sync/async child API is internally inconsistent

Sections: Inputs, SP4 steps 12/18, SP1 wait discussion.

The scope chooses the sync lockin command path in Inputs and SP4 step 12:

```text
SandboxBuilder::command(self, program: &Path) -> anyhow::Result<SandboxedCommand>
SandboxedCommand::spawn() -> std::io::Result<SandboxedChild>
SandboxedChild::{wait, try_wait, kill, id, ...}
```

But SP4 step 18 says:

```text
Spawn the reaper task: tokio::spawn(async move { child.wait().await })
```

That does not compile for the sync `lockin::SandboxedChild`: its `wait()` is a blocking method returning `std::io::Result<ExitStatus>`, not a future. The async `.await` form exists only on the tokio-flavored API (`SandboxBuilder::tokio_command(...)` from `lockin::tokio`), which the spawn sequence does not use.

SP1 also hedges between two models:

```text
tokio::task::spawn_blocking wrapping child.wait() since lockin's SandboxedChild is std::process::Child-backed; the lockin/tokio feature provides an async wrapper if available
```

A ratified scope should not leave the implementation to choose after it has already written pseudocode using the other API.

Fix by choosing exactly one concrete path:

- **Sync path:** keep `builder.command(...)`, keep sync `SandboxedChild`, and specify `tokio::task::spawn_blocking(move || child.wait())` for the reaper.
- **Tokio path:** use `builder.tokio_command(...)`, use `lockin::tokio::SandboxedChild`, adjust `id()` because it returns `Option<u32>`, and use async wait consistently.

### 4. Fittings `PeerHandle` examples still use non-real parameter types

Sections: B7, H5, F2/H4.

Several examples are still not type-correct against the real fittings API:

- B7 says:

  ```text
  peer.notify("bus.event", &bus_event)
  ```

  But `PeerHandle::notify` is `notify(method, params: serde_json::Value)`. It does not accept `&BusEvent` or a generic `Serialize` value.

- H5 says:

  ```text
  spawn_handle.peer().call("core.fixture.start", ()).await
  ```

  But `PeerHandle::call` also requires a `serde_json::Value`. `()` is not accepted.

- F2/H4 say the observer's notification handler calls `peer.call("core.fixture.observed", event_payload)` on every inbound `bus.event`. `Client::with_notification_handler` takes a synchronous closure (`Fn(String, Value)`), so it cannot directly `.await` a `peer.call` inside the handler.

Fix: normalize the examples to real calls, e.g.:

```rust
let params = serde_json::to_value(&bus_event)?;
peer.notify("bus.event", params)?;
peer.call("core.fixture.start", serde_json::Value::Null).await?;
```

For the observer, state explicitly that the synchronous notification handler clones a `PeerHandle` and `tokio::spawn`s an async task to perform the forwarding call, or queues the event onto an async worker.

### 5. Fixture readiness handshake is not actually deterministic

Sections: H5, F2, positive supervisor tests.

H5 says every publishing fixture waits for `core.fixture.start`, and the harness calls that after the observer is registered and its `core.fixture.observed` service is installed. This removes one race, but it does not cover another real one: fixture-side service installation.

In current fittings, `Client::connect(connector).await` starts the client read loop immediately. `Client::with_service(...)` and `Client::with_notification_handler(...)` are called after connection construction. If core sends `core.fixture.start`, `core.fixture.echo`, or `core.fixture.dump_env` immediately after supervisor spawn returns, the fixture may not yet have installed its service/handler, and fittings will answer `MethodNotFound`.

The harness currently has no fixture-ready signal proving that the fixture has completed:

1. fd setup,
2. `Client::connect`,
3. `with_service` / `with_notification_handler`, and
4. mode-specific handler registration.

Fix: add an explicit fixture-to-core readiness signal, for example:

- fixture calls `peer.call("core.fixture.ready", { "mode": ... })` after its service/handler is installed;
- harness registers a `core.fixture.ready` extra service and waits for it before issuing `core.fixture.start` or any other core-to-plugin call.

Without this, the supposed deterministic handshake can still fail nondeterministically.

### 6. Publish-and-exit fixture modes may drop their own notifications

Sections: F2, H5, positive/negative publish tests.

Modes like `publish_one`, `publish_bad_namespace`, `publish_bad_grammar`, `publish_outside_grant`, and the bad `in_reply_to` modes say:

```text
wait for start, publish one bus.publish notification, exit 0
```

With current fittings client semantics, `client.notify(...)` enqueues a notification to the client worker; dropping the client immediately aborts that worker. A fixture that exits immediately after enqueueing `bus.publish` can lose the frame before it is written to the socket, producing flaky tests where the broker never sees the publish.

The readiness handshake does not solve this because it happens before the publish.

Fix: require a post-publish flush/ack pattern. Options:

- after `bus.publish`, call a core method like `core.fixture.after_publish` and exit only after it returns;
- keep publish modes alive until SIGTERM after publishing, so the worker has time to flush and tests can shut them down; or
- use a fittings API that guarantees the notification has been written before returning, if one exists (current `PeerHandle::notify` does not provide that guarantee).

### 7. `supervisor_taint_round_trip` is not implementable with the scoped fixture controls

Sections: F2, positive test matrix.

The positive test matrix says:

```text
supervisor_taint_round_trip.rs | A in publish_one with payload that includes taint: [{...}] in the bus.publish params.
```

But F2's `publish_one` mode only takes:

- `RFL_FIXTURE_TOPIC`
- `RFL_FIXTURE_PAYLOAD_JSON`

`taint` is not part of `payload`; it is a top-level field in `PublishMsg`:

```rust
pub struct PublishMsg {
    pub topic: String,
    pub payload: Value,
    pub in_reply_to: Option<Vec<JsonRpcId>>,
    pub taint: Option<Vec<TaintEntry>>,
}
```

Putting `taint` inside `payload` will not exercise the broker's `taint` round-trip path. As scoped, the fixture has no way to emit a `bus.publish` params object with top-level `taint`.

Fix: add fixture controls such as:

- `RFL_FIXTURE_TAINT_JSON`, and possibly `RFL_FIXTURE_IN_REPLY_TO_JSON`; or
- a full `RFL_FIXTURE_PUBLISH_PARAMS_JSON` override used verbatim as the `bus.publish` params.

### 8. `SpawnHandle::wait` storage semantics are internally wrong

Sections: SP1 lifecycle/wait text.

SP1 says:

```text
wait resolves once the reaper task has observed child exit and published the status to the shared broadcast slot. Multiple awaits return the same status (broadcast slot retains the most-recent value).
```

Tokio `broadcast` does not provide this semantic for new subscribers. A new receiver created after the value was sent does not automatically receive the last value. The described behavior is closer to `watch`, `OnceCell`, or an explicit cached `Option<ExitStatus>` plus notification.

There is a second issue in the same paragraph:

```text
if the reaper task panicked the broadcast resolves to a structured "reaper-died" status
```

`std::process::ExitStatus` cannot represent a synthetic structured status. The public signature is:

```rust
pub async fn wait(&self) -> ExitStatus;
pub fn try_wait(&self) -> Option<ExitStatus>;
```

So there is no type-level place for `reaper-died` unless the implementation fabricates an OS exit status, which is not portable or honest.

Fix: define one of these models explicitly:

- `wait() -> Result<ExitStatus, ReaperError>` and `try_wait() -> Option<Result<ExitStatus, ReaperError>>`; or
- a custom `PluginExitStatus` enum with `Exited(ExitStatus)` and `ReaperDied`; or
- keep `ExitStatus` infallible but remove the `reaper-died` promise and specify that reaper panics abort/log and are not represented through `wait()`.

Also replace the broadcast-slot wording with a concrete cached-status mechanism.

## Non-blocking but should fix before ratification

1. **SP3's enum block omits `TransportSetup`.**
   - SP3 first shows the `SpawnError` enum without `TransportSetup`, then prose below adds it.
   - Put `TransportSetup { canonical, source }` directly in the enum block so implementers do not miss it.

2. **`RegisteredPlugin::drop` cannot literally close all notification channels.**
   - B1 says dropping the guard removes the entry from `plugins` and closes the in-flight notification channel for that plugin.
   - But `PeerHandle` is cloneable and may be held by `SpawnHandle`, tests, or the server. The broker can stop fan-out by dropping its clone, but it cannot guarantee every channel clone is closed.
   - Reword to “removes the broker's registration and drops the broker-held `PeerHandle` clone.”

3. **Socketpair pseudocode is still C-shaped.**
   - SP4 says `socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0)`.
   - For nix 0.29 the Rust shape is `socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::SOCK_CLOEXEC)`.
   - This is not a major blocker because the scope already names nix, but using the Rust form would avoid another coordinate lookup.

4. **`SpawnError::ReservedEnvInPlan` and `InvalidPlanReason::ReservedEnvVar` overlap.**
   - SP3 includes both. The scope says the separate variant is “for clarity,” but tests only assert the top-level variant.
   - This is implementable, but redundant. Consider dropping the nested reason or stating exactly when each is used.

5. **`create_dir_all` failure is mapped to `TransportSetup`, which is semantically odd.**
   - SP4 step 11 maps private-state directory creation failure to `TransportSetup` as a general setup bucket.
   - It will compile, but a dedicated `SetupIo` / `PrivateStateDirCreate` variant would make failures clearer.

6. **Publish rejection event recursion bypass wording should mention serialization failures.**
   - B9 says rejection-event construction cannot recurse because it does not call rejection emission.
   - If `BusEvent` serialization to `Value` is used before fan-out, that operation should be infallible for the declared schema, but the scope should either rely on `json!` construction or state serialization failure maps to `Internal` and does not emit another rejection.

## Summary of required edits

Before ratification, make these concrete edits:

- Add explicit Phase A validation for `SpawnPaths` absolute paths, and extend `PathKind` / error reporting accordingly.
- Validate control characters in all plan paths passed to lockin, or narrow the anti-panic spot-check claim.
- Normalize the lockin child model: either sync `command` + `spawn_blocking`, or `tokio_command` + async child throughout.
- Change fittings examples to pass `serde_json::Value`; serialize `BusEvent` before `peer.notify`; use `Value::Null` for empty params.
- Specify that observer notification forwarding spawns/queues async work because `with_notification_handler` is synchronous.
- Add a fixture-ready signal before the harness sends `core.fixture.start` or other core-to-plugin calls.
- Add a post-publish flush/ack or keep publish fixtures alive so publish-and-exit modes cannot drop queued notifications.
- Add fixture controls for top-level `taint` in `bus.publish` params.
- Replace the incorrect broadcast-slot wait model with a real cached-status mechanism and resolve the `reaper-died` type mismatch.
