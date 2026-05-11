# m5b scope.md round-4 pi review

> Verdict: blocking
>
> Counts: B/3 M/6 N/4

I reviewed round 4 (`scope.md` at commit `d01b60e`) against pi round 3, the prior round reviews, Stream A §7.2.1 / §7.2.2 / §7.2.6, m5a inheritance, and live source spot checks. Round 4 materially folds the exact pi-3 findings: no pi-3 item is carried verbatim. The fresh blockers are all consistency/mechanics issues introduced or exposed by the fold: the request-id side of `ReferencedTaintIndex` has the same post-publish race that round 4 fixed for result ids, PT1's synthetic-result payload names a tool value the broker does not store, and EXFIL1 still expects the persisted `entries` row to carry an `error` field the live agent loop drops.

## Round-3 verification table

| pi-3 finding | Round-4 disposition | Verification |
|---|---|---|
| B-1 `record_result` after fan-out | Resolved for result ids | §TR1 now captures `event.request_id`, records both the match-map and result-id index before `publish_core_with_taint`, and adds `reemit_tool_result_records_both_indexes_before_fan_out.rs`. Fresh analogous request-id race below. |
| B-2 broker audit writer plain `Option` builder | Resolved | §PT1 uses `BrokerInner.audit: Mutex<Option<Arc<AuditWriter>>>` + `Broker::set_audit_writer(&self, ...)`; clone visibility and production wiring tests are named. |
| B-3 JSON-encoded substring matching | Resolved | §TM1 splits literal-hash canonical bytes from raw-string substring normalization, with quote/backslash/non-ASCII/string-only tests. |
| M-1 goal item named withdrawn audit kind | Resolved | Goal item 7 names the three real audit kinds and explicitly withdraws `tool_request_rejected_taint_superset`. |
| M-2 state lock release before publish | Resolved | §PT1 step 6 explicitly drops `state` before audit/lifecycle/synthetic publishes and names a deadlock regression test. |
| M-3 internal split row 20 stale | Resolved | Row 20 includes audit golden, entries golden, and plugin-log expectations. |
| M-4 production broker audit wiring untested | Resolved | §PT1 adds `rfl_chat_wires_broker_audit_writer_before_plugin_spawn.rs`. |
| M-5 provider-extracted grants “m6 / v2” wording | Resolved | Out-of-scope item 4 says v2 / known v1 limitation. |
| N-1 footer stale | Resolved | Footer says round 4 / folds pi-3. |
| N-2 `parse_test_env` invented | Resolved | §TUI-MA1 refers to `rafaello-tui::env::load_from` and `parse_confirm_answers`. |
| N-3 TR1 ordering test too narrow | Resolved | Test renamed to cover both indexes before fan-out. |
| N-4 self-review wording in status history | Resolved | History says pi-2 found the gaps. |

## Blockers

### B-1. The request-id side of `ReferencedTaintIndex` still records after fan-out

**Anchor:** §TR3 (`scope.md:1244-1271`), §TR4a (`scope.md:1273-1368`), §TR1 lookup step (`scope.md:1128-1137`).

**Issue:** Round 4 fixed the result-id race in §TR1, but §TR3 still says `handle_tool_request` calls `publish_core_with_taint` and only afterward records the request id via `referenced_taint_index.record_request(...)`. That is the same ordering bug on the other arm of the same cache. The gate is an internal subscriber of `core.session.tool_request`; fan-out queues the event before `record_request` runs. On a multi-threaded runtime the gate can process the request, dispatch to the plugin, and the plugin can return `plugin.<id>.tool_result` quickly enough for §TR1's `lookup_request` to miss.

The request id is already known before publish (`event.request_id`, forwarded to the canonical envelope), just like the result id in the pi-3 B-1 fix.

**Smallest acceptable fix:** Mirror §TR1's round-4 ordering: capture the provider tool_request `event.request_id`, compute canonical taint, call `record_request` before `publish_core_with_taint`, and accept/remove a TTL-bounded stale request entry on publish failure. Add an ordering test proving `record_request` completes before fan-out to the gate.

### B-2. PT1's synthetic `tool_result` payload contains `<tool>`, but `OutstandingDispatch` does not store a tool name

**Anchor:** §PT1 data model (`scope.md:1519-1530`), §PT1 synthetic path (`scope.md:1385-1402`).

**Issue:** The synthetic violation result payload is specified as `{"tool": <tool>, "ok": false, "error": "plugin_taint_superset_violation"}`. At the violation point the broker has only `OutstandingDispatch { request_id, dispatched_at, tool_request_taint }`, the result id, and the plugin's rejected `tool_result` publish. The originating dispatch payload/tool name is not stored in `OutstandingDispatch`, and m5a plugin `tool_result` payloads are not guaranteed to echo the tool name.

As written, implementers must invent where `<tool>` comes from or emit a synthetic payload that does not match the scope.

**Smallest acceptable fix:** Either remove `tool` from the synthetic payload and use the m5a deny-shaped `{ok:false,error,content:""}` style, or extend `OutstandingDispatch` with the originating tool name/payload and populate it in `publish_for_tool_dispatch`. Add a test for the chosen payload shape through the agent-loop persistence path.

### B-3. EXFIL1 expects `error: "user_denied"` in the persisted `entries` table, but the live agent loop drops `error`

**Anchor:** §EXFIL1 entries assertion (`scope.md:2176-2184`), live `agent/mod.rs::handle_tool_result` reads only `ok`, `content`, and `in_reply_to[0]`.

**Issue:** §EXFIL1 says the `entries` table golden asserts the turn-2 `tool_result` carries `{ok:false,error:"user_denied"}`. Live m5a's agent loop persists `ToolResultPayload { call_id, ok, content, details: None }`; it reads `error` from neither the payload nor the envelope. The existing m5a synthetic-deny persistence test asserts `ok:false` + `call_id`, not `error`.

The headline demo's acceptance is therefore untestable unless m5b scopes a ToolResultPayload schema change.

**Smallest acceptable fix:** For m5b, keep the live persistence shape: entries-table golden asserts `kind = tool_result`, `ok = false`, `call_id = <turn-2 request id>`, and empty/default content. Assert the raw `error:"user_denied"` only on the core event or audit row if such an observer is in scope; otherwise drop the persisted-error expectation.

## Major

### M-1. §TR4a API docs still say both cache arms record after canonical publish

**Anchor:** §TR4a API comments (`scope.md:1304-1318`) vs §TR1 fixed ordering (`scope.md:1138-1168`) and requested §TR3 fix above.

The `record_request` / `record_result` doc comments still say “Called after ... publishes.” For result ids this contradicts the round-4 fix; for request ids it is the blocker above. Once B-1 is fixed, update both comments to “before canonical publish, using the id that will be forwarded on the canonical envelope.”

### M-2. Internal split row 9 still says result id is recorded after publish

**Anchor:** internal split row 9 (`scope.md:2582-2584`).

Row 9 says `handle_tool_result` “records result id in `by_result_id` after publish,” contradicting §TR1's round-4 pre-publish ordering. This will mislead `commits.md` drafting. Update row 9 to “before publish / before fan-out.”

### M-3. A2 still describes the withdrawn `Broker::with_audit_writer` builder

**Anchor:** §A2 (`scope.md:2533-2545`) vs §PT1 (`scope.md:1539-1566`) and internal split row 1' (`scope.md:2869-2871`).

The architectural choice section still says broker audit plumbing uses `Broker::with_audit_writer(Arc<AuditWriter>)`. Round 4 changed the operative design to `Broker::set_audit_writer(&self, Arc<AuditWriter>)` plus `Mutex<Option<_>>`. Update §A2 so the owner-ratified choice matches the implementation section.

### M-4. EXFIL1 does not explicitly pin turn 2's `in_reply_to` shape

**Anchor:** §EXFIL1 turn descriptions/assertions (`scope.md:2102-2197`), §EXFIL3 explicit `in_reply_to = []` (`scope.md:2260-2298`).

EXFIL3 carefully says turn 2 scripts `in_reply_to = []` to keep provider-only taint. EXFIL1 never explicitly says turn 2 cites the fetch result id, even though later prose says the referenced-union arm is redundant and no `tool_request_taint_unioned_from_in_reply_to` row is expected. Value matching makes the demo pass either way, but the audit-row negative depends on whether the referenced-union arm was present and redundant or absent. Pin EXFIL1 turn 2 as `in_reply_to = [<fetch-result-id>]` if the redundant-union assertion is intended.

### M-5. Commit budget arithmetic is inconsistent after adding row 1'

**Anchor:** internal split table and sizing (`scope.md:2859-2925`).

The default table has rows 1..25 plus row 1', and row 3 is estimated at 2 commits. That sums to 27 default commits, not “~23-26.” The section also still says “Targets 22-27” but “28 max if owner takes §A9.” Recompute the default and max ranges so the owner and commits.md reviewer have an honest budget.

### M-6. Owner-item cross-references are stale after renumbering

**Anchor:** §A3 (`scope.md:2547-2551`), §A10 (`scope.md:2615-2625`), status banner (`scope.md:7-10`), owner list (`scope.md:3010-3095`).

§A3 says substring threshold is owner item 2, but owner item 2 is EXFIL2 and threshold is item 5. §A10 says unknown-id semantics are owner item 10, but owner item 10 is the location split and unknown-id semantics are item 8. The status banner's “1, 2, 8, 3 / 6 mapping” is also hard to reconcile with the footer. Update these before convergence so owner pings point at the right rows.

## Nits

### N-1. §TR1 stale-entry rationale overstates “no downstream consumer can build on it”

**Anchor:** §TR1 publish-failure paragraph (`scope.md:1178-1187`).

For the result-id arm, provider intake's observed-results set should prevent fabricated use of an unpublished result id, so the stale entry is harmless. For the match-map arm, a later value match could still inherit a TTL-bounded taint from a failed publish if the same bytes appear. The final conclusion is probably acceptable, but the rationale should distinguish the two arms rather than saying no downstream consumer can build on the failed canonical event.

### N-2. `taint_match_substring_handles_non_ascii_utf8` allows byte-internal hits in a UTF-8 string

**Anchor:** §TM1 non-ASCII acceptance (`scope.md:1048-1057`).

The test description says “preserves character boundaries” and then says byte-internal hits are acceptable by default. Pick one. Since the substring inputs are Rust `&str`, normal `str::contains` naturally preserves UTF-8 boundaries; prefer that over byte-internal matching unless there is a concrete reason to scan arbitrary byte windows.

### N-3. TUI-MA mutual-exclusion error names `parse_confirm_answers` but not the exact error text

**Anchor:** §TUI-MA1 (`scope.md:1854-1898`).

This is not blocking, but prior milestones benefited from exact stderr/error strings for env parser negatives. Add the exact message for “both singular and plural set” if the TUI env tests are going to snapshot errors.

### N-4. Header preserves long round-2/round-3 history that may be better left to review files

**Anchor:** status/history banner (`scope.md:121-236`).

The history is useful during review but now consumes over 200 lines before the goal. Once blockers are gone, consider trimming to the current round's fix list plus links to review files, leaving detailed trajectory in `scope-pi-review-N.md`.

## Convergence call

Blocking count: **3**. Major count: **6**. Nit count: **4**.

Round 4 is close, but not ready for `commits.md`. Fix the request-id cache ordering to mirror the result-id fix, make the PT1 synthetic payload source mechanically available (or remove `tool`), and align EXFIL1 with the live persisted `ToolResultPayload` shape. The remaining majors are consistency/sizing/cross-reference cleanup.

Owner-judgment items still worth surfacing once blockers clear:

1. Canonical `tool_result` ancestry union remains coherent after the cache-ordering fixes.
2. `ReferencedTaintIndex` observed-but-expired fail-open policy.
3. `assistant_message` / `confirm_*` narrowing as known v1 limitation / v2 candidate.
4. File-backed fetch and EXFIL2 inclusion.
