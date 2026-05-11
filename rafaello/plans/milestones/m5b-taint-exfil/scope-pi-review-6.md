# m5b scope.md round-6 pi review

> Verdict: not converged — zero blockers, but commit-plan polish remains
>
> Counts: B/0 M/2 N/5

I reviewed round 6 (`scope.md` at commit `14769f6`) against pi round 5, the prior review trajectory, Stream A §7.2.1 / §7.2.2 / §7.2.6, m5a inheritance, and live source spot checks. The two pi-5 blockers are materially resolved: the publish-side failure tests now have a correctly placed broker hook, and the production audit-wiring test no longer relies on an impossible pre-handshake plugin publish.

I found no blockers. The remaining issues are commit-plan / cross-reference polish: §TM4 is a new broker test seam but is not represented in the internal split/budget, and its compile-fence `cfg` spelling is malformed. Fixing those should be a small round-7 editorial fold; the design itself looks stable.

## Round-5 verification table

| pi-5 finding | Round-6 disposition | Verification |
|---|---|---|
| B-1 stale-entry tests cited wrong re-emit fault injector | Resolved | New §TM4 scopes `Broker::install_publish_test_hook`, firing inside `publish_core_with_taint` after handler records and before `fan_out`; TR1/TR3 stale-entry tests now use that hook. |
| B-2 production audit-wiring test described impossible pre-handshake PT1 violation | Resolved | §PT1 splits the concerns into `rfl_chat_calls_set_audit_writer_before_first_plugin_spawn.rs` plus a separate post-spawn PT1 violation audit test. |
| M-1 `AuditKind` variants after consumers | Resolved | Internal split moves the enum/table extension to row `1''`, before rows 8/10/14 consumers. |
| M-2 lifecycle rejection ownership ambiguous | Resolved | §PT1 says the outer `emit_publish_rejected_for_plugin` mapper owns `core.lifecycle.publish_rejected`; inner PT1 owns audit + synthetic result only. |
| M-3 `ReemitRouter::Drop` cleanup stale | Resolved | §TM3 scopes cleanup in the spawned re-emit task shutdown branch, not `Drop`. |
| M-4 stale 27-commit ceiling for §A9 fallback | Resolved | §TR5 / §A9 point to 29-31 max, matching the recomputed budget. |
| M-5 hash traceability mismatch | Resolved | Scope does not cite round commit hashes. |
| N-1 env parser error described as stderr | Resolved | §TUI-MA1 says returned error string; CLI stderr is out of scope. |
| N-2 brittle count wording | Resolved | Status says “all pi-N findings” rather than repeating the count. |
| N-3 row 20 “synthetic user_denied result” ambiguity | Resolved | Row 20 mirrors the live persisted shape (`ok`, `call_id`, `content`, `details`). |
| N-4 long history banner | Resolved | Status is trimmed; detailed trajectory remains in review files plus a changelog pointer. |

## Blockers

None.

## Major

### M-1. §TM4 is missing from the internal split and budget

**Anchor:** §TM4 (`scope.md:870-924`), internal split (`scope.md:2862-2935`).

Round 6 adds a new broker-level test-only API (`Broker::install_publish_test_hook`) plus two named acceptance tests, but the internal split has no row for §TM4. This is not just bookkeeping: the hook is a new broker field/method and a publish-path branch, and the stale-entry tests in §TR1/§TR3 depend on it. If it lands inside row 5 or row 6, those commits become larger than advertised; if it lands earlier, the budget needs a row.

Smallest fix: add a dedicated §TM4 / broker publish-test-hook row before the TR1/TR3 rows (or explicitly fold it into row 5 with the size called out). Recompute the default/max commit totals if it adds a commit.

### M-2. TM4's compile-fence `cfg` expression is malformed

**Anchor:** §TM4 acceptance (`scope.md:918-924`).

The compile-fence bullet says ``cfg(not(test, not feature = "test-fixture"))``. That is not valid Rust `cfg` syntax and reverses the intended predicate. The intended production cfg is presumably:

```rust
#[cfg(not(any(test, feature = "test-fixture")))]
```

or the test should be described without inline cfg syntax. As written, an implementation agent copying the scope text will write a compile-fence that does not compile.

## Nits

### N-1. Stale owner-item references for substring threshold remain in §TM2 / §TM3

**Anchor:** §TM2 (`scope.md:833-836`), §TM3 (`scope.md:850-855`), owner items (`scope.md:3020-3097`).

Both references say substring threshold is owner-judgment item 2. Owner item 2 is EXFIL2; substring threshold is item 5. §A3 is correct; these local references need the same correction.

### N-2. Stale owner-item reference for unknown-id semantics remains in §TR4a

**Anchor:** §TR4a (`scope.md:1228-1234`), §A10 (`scope.md:2617-2628`), owner item 8 (`scope.md:3052-3057`).

§TR4a still says fail-open is surfaced in owner-judgment item 10. §A10 and the footer correctly say owner item 8.

### N-3. Forced-monolithic section still names row 13 for `AuditKind`

**Anchor:** forced-monolithic bullets (`scope.md:2919-2929`).

The table moved `AuditKind` extension to row `1''` and vacated row 13, but the forced-monolithic bullet still says “Row 13 (`AuditKind` table extension).” Update to row `1''`.

### N-4. Internal split text still says “round 5 in flight”

**Anchor:** sizing paragraph (`scope.md:2906-2912`).

Round 6 is now under review; replace “round 5 in flight” with “round 6 in review” or remove the parenthetical.

### N-5. TM4 hook overwrite/clear semantics are not stated

**Anchor:** §TM4 (`scope.md:889-909`).

Not implementation-blocking, but tests are easier to reason about if `install_publish_test_hook` says whether a second install replaces the first hook and whether passing `None` / a separate clear method is available. If no clear is needed because each test uses a fresh `Broker`, say that.

## Convergence call

Blocking count: **0**. Major count: **2**. Nit count: **5**.

No security/design blockers remain. I would do one short editorial round before declaring scope converged: add §TM4 to the internal split/budget, fix the malformed cfg, and sweep the stale row/owner references. After that, this should be ready for owner convergence and `commits.md` drafting.

Owner-judgment items remain the same stable set:

1. Canonical `tool_result` ancestry union.
2. `ReferencedTaintIndex` observed-but-expired fail-open policy.
3. `assistant_message` / `confirm_*` narrowing as known v1 limitation / v2 candidate.
4. File-backed fetch and EXFIL2 inclusion.
