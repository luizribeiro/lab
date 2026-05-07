# Pi review 2 — fittings RFCs

Round 2 review of Claude's revised fittings RFCs. This review checks whether the issues from `pi-review-1.md` were addressed and records new issues found in the revised text.

## Scope reviewed

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md` (557 lines)
- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md` (504 lines)
- `rafaello/plans/stream-b-fittings/pi-review-1.md`
- Current fittings implementation, especially:
  - `fittings/crates/server/src/server.rs`
  - `fittings/crates/client/src/lib.rs`
  - `fittings/crates/core/src/message.rs`
  - `fittings/crates/macros/src/parse.rs`
  - `fittings/crates/wire/src/error_map.rs`

## Verdict

The revised RFCs address most Round 1 findings well. The direction is now solid, but I would still request revisions before implementation.

Most Round 1 blockers are resolved. Remaining must-fix issues are mostly around the new bounded-channel design, request-id edge cases, and the exact meaning of "information preserving" for predefined JSON-RPC errors.

## Round 1 findings status

### 1. JSON-RPC error-code policy is incorrect — RESOLVED

Round 1 issue: the previous draft incorrectly allowed `-32768..=-32100` as application-defined and had inconsistent positive-code limits.

Current citations:

- `rfc-fittings-errors.md:79-84`
- `rfc-fittings-errors.md:126-175`

The revised RFC now correctly states that:

- `-32099..=-32000` is the JSON-RPC server-error range open to applications;
- the rest of `-32768..=-32000` is reserved for predefined/future JSON-RPC errors;
- application codes outside the reserved negative cluster are allowed;
- the positive range is consistently `1..=i32::MAX`.

This resolves the Round 1 correctness issue.

### 2. Cancellation design does not fit the current server loop — PARTIAL

Round 1 issue: cancellation notifications could get stuck behind the same request-worker semaphore as the long-running calls they are meant to cancel.

Current citations:

- `rfc-fittings-notifications.md:174-194`
- `fittings/crates/server/src/server.rs:79-124`

The revised RFC now explicitly fast-paths cancellation before semaphore acquisition. That addresses the original cancellation-specific deadlock.

However, the revised bounded-channel design introduces a new related deadlock risk. See **New finding 11** below. The cancellation fast-path itself is fixed, but the broader dispatcher/semaphore/channel architecture still needs revision.

### 3. Cancellation response policy is inconsistent / still open — RESOLVED

Round 1 issue: the notifications RFC required no response for cancelled calls, while the errors RFC still left cancellation open.

Current citations:

- `rfc-fittings-notifications.md:396-436`
- `rfc-fittings-errors.md:106-111`
- `rfc-fittings-errors.md:475-486`

The revised RFCs now define normative cancellation behavior:

- cancelled requests receive no response frame;
- handlers should observe `ctx.cancelled()` and return promptly;
- `FittingsError::Cancelled { reason }` exists for handler/helper propagation;
- the dispatcher suppresses the response when the cancellation token fired;
- no LSP-style cancellation error response is emitted in v1.

One edge remains around handlers returning `Cancelled` when the token has not fired; see **New finding 18**.

### 4. Core request ID model remains lossy — PARTIAL

Round 1 issue: current `Request.id` / `Response.id` are `String`, while wire IDs are `JsonRpcId`, causing numeric/string/null ambiguity.

Current citations:

- `rfc-fittings-notifications.md:96-131`
- `fittings/crates/core/src/message.rs:8-20`

The revised RFC decides to move `JsonRpcId` into `fittings-core` and change `Request.id` and `Response.id` to `JsonRpcId`. This fixes numeric-vs-string preservation.

However, request IDs are optional on the wire, and inbound notifications have no ID. A non-optional `Request.id: JsonRpcId` still needs a sentinel for notifications, likely `JsonRpcId::Null`, which conflates a missing ID with explicit `"id": null`. See **New finding 13**.

### 5. Macro migration is underspecified — RESOLVED

Round 1 issue: the macro plan said context might be optional, but current parser required exactly `(&self, params)`.

Current citations:

- `rfc-fittings-notifications.md:221-265`
- `fittings/crates/macros/src/parse.rs:193-217`

The revised RFC chooses one shape, no overloading:

```rust
async fn name(&self, ctx: ServiceContext, params: P) -> Result<R, FittingsError>
```

It also specifies the `MethodRouter` signature and generated dispatch behavior. This is implementable and resolves the ambiguity.

### 6. Middleware section requires a trait redesign — RESOLVED

Round 1 issue: the previous RFC said middleware could emit notifications via `ctx.notify`, but the middleware trait had no context parameter.

Current citations:

- `rfc-fittings-errors.md:334-389`
- `fittings/crates/core/src/middleware.rs`

The revised RFC now explicitly redesigns the middleware trait to include `ctx: ServiceContext`, defines that the same context flows through the stack, and specifies cancellation, notification, panic, decode-error, and transformation behavior.

This resolves the Round 1 issue.

### 7. Client notification handler API needs failure semantics — RESOLVED

Round 1 issue: the proposed handler ran inline on the client loop and lacked blocking/panic/re-entrancy semantics.

Current citations:

- `rfc-fittings-notifications.md:297-339`
- `rfc-fittings-notifications.md:341-356`
- `fittings/crates/client/src/lib.rs:112-160`

The revised RFC now says notification handlers are run in spawned tasks, not inline, and defines behavior for blocking, panics, re-entrancy, expensive synchronous work, and missing handlers.

It also defines what the client does for unsupported server-originated requests with IDs: respond `-32601`, warn, and keep the connection open.

One wording/code-shape nit remains: `tokio::spawn(handler(method, params))` is not valid for a sync `Fn`; see **New finding 17**.

### 8. Backpressure story contradicts itself — RESOLVED, with new implementation risk

Round 1 issue: the non-goals said no backpressure, while open questions suggested bounded `try_send` semantics.

Current citations:

- `rfc-fittings-notifications.md:19-28`
- `rfc-fittings-notifications.md:377-385`
- `rfc-fittings-notifications.md:438-469`

The revised RFC now resolves the policy: bounded sink, default capacity 1024, non-blocking `notify`, drop-on-full with metric/warn and `Ok(())`.

The policy contradiction is resolved. However, the chosen implementation model creates a new deadlock risk if the same bounded channel is used for responses and notifications while the dispatcher can block on semaphore acquisition. See **New finding 11**.

### 9. Outbound error preservation must be explicit — PARTIAL

Round 1 issue: the RFC emphasized `from_error_envelope` preservation but did not explicitly require `to_error_envelope` preservation.

Current citations:

- `rfc-fittings-errors.md:184-213`
- `fittings/crates/wire/src/error_map.rs`

The revised RFC now explicitly specifies outbound `to_error_envelope` behavior for each variant and requires preserving messages/data for predefined errors where the variant can carry them.

This addresses the missing outbound contract. However, the proposed predefined variants still cannot carry all wire `message`/`data` fields, so the acceptance criterion that all five predefined codes round-trip "without information loss" is not yet true. See **New finding 14**.

### 10. `ctx.notify` cannot reliably report “peer gone” — RESOLVED

Round 1 issue: the earlier RFC implied `ctx.notify` could synchronously report peer disconnect, which is not true with the mpsc writer architecture.

Current citations:

- `rfc-fittings-notifications.md:141-157`
- `rfc-fittings-notifications.md:471-499`
- `fittings/crates/server/src/server.rs:47-77`

The revised RFC now clearly states that `ctx.notify` reports local enqueue status only and does not confirm peer delivery. Peer disconnect is discovered asynchronously by the dispatcher on `transport.send`.

This resolves the Round 1 issue.

## New / remaining findings from Round 2

### 11. Bounded channel + semaphore can deadlock unless the server loop is redesigned — NEW / HIGH

Current citations:

- `rfc-fittings-notifications.md:174-194`
- `rfc-fittings-notifications.md:438-466`
- `fittings/crates/server/src/server.rs:79-124`

The revised RFC keeps the idea that non-cancellation frames "fall through to the existing semaphore-gated worker spawn," but also changes the outbound channel to bounded and says responses block when the channel is full.

Current server architecture awaits semaphore acquisition inside the receive branch. With a bounded shared response/notification channel, this can deadlock:

1. all worker permits are held;
2. notification frames fill the bounded outbound channel;
3. workers complete and try to send responses, but block because the channel is full;
4. the dispatcher is blocked waiting for a semaphore permit, so it is not draining the outbound channel;
5. permits never release because workers are blocked sending responses.

The RFC should explicitly require that semaphore waiting never blocks the dispatcher from draining outbound frames.

Possible fixes:

- move semaphore acquisition into a separate task/queue;
- make workers return responses via `JoinSet` and let the dispatcher send;
- use separate response and notification channels, with responses on an unbounded or guaranteed-capacity path;
- or otherwise prove the bounded channel cannot block worker completion.

This is the most important remaining implementation risk.

### 12. Channel type is internally contradictory — NEW / HIGH

Current citations:

- `rfc-fittings-notifications.md:133-136`
- `rfc-fittings-notifications.md:161-162`
- `rfc-fittings-notifications.md:438-446`

Earlier sections still describe the notifier as `mpsc::UnboundedSender<Vec<u8>>`, while the resolved backpressure section mandates a bounded `tokio::sync::mpsc` with `try_send`.

The RFC needs one canonical channel type and contract throughout. Since the backpressure section is normative, the earlier `UnboundedSender` references should be replaced with bounded-sender wording.

### 13. `Request.id: JsonRpcId` still cannot represent inbound notifications cleanly — NEW / HIGH

Current citations:

- `rfc-fittings-notifications.md:90-92`
- `rfc-fittings-notifications.md:115-117`
- `rfc-fittings-notifications.md:274-283`
- `fittings/crates/core/src/message.rs:8-20`

The revised RFC says both `Request.id` and `Response.id` become `JsonRpcId`, but request IDs are optional on the wire. Inbound notifications have no ID and still go through `Service::call`.

That forces a sentinel, presumably `JsonRpcId::Null`, which conflates:

- an actual request with `"id": null`; and
- a notification with no `id`.

`ctx.request_id() -> Option<&JsonRpcId>` helps, but the core `Request` model remains ambiguous.

Recommended design:

- `Request.id: Option<JsonRpcId>`;
- `Response.id: JsonRpcId`.

At minimum, add normative rules for:

- no-id notifications;
- explicit `id: null`;
- whether `id: null` is accepted, rejected, or tracked in `in_flight`;
- how handlers should construct a `Response` for an inbound notification whose result will be dropped.

### 14. “Predefined errors round-trip without information loss” is not true with the proposed variants — NEW / HIGH

Current citations:

- `rfc-fittings-errors.md:60-112`
- `rfc-fittings-errors.md:184-201`
- `rfc-fittings-errors.md:215-238`
- `rfc-fittings-errors.md:503-504`

The RFC says all five JSON-RPC predefined codes round-trip without information loss, but several proposed variants cannot hold all wire fields:

- `Parse { message }` has no `data`.
- `MethodNotFound { method }` has no original wire `message` or arbitrary `data`.
- `Internal { message }` has no arbitrary `data`.

The proposed `from_error_envelope` for method-not-found extracts a method and discards the rest. That is still lossy.

Either:

- add `message` and `data` fields to all predefined variants; or
- weaken the acceptance criterion and explicitly document which fields are intentionally not preserved.

As written, the acceptance criterion is stronger than the proposed data model.

### 15. Cancellation parsing edge cases are still underdefined — NEW / MEDIUM

Current citations:

- `rfc-fittings-notifications.md:185-209`
- `rfc-fittings-notifications.md:423-424`

The RFC specifies unknown cancellation IDs as a benign race and silently drops them. It does not define malformed cancellation payload behavior.

The RFC should define behavior for:

- missing `params`;
- missing `id` / `requestId`;
- malformed ID type, e.g. object or array;
- string `"1"` vs number `1`;
- duplicate cancellation notifications.

Recommendation: malformed cancellation notifications should be logged and dropped, not sent to handlers and not treated as fatal protocol errors.

### 16. Batch semantics under cancellation are missing — NEW / MEDIUM

Current citations:

- `fittings/crates/server/src/server.rs:168-219`
- `rfc-fittings-notifications.md:183-194`
- `rfc-fittings-notifications.md:396-436`

Current server code supports JSON-RPC batches and handles a batch inside one worker. The revised RFC does not define how cancellation interacts with batches.

Missing questions:

- Are cancellation notifications inside batches fast-pathed by the dispatcher?
- Are batch item requests individually registered in `in_flight`?
- Is a cancelled batch item omitted from the batch response array?
- What happens if all batch item responses are suppressed?
- Does a batch worker hold one semaphore permit for the entire batch or one permit per item?

Even if rafaello does not rely on batches, fittings already supports them. The RFC should preserve current behavior explicitly or consciously narrow it.

### 17. `tokio::spawn(handler(method, params))` is not valid for a sync `Fn` — NEW / MEDIUM

Current citations:

- `rfc-fittings-notifications.md:299-317`

The proposed API is:

```rust
F: Fn(String, Value) + Send + Sync + 'static
```

That closure returns `()`, not a future. The implementation wording says the client loop calls:

```rust
tokio::spawn(handler(method, params))
```

That is not valid Rust. It should be:

```rust
tokio::spawn(async move {
    handler(method, params);
});
```

This is minor but worth fixing in the RFC to avoid implementation drift.

### 18. `Cancelled` variant behavior is inconsistent when the token has not fired — NEW / MEDIUM

Current citations:

- `rfc-fittings-errors.md:106-111`
- `rfc-fittings-notifications.md:400-413`
- `rfc-fittings-errors.md:475-486`

The errors RFC says `Cancelled` has no wire mapping and the response frame is suppressed. The notifications RFC says suppression happens when the request's cancellation token has fired.

The RFC should define what happens if a handler returns:

```rust
Err(FittingsError::Cancelled { reason: ... })
```

without the request token being set. This can happen if application code decides to abort work locally or maps a helper-level cancellation into `FittingsError::Cancelled`.

Recommended rule: suppress on either condition:

- the cancellation token fired; or
- the handler returned `FittingsError::Cancelled { .. }`.

Alternatively, if `Cancelled` is only valid when the token fired, say so explicitly and require the dispatcher to map an unexpected `Cancelled` to an internal error or warning. The current text is ambiguous.

## Summary: must-fixes before implementation

1. **Fix the bounded-channel/semaphore deadlock risk.** The dispatcher must never block on semaphore acquisition in a way that prevents it from draining outbound frames.
2. **Use one canonical notification/response channel type.** Replace stale `UnboundedSender` references or revise the bounded-drop policy.
3. **Define request ID semantics for notifications and `id: null`.** Prefer `Request.id: Option<JsonRpcId>` and `Response.id: JsonRpcId`.
4. **Align predefined-error variants with the “no information loss” goal.** Either add `message`/`data` to all predefined variants or weaken the acceptance criterion.
5. **Specify malformed cancellation handling and batch cancellation semantics.** These are existing protocol surfaces in fittings and should not be left to implementation guesswork.
6. **Clarify `Cancelled` suppression rules.** Decide whether returned `FittingsError::Cancelled` suppresses even if the token did not fire.

## Overall assessment

Round 2 is a major improvement. Most of `pi-review-1.md` is addressed. The remaining issues are narrower but still important because they affect runtime correctness and API semantics. Once the bounded-channel/server-loop interaction and ID/error-model edge cases are resolved, the RFCs should be ready to implement.
