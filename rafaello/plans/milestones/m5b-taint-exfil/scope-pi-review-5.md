# m5b scope.md round-5 pi review

> Verdict: blocking
>
> Counts: B/2 M/5 N/4

I reviewed round 5 (`scope.md` at commit `c68ade2`) against pi round 4, prior review trajectory, Stream A §7.2.1 / §7.2.2 / §7.2.6, m5a inheritance, and live source spot checks. Round 5 materially folds the pi-4 blockers/majors/nits. The remaining blockers are fresh acceptance-test mechanics: two stale-entry tests cite a live fault injector that runs before the handlers and therefore cannot observe post-record publish failure, and the production broker-audit wiring test describes a PT1 violation before plugin handshake, which cannot happen in the live process model.

## Round-4 verification table

| pi-4 finding | Round-5 disposition | Verification |
|---|---|---|
| B-1 request-id side of `ReferencedTaintIndex` records after fan-out | Resolved | §TR3 now captures `event.request_id`, records `record_request` before publish, and adds `reemit_tool_request_records_request_id_before_fan_out.rs`. |
| B-2 PT1 synthetic payload used unstored `<tool>` | Resolved | §PT1 synthetic payload drops `tool` and uses m5a deny-shaped `{ok:false,error,content:""}`. |
| B-3 EXFIL1 expected persisted `error` field | Resolved | §EXFIL1 entries golden now asserts live `ToolResultPayload` shape: `ok`, `call_id`, `content`, `details`, not `error`. |
| M-1 §TR4a docs said records after publish | Resolved | §TR4a comments now say `record_request` / `record_result` run before canonical publish using the forwarded id. |
| M-2 internal split row 9 stale after-publish wording | Resolved | Row 9 now says `by_result_id` records before publish/fan-out. |
| M-3 §A2 stale `with_audit_writer` builder | Resolved | §A2 now describes `BrokerInner.audit: Mutex<Option<_>>` + `Broker::set_audit_writer(&self, ...)`. |
| M-4 EXFIL1 turn-2 `in_reply_to` not pinned | Resolved | §EXFIL1 turn 2 explicitly uses `in_reply_to = [<fetch-result-id>]`; EXFIL3 remains the empty-in_reply_to negative. |
| M-5 commit budget arithmetic | Resolved | Internal split recomputes 27 default / 29-31 max. |
| M-6 owner-item cross-refs stale | Resolved | §A3 → item 5; §A10 → item 8; banner/footer map convergence items to 1/2/3/8. |
| N-1 stale-entry rationale over-broad | Resolved | §TR1 distinguishes `by_result_id` stale-entry unreachability from match-map provenance overreach. |
| N-2 UTF-8 byte-internal hit ambiguity | Resolved | §TM1 now pins `str::contains`, character-boundary preserving. |
| N-3 TUI-MA exact mutual-exclusion error | Resolved | §TUI-MA1 gives the exact error string and snapshot test. |
| N-4 long history banner | Accepted for now | Status says the history stays through round 5 and trims after a future low-noise round. This is not implementation-blocking. |

## Blockers

### B-1. Publish-failure stale-entry tests cite the wrong fault-injection seam

**Anchor:** §TR1 acceptance (`scope.md:1394-1399`), §TR3 acceptance (`scope.md:1475-1479`); live `reemit/mod.rs:179-219`.

**Issue:** Both stale-entry tests say to “inject a publish failure via the existing test fault injector.” The live `ReemitRouter` fault injector runs before `dispatch_event` calls the per-direction handler; if it returns an error, `dispatch_event` emits `reemit_rejected` and returns before `handle_tool_result` / `handle_tool_request` records anything. It cannot simulate `publish_core_with_taint` failing after the new pre-publish cache records.

That makes both named acceptance tests untestable as written, and those tests are load-bearing because they justify the new “record before publish, tolerate TTL-bounded stale entries” policy.

**Smallest acceptable fix:** Scope a publish-side fault seam explicitly (e.g. a test-only `Broker` publish hook / wrapper that fails inside `publish_core_with_taint` after handler records but before fan-out), or change the tests to call the handler-level helper directly with a mock publish function. Do not call it the existing re-emit fault injector unless that injector is moved to the publish boundary.

### B-2. `rfl_chat_wires_broker_audit_writer_before_plugin_spawn.rs` describes an impossible pre-handshake PT1 violation

**Anchor:** §PT1 acceptance (`scope.md:1881-1889`).

**Issue:** The production wiring test says to spawn `rfl chat`, “inject a fault that would trigger §PT1 before any plugin completes its handshake,” and assert the violation audits. But §PT1 is `handle_plugin_publish` on `plugin.<id>.tool_result`; a plugin cannot publish until it has spawned and registered/handshaken enough to be in the broker registry. Triggering a plugin `tool_result` before any plugin completes handshake is not a reachable live state.

The production invariant is important, but this acceptance recipe is mechanically impossible.

**Smallest acceptable fix:** Test the ordering with a startup instrumentation hook instead: assert `broker.set_audit_writer` is called before the first `PluginSupervisor::spawn` / broker registration attempt. Then keep a separate end-to-end PT1 violation test after plugin spawn to prove the audit row is written. Alternatively run a normal violating plugin publish after spawn and assert the audit row, but drop the “before any plugin completes handshake” claim.

## Major

### M-1. Internal split orders `AuditKind` variants after their consumers

**Anchor:** internal split rows 8, 10, 13, 14 (`scope.md:3151-3174`), §AL4 (`scope.md:2232-2246`).

Rows 8, 10, and 14 consume the new audit kinds (`tool_request_taint_unioned_from_in_reply_to`, `plugin_publish_rejected_taint_superset`, `confirm_request_taint_attached`) before row 13 adds the `AuditKind` variants/table entries. That is not per-commit green unless each earlier row avoids compiling its audit write, which would undercut the named acceptance.

Smallest fix: move the `AuditKind` table-extension row before the first consumer (or split “type/table addition” into an early row and writer tests into later rows). Keep the m4/m5a precedent of one table-extension commit, but put it before §TR4b / §PT1 / §AL1 consumers.

### M-2. PT1 lifecycle rejection ownership is still ambiguous relative to the live wrapper

**Anchor:** §PT1 violation path (`scope.md:1826-1858`); live `bus.rs:433-439` wraps `handle_plugin_publish_inner` and calls `emit_publish_rejected_for_plugin` on any error.

§PT1 says the broker publishes `core.lifecycle.publish_rejected` on violation, but it does not specify whether that happens inside `handle_plugin_publish_inner` before returning `BrokerError::TaintSupersetViolated`, or by extending the existing outer `emit_publish_rejected_for_plugin` mapper. The distinction matters: doing both creates duplicate lifecycle events; doing neither for the new variant loses the required code `taint_superset_violated`.

Smallest fix: pin one path. Preferred: extend the existing outer rejection mapper for `TaintSupersetViolated` and keep synthetic-result/audit side effects in the inner violation path, or explicitly say the inner path publishes and the outer mapper must ignore this variant.

### M-3. `ReemitRouter::Drop` cleanup remains stale against `start(self)` ownership

**Anchor:** §TM1 API prose (`scope.md:924-929`, `scope.md:1142-1144`); live `ReemitRouter::start(self)` consumes the router and moves fields into the spawned task.

The scope still says `TaintMatchMap::clear()` runs in `ReemitRouter::Drop`. Live `start(self)` consumes `ReemitRouter`; adding a meaningful `Drop` impl on the struct is awkward because fields are moved into the task, and a Drop impl would run at `start` time rather than session shutdown unless the ownership model changes. The map/cache are per-process and can simply drop with the task, or be cleared in the shutdown branch of the spawned task.

Smallest fix: replace “called from `ReemitRouter::Drop`” with a shutdown-task/RAII guard shape that matches `start(self)`, or drop the explicit cleanup hook and rely on Arc/task drop at `rfl chat` shutdown.

### M-4. TR5 / A9 still cite stale “high end (27 commits)” for the fallback

**Anchor:** §TR5 (`scope.md:1714-1717`), §A9 (`scope.md:2880-2882`), internal split sizing (`scope.md:3132-3140`).

The internal split now says 27 default and 29-31 max. §TR5 and §A9 still say the §A9 fallback is absorbed at the “high end (27 commits).” Update those references to the new max range or avoid a number.

### M-5. Status says the exact pi-4 commit was `fed0a28`, but the worktree shows `c68ade2`

**Anchor:** user handoff vs `git log`; scope status does not name the hash.

This is not a scope.md content blocker, but it matters for review traceability: the user prompt says round 5 landed at `fed0a28`, while `git log` in this worktree shows `c68ade2 docs(rafaello-m5b): scope.md round 5`. If the driver notebook uses the other hash, reconcile before convergence to avoid citing the wrong round in later review/retro docs.

## Nits

### N-1. TUI-MA says the env parser test snapshots a “stderr line” for a returned error

**Anchor:** status N-3 (`scope.md:132-139`), §TUI-MA1 acceptance (`scope.md:2128-2134`).

`rafaello_tui::env::load_from` returns an error; stderr formatting belongs to the binary wrapper. Say the unit test snapshots the error string, not a stderr line, unless a CLI-level stderr test is actually intended.

### N-2. Status still says pi-4 verified “all 12 pi-3 findings,” but pi-3 had 12 only because counts were 3/5/4

**Anchor:** status banner (`scope.md:2-5`).

This is mathematically fine, but brittle in prose. Prefer “all pi-3 findings” to avoid future count churn in the banner.

### N-3. Internal split row 20 still says “synthetic user_denied result” without the live persisted shape

**Anchor:** row 20 (`scope.md:3169-3170`).

The row says entries golden includes “the synthetic user_denied result,” which can be read as asserting `error = user_denied` again. Mirror §EXFIL1's precise persisted shape: `ok=false`, `call_id=<turn-2>`, `content=""`, `details=None`; raw `error` is not in the entries row.

### N-4. Long status history is now over 500 lines before the goal

**Anchor:** status/history preamble (`scope.md:1-526`).

Round 5 intentionally keeps history, and this is not blocking. But before ratification, trim aggressively or move detailed trajectory to a changelog appendix; reviewers should not have to page through five rounds before the goal.

## Convergence call

Blocking count: **2**. Major count: **5**. Nit count: **4**.

Round 5 is close, but still not ready for `commits.md` because two named acceptance tests are mechanically untestable as written. Fix the publish-side fault-injection seam and the broker-audit production wiring test, then do one more cross-reference pass over the internal split / audit-kind ordering. The design choices themselves look stable.

Owner-judgment items still worth surfacing once blockers clear:

1. Canonical `tool_result` ancestry union.
2. `ReferencedTaintIndex` observed-but-expired fail-open policy.
3. `assistant_message` / `confirm_*` narrowing as known v1 limitation / v2 candidate.
4. File-backed fetch and EXFIL2 inclusion.
