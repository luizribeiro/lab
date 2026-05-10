# m4 scope.md round-4 pi review

> Verdict: blocking
>
> Counts: b/2 h/0 m/1 l/0

## Round-3 fix verification

| pi-3 finding | status | verification |
|---|---|---|
| B1 `request_id` invariant/table | partially fixed / still reopened | §B0 table exists and B2/B4/CR5 now cite it (`scope.md:898-922`, `958-970`, `1023-1028`, `1869-1876`), but TP2 still publishes `plugin.<topic-id>.tool_result` with only payload + `in_reply_to`, no required `request_id` (`scope.md:2307-2309`). |
| B2 observed-id class split | closed | B7b now splits tool_request = observed tool_results only, assistant_message = results ∪ user_messages (`scope.md:1243-1254`) and names both negative/positive tests (`scope.md:2625-2639`). |
| B3 C3 live APIs | closed | C3 uses `manifest.canonical_bytes()` into a `Vec<u8>`, `digest::manifest_digest(&canonical_bytes)`, and `topic_id::derive(&canonical.to_string())` (`scope.md:1417-1424`), matching live symbols (`manifest/top_level.rs:86`, `digest.rs:26`, `topic_id.rs:15`). |
| B4 tool_result wire shape | closed | CR3 forwards the bus payload unchanged (`scope.md:1822-1830`); PR2 reads `payload.content` directly and explicitly excludes `ToolResultPayload`/`RenderNode` on the wire (`scope.md:2155-2174`). |
| H1 multi-turn mock-provider test | closed | Named test now drives turn 1 result → turn 2 request and asserts the prior result id is cited and accepted (`scope.md:2679-2694`). |
| H2 origin-provider exclusion hook | closed | `publish_core_with_taint` has `origin_provider: Option<CanonicalId>` and excludes that provider (`scope.md:1271-1293`); CR2 passes the source provider (`scope.md:1794-1802`) and coverage is named (`scope.md:2540-2546`). |
| M1 table-of-truth | closed except TP2 drift | §B0 is the requested table (`scope.md:898-916`); the remaining failure is a caller not following it. |
| M2 stale tool-result correlation gap | mostly closed | Gap is recorded out-of-scope (`scope.md:2897-2911`), but B6 still says the stale id is “rejected indirectly at the reemit path” while also saying CR3 forwards it (`scope.md:1131-1135`). Counted Medium wording/alignment. |
| L1 taint heading | closed | Negative heading says “discarded/replaced,” not “rejected” (`scope.md:2827-2829`). |

Re-confirmed cross-round items: pi-1 B1/C3 API correctness is closed; pi-2 B3/request_id model is still reopened by TP2; pi-2 B4/mock multi-turn is closed.

## Blockers

1. **TP2 still violates the `request_id` table-of-truth.** §B0 requires `.tool_result` inbound and canonical events to carry `request_id` (`scope.md:906-913`), and B6 says missing is `MissingRequestId` (`scope.md:1139-1146`). TP2 still says readfile publishes `plugin.<topic-id>.tool_result` with payload and `in_reply_to` only (`scope.md:2307-2309`). The broker will reject the demo-bar tool result before CR3.

2. **`rafaello-mockprovider` cannot compile as specified: it uses ULIDs but W2 omits `ulid`.** PR2 requires fresh request ids as “a ULID stringified” (`scope.md:2130-2133`), but W2 dependencies list no `ulid` (`scope.md:857-862`). W5 adds `ulid` only for `rafaello-tui` (`scope.md:877-884`). Add `ulid = { workspace = true }` to the mockprovider crate (and likely readfile once TP2 is fixed to generate a result id).

## High

None.

## Medium

1. **Stale tool-result correlation deferral still has contradictory wording.** The out-of-scope section clearly says m4 accepts stale `plugin.<id>.tool_result.in_reply_to` and defers broker validation (`scope.md:2897-2911`), which is the requested recorded gap against RFC §7.2.6 (`rfc-security-model.md:1029`). B6 still says an unrecognised id is “rejected indirectly at the reemit path” while CR3 forwards it (`scope.md:1131-1135`). Reword B6 to match the recorded gap.

## Low

None.

## Notes

- RFC §7.2.6 alignment is otherwise improved: provider tool_request may cite only observed tool_results, while assistant_message may cite conversation context (`scope.md:1243-1254`; `rfc-security-model.md:1029-1031`).
- No remaining `topic_id_of` / C3 `.as_bytes()` copyability issue found in the current C3 recipe.
