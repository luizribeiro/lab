# Pi review 19 — m3 TUI sessions scope

Reviewed: `rafaello/plans/milestones/m3-tui-sessions/scope.md`  
Commit: `c13c79febd0bc31abead7da3f5f5f4aa442b107a`  
Scope draft status: round-19 draft

## Summary

Round 19 addresses the major round-18 themes in intent: the frontend publish two-segment case is now specified, the post-spawn/pre-register ownership window is acknowledged, and lock-side validation deferral is made explicit. However, one critical contradiction remains in the step-10 teardown text, and two traceability/coverage gaps need cleanup before this scope is implementation-ready.

Findings: **1 blocker, 2 medium**.

## Findings

### Blocker — step 10 still contradicts the cleanup-guard teardown contract

**References:**

- `scope.md` lines 2160–2178: cleanup-guard pseudocode runs `handle.shutdown().await` then `forwarder.await` exactly once after steps 8–10.
- `scope.md` lines 2233–2249: step 10 still explicitly prescribes `frontend_handle.shutdown().await` followed by `stderr_forwarder.await`.
- `scope.md` lines 2330–2341: later text says step 10 does **not** call shutdown and the cleanup guard performs teardown.

Round 19 says the cleanup guard is the sole teardown path, but the older step-10 shutdown/drain block remains in place:

```rust
let report = frontend_handle.shutdown().await;
let _ = stderr_forwarder.await;
```

That directly conflicts with the later round-19 text that step 10 only reads `ReaperOutcome` and never calls shutdown. It also reintroduces the same double-consume/double-shutdown ambiguity that round 19 claims to have fixed: both `FrontendHandle::shutdown(self)` and awaiting the stderr forwarder consume owned resources, while the cleanup guard also owns and consumes them.

**Why this matters:** an implementer following the literal step-10 sequence can still write the unimplementable/double-teardown path from round 18. The most important round-19 correction is therefore not actually represented consistently in the normative step list.

**Required fix:** remove or rewrite the `scope.md` lines 2233–2249 block so it describes the cleanup guard's teardown order, not an explicit step-10 action. Step 10 should only classify the `ReaperOutcome` and set the final result; the guard should be the single owner of shutdown + stderr drain on every post-readiness path.

---

### Medium — M1.4 claims lock-side validation deferral is recorded in Acceptance summary, but it is missing

**References:**

- `scope.md` lines 2567–2576: M1.4 says lock-side unknown-namespace validation is not touched in m3 and is “Recorded as anticipated drift in §\"Acceptance summary\".”
- `scope.md` lines 3407–3464: the Acceptance summary anticipated-drift list does not include this item.

The new M1.4 text intentionally leaves `check_lock_publish_topic` unchanged even though the manifest-side validator gains `PublishUnknownNamespace`. That may be an acceptable scope choice, but the traceability claim is currently false: the Acceptance summary does not list this as an anticipated drift item.

**Why this matters:** this is a deliberate validator asymmetry. If it is not listed in the retrospective/acceptance drift checklist, implementation review can either forget to document it or mistake it for an accidental omission.

**Recommended fix:** add an Acceptance-summary retrospective bullet for the lock-side unknown-namespace deferral, e.g. that hand-authored lock publish topics with unknown top-level namespaces remain broker-enforced only in m3, with m4+ revisit criteria. Alternatively, remove the “Recorded as anticipated drift” sentence from M1.4 if the team does not want it in the acceptance checklist.

---

### Medium — new post-spawn/pre-register fault window has only Linux-only coverage

**References:**

- `scope.md` lines 2461–2479: new `post_spawn_pre_register` inject point and test assert Linux fd-count and `/proc` child absence.
- `scope.md` lines 2670–2672: `supervisor_spawn_unwinds_post_spawn_pre_register.rs` is listed as Linux-only.
- `scope.md` lines 3381–3388: macOS CI is a hard gate except for tests explicitly gated on inherent platform limits.

Round 19 correctly identifies the post-spawn/pre-register ownership state as distinct: a `Child` exists, but no broker registration or reaper task exists yet. The only named test for this new logic is Linux-only because it uses `/proc`/fd-count style assertions.

The Linux-only process/fd assertions are fine, but the entire behavior is not inherently Linux-only. The core cross-platform contract should still be testable on macOS at a higher level: the hook is consumed, spawn returns the injected `SpawnError::SandboxBuild`, no broker registration is acquired, `in_flight` is cleared or never remains set, and the directly-owned child cleanup path is exercised to the extent the platform exposes.

**Why this matters:** round 19 adds new production unwind logic, but macOS CI will not exercise it at all. Since m3 explicitly makes macOS CI a hard ratification gate for cross-platform spawn/session behavior, gating the whole new ownership-state regression test to Linux leaves a meaningful blind spot.

**Recommended fix:** keep the Linux-only deep leak/reap test, but add a platform-agnostic companion test for the behavioral contract that does not rely on `/proc` or Linux fd enumeration. If the team believes the whole path is Linux-specific, the scope should explicitly justify that; otherwise, macOS should cover the non-`/proc` aspects of the new inject point.
