# Pi review 1 — fittings RFCs

Reviewed the draft RFCs against the current `fittings` code. Verdict: good direction, but I would request major revisions before implementation.

## Scope reviewed

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md`
- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md`
- Current fittings implementation under `fittings/crates/*/src/`

## Blocking / high-priority findings

### 1. JSON-RPC error-code policy is incorrect

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md:79-82`
- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md:143-159`

The draft allows `-32768..=-32100` as application-defined. That range is reserved by JSON-RPC for predefined/future errors, not application errors.

Recommended valid service ranges:

- `-32099..=-32000`, the JSON-RPC server-error range; plus
- codes outside `-32768..=-32000`, if fittings intentionally wants to allow them.

Also, the prose says positive `1..=32767`, while the sample code accepts `1..=i32::MAX`. The RFC should make this consistent.

### 2. Cancellation design does not fit the current server loop

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:107-125`
- `fittings/crates/server/src/server.rs:79-124`

The current server reads a frame, then waits for a semaphore permit before spawning a worker that decodes the frame. If `max_in_flight` is saturated by long-running calls, a cancellation notification can get stuck waiting for the same semaphore as the work it is trying to cancel.

The RFC needs to require one of these designs:

- decode cancellation notifications in the receive loop before semaphore acquisition; or
- introduce a separate dispatcher path for cancellation frames that is not blocked by the request-worker semaphore.

Without this, cancellation will be unreliable exactly when it is most needed.

### 3. Cancellation response policy is inconsistent / still open

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:255-263`
- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md:385-391`

The notifications RFC acceptance criteria require the absence of a response frame for cancelled calls. The error RFC still leaves cancellation as an open question and only recommends a future `Cancelled` variant.

This must be normative before implementation. The RFCs should answer:

- Does the server always suppress responses for cancelled requests?
- Does it ever return an LSP-style cancellation error?
- Is response suppression configurable for MCP vs non-MCP users?
- What should a handler return when `ctx.cancelled()` fires?

### 4. Core request ID model remains lossy

References:

- `fittings/crates/core/src/message.rs:8-20`
- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:84-87`

Current core `Request.id` and `Response.id` are `String`, while the wire layer supports `JsonRpcId` values that may be strings, numbers, or null. The notifications RFC adds `ServiceContext::request_id() -> Option<&JsonRpcId>`, but does not decide whether the core `Request` type should also change.

This matters for:

- numeric request IDs;
- cancellation correlation;
- middleware and router APIs;
- consistent application-facing semantics.

The RFC should explicitly choose whether `Request`/`Response` remain stringified or migrate to `JsonRpcId` / `Option<JsonRpcId>`.

### 5. Macro migration is underspecified

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:146-150`
- `fittings/crates/macros/src/parse.rs:193-217`

The notifications RFC says handlers may optionally accept context, then says the simpler path is to always pass `ctx`. Current macro parsing requires exactly this shape:

```rust
async fn name(&self, params: P) -> Result<R, FittingsError>
```

The RFC should choose one macro strategy and spell out the generated API changes. For example:

- keep user trait methods as `(&self, params)` and only pass `ServiceContext` to `MethodRouter`; or
- change macro service methods to always be `(&self, params, ctx)`; or
- support both arities and define the parser/expansion behavior.

Right now the macro plan is not specific enough to implement safely.

## Medium-priority findings

### 6. Middleware section requires a trait redesign

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md:287-300`
- `fittings/crates/core/src/middleware.rs`

The error RFC says middleware may emit notifications via `ctx.notify`, but the current middleware trait has no context parameter. This is a trait/API redesign, not just documentation.

The RFC should include middleware in the migration surface and specify:

- the new middleware signature;
- whether middleware receives the same `ServiceContext` as the handler;
- whether middleware can observe cancellation;
- ordering between middleware, panic catching, and response suppression.

### 7. Client notification handler API needs failure semantics

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:187-195`
- `fittings/crates/client/src/lib.rs:112-160`

The proposed `on_notification(Fn(&str, Value))` handler runs inline on the client loop. The RFC should define what happens if the handler:

- blocks for a long time;
- panics;
- attempts to re-enter client APIs;
- performs expensive synchronous work.

An inline handler can stall all response handling. The RFC should either isolate handler execution or clearly document that notification handlers must be fast and non-panicking.

The client-side RFC should also define behavior for inbound server-originated requests with IDs while server-to-client requests are deferred. Options include rejecting with `-32601`, logging and dropping, or treating this as a fatal protocol error.

### 8. Backpressure story contradicts itself

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:21-25`
- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:227-232`

The non-goals say per-handler backpressure is out of scope, but the open questions say bounded `try_send` semantics are probably desirable for v1.

This must be resolved before implementation because it changes the API contract for `notify`:

- Does `notify` always enqueue unless the connection is closed?
- Can `notify` fail or drop on overflow?
- Is `notify` synchronous or async?
- Are notifications considered lossy?

The answer affects rafaello streaming behavior and operator expectations under load.

### 9. Outbound error preservation must be explicit

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md:31-40`
- `rafaello/plans/stream-b-fittings/rfc-fittings-errors.md:168-191`
- `fittings/crates/wire/src/error_map.rs`

The error RFC strongly states that message/data should be preserved over the wire, but most concrete text focuses on `from_error_envelope`. The RFC should also explicitly require `to_error_envelope` to preserve meaningful messages and data for predefined errors where possible.

Otherwise an implementer may preserve inbound error details while still encoding local predefined errors as canonical strings like `"Invalid params"`, which would fail the stated information-preservation goal.

### 10. `ctx.notify` cannot reliably report “peer gone”

References:

- `rafaello/plans/stream-b-fittings/rfc-fittings-notifications.md:98-105`
- `fittings/crates/server/src/server.rs:47-77`

The notifications RFC says `notify` returns a transport error if the outbound channel is closed and describes that as “peer gone.” With the current mpsc writer architecture, enqueueing can succeed even if the peer is already gone. The disconnect is usually discovered later by `transport.send`, not by `mpsc::UnboundedSender::send`.

The RFC should soften or correct this contract. A more accurate contract would be:

- `notify` reports local enqueue/encoding failure;
- transport failure is detected asynchronously by the serve loop;
- cancellation is triggered when the serve loop observes connection shutdown;
- successful `notify` does not guarantee delivery to the peer.

## Suggested next revision

Before coding, update the RFCs to settle:

- valid JSON-RPC service code ranges;
- cancellation dispatch and response semantics;
- `JsonRpcId` vs `String` in core types;
- macro method signature strategy;
- client notification panic/blocking behavior;
- bounded vs unbounded notification sink.
