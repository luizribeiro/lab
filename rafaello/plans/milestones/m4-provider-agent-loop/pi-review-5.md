# m4 scope.md round-5 pi review

> Verdict: non-blocking-fixes
>
> Counts: b/0 h/0 m/0 l/1
>
> All blockers/highs are gone; pi-4's 2 blockers + 1 medium are closed.

## Round-4 fix verification

| pi-4 finding | status | verification |
|---|---|---|
| B1 TP2 missing `request_id` | closed | §B0 requires `.tool_result` ids (`scope.md:946-949`); B6 rejects missing ids (`scope.md:1184-1191`); TP2 now publishes `request_id: Some(JsonRpcId::String(Ulid::new().to_string()))` (`scope.md:2348-2358`); CR3 now receives `request_id: Some(rid)` or never sees the event (`scope.md:1849-1856`). |
| B2 mockprovider/readfile `ulid` dep | closed | W2 includes `ulid` for mockprovider and ties it to PR2 fresh ids (`scope.md:890-897`); W3 includes `ulid = { workspace = true }` for readfile (`scope.md:900-904`); workspace alias exists (`rafaello/Cargo.toml:38`). |
| M1 stale tool-result wording | closed | B6 now says stale `plugin.<id>.tool_result.in_reply_to` is accepted by the broker and forwarded verbatim (`scope.md:1164-1178`); Out-of-scope records the same deferred broker-side stale-correlation gap (`scope.md:2950-2963`). |

## Request-id table re-check

No current caller is off-table. §B0 is the single table (`scope.md:934-961`); `publish_core_with_taint` validates all four required suffixes (`scope.md:1311-1321`); CR2/CR3/CR4/CR5 forward required ids (`scope.md:1816-1846`, `scope.md:1849-1865`, `scope.md:1873-1884`, `scope.md:1908-1929`); AL5 dispatch keeps the same id (`scope.md:1996-2004`); PR2 generates fresh provider ids (`scope.md:2171-2189`, `scope.md:2204-2222`); TP2 generates a fresh readfile result id (`scope.md:2348-2362`).

No other `BusEvent`/result-publish gap found in the current m4 work plan. No other planned ULID consumer is missing the dependency: W2/W3/W5 cover mockprovider/readfile/TUI (`scope.md:890-904`, `scope.md:914-918`).

## Remaining issues

### Low

1. Future-work wording for the decisions row still says `request_id` is mandatory only on “request-shaped” topics and lists `.tool_request`, `.user_message`, `.assistant_message`, omitting `.tool_result` (`scope.md:3265-3269`). This is outside the current m4 implementation sections but should be fixed before drafting that decisions row.
