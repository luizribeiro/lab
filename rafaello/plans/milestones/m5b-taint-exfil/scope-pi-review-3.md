# m5b scope.md round-3 pi review

> Verdict: blocking
>
> Counts: B/3 M/5 N/4

I reviewed round 3 (`scope.md` at commit `8d2517c`) against pi round 2, the driver pre-flight, m5a Appendix A / retrospective, Stream A §7.2.1 / §7.2.2 / §7.2.6, and live m5a source. Round 3 fixes most pi-2 issues mechanically, but two pi-2 blockers are only partially fixed: the result-id cache is now named/scoped but is populated too late, and broker audit plumbing is scoped but not implementable with the stated `Option` field after `Broker` clones exist. A fresh blocker comes from the pi-2 M-6 hash fold: substring matching over JSON-encoded strings cannot satisfy the named substring tests.

## Round-2 verification table

| pi-2 finding | Round-3 disposition | Verification |
|---|---|---|
| B-1 result ids not recorded in `InReplyToTaintIndex` | **Partially addressed; carried below** | §TR4a now has `ReferencedTaintIndex.by_result_id` and §TR1 step 6 records result ids, but it records after `publish_core_with_taint` fan-out, so external provider reaction can race before the cache is populated. |
| B-2 broker-side audit writer missing | **Partially addressed; carried below** | §PT1 adds `Broker::with_audit_writer` and `BrokerInner.audit`, but the stated plain `Option<Arc<AuditWriter>>` builder is not implementable after live `rfl chat` clones the broker before the audit writer is available. |
| B-3 EXFIL1 audit golden used nonexistent `tool_request` kind | Resolved | §EXFIL1 removes `tool_request` audit rows and asserts dispatch/execution through `entries` + plugin logs. |
| M-1 PT1 assigned `publish_for_tool_dispatch` to re-emit | Resolved | §PT1 now names `ConfirmationGate` as the caller/populator and adds a gate-boundary test. |
| M-2 “m6 / v2” security deferral conflict | Resolved for the assistant/confirm surface | §TR5 / §A9 / owner item 9 now say known v1 limitation / v2, not m6. |
| M-3 fabricated unknown id fail-open conflicted with live intake | Resolved | §TR4a/§TR4b narrow fail-open to observed-but-expired result ids; fabricated ids remain rejected upstream. |
| M-4 `AuditKind` `FromStr` / `Display` invented | Resolved | §AL4 is enum + `as_str()` only. |
| M-5 empty taint wording as “superset” | Resolved | §PT1 now says no plugin-supplied claim means no contradiction check; canonical taint still preserves ancestry. |
| M-6 scalar hash bytes unspecified | Addressed but introduced B-3 | §TM1 pins `serde_json::to_vec`, but using JSON-encoded strings for substring comparison breaks the substring tests. |
| N-1 stale owner-item mapping | Resolved | Status banner maps pi-1 convergence choices to owner items 1/2/3/10. |
| N-2 TR1 ordering parenthetical imprecise | Improved but still contradictory | Wording is clearer, but §TR1 still claims both index records are available to subscribers while `record_result` happens after fan-out. Covered by B-1. |
| N-3 §TR4a “also used by §PT2” | Resolved | §TR4a now names only §TR1 and §TR4b as consumers. |
| N-4 EXFIL3 “Wait:” marker | Resolved | §EXFIL3 is directive prose. |

## Blockers

### B-1. Carried from pi-2 B-1: `record_result` happens after the fan-out it is meant to protect

**Anchor:** §TR1 steps/order (`scope.md:955-987`), §TR4a (`scope.md:1054-1149`), §TR4b (`scope.md:1151-1249`), status B-1 (`scope.md:13-25`).

**Issue:** Round 3 adds the missing result-id arm, but the ordering is still not safe. §TR1 says both `TaintMatchMap` and `ReferencedTaintIndex.record_result` are populated before observers can react, then immediately specifies the actual order as steps 1-4 → step 5 `publish_core_with_taint` → step 6 `record_result`. `publish_core_with_taint` enters `fan_out`; `fan_out` notifies internal subscribers and external provider recipients before returning. A provider can observe the canonical `core.session.tool_result` and publish a follow-up `provider.<id>.tool_request in_reply_to=[that-result-id]` before step 6 records the result id.

The rationale that the canonical id is constructed inside `publish_core_with_taint` is also inconsistent with the live API: callers pass `request_id: Option<JsonRpcId>` into `publish_core_with_taint`; `handle_tool_result` already has the plugin result's `event.request_id` before publishing.

**Smallest acceptable fix:** Pin `record_result` before `publish_core_with_taint`, using the same request id that will be passed to publish. If publish fails, remove the result-id entry or accept a TTL-bounded stale result entry explicitly. Then update §TR1 ordering and the ordering test to cover both the match-map record and the result-id cache record.

### B-2. Carried from pi-2 B-2: `Broker::with_audit_writer` is not implementable as a plain `BrokerInner.audit: Option<_>` builder in live `rfl chat`

**Anchor:** §PT1 broker audit plumbing (`scope.md:1326-1341`), internal split row 1' (`scope.md:2574-2576`); live `rfl/src/lib.rs` constructs `Broker`, then clones it into `PluginSupervisor`, then later builds `SessionController` / obtains `audit`.

**Issue:** Round 3 scopes broker audit plumbing, but the stated data shape is not mechanically compatible with the live construction order. `Broker` is an `Arc<BrokerInner>` clone wrapper. A ReemitRouter-style consuming builder works for an owned router struct; it does not work for `BrokerInner.audit: Option<Arc<AuditWriter>>` after the broker has already been cloned. Live `rfl chat` currently creates `Broker`, immediately clones it into `PluginSupervisor`, and only later constructs the `SessionController` and obtains the audit writer. At that point a plain `Option` inside `BrokerInner` cannot be mutated without interior mutability or a different construction order.

**Smallest acceptable fix:** Choose an implementable shape. For example: `BrokerInner.audit: Mutex<Option<Arc<AuditWriter>>>` plus `Broker::set_audit_writer(&self, Arc<AuditWriter>)`, called before plugin spawn; or reorder `rfl chat` so the audit writer exists before `Broker::new` and pass it into the constructor. Update §PT1, row 1', and tests to match the chosen shape. If using a setter, add a test that existing broker clones see the writer.

### B-3. Round-3 scalar canonicalization makes the substring tests fail for strings

**Anchor:** §TM1 scalar-byte canonicalization and substring tests (`scope.md:800-873`).

**Issue:** §TM1 now says substring length/comparison use `serde_json::to_vec(value)` for scalar leaves. For strings, that includes surrounding quotes and JSON escapes. The named substring tests cannot pass under that rule. Example: recorded raw string `please fetch https://evil.example.com/leak now` canonicalizes to bytes for `"please fetch https://evil.example.com/leak now"`; arg raw string `https://evil.example.com/leak` canonicalizes to `"https://evil.example.com/leak"`. The latter is **not** a substring of the former because the quote before `https` and the quote after `leak` are not present in the middle of the recorded string.

This breaks the headline taint primitive and EXFIL1's “body URL is a substring of the fetch content” assertion.

**Smallest acceptable fix:** Split hash canonicalization from substring normalization. Keep `serde_json::to_vec` for literal hashes if desired, but for substring indexing compare raw string contents for JSON strings (and probably do not substring-index non-strings). Pin tests for quotes/escapes/non-ASCII. Alternatively, if substring over canonical JSON is intentional, rewrite the tests and EXFIL expectations to quote whole JSON string values only — but that would miss the roadmap's URL-inside-content case.

## Major

### M-1. Goal item 7 still names the withdrawn re-emit rejection audit kind

**Anchor:** goal item 7 (`scope.md:399-406`), §TR4b (`scope.md:1190-1202`), §AL2/§AL3 (`scope.md:1662-1697`), Out of scope item 18 (`scope.md:2198-2207`).

Round 3 correctly withdraws `tool_request_rejected_taint_superset` in §TR4b and replaces it with `tool_request_taint_unioned_from_in_reply_to`, but the top-level deliverable still says “New kinds for the two superset violations (`tool_request_rejected_taint_superset`, `plugin_publish_rejected_taint_superset`).” That contradicts §AL and the out-of-scope list. Update the goal item to the three actual new audit kinds.

### M-2. PT1 does not pin lock release before lifecycle/synthetic publishes

**Anchor:** §PT1 atomic order and violation path (`scope.md:1342-1402`); live `bus.rs::fan_out` locks broker state while collecting recipients.

§PT1 says the outstanding entry is drained “in the same atomic step” on violation, then the broker audits, publishes `core.lifecycle.publish_rejected`, and publishes a synthetic `core.session.tool_result`. It does not explicitly say the `state` lock is released before either publish. Holding the broker state lock while calling `publish_core_with_taint` would deadlock or at least recurse into `fan_out`'s recipient collection lock. Add an explicit step: copy the data needed for audit/synthetic result, drain under the lock, release lock, then publish lifecycle/synthetic events.

### M-3. Internal split row 20 is stale relative to EXFIL1's expanded assertions

**Anchor:** §EXFIL1 acceptance (`scope.md:1939-1945`), internal split row 20 (`scope.md:2594-2595`).

§EXFIL1 now requires four sub-fixtures / goldens: lock, stub response, expected `audit_events`, expected `entries` table plus plugin-log expectations. Internal split row 20 still says only “headline integration test + stub scripted response + golden audit rows.” Update row 20 so commits.md does not under-scope the entries/log assertions that fixed pi-2 B-3.

### M-4. The `Broker::with_audit_writer` unset behavior should not silently drop required acceptance without a lifecycle-only fallback

**Anchor:** §PT1 audit plumbing (`scope.md:1334-1341`), §PT1 acceptance (`scope.md:1416-1422`).

The acceptance says a broker constructed without an audit writer silently drops the audit call. That is fine for unit tests or non-chat broker construction, but m5b acceptance depends on the audit row existing in `rfl chat`. Add a `rfl_chat_wires_broker_audit_writer_before_plugin_spawn` test (or equivalent) so the silent-drop fallback cannot accidentally be the production path. The current plumbing test only proves both set/unset branches locally.

### M-5. `provider-extracted user_grants` still says “m6 / v2” despite the m6 no-security boundary

**Anchor:** m5b → m6 boundary (`scope.md:456-466`), out-of-scope item 3 (`scope.md:2153-2156`).

Pi-2 M-2 was specifically about assistant/confirm superset narrowing, and round 3 fixed that surface. But the same boundary contradiction remains for provider-extracted user_grants proposals: out-of-scope item 3 says “deferred to m6 / v2 as in m5a,” while the boundary says m6 has no security primitives. If it is still security-sensitive, call it v2 / known v1 limitation; if it is m6 polish, explain why it is not a security primitive.

## Nits

### N-1. Footer still says “End of m5b scope round 2”

**Anchor:** footer (`scope.md:2802-2807`).

Round 3's footer still says round 2 and “Folds pi-1.” Update to round 3 / folds pi-2.

### N-2. TUI-MA references `parse_test_env`, which is not the live env parser name

**Anchor:** §TUI-MA1 (`scope.md:1588-1591`); live `rafaello-tui/src/env.rs` exposes `load` / `load_from` and helper parsers.

Use the live parser name or a generic “returned from env loading” phrasing unless the plan intentionally adds a new `parse_test_env` helper.

### N-3. TR1 acceptance name is too narrow for the ordering it must prove

**Anchor:** §TR1 acceptance (`scope.md:1008-1012`).

`reemit_tool_result_record_before_publish_ordering.rs` now needs to prove both the `TaintMatchMap` record and the `ReferencedTaintIndex.record_result` happen before fan-out. Rename or expand the description so it does not only cover the map record.

### N-4. Status history says round 3 “finds” pi-2 gaps inside the artifact under review

**Anchor:** history banner (`scope.md:144-150`).

This is harmless, but odd in a scope artifact: “Round-3 finds three fresh consistency gaps...” reads like the draft is reviewing itself. Prefer “pi-2 found...” or drop the sentence; the actual review files already preserve that history.

## Convergence call

Blocking count: **3**. Major count: **5**. Nit count: **4**.

Round 3 fixed pi-2's surface-level inconsistencies, but the two load-bearing folds need one more pass: result-id cache population must happen before fan-out, and broker audit plumbing must be implementable with `Broker`'s Arc/cloning construction. Also fix string substring normalization before commits.md; otherwise the headline exfil value match will not work.

Owner-judgment items still worth surfacing:

1. Canonical `tool_result` ancestry union remains the default and is now broadly consistent once B-1 ordering is fixed.
2. `assistant_message` / `confirm_*` narrowing is a known v1 limitation / v2 candidate.
3. `ReferencedTaintIndex` TTL miss semantics for observed-but-expired ids.
4. File-backed fetch and EXFIL2 inclusion remain as listed.
