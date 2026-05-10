# Pi review 18 — m3 TUI sessions scope

Reviewed: `rafaello/plans/milestones/m3-tui-sessions/scope.md`  
Commit: `ea604427608509721a2bb12b66dd5bf2cd8ae793`  
Scope draft status: round-18 draft

## Summary

Round 18 is substantially tighter on readiness ordering, teardown intent, project-root canonicalisation, and m1 unknown-namespace tightening. Remaining issues are concentrated in implementability/coverage edges rather than broad architectural drift.

Findings: **1 blocker, 2 high, 2 medium**.

## Findings

### Blocker — cleanup guard conflicts with step-10 teardown

**References:**

- `scope.md` lines 2125–2155: cleanup-guard contract for steps 8 onward.
- `scope.md` lines 2205–2221: step-10 shutdown-first/drain-stderr sequence.

The cleanup-guard block says orchestration must always run the canonical teardown:

```rust
if let Some((handle, forwarder)) = cleanup_state.take() {
    let _ = handle.shutdown().await;
    let _ = forwarder.await;
}
```

But step 10 also explicitly prescribes:

1. `let report = frontend_handle.shutdown().await;`
2. `let _ = stderr_forwarder.await;`

Both `FrontendHandle::shutdown(self)` and awaiting the forwarder consume owned values. As written, the plan is not implementable without either double-consuming the handle/forwarder or introducing an unstated ownership indirection. It also contradicts the nearby claim that shutdown+drain happens “exactly once”.

**Why this matters:** the m3 happy path and every post-readiness error path pass through this region. A driver following the literal text can easily reintroduce the earlier double-shutdown problem that round 10/round 18 were trying to eliminate.

**Required fix:** choose one canonical teardown mechanism:

- Preferably keep the cleanup guard as the single owner of teardown and make step 10 only classify the `ReaperOutcome` / set the eventual `Result`, without directly calling shutdown/drain; or
- Remove the guard and keep explicit teardown in every branch, with tests asserting all fallible paths drain stderr.

Also fix the pseudocode ownership mismatch (`cleanup_state` owns `frontend_handle`, but the async block refers to `frontend_handle_ref`).

---

### High — H6 dismisses a distinct plugin-spawn fault window

**References:**

- `scope.md` lines 2386–2440, especially lines 2430–2438.
- Current m2 `PluginSupervisor::spawn` code order: child spawn happens before transport/server construction and before `register_plugin`; reaper is spawned after registration.

Round 18 explicitly declines to inject between `tokio_command.spawn()` and `register_plugin`, saying that a fault in this span has “the same unwind shape as the pre-spawn-post-socketpair point but with a child to reap”. That is a distinct ownership state, not equivalent to either named inject point:

- Pre-spawn-post-socketpair: no child exists.
- Spawn-to-register: child exists, but no broker registration exists and no reaper owns the child yet.
- Post-register: child exists, broker registration exists, and the reaper/watcher are running.

The spawn-to-register window must kill and reap a directly-owned `Child`, while also unwinding partially built transport/server/proxy/socket resources without dropping a registration guard. That is materially different from both existing injection points.

**Why this matters:** H6 is meant to close the m2 retro §3.3 unwind coverage gap. The current wording creates false confidence that two inject points cover all load-bearing ownership transitions, while one real transition remains untested.

**Recommended fix:** add a third one-shot hook, e.g. `inject_post_child_spawn_pre_register_fault`, and a test such as `supervisor_spawn_unwinds_after_child_spawn_before_register.rs` asserting:

- spawn returns `SpawnError::SandboxBuild` with the injected source;
- no broker registration remains;
- `in_flight` is cleared;
- the directly-owned child is killed/reaped;
- fd/proxy/private-state cleanup returns to baseline where platform support allows.

If the team intentionally defers this, downgrade the completeness claim and record it as an explicit known coverage gap.

---

### High — frontend publish authority misses two-segment own-id rejection

**References:**

- `scope.md` lines 1277–1305, especially lines 1293–1303.
- Existing m2 plugin behavior/test: `broker_publish_short_plugin_topic_rejected.rs` rejects `plugin.<own-topic-id>` as `PublishOnReservedNamespace`, not `PublishOutsideGrant`.

B4 says:

- `frontend` alone or any topic with `<2` segments is rejected earlier by `validate_topic`.
- `frontend` with `segments[1] != attach_id` is `PublishOnReservedNamespace`.
- `frontend.<own-attach-id>.*` goes to exact grant checking and becomes `PublishOutsideGrant` if not granted.

This misses the two-segment own-id topic `frontend.tui`. It is grammar-valid because `validate_topic` requires at least two segments, but it is semantically empty in the same way as `plugin.<id>`. Under the current wording, `frontend.tui` would fall through to grant checking and likely return `PublishOutsideGrant`.

**Why this matters:** frontend publish classification is supposed to mirror plugin publish classification. Leaving `frontend.tui` as outside-grant creates a visible asymmetry and likely test drift once frontend broker tests are implemented.

**Recommended fix:** specify that `frontend.*` topics with fewer than three segments are rejected as `PublishOnReservedNamespace`, including `frontend.<own-attach-id>`. Add a negative test for `frontend_publish_short_own_topic_rejected.rs` or fold it into the reserved-namespace frontend publish test.

---

### Medium — M1 tightens manifests but leaves lock validation gap

**References:**

- `scope.md` lines 2462–2520.
- Current code shape: `check_publish_topic` rejects manifest publish grants, while `check_lock_publish_topic` still has an `_ => {}` arm for unknown namespaces.

M1 correctly adds `ValidationError::PublishUnknownNamespace { topic, namespace }` to reject manifest `publishes = ["evil.foo"]` at parse/validation time. However, the existing lock-validation path has a parallel namespace gate for lock publish topics and currently accepts unknown top-level namespaces in its catch-all branch.

Round 18 says “The `manifest_with_id` layer is unchanged” and frames the patch as manifest-only. That may be intentional, but it leaves a hand-authored or stale lockfile able to carry `evil.foo` until broker runtime rejection.

**Why this matters:** m2 retro §2.8 identified a parse-time mirror gap between grants and broker runtime. If only manifests are tightened, lock validation remains a second parse-time mirror gap and can surprise the implementation driver/test author.

**Recommended fix:** either:

- add a lock-side unknown namespace variant and reject unknown top-level namespaces in `check_lock_publish_topic`, with a lock regression test; or
- explicitly state that m3 only tightens manifests and intentionally leaves hand-authored lockfiles runtime-checked, with rationale.

---

### Medium — manual validation command is weaker than acceptance

**References:**

- `scope.md` line 2969: manual validation item says `cargo test --workspace`.
- `scope.md` lines 3302–3304 and 3310–3311: acceptance requires `cargo test --workspace --features test-fixture`.

The manual validation checklist omits the `test-fixture` feature, while the acceptance gate requires it. In m3, many subprocess/fixture paths are feature-gated and central to the milestone’s correctness.

**Why this matters:** the weaker manual command can pass while fixture-gated tests or the `rfl-bus-fixture` binary are broken, creating ratification drift.

**Recommended fix:** change the manual validation item to match acceptance, e.g.:

```text
nix develop --impure --command cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture
```

and keep the separate `cargo build --workspace --bins --features rafaello-core/test-fixture` acceptance gate for binary-build coverage.
