# Pi review 20 — m3 TUI sessions scope

Reviewed: `rafaello/plans/milestones/m3-tui-sessions/scope.md`  
Commit: `cf9c70133b5b5b7a716b5772f3055a793ac8d75d`  
Scope draft status: round-20 draft

## Summary

Round 20 fixes the main round-19 blocker: step 10 no longer contains an explicit shutdown/drain sequence competing with the cleanup guard. The lock-side validation deferral is now present in the Acceptance-summary drift list, and the post-spawn/pre-register unwind coverage was split in the matrix into cross-platform behavioral coverage plus Linux-only fd-baseline coverage.

The remaining issues are implementation-guidance inconsistencies introduced or left behind by that cleanup: step 10 now names a private field instead of the public wait result, §H6.2 still contains the old Linux-only post-spawn/pre-register test wording, and the new cross-platform unwind assertions reference nonexistent or underspecified inspection hooks.

Findings: **0 blockers, 1 high, 2 medium, 1 low**.

## Findings

### High — step-10 outcome source contradicts the public `FrontendHandle` API

**References:**

- `scope.md` lines 1041–1043: the public lifecycle API is `FrontendHandle::wait(&mut self) -> Arc<ReaperOutcome>`.
- `scope.md` lines 1061–1062: `reaper_outcome` is an internal `FrontendHandle` field in the implementation sketch.
- `scope.md` lines 2212–2214: step 10 says `rfl chat` waits on `frontend_handle.wait().await -> Arc<ReaperOutcome>`.
- `scope.md` lines 2256–2258: the new round-20 teardown text says step 10 reads `*reaper_outcome.borrow()` for outcome mapping.

Round 20 correctly removes the old explicit step-10 `shutdown().await` + forwarder drain block, but the replacement text introduces a new contradiction. The normative step-10 entry point for `rfl chat` is the public handle method:

```rust
let outcome = frontend_handle.wait().await;
```

The new cleanup-guard paragraph instead says:

```rust
*reaper_outcome.borrow()
```

That field is private implementation state of `FrontendHandle`, not a public API exposed to the `rafaello` CLI crate. Following the new wording literally would either fail to compile or push the implementer to expose internal watch state unnecessarily.

**Why this matters:** this is in the critical teardown/outcome path that round 19/20 tried to make unambiguous. It reopens implementation ambiguity right where the plan needs to be most precise: step 10 should classify the already-returned `Arc<ReaperOutcome>` from `wait().await`, while the cleanup guard remains the sole owner of `shutdown(self)` and stderr-forwarder drain.

**Required fix:** rewrite the round-20 sentence to say step 10 uses the `Arc<ReaperOutcome>` returned by `frontend_handle.wait().await` for outcome mapping. Do not reference `reaper_outcome.borrow()` from `rfl chat` unless the scope explicitly adds a public accessor and justifies exposing that internal watch.

---

### Medium — post-spawn/pre-register split is only partially applied

**References:**

- `scope.md` lines 16–23: round-20 highlights say the post-spawn/pre-register unwind test is split into cross-platform behavioral coverage and a Linux-only fd-baseline complement.
- `scope.md` lines 2482–2489: §H6.2 still says the post-spawn/pre-register test asserts Linux fd-count baseline plus `/proc` child absence and is Linux-only.
- `scope.md` lines 2680–2698: the integration-test matrix contains the new split: `supervisor_spawn_unwinds_post_spawn_pre_register.rs` cross-platform and `supervisor_spawn_unwinds_post_spawn_pre_register_fd_baseline.rs` Linux-only.

The round-20 matrix now has the right high-level shape, but §H6.2 still contains the old unsplit wording. That leaves two conflicting definitions for the same behavior:

- §H6.2: one Linux-only test that checks fd-count and `/proc` child absence.
- §I matrix: one cross-platform behavioral test plus one Linux-only fd-baseline test.

**Why this matters:** §H6 is the detailed fault-injection contract, while §I is the test inventory. Implementers could reasonably follow either section and produce different coverage. In particular, macOS could still lose coverage if the implementer follows §H6.2's stale “Linux-only” sentence.

**Recommended fix:** update §H6.2 to match the matrix. The post-spawn/pre-register detailed text should describe the cross-platform assertions separately from the Linux-only fd-baseline/proc-style assertions, or simply forward-reference the two named tests in §I.

---

### Medium — new cross-platform unwind test names nonexistent or underspecified inspection APIs

**References:**

- `scope.md` lines 2683–2686: the cross-platform test must assert no broker registration via `broker.is_registered(canonical) == false`.
- `scope.md` lines 2686–2691: the same paragraph says the child reap/no-zombie property is covered by reading `exit_status` from the wait call inside the unwind path.
- Current `Broker` surface has `contains_plugin`, but it checks ACL membership, not live registration (`rafaello/crates/rafaello-core/src/bus.rs` lines 138–140).

The new cross-platform test is directionally correct, but two of its observability points are not specified against an actual API:

1. `broker.is_registered(canonical)` is not part of the current broker API, and the closest existing method, `contains_plugin`, answers a different question: whether the canonical exists in the ACL. It remains true even when no live registration exists.
2. The plan says the test can verify cleanup by reading `exit_status` from the wait call inside the unwind path, but no hook or return value is specified to expose that internal `child.wait().await` result to an integration test.

**Why this matters:** this can strand implementers between adding ad hoc test-only APIs, weakening the assertions, or accidentally using `contains_plugin` and asserting the wrong thing. The round-20 split was meant to make the non-Linux test clean and portable; it needs portable observability too.

**Recommended fix:** either specify the required test-only accessors, or rewrite the assertions to use already-planned public behavior. For the registration check, `broker.try_reserve_registration(&canonical)` succeeding after the failed spawn would prove no live registration remains. For the direct child reap path, add an explicit `TestHooks` accessor if the exit status must be asserted, or limit the cross-platform test to hook-consumed/error/in-flight/registration-slot assertions and leave process-leak depth to the Linux fd-baseline test.

---

### Low — H6.1 still says “two” inject points while listing three

**References:**

- `scope.md` lines 2425–2426: “Extend m2's `TestHooks` with two one-shot inject points”.
- `scope.md` lines 2432–2437: the listed API contains three inject points and three consumed accessors: pre-spawn, post-spawn-pre-register, and post-register.

This is a stale count left over from the earlier two-window framing. The API and later prose correctly identify three windows.

**Recommended fix:** change “two one-shot inject points” to “three one-shot inject points”.
