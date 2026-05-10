# m4 scope.md round-2 pi review

> Verdict: blocking
>
> Counts: b/6 h/5 m/3 l/2

## Round-1 fix verification

| pi-1 finding | status | verification |
|---|---|---|
| B1 `rfl chat` lock/plugin orchestration | partially fixed / reopened | §C now adds lock-driven orchestration and eager spawns (`scope.md:965-1049`), but it cites non-existent live APIs (`Lock::load`, `validate::validate_lock`, wrong `compile_plugin` arity, `shutdown(grace)`) and later contradicts eager tool spawn with first-dispatch lazy loading (`scope.md:2311-2315`). |
| B2 invalid fixture manifests | mostly closed | PR1/TP1 use top-level m1 fields, `${project}`, `[load]`, and sibling `openrpc.json` (`scope.md:1515-1545`, `1690-1735`). Remaining API nits are in High (non-existent `Manifest::parse_at`). |
| B3 taint semantics contradictory | reopened | Header/status says inbound taint is discarded (`scope.md:28-36`), but the deliverable still says plugin-supplied taint is rejected and then “carried into the re-emit” after validation (`scope.md:223-228`). |
| B4 `in_reply_to`/`request_id` model | partially fixed / reopened | §7.2.6 table alignment is improved (`scope.md:38-51`, `775-793`), but result `request_id` remains self-contradictory (`scope.md:628-636`, `799-812`, `1330-1340`) and provider observed-id consumption breaks multi-turn mockprovider behaviour (`scope.md:884-887`, `1605-1608`, `1622-1624`). |
| B5 raw provider fan-out | partially fixed / reopened | B7 says provider inbound is internal-intake-only (`scope.md:841-856`), but the positive matrix still asserts a provider publish is fanned out to a subscriber (`scope.md:1958-1964`). |
| B6 `plugin.<topic-id>.tool_result` publish grant | closed | Compiler-inserted auto-publish is explicitly specified with tests (`scope.md:1730-1740`, `1858-1882`, `2022-2026`). |
| B7 m2 row-39 successor / APIs | closed | Live file name and `SpawnHandle::wait()`/`try_wait()` are now pinned (`scope.md:1827-1848`), matching live `supervisor.rs:134-150`. |
| B8 entry/rendering kinds | partially fixed | Text/tool_call/tool_result entry mapping is pinned (`scope.md:1426-1483`), but it names a non-existent `RenderNode::CodeBlock` variant (`scope.md:1480-1481`; live variant is `RenderNode::Code`, `entry/render_node.rs:50-58`). |
| H1 provider active env | closed | `RFL_PROVIDER_ACTIVE` is dropped; only `RFL_PROVIDER_ID` remains (`scope.md:1139-1164`). |
| H2 assistant_message negatives | partially fixed | Missing/stale tests exist (`scope.md:2027-2033`), but the status still says empty array is rejected (`scope.md:91-93`) while §7.2.6-aligned prose says `assistant_message` is required with ≥0 entries (`scope.md:43-45`, `782-789`). |
| H3 lockin outside-grant proof | closed | Plugin-level and sandbox-level tests are separated (`scope.md:2088-2108`, `2189-2197`). |
| H4 mock-provider parser/correlation | mostly closed | Path punctuation and request_id→path state are specified (`scope.md:1581-1640`, `2062-2083`); multi-turn id consumption issue is counted as a blocker below. |
| H5 `ProviderIdMismatch` | closed | No such variant; `register_provider` derives provider_id from ACL (`scope.md:730-746`). |
| M1 `subscribe_internal` lifecycle | mostly closed | RAII, capacity, drop behaviour, ordering, and tests are specified (`scope.md:1238-1284`). One wording contradiction remains in Medium. |
| M2 TUI test message hook | mostly closed | Env hook/timing/test are specified (`scope.md:1079-1102`), but it uses `Ulid` without adding the dependency to `rafaello-tui` (High). |
| M3 regex dep | closed | Parser is hand-written (`scope.md:1584-1595`). |
| M4 sizing | closed | Commit budget is revised to ~26-32 with monolithic cutover called out (`scope.md:2386-2460`). |
| L1 publisher shape | closed | Canonical shapes are consistently stated in the status/header and B1/B3 (`scope.md:132-138`, `596-657`). |
| L2 read-file spelling | closed | Naming convention is explicit (`scope.md:1774-1794`). |
| L3 user taint | closed | CR5 synthesises user taint and discards frontend-supplied taint (`scope.md:1354-1398`). |

## Blockers

1. **Taint handling is still self-contradictory in the load-bearing deliverable.** Round-2 status and B6 say inbound provider/plugin/frontend `msg.taint` is discarded and replaced (`scope.md:28-36`, `814-838`, `1363-1374`). But the top deliverable still says “plugin-supplied taint on `core.*` is rejected” and then that taint arriving on the plugin namespace is “carried into the re-emit” after validation (`scope.md:223-228`). That is exactly the round-1 ambiguity. Fix by making the deliverable match one mechanical rule: discard+replace, or reject; do not keep both.

2. **The new `rfl chat` orchestration section is not implementable against live m1/m2 APIs.** Scope names `Lock::load(&path)` (`scope.md:972-977`), but live `Lock` only has `to_toml`/`from_toml` (`crates/rafaello-core/src/lock/lock_file.rs:44-51`). It names `validate::validate_lock(&lock)` (`scope.md:978-984`), but the live V3 entry point is `validate::lock(lock, ctx)` with a required `LockValidationContext` (`crates/rafaello-core/src/validate/mod.rs:99-104`). It calls `compile_plugin(&lock, canonical)` (`scope.md:985-989`), but live `compile_plugin` requires `ctx` and `recomputed_digests` (`crates/rafaello-core/src/compile.rs:117-122`). It also specifies `supervisor.shutdown(grace).await` (`scope.md:1049-1050`), while live m2 exposes `shutdown(self)` with no grace arg (`crates/rafaello-core/src/supervisor.rs:812`). This will force commit authors to invent semantics ad hoc. Pin exact file-read, validation-context, digest recomputation, and shutdown surfaces now.

3. **`request_id` is still inconsistent for result events, and the fallback synthesis path is underspecified.** B2 says `request_id` is optional on result/reply topics (`scope.md:628-636`). B6 then says `core.session.tool_result` requires it, inbound `plugin.<id>.tool_result` may omit it, and “all response-shaped topics have `request_id: Option`” (`scope.md:799-812`). CR3 then says the reemit router recovers a missing inbound id from the agent loop's `outstanding_tool_requests` map (`scope.md:1330-1340`), but that map is described as agent-loop state, not router/broker shared state, with no API or ownership path. Fix by choosing one invariant: require inbound tool_result `request_id`, or specify the shared correlation store and ownership explicitly.

4. **Provider observed-result ids are consumed by the broker but retained forever by the mock provider, so the second tool turn fails closed.** B7b says every `core.session.tool_result` id delivered to a provider is consumed when cited by the next provider tool_request (`scope.md:884-887`). The mock provider records all seen tool_result ids in a `BTreeSet` (`scope.md:1575-1579`) and sends the entire set on every later `provider.mock.tool_request` (`scope.md:1605-1608`), only appending new ids on results (`scope.md:1622-1624`). After one follow-up, the broker has consumed the old id; the provider will cite it again and trigger `StaleRequestId`. Either make observed ids reusable conversation context, or make the mock provider remove consumed ids / cite only unconsumed ids.

5. **The provider-inbound fan-out fix is contradicted by the positive matrix.** B7 says validated provider inbound events go to the trusted `ReemitRouter` queue and “not the external fan-out path” (`scope.md:841-856`). The test `broker_provider_event_not_fanned_to_external_subscribers.rs` encodes that (`scope.md:2019-2021`). But the positive `broker_publish_provider_topic_authorised.rs` still publishes `provider.mock.assistant_message` and asserts “fan-out reaches a subscribed in-process recipient” (`scope.md:1958-1964`). That reopens round-1 B5 unless “in-process recipient” is explicitly the internal subscriber. Rename/rewrite the positive test to observe `subscribe_internal`, not external fan-out.

6. **Lazy/eager loading is still contradictory.** The B1 fix and §C require eager spawning the active provider and every installed tool plugin (`scope.md:15-18`, `1017-1026`). Risk #12 repeats that lazy-load is out of scope (`scope.md:2384-2391`). But Out of scope says m4 spawns “the tool plugin on first dispatch (`load.triggers.kind = "tool"`)” (`scope.md:2311-2315`). That is the exact round-1 failure mode: no lazy-spawn-on-publish primitive exists. Delete the first-dispatch/tool-trigger wording or explicitly scope and test lazy spawning.

## High

1. **`RenderNode::CodeBlock` is an invented live type.** AL6 and the test matrix require `RenderNode::CodeBlock` (`scope.md:1480-1481`, `2011-2013`), but live `RenderNode` has `Code { code, lang }`, not `CodeBlock` (`crates/rafaello-core/src/entry/render_node.rs:50-58`). Use `RenderNode::Code` while `Entry.kind = "tool_result"` carries the payload.

2. **`RFL_TUI_TEST_MESSAGE` requires `Ulid` in `rafaello-tui`, but the dependency is not planned.** T1 says the TUI binary constructs `JsonRpcId::String(Ulid::new().to_string())` (`scope.md:1079-1087`) and W says no new third-party dependencies (`scope.md:548-570`). Live `rafaello-tui` dependencies do not include `ulid` (`crates/rafaello-tui/Cargo.toml:12-28`). Either add `ulid = { workspace = true }` to `rafaello-tui` or use an existing fittings/core id allocator that is actually available.

3. **`provider_assistant_message` empty-`in_reply_to` semantics conflict.** The round-2 status says the H2 matrix includes “missing / empty array / unknown tool_result id” rejections (`scope.md:91-93`), but the security RFC-aligned rule says `provider.<id>.assistant_message` is required with ≥0 entries (`scope.md:43-45`, `782-789`), so `[]` is valid. The positive/negative matrix only names missing and stale files (`scope.md:2027-2033`), leaving the implementation driver to guess. Decide whether empty is valid; if valid, remove the empty-array rejection claim.

4. **The headline demo is still not fully reproducible.** The bar now names a tempdir `README.md` and asserts the assistant prefix (`scope.md:2123-2143`), but it never pins the exact bytes written to `README.md` or the exact expected returned body. The prompt explicitly needs a concrete README path and concrete bytes. Add a literal fixture body, e.g. `"m4 demo readme\n"`, and assert exact assistant text, not just “begins with”.

5. **Fixture manifest tests cite a non-existent parser API.** PR3/TP3 say `Manifest::parse_at(...)` (`scope.md:1651-1654`, `2050-2054`), but live manifest parsing is `Manifest::parse(&str)` plus `manifest::validate_with_package(manifest_path, package_dir, manifest)` (`crates/rafaello-core/src/manifest/top_level.rs:67-68`, `crates/rafaello-core/src/manifest/validate_with_package.rs:18-22`). Replace `parse_at` with the actual load/read/parse/validate sequence.

## Medium

1. **The internal-subscriber risk text contradicts the internal-intake design.** B7 intentionally sends provider inbound events to internal subscribers even when external subscribers must not see them (`scope.md:841-856`). Risk #6 says the side-channel “cannot leak events that would not have reached external subscribers” (`scope.md:2372-2380`). That is false for provider inbound under this scope. Reword the risk: internal subscribers are allowed to see trusted-core intake events that external subscribers cannot.

2. **The direct-tool negative includes an unowned m1 validator requirement.** The negative matrix says a tool plugin subscribing to `core.session.tool_request` should “warn or refuse” in the m1 grant compiler (`scope.md:2177-2188`). No concrete m1 patch/test is named, and m1 currently treats `bus.subscribes` as validated patterns, not semantic tool-dispatch exclusions. Either make this an m4 broker/agent-loop test only, or add a concrete m1 back-reach row and test.

3. **The mock provider's Unicode parser is underspecified around lowercasing and slicing.** PR2 says lower-case the input, strip a prefix, then take “the remaining slice” (`scope.md:1584-1593`) and also requires multibyte UTF-8 path support (`scope.md:2082-2084`). If the implementation slices offsets from the lowercased string back into the original string, Unicode case expansion can corrupt boundaries. Pin the algorithm as prefix detection on a case-folded copy plus path extraction from the original after the matched prefix byte length, or restrict to ASCII prefixes only.

## Low

1. **B8 is duplicated in the broker section.** There are two `B8` bullets: taint-envelope synthesis (`scope.md:897`) and topic validation lifecycle (`scope.md:920`). Renumber the latter to B9 and cascade, or use subletters.

2. **`broker_publish_provider_topic_authorised.rs` asserts `in_reply_to: Some([_])` for an assistant message without specifying how the provider observed a citeable id.** The line is small but will make the test setup ambiguous (`scope.md:1958-1964`). Say whether the setup first fans out a `core.session.user_message` or `core.session.tool_result` to populate the observed-id map.

## Notes

- The m4 → m5 boundary is much improved: sinks, confirmation, `user_grants`, slash commands, propagation, and matching are explicitly out of m4 (`scope.md:337-356`, `2262-2274`).
- The workspace-wide `BusEvent.request_id` cutover is correctly called out as a monolithic commit (`scope.md:2332-2339`, `2401-2407`).
- The new `core.session.user_message` shape is mostly pinned (`payload.text`, synthesized user taint, synthesized/forwarded request_id: `scope.md:1354-1398`, `1426-1435`).
