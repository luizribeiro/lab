# m5b scope.md round-2 pi review

> Verdict: blocking
>
> Counts: B/3 M/6 N/4

I reviewed round 2 (`scope.md` at commit `220374a`) against pi round 1, the driver pre-flight, m5a Appendix A / retro §5/§9/§10, Stream A §7.2.1 / §7.2.2 / §7.2.6, overview/decisions/glossary, and live m5a source under `rafaello/crates`.

Round 2 mechanically addressed most pi-1 findings. I do **not** see a pi-1 finding that needs to be carried verbatim as the same defect: B-1/B-2/B-3/B-5/B-6 and M-2/M-3/M-4/M-5/M-6/M-7/M-8/N-1..N-5 are materially folded; M-1 is now explicitly narrowed as an owner-judgment item. However, the fold introduced three new blocking inconsistencies: the `InReplyToTaintIndex` is not populated for `tool_result` ids even though §TR4b looks up result ids; broker-side audit rows are assigned to a component that has no audit writer plumbing; and the headline EXFIL audit golden names a `tool_request` audit kind that is neither live nor scoped.

## Round-1 verification table

| pi-1 finding | Round-2 disposition | Verification |
|---|---|---|
| B-1 exfil flow two incompatible targets | Resolved | Goal item 10 and §EXFIL1 now use one flow: first `web-fetch` allowed, second `send-mail` denied; Demo bar matches. |
| B-2 per-turn TUI scripting missing | Resolved | §TUI-MA defines `RFL_TUI_TEST_CONFIRM_ANSWERS`, mutual exclusion, exhaustion, rfl env allowlist, and tests. |
| B-3 `_taint_attached` predicate contradicted provider-only taint | Resolved | Goal item 6, §CD2, §AL1, §EXFIL3 use the precise `source != "provider"` predicate. |
| B-4 `OutstandingDispatch` lacked taint / drain order | Mechanically resolved, with fresh fallout below | §PT1 extends `OutstandingDispatch` with `tool_request_taint` and pins inspect → check → drain. Fresh issues: audit plumbing and caller attribution. |
| B-5 canonical `tool_result` stripped ancestry | Resolved by default-selected owner choice | Goal item 4, §TR1, §PT2, §A8 choose `tool-source ∪ referenced-tool_request-taint`. |
| B-6 TR4 rejection/synthetic result unpinned | Resolved by policy change | §TR4b now constructs the superset and withdraws the re-emit rejection. Synthetic-deny remains only in §PT1. |
| M-1 every provider/frontend re-emit surface dropped | Addressed as deliberate narrowing | §TR5 + §A9 + owner item 9 explicitly defer/narrow assistant/confirm/rpc rows, though see M-2 below for the m6/v2 wording conflict. |
| M-2 `SessionId` not live in re-emit path | Resolved | §TM1/§TM3 remove `SessionId`; map is per-router with `clear()`. |
| M-3 TR1 record/publish ordering contradiction | Resolved | §TR1 pins record-before-publish and the stale-record trade-off. |
| M-4 CD1 stale / null-vs-omit deferred | Resolved | §CD1 preserves live `[]`, not `null`, and scopes regression tests. |
| M-5 fetch env path not locked | Resolved | §TF3 adds `env.pass = ["RFL_FETCH_TEST_BODY_PATH"]` and an env-reaches-plugin test. |
| M-6 manual validation real-network claim | Resolved | §TF2 and Manual validation are file-backed only. |
| M-7 substring test / hash unspecified | Mostly resolved | §TM1 adds directional substring tests and fixed SipHasher key. See M-6 for remaining scalar-byte canonicalization precision. |
| M-8 size risk understated | Resolved | §Internal split rebudgets to 22-27 commits and separates TR4/TUI/data-model work. |
| N-1 wrong lifecycle topic | Resolved | §PT1 uses live `core.lifecycle.publish_rejected` with code `taint_superset_violated`. |
| N-2 fetch owner-choice cross-ref | Resolved | §A6 / owner item 3. |
| N-3 TTL cross-ref | Resolved | §TM3 points to §A4 / owner item 4. |
| N-4 cargo manifest path | Resolved | §TF1/§TF3 use `--manifest-path rafaello/Cargo.toml`. |
| N-5 deterministic HTTP-client wording | Resolved | §TF1 says fittings only / no HTTP dependency. |

## Blockers

### B-1. `InReplyToTaintIndex` is never populated for the `tool_result` ids that §TR4b looks up

**Anchor:** goal item 3 (`scope.md:184-204`), §TR1 (`scope.md:772-827`), §TR4a (`scope.md:856-906`), §TR4b (`scope.md:908-992`), §EXFIL3 (`scope.md:1625-1658`).

**Issue:** §TR4b handles `provider.<id>.tool_request in_reply_to = [<result_id>, ...]` by looking up each `<result_id>` in `InReplyToTaintIndex`. But §TR4a only specifies `record()` being called after `handle_tool_request` publishes a canonical `core.session.tool_request`, keyed by the tool_request id. §TR1 uses the cache to look up the referenced **request** id when canonicalising a plugin result, but never records the resulting `core.session.tool_result` id back into the cache.

Therefore the acceptance test `reemit_tool_request_unions_referenced_ancestry.rs` cannot pass as written: the provider cites an earlier result id, but the cache contains request ids only. The lookup returns `None`, §TR4b fail-opens, and the referenced union is empty.

**Smallest acceptable fix:** Make the cache explicitly hold every canonical event class that later `in_reply_to` can cite. At minimum:

- `handle_tool_request` records the canonical tool_request id + taint (for §TR1 tool_result ancestry);
- `handle_tool_result` records the canonical tool_result id + taint (for §TR4b provider tool_request ancestry);
- tests assert both keys exist and that §TR4b reads the result-id entry.

If `assistant_message` / `confirm_*` remain narrowed, say their ids are not recorded or are recorded but unused.

### B-2. Broker-side audit rows are assigned to a broker that has no audit writer plumbing

**Anchor:** §PT1 violation path (`scope.md:1080-1116`), §AL2 (`scope.md:1350-1365`), internal split row 10 (`scope.md:2264-2267`); live `bus.rs:218-280`, `bus.rs:1078-1154`, `audit/mod.rs:28-70`.

**Issue:** §PT1 says `handle_plugin_publish` audits `plugin_publish_rejected_taint_superset`. Live `Broker` has no `AuditWriter`, `Broker::new` accepts only `BrokerAcl`, and existing publish-rejection observability is only `core.lifecycle.publish_rejected`. The m5a audit writer is wired through `SessionController`, `ConfirmationGate`, slash/install surfaces, not the broker.

As written, the implementation agent must invent a new audit plumbing boundary (broker owns `Option<AuditWriter>`? a subscriber writes lifecycle events? gate/controller observes rejections?) and reconcile it with tests. That is too load-bearing to leave implicit.

**Smallest acceptable fix:** Pick and scope the audit plumbing. For example: add `Broker::with_audit_writer(Arc<AuditWriter>)` used by `rfl chat` before plugin spawn, with tests proving PT1 writes the audit row; or drop the direct broker audit and make §AL2 a lifecycle-observer/audit-writer responsibility with a named subscriber. Update §PT1, §AL2, internal split, and acceptance accordingly.

### B-3. EXFIL1's audit golden names a `tool_request` audit kind that is neither live nor scoped

**Anchor:** §EXFIL1 audit table (`scope.md:1566-1580`), §AL4 (`scope.md:1378-1392`); live `audit/mod.rs:28-70`.

**Issue:** The headline integration test's expected audit table contains `tool_request` rows for turn 1 and turn 2. Live `AuditKind` has no `ToolRequest` variant, and §AL4 adds exactly three new variants: `confirm_request_taint_attached`, `plugin_publish_rejected_taint_superset`, and `tool_request_taint_unioned_from_in_reply_to`. No scope section adds a generic `tool_request` audit kind or writer.

That makes the headline golden untestable as written. If the intent is to inspect persisted conversation entries, those are not `audit_events` rows and should not appear in the audit table. If the intent is to add generic tool_request auditing, that is a new audit surface and must be scoped/tested.

**Smallest acceptable fix:** Either remove `tool_request` from the audit-events golden and assert tool calls via session entries / plugin logs, or add a scoped `AuditKind::ToolRequest` with writer location and tests. The former is probably smaller and matches m5a's existing audit surface.

## Major

### M-1. PT1 assigns `publish_for_tool_dispatch` population to the wrong component

**Anchor:** §PT1 (`scope.md:1048-1054`); live `gate/mod.rs:296-321`, `gate/mod.rs:558-610`, `bus.rs:994-1038`.

Round 2 says “m5b's `handle_tool_request` re-emit path is the only caller” of `publish_for_tool_dispatch`. Live m5a routes canonical `core.session.tool_request` through the `ConfirmationGate`; the gate calls `publish_for_tool_dispatch` on passthrough, grant match, allow, and grant-short-circuit paths. `handle_tool_request` publishes the canonical core event; it does not dispatch to plugins.

The data-model extension can still be small because `publish_for_tool_dispatch` already accepts `taint`, but the scope should name the gate as the populator/caller and add tests at the gate boundary, not only the re-emit boundary.

### M-2. TR5's “defer to m6 / v2” conflicts with the m5b → m6 boundary

**Anchor:** m5b → m6 boundary (`scope.md:327-349`), §TR5 (`scope.md:994-1034`), §A9 (`scope.md:1968-1983`), owner item 9 (`scope.md:2415-2423`).

The boundary says m6 is polish and has **no further security primitives**; gaps found in m5b become v2 territory or known v1 limitations. §TR5/§A9/owner item 9 instead say assistant/confirm/rpc superset narrowing is deferred to “m6 / v2.” Pick one. If this is a security primitive, it cannot be deferred to m6 under the boundary. If it is accepted v1 drift, label it “known v1 limitation / v2,” not m6.

### M-3. The unknown-id fail-open acceptance conflicts with live provider intake for fabricated ids

**Anchor:** §TR4a (`scope.md:889-894`), §TR4b acceptance (`scope.md:970-976`), §A10 (`scope.md:1984-1995`); live `bus.rs:626-653`.

`reemit_tool_request_unknown_in_reply_to_id_fails_open.rs` says a provider can cite a TTL-expired or fabricated id and the canonical event publishes. Live `handle_provider_publish` already rejects provider `tool_request` ids that are not in `provider_observed_results`; a fabricated id never reaches re-emit. TTL-expired-but-genuinely-observed ids are the only path that can reach §TR4b with a cache miss.

Smallest fix: narrow the test/wording to “observed by provider but expired from `InReplyToTaintIndex`,” and keep fabricated-id rejection covered by the existing broker stale-id tests.

### M-4. `AuditKind` still references non-live `FromStr` / `Display` tables

**Anchor:** §AL4 (`scope.md:1378-1392`); live `audit/mod.rs:28-70`.

The live authoritative audit table is `AuditKind::as_str()` only. There is no `FromStr` or `Display` impl for `AuditKind`. If m5b wants those impls, scope them as new surface with tests. Otherwise §AL4 should say “enum + `as_str()`” and the acceptance should remain the as_str table test.

### M-5. PT1's empty-taint wording undermines the claimed superset check

**Anchor:** §PT1 (`scope.md:1118-1121`), Stream A §7.2.2 / §7.2.6.

The text says `msg.taint == None` or `[]` “trivially satisfies the superset rule (the union of nothing is nothing).” The referenced request's taint is usually non-empty, so an empty published set is not a mathematical superset. The scoped policy may intentionally validate only non-empty plugin claims (per m5a Appendix A), but the wording should say that: “no plugin-supplied claim, so no contradiction check is run; canonical core taint still preserves ancestry via §TR1.” Otherwise the section reads like omission is a valid superset proof.

### M-6. The stable hash still lacks a scalar-byte canonicalization rule

**Anchor:** §TM1 (`scope.md:626-725`), §TM2 (`scope.md:727-751`).

Round 2 pins SipHasher and the fixed key, but not the bytes fed into the hasher for scalar leaves. Strings, numbers, booleans, and null need one canonical encoding (e.g. `serde_json::to_vec` of the scalar, or tagged bytes like `s:<raw>`, `n:<serde_json number>`). Without that, `"1"` vs `1`, integer formatting, and null/boolean representations are implementation-defined.

## Nits

### N-1. Status banner has a stale owner-item mapping

**Anchor:** status banner (`scope.md:6-8`), owner footer (`scope.md:2448-2450`).

The banner says pi's four convergence-call owner choices are owner items 1, 2, 3, 4. The footer says they map to 1, 2, 3, 10. The footer looks correct after the location split; update the banner.

### N-2. TR1's subscriber-ordering parenthetical is imprecise

**Anchor:** §TR1 ordering (`scope.md:796-807`); live `bus.rs:1160-1172`.

“`notify_internal_subscribers` (which publish calls last)” is easy to misread. Live `publish_core_with_taint` calls `fan_out`, and `fan_out` calls internal subscribers before external recipients. The intended ordering is “record before calling `publish_core_with_taint`; once publish enters `fan_out`, internal subscribers observe before external recipients.”

### N-3. `InReplyToTaintIndex` says “Also used by §PT2” even though PT2 is not a consumer

**Anchor:** §TR4a (`scope.md:896-899`), §PT2 (`scope.md:1148-1173`).

§PT2 is explanatory prose. The actual consumer is §TR1 / `handle_tool_result`. Reword to avoid suggesting broker-side PT2 calls into the router cache.

### N-4. EXFIL3 leaves a “Wait:” editorial marker in ratification prose

**Anchor:** §EXFIL3 (`scope.md:1634-1658`).

The self-correction is useful reasoning, but `scope.md` should converge to directive prose. Replace “Wait:” with a normal note, e.g. “Because §TR4b would inherit referenced result taint, this negative scripts `in_reply_to = []`.”

## Convergence call

Blocking count: **3**. Major count: **6**. Nit count: **4**.

Round 2 made real progress and no pi-1 finding is carried verbatim. The next round should focus on the new consistency gaps: record `core.session.tool_result` taint into the `InReplyToTaintIndex`, decide/plumb broker-side audit writing, and fix EXFIL1's audit golden. After that, the remaining majors are mostly live-source alignment and wording precision.

Owner-judgment items still worth surfacing:

1. Canonical `tool_result` ancestry union vs deliberate Stream A drift (round 2 defaults to union).
2. `assistant_message` / `confirm_*` superset narrowing as v2/known limitation, not m6 polish, unless owner expands m5b.
3. `InReplyToTaintIndex` TTL miss semantics (fail-open vs fail-closed) for genuinely observed but expired ids.
4. File-backed `rafaello-fetch` and EXFIL2 allow-arm inclusion as already listed.
