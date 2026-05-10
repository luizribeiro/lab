# m4 scope.md round-3 pi review

> Verdict: blocking
>
> Counts: b/4 h/2 m/2 l/1

## Round-2 fix verification

| pi-2 finding | status | verification |
|---|---|---|
| B1 taint deliverable contradiction | mostly closed | The load-bearing rule is now discard+replace (`scope.md:377-382`, `993-1013`). One negative-matrix heading still says "rejected" (`scope.md:2536-2545`); counted Low only. |
| B2 live `rfl chat` APIs | partially fixed / reopened | `Lock::from_toml`, `validate::lock`, four-arg `compile_plugin`, and no-arg `shutdown()` now match live code (`scope.md:1173-1241`, `1292-1299`; live `lock_file.rs:49`, `validate/mod.rs:99`, `compile.rs:117-122`, `supervisor.rs:812`). But C3 still uses non-compiling / non-existent helpers: `manifest.canonical_bytes().as_bytes()` and `topic_id_of` (`scope.md:1219-1229`). |
| B3 `request_id` invariant | reopened | Header/B6 require `request_id` on all four suffixes (`scope.md:43-53`, `979-985`), but B2/B4 still make results optional/request-shaped-only (`scope.md:803-809`, `866-868`), CR5 allows missing frontend ids (`scope.md:1634-1641`), and TP2 omits a tool_result id (`scope.md:2060-2062`). |
| B4 provider observed ids / mock turn 2 | partially fixed | The consumed-id bug is fixed by retained-context semantics (`scope.md:1059-1076`) and PR2 cites the retained set (`scope.md:1895-1903`). Coverage is still missing: no named test drives turn 1 result → turn 2 tool_request. |
| B5 provider inbound fan-out | closed | Positive test is renamed to internal subscriber and asserts no external recipient (`scope.md:2266-2280`); separate negative remains (`scope.md:2327-2332`). |
| B6 lazy/eager contradiction | closed | C8 eager-spawns tools (`scope.md:1275-1283`), fixtures are `[load] eager = true` (`scope.md:1799-1800`, `2020-2023`), and lazy load is explicitly out (`scope.md:2643-2648`). |
| H1 `RenderNode::CodeBlock` | closed | AL6/test matrix use `RenderNode::Code` (`scope.md:1735-1740`, `2320-2323`), matching live `entry/render_node.rs:50-64`. |
| H2 `ulid` in `rafaello-tui` | closed | W5 adds `ulid = { workspace = true }` to `rafaello-tui` (`scope.md:742-752`); workspace alias exists (`rafaello/Cargo.toml:38`). |
| H3 assistant empty `in_reply_to` | closed | Empty is consistently valid; missing/stale tests only (`scope.md:85-93`, `958-962`, `2485-2492`). |
| H4 demo reproducibility | closed | README bytes and exact assistant text are pinned (`scope.md:2442-2468`). |
| H5 `Manifest::parse_at` | closed | PR3/TP3 use `Manifest::parse` + `manifest::validate_with_package` (`scope.md:1941-1958`, `2063-2066`). |
| M1 internal subscriber risk wording | closed | Risk #6 now frames `subscribe_internal` as privileged core read path (`scope.md:2702-2719`). |
| M2 direct-tool negative | closed | The m1 validator speculation is gone; the test is m4 broker/agent-loop only (`scope.md:2500-2524`). |
| M3 Unicode parser slicing | closed | Parser uses ASCII-only folded copy for prefix detection and slices the original input (`scope.md:1843-1888`). |
| L1 duplicate B8 | closed | Broker sections are B8/B9/B10/B11 (`scope.md:1090-1154`). |
| L2 provider positive citeable id setup | closed | Setup first publishes a core tool_result before provider assistant_message (`scope.md:2266-2276`). |

Reopened / partial pi-1 items not fully closed: pi-1 B1 is still partially reopened by the C3 API mistakes; pi-1 B4 remains reopened via the `request_id` contradictions; pi-1 H4 is partially reopened because the multi-turn observed-id behaviour lacks a test.

## Blockers

1. **`request_id` is still not governed by one invariant.** The round-3 banner says every `.tool_request` / `.tool_result` / `.assistant_message` / `.user_message` event requires `request_id` on inbound and canonical sides (`scope.md:43-53`), and B6 repeats that (`scope.md:979-985`). But B2 says result/reply topics are optional (`scope.md:803-809`), B4 says `MissingRequestId` only fires for request-shaped topics excluding `tool_result` (`scope.md:866-868`), CR5 synthesises a missing frontend id instead of rejecting inbound absence (`scope.md:1634-1641`), and TP2 publishes `plugin.<topic-id>.tool_result` with only `in_reply_to` and no fresh `request_id` (`scope.md:2060-2062`). Pick one table-of-truth and make TP2/tests match it.

2. **The §7.2.6 observed-id store now allows the wrong id class for provider tool requests.** B6 correctly says `provider.<id>.tool_request.in_reply_to` entries must reference observed `core.session.tool_result.request_id`s (`scope.md:946-955`; security RFC `rfc-security-model.md:1027-1031`). B7b then checks membership in the union of observed tool_results and observed user_messages for all provider replies (`scope.md:1059-1071`). That lets a provider cite a user_message id on a tool_request, violating the RFC row and the B6 prose. Enforcement must be topic-specific: tool_request = results only; assistant_message = conversation context.

3. **The C3 live-API replacement still contains non-compiling code.** Live `Manifest::canonical_bytes()` returns `Vec<u8>` (`manifest/top_level.rs:86`) and `digest::manifest_digest` wants `&[u8]` (`digest.rs:26`), but C3 calls `manifest.canonical_bytes().as_bytes()` (`scope.md:1219-1222`). `topic_id_of(canonical)` is also not a live symbol (`scope.md:1228-1229`; live helper is `topic_id::derive`, `topic_id.rs:14`). The orchestration recipe is still not copyable.

4. **The mock provider reads the wrong shape for `core.session.tool_result`, threatening the demo bar.** CR3 re-emits the plugin tool_result payload unchanged (`scope.md:1589-1605`), so the provider receives the readfile bus payload `{ok, content}`. PR2 instead says `content_text` is extracted from `ToolResultPayload.content` / `RenderNode::Code` as produced by AL6 persistence (`scope.md:1923-1930`). Persistence entry payloads are not the bus payload delivered to providers. Pin one wire shape; otherwise the assistant may not echo the file body.

## High

1. **The mock-provider observed-id fix is not test-proven.** Round-3 says retained ids make turn 2 succeed (`scope.md:1072-1076`) and PR2 cites all prior results on each request (`scope.md:1895-1903`), but the named mockprovider tests stop at first-turn request, path mapping, and assistant-on-result (`scope.md:2373-2400`). Add a multi-turn test: user asks file A → tool_result A → user asks file B → second `provider.mock.tool_request.in_reply_to` includes A's result id and broker accepts it.

2. **Provider-origin exclusion from canonical tool_request fan-out lacks a mechanical hook.** B10 requires excluding the originating provider when fan-out emits `core.session.tool_request` (`scope.md:1134-1139`), but `publish_core_with_taint` only takes topic/payload/request_id/in_reply_to/taint (`scope.md:1090-1104`). If the broker is expected to infer origin from taint detail, say so and cover ambiguous provider-id/canonical cases; otherwise add an explicit origin argument.

## Medium

1. **`request_id` rollout still lacks the requested table-of-truth.** The document has several prose rules but no single table enumerating required/optional per topic class. That omission is why B2, B4, B6, CR5, and TP2 diverged.

2. **Tool-result stale-correlation remains deliberately unenforced.** B6 says an unrecognised plugin tool_result id is not checked by m4 and is left to a future provider-side concern (`scope.md:962-971`), while the security RFC requires `plugin.<id>.tool_result` to reference a matching routed tool_request (`rfc-security-model.md:1027-1029`). If m4 intentionally defers this, record it as a decision/negative gap; otherwise add the agent-loop outstanding map back for validation.

## Low

1. **Taint wording has one stale "rejected" label.** The rule itself is discard+replace (`scope.md:377-382`, `993-1013`), but the negative matrix heading still says plugin-supplied taint is "rejected" (`scope.md:2536-2538`). Rename it to "discarded/replaced" to keep the mechanical wording identical everywhere.

## Notes

- The explicitly requested grep is clean for the round-2 invented names in live code: no live `Lock::load`, `validate::validate_lock`, `Manifest::parse_at`, `RenderNode::CodeBlock`, or `shutdown(grace)` symbols; the replacements are real except for the new C3 `as_bytes()` / `topic_id_of` mistakes.
- Demo reproducibility and lazy/eager consistency are now materially fixed.
