# Pi review 21 — m3 TUI sessions scope

Reviewed: `rafaello/plans/milestones/m3-tui-sessions/scope.md`  
Commit: `3628365f70909d1f63456e5e9b6eecb4c447d5d6`  
Scope draft status: round-21 draft

## Summary

Round 21 cleanly incorporates the round-20 corrections that were most likely to block implementation: step 10 now names the public `frontend_handle.wait().await` outcome API, §H6.2's post-spawn/pre-register test split is reflected in both the detailed fault-injection section and the integration-test matrix, the cross-platform post-spawn/pre-register assertion now uses the existing `try_reserve_registration` API instead of an invented broker accessor, and the H6.1 inject-point count is three.

The remaining issues are implementation-guidance problems, not scope-shaping disagreements. The most important one is the `rfl chat` cleanup-guard pseudocode: it still describes an ownership/borrow pattern that cannot be implemented literally while preserving the single teardown owner. The other findings are smaller ambiguities around Rust pattern sketches, H6 hook placement and injected-fault naming, and two stale m2-era wording remnants.

Findings: **0 blockers, 1 high, 3 medium, 2 low**.

## Findings

### High — §C2 cleanup-guard pseudocode is still ownership-invalid

**References:**

- `scope.md` lines 2187–2188: `let mut cleanup_state = Some((frontend_handle, stderr_forwarder));`
- `scope.md` line 2195: the async block calls `frontend_handle_ref.wait().await`.
- `scope.md` lines 2200–2202: teardown later `take()`s the owned handle and calls `handle.shutdown().await`.
- `scope.md` lines 2267–2270: step-10 prose says the guard takes ownership before the async block starts.

Round 21 correctly says step 10 reads the public `FrontendHandle::wait()` result instead of reaching into `reaper_outcome.borrow()`. However, the surrounding cleanup-guard pseudocode still parks ownership of the handle inside `cleanup_state` and then uses an undefined `frontend_handle_ref` inside the async block:

```rust
let mut cleanup_state = Some((frontend_handle,
    stderr_forwarder));
let result: Result<(), RflChatError> = async {
    // ...
    let outcome = frontend_handle_ref.wait().await;
    // ...
}.await;
if let Some((handle, forwarder)) = cleanup_state.take() {
    let _ = handle.shutdown().await;
    let _ = forwarder.await;
}
```

That is not an implementable Rust ownership pattern as written. If the handle is moved into `cleanup_state`, the async block cannot also hold a long-lived mutable reference to it unless the plan specifies exactly how that borrow is obtained and scoped. Conversely, if the handle is held outside `cleanup_state`, the guard no longer clearly owns the later `shutdown(self)` call.

**Why this matters:** this is the critical `rfl chat` teardown path. Ambiguous guidance here can lead to double-consumption of `FrontendHandle`, missed teardown on an error branch, or a compile-time dead end during implementation. This is especially risky because the previous rounds intentionally converged on “cleanup guard is the sole teardown path.”

**Required fix:** replace the pseudocode with an ownership-correct pattern. For example, keep `Option<FrontendHandle>` and `Option<JoinHandle<_>>`, borrow the handle only inside a narrow block for `wait().await`, and then `take()` the same option in one cleanup section for `shutdown(self)` and forwarder drain. The exact implementation can vary, but the normative sketch should no longer require an undefined `frontend_handle_ref` while ownership is stored elsewhere.

---

### Medium — §F4 `reaper_outcome.borrow()` sketches are non-compilable if followed literally

**References:**

- `scope.md` line 1114: shutdown says to read `*self.reaper_outcome.borrow()`.
- `scope.md` line 1145: live-watch recheck repeats `*self.reaper_outcome.borrow()`.
- `scope.md` line 1176: report population reads `*self.reaper_outcome.borrow()` after the signal flow.
- `scope.md` line 1192: Drop also reads `*self.reaper_outcome.borrow()`.

These references are inside `FrontendHandle` internals rather than `rfl chat`, so they do not repeat the round-20 public/private API bug. The problem is subtler: `tokio::sync::watch::Receiver::borrow()` returns a watch reference, and the stored value is `Option<Arc<ReaperOutcome>>`. A literal `*self.reaper_outcome.borrow()` attempts to move a non-`Copy` `Option<Arc<_>>` out of that borrow. Likewise, prose patterns such as `Some(Arc<ReaperOutcome::Exited(_)>)` are not Rust patterns.

**Why this matters:** §F4 is a detailed implementation algorithm for shutdown and Drop. It is close enough to code that implementers are likely to transliterate it. Transliterating these snippets will not compile, and may cause unnecessary churn in the already delicate shutdown code.

**Recommended fix:** spell the cache checks in implementable terms, for example:

```rust
match self.reaper_outcome.borrow().as_ref().map(Arc::as_ref) {
    Some(ReaperOutcome::Exited(status)) => { /* ... */ }
    Some(ReaperOutcome::WaitFailed(_)) | Some(ReaperOutcome::ReaperPanicked) => { /* ... */ }
    None => { /* ... */ }
}
```

Alternatively, say “clone the borrowed `Option<Arc<ReaperOutcome>>` before matching” wherever ownership of the cached `Arc` is needed.

---

### Medium — H6 pre-spawn hook placement is ambiguous relative to private-state-dir cleanup assertions

**References:**

- `scope.md` lines 2486–2490: **Pre-spawn-post-socketpair** injects after socketpair / proxy / sandbox-builder allocation and before `tokio_command.spawn()`.
- `scope.md` lines 2489–2490: the same bullet says unwind verifies fd count returns to baseline plus proxy / private-state dirs cleaned up.
- `scope.md` lines 2535–2540: `supervisor_spawn_unwinds_after_socketpair.rs` arms the pre-spawn-post-socketpair fault and asserts proxy and private-state dirs are cleaned up.

The hook name and placement say “after socketpair / proxy / sandbox-builder allocation, before spawn.” The cleanup assertion also includes private-state dirs, but the detailed bullet does not say whether the private-state dir has already been created before this hook fires.

Current m2 code creates the private-state dir after sandbox-builder setup and before command spawn. If the new hook fires before `create_dir_all`, there is no private-state dir to clean up and the assertion is meaningless or impossible. If it fires after `create_dir_all`, then the hook placement should say so explicitly.

**Why this matters:** H6 is meant to re-add deleted unwind tests against precise ownership windows. An imprecise pre-spawn hook location can make the test either flaky or over-specified, depending on where the implementer places the injection check.

**Recommended fix:** define the pre-spawn hook as either:

- after private-state-dir creation as well as socketpair/proxy/sandbox-builder allocation, if the cleanup assertion should remain; or
- before private-state-dir creation, in which case remove private-state-dir cleanup from this test’s expected assertions.

---

### Medium — H6.1 injected-fault identity is under-specified for three hooks

**References:**

- `scope.md` lines 2443–2455: H6.1 now lists three hook methods and three consumed accessors.
- `scope.md` line 2478: all injected faults are described as returning `anyhow!("test-injected pre-register fault")` “or post-register equivalent.”

Round 21 fixed the inject-point count, but the injected error marker still uses the old two-window language. There are now three distinct hooks:

1. `inject_pre_spawn_fault`
2. `inject_post_spawn_pre_register_fault`
3. `inject_post_register_fault`

The prose only names “pre-register” and “post-register equivalent.” It does not say whether “pre-register” means pre-spawn or post-spawn/pre-register, nor does it define an expected marker for the remaining hook.

**Why this matters:** tests often match injected-fault source text to distinguish synthetic faults from real sandbox/build failures. Without explicit per-hook markers, implementations and tests may drift: one side may assert exact strings while another treats all pre-register faults as the same marker.

**Recommended fix:** either specify three exact markers, for example:

- `test-injected pre-spawn fault`
- `test-injected post-spawn-pre-register fault`
- `test-injected post-register fault`

or explicitly say tests assert only the `SpawnError::SandboxBuild` variant plus the relevant `*_fault_consumed()` accessor, not the source string.

---

### Low — §F3 frontend unwind checklist still contains a non-existent proxy-handle step

**Reference:**

- `scope.md` lines 1035–1037: “Drop the proxy handle (m3 frontends don't have one but the unwind framework should be the same as m2).”

This is low severity because the sentence admits frontends do not have a proxy. Still, it appears as item 5 in the frontend supervisor’s numbered Phase-B unwind rules. A literal implementer may waste time inventing placeholder proxy plumbing or carrying an irrelevant no-op through the frontend code.

**Recommended fix:** remove the proxy-handle item from the frontend-specific numbered unwind checklist. If the m2 symmetry is worth documenting, move it to a short note outside the checklist: “Unlike the m2 plugin supervisor, the frontend path has no proxy handle to drop.”

---

### Low — risk section still names deprecated `nix::fcntl::flock` instead of the RAII `Flock` helper

**References:**

- `scope.md` lines 1566–1569: §S5 explicitly rejects deprecated `nix::fcntl::flock(fd, FlockArg::...)` and mandates `nix::fcntl::Flock`.
- `scope.md` lines 3294–3298: Risk 3 says “`nix::fcntl::flock` works on both Linux and macOS” and “m3 only uses `flock` from a single process holding the fd.”

The normative session-store section is correct: m3 should use `nix::fcntl::Flock` so warning-free build/doc gates are not jeopardized by deprecated function usage. The risk section still uses the deprecated function name, which is a stale wording remnant from earlier rounds.

**Why this matters:** this is unlikely to mislead implementation as badly as the normative sections, but it undercuts a hard acceptance concern: warning-free cargo doc/build. It is also easy to fix.

**Recommended fix:** rewrite the risk text to say “the underlying `flock(2)` semantics” and “m3 uses `nix::fcntl::Flock<File>` holding the fd for the store lifetime,” avoiding the deprecated `nix::fcntl::flock` function name.

## Non-findings / round-21 verification notes

- The step-10 `rfl chat` outcome source now names the public `frontend_handle.wait().await -> Arc<ReaperOutcome>` API rather than an internal `reaper_outcome.borrow()` field access.
- The post-spawn/pre-register unwind coverage is now split into a cross-platform behavioral test and a Linux-only fd-baseline complement in both §H6.2 and the §I matrix.
- The cross-platform post-spawn/pre-register registration assertion now uses the existing `broker.try_reserve_registration(canonical)` API rather than an invented `broker.is_registered` accessor.
- H6.1’s inject-point count now says three and lists three hook methods.
