# Pi review round 6 — m2 broker + locked plugin spawn scope

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: adversarial round 6, focused on whether the round-5 fixes made the scope ratifiable and whether remaining wording can be followed literally by implementers.

## Verdict

**Not ratifiable yet, but close.**

Round 6 closes the substantive round-5 blockers: the cached wait model now uses `watch<Option<Arc<ReaperOutcome>>>`, the fixture publish flush now uses the `ClientCommand` FIFO, unknown-vs-reserved namespace wording is fixed, `in_flight` is in the numbered spawn sequence, post-spawn failures now have a kill-and-reap contract, and the lowercase proxy env allowlist is fixed.

The remaining issues are much smaller, but there are still a few literal API/type contradictions in the authoritative supervisor section. If implemented as written, one line selects the wrong lockin API, shutdown cannot faithfully report a non-cloneable wait error from the cached reaper outcome, and Phase A references an ACL field the supervisor is not supposed to access.

## Things newly right in round 6

1. **Wait/reaper storage is now mostly type-correct.**
   - `wait()` / `try_wait()` return `Arc<ReaperOutcome>`.
   - The cached state is a `tokio::sync::watch<Option<Arc<ReaperOutcome>>>`.
   - `ReaperPanicked` is now backed by a second watcher task that awaits the reaper `JoinHandle`.

2. **Publish-and-exit flush now uses the correct fittings queue.**
   - F2 requires `client.notify("bus.publish", ...)` followed by `client.call("core.fixture.after_publish", Value::Null)`.
   - The prior `PeerHandle::notify` / `PeerHandle::call` ordering bug is closed.

3. **Core publish namespace precedence is now coherent.**
   - Unknown top-level namespaces are `UnknownNamespace`.
   - Known-but-unauthorised namespaces are `PublishOnReservedNamespace`.

4. **Duplicate-spawn protection is now in SP4 Phase A.**
   - Step 1a acquires a supervisor-local `in_flight` guard before the broker precheck.

5. **Post-spawn unwind now has an explicit reap requirement.**
   - The post-spawn unwind contract says every failure after `cmd.spawn()` and before the reaper task must SIGKILL and `child.wait().await`.

6. **Proxy env test can now observe lowercase vars.**
   - The `RFL_FIXTURE_ENV_KEYS` allowlist includes uppercase and lowercase proxy variables.

## Blocking findings

### 1. SP4 still calls the sync lockin API while claiming to produce a tokio child

Sections: Inputs lockin API, SP4 step 12.

The Inputs section correctly chooses lockin's tokio API:

```text
SandboxBuilder::tokio_command(self, program: &Path)
  -> anyhow::Result<lockin::tokio::SandboxedCommand>
```

But the authoritative spawn sequence still says:

```text
builder.command(&plan.entry_absolute).map_err(|e| Lockin { source: e })?
  -> lockin::tokio::SandboxedCommand
```

That is not the repo API. In current lockin:

- `SandboxBuilder::command(...)` returns the sync `lockin::SandboxedCommand`;
- `SandboxBuilder::tokio_command(...)` returns `lockin::tokio::SandboxedCommand`.

If an implementer copies SP4 literally, the later `child.wait().await` / `lockin::tokio::SandboxedChild` design does not typecheck.

Fix: change SP4 step 12 to call `builder.tokio_command(&plan.entry_absolute)`, and adjust the nearby `Lockin.source` note that still talks about `command(...)`.

### 2. `ShutdownFailure::WaitFailed(std::io::Error)` cannot be produced from the cached `Arc<ReaperOutcome>` model

Sections: SP1 `ReaperOutcome`, `ShutdownReport`, lifecycle/shutdown.

Round 6 fixed repeatable `wait()` by storing the non-cloneable `std::io::Error` behind an `Arc<ReaperOutcome>`:

```rust
pub enum ReaperOutcome {
    Exited(std::process::ExitStatus),
    WaitFailed(std::io::Error),
    ReaperPanicked,
}

pub async fn wait(&self) -> Arc<ReaperOutcome>;
```

But `ShutdownReport` still wants to own a fresh non-cloneable wait error:

```rust
pub enum ShutdownFailure {
    SignalSendFailed(nix::errno::Errno),
    WaitFailed(std::io::Error),
}
```

During `shutdown`, the child is owned by the reaper task; shutdown observes the result through the same cached `Arc<ReaperOutcome>` as everyone else. If the outcome is `WaitFailed(e)`, shutdown cannot move `e` out of the `Arc` and cannot clone it. `Arc::try_unwrap` is not reliable because the watch channel and other waiters may hold clones.

Fix options:

- make `ShutdownFailure::WaitFailed` store a cloneable projection (`ErrorKind` + message string);
- store `Arc<std::io::Error>` / `Arc<ReaperOutcome>` in the failure variant;
- make `ReaperOutcome::WaitFailed` itself store a cloneable projection.

The scope should pick one so shutdown reporting remains implementable.

### 3. SP4 Phase A still reaches for `broker_acl.plugins`, contradicting the public broker lookup path

Sections: B1 `Broker::plugin_acl`, SP4 Phase A step 2.

B1 explicitly defines the supervisor's ACL lookup API:

```text
Broker::plugin_acl(&self, canonical) -> Option<PluginAcl>
— ... this is the canonical lookup path; test helpers MUST NOT reach for an acl() accessor
```

But SP4 step 2 says:

```text
Look up plan.canonical in broker_acl.plugins, compare ACL's topic_id ...
```

`PluginSupervisor::spawn` receives only `&CompiledPlugin` and `&SpawnPaths`; it owns a `Broker`, not a `BrokerAcl`, and the scope intentionally avoids exposing a raw `acl()` accessor. The numbered sequence should not tell implementers to use an unavailable/private field.

Fix: rewrite step 2 to use `broker.plugin_acl(&plan.canonical)` and map `None` consistently with the prior `NotInAcl` path.

## Non-blocking but should fix before owner ratification

1. **Stale `broadcast channel` fragment remains in SP4 step 18.**
   The wait design is now `watch`, but lines after the watcher task still say "broadcast channel inside the `SpawnHandle`'s shared state". Delete the stale fragment.

2. **Boot-event test row contradicts `PluginSupervisor::new`.**
   The API block says `PluginSupervisor::new` does **not** auto-emit boot, but `broker_publish_boot_event.rs` says explicit `broker.publish_boot()` is "the same call `PluginSupervisor::new` makes". Remove that parenthetical.

3. **`SpawnPaths` directory creation wording conflicts with itself.**
   SP4/Risks correctly say `create_dir_all(<leaf>)` creates intermediate dirs. H3 says the harness must create the parent because the supervisor "only `create_dir_all`s the leaf". Since `create_dir_all` on the leaf creates parents too, the harness parent step is redundant and the wording is confusing.

4. **`PluginSupervisor::Drop` should not say it "lets the OS reap".**
   After a parent sends SIGKILL, the OS does not reap the child while the parent is still alive; the reaper task does. Reword to say Drop sends SIGKILL and relies on the already-running reaper task/best-effort process cleanup, while graceful deterministic cleanup requires `shutdown().await`.

5. **`in_flight` guard success wording is confusing.**
   SP1 says successful `register_plugin` removes from `in_flight`, but SP4 step 1a says the success path moves the guard into `SpawnHandle` shared state and releases it after `register_plugin`. Prefer the simpler rule: hold the RAII guard through Phase B, remove it immediately after successful `broker.register_plugin`, and remove it on every failure.

6. **Step 17 should repeat the reap wording.**
   The global post-spawn contract is correct, but step 17's local unwind sentence still says only "kill child via SIGKILL, drop proxy". Add "and reap" there too so future edits do not regress the zombie fix.

## Summary of required edits before ratification

- Use `builder.tokio_command(...)` in SP4 step 12 and in the nearby lockin-source note.
- Make shutdown wait-failure reporting cloneable/shareable under the `Arc<ReaperOutcome>` cache model.
- Replace `broker_acl.plugins` in SP4 Phase A with `broker.plugin_acl(...)`.
- Clean up the stale non-blocking wording above.
