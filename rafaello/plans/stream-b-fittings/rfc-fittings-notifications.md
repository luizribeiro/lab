# RFC: outbound notifications + ServiceContext

Status: draft (stream-b-fittings)
Owner: rafaello
Affects: `fittings-core`, `fittings-server`, `fittings-client`,
`fittings-macros`, `fittings/examples/mcp-server`

## Goal

Give a `Service` handler a way to push JSON-RPC notifications to its peer
during a call, without bypassing `Server::serve`. Also expose a
per-request cancellation signal in the same place, since handlers that
emit progress notifications almost always want to know when the peer
gave up.

The wire already speaks notifications (`RequestEnvelope::notification`).
The gap is purely on the trait/runtime side.

Non-goals for v1:

- Server-originated *requests* to the client (sampling, elicitation).
  Designed-around, not designed-in. See "Future work" and the
  explicit "Server-originated requests: v1 cut" section below.
- Async-flow-control backpressure (handlers being suspended when
  the transport is slow). v1 ships a **bounded sink with drop-on-
  full semantics** — see "Notification sink: bounded with drop"
  below for the resolved contract. Earlier drafts oscillated; this
  is the settled form.
- Ordering guarantees stronger than "notifications observed by the
  framework in order N are written to the transport in order N
  *if not dropped*".

## Today

```rust
#[async_trait]
pub trait Service: Send + Sync {
    async fn call(&self, req: Request) -> Result<Response, FittingsError>;
}
```

`Server::serve` owns a private `mpsc::UnboundedSender<Vec<u8>>` that
only its workers reach (`server/src/server.rs:49`). The proposal
splits this into a separate `notification_tx` (bounded) and
`response_tx` (unbounded); see §3b. One worker produces
exactly one `ResponseEnvelope`. The example MCP server replaced
`Server::serve` with its own loop because of this; see
`fittings/examples/mcp-server/src/mcp.rs:587–705`.

## Proposed change

### 1. New trait `Service` signature

```rust
#[async_trait]
pub trait Service: Send + Sync {
    async fn call(
        &self,
        req: Request,
        ctx: ServiceContext,
    ) -> Result<Response, FittingsError>;
}
```

`ServiceContext` is **owned by value, cheap to clone** (internally
`Arc`-backed) so handlers can hand it to spawned tasks.

### 2. `ServiceContext`

Lives in `fittings-core` next to `Service`.

```rust
#[derive(Clone)]
pub struct ServiceContext {
    inner: Arc<ServiceContextInner>,
}

impl ServiceContext {
    /// Push an outbound JSON-RPC notification to the peer.
    /// Non-blocking; ordering between calls on the same `ServiceContext`
    /// clone tree is preserved.
    pub fn notify(&self, method: &str, params: Value) -> Result<(), FittingsError>;

    /// Resolves when the peer has cancelled this request, the connection
    /// has dropped, or the server is shutting down. Implementations
    /// should `select!` between their own work and `cancelled()`.
    pub async fn cancelled(&self);

    /// Non-async polling form, mirrors `tokio_util::sync::CancellationToken`.
    pub fn is_cancelled(&self) -> bool;

    /// JSON-RPC id of the originating request, if any. `None` for
    /// notifications-handling code paths (see §5).
    pub fn request_id(&self) -> Option<&JsonRpcId>;
}
```

### 2a. Core `Request`/`Response` migrate to `JsonRpcId`

Today `Request.id` and `Response.id` are `String`
(`fittings/crates/core/src/message.rs:8–20`). The wire layer already
uses `JsonRpcId` (string | number | null), and `Server::execute_request`
stringifies it before handing it to the handler — at which point
numeric ids and string ids are indistinguishable, and a null id
arrives as the literal string `"null"`.

This is fine today only because handlers never use the id for
correlation. As soon as cancellation joins the picture (this RFC's
`in_flight: HashMap<JsonRpcId, _>`), the dispatcher needs to look
up the *exact* id the peer sent. Round-tripping through `String`
breaks the lookup for numeric ids: `JsonRpcId::Number(1)` formats
as `"1"`, but the `in_flight` map is keyed on `JsonRpcId`, so a
subsequent inbound `notifications/cancelled` with `requestId: 1`
must match — and will only match if `Request.id` carried the
original `JsonRpcId`, not a stringified copy.

Decision (revised after pi-review-2):

- **`Request.id: Option<JsonRpcId>`** — `None` for inbound
  notifications, `Some(id)` for inbound requests. This is the
  only model that preserves the wire distinction between "no
  id" (notification) and `"id": null` (a request whose id
  happens to be the JSON null literal).
- **`Response.id: JsonRpcId`** — required. Notifications never
  produce responses, so the `Service::call` path for a `None`
  `Request.id` returns `Result<(), FittingsError>` semantically
  (we simulate this by *dropping* the returned `Response`
  entirely; see §5).

Wire-level rules for `id: null`:

- An inbound request envelope with `"id": null` is a request
  (not a notification). It enters `in_flight` keyed on
  `JsonRpcId::Null`. The dispatcher accepts at most one
  in-flight `Null`-keyed entry at a time; a second inbound
  request with `"id": null` while one is in flight is a
  protocol error and produces an `-32600 Invalid Request`
  response with `id: null`.
- An inbound notification envelope (no `id` field at all)
  has `Request.id = None`.
- Cancellation matching against `Null` works the same as any
  other id, but is rare in practice.
- Every response carries a non-Null id when possible: if the
  request used `id: null`, the response also uses `id: null`,
  per spec.

This is a breaking change to every `Service` and `MethodRouter`
impl. Migration:

- `req.id.clone()` (was `String`) → `req.id.clone()` (now
  `Option<JsonRpcId>`). Handlers that assumed an id was always
  present must `.expect(...)` or pattern-match.
- Constructing a `Response` requires producing a `JsonRpcId`,
  not a `String`. For id-bearing requests this is `req.id
  .clone().unwrap()` (safe — guaranteed `Some` for any path
  that returns `Ok(Response)`); for inbound notifications,
  the framework discards the `Response` so its `id` is
  immaterial — see §5.

`fittings-core` becomes the canonical owner of `JsonRpcId`. We move
the type from `fittings-wire` down to `fittings-core` and re-export
it from `fittings-wire` for source compatibility. This avoids the
awkward `core` depending on `wire` cycle.

Future RFCs that add server-originated requests will use the same
type and benefit from the unified model.

Internally `ServiceContextInner` holds:

- `notifier: NotificationSink` — a bounded
  `tokio::sync::mpsc::Sender<Vec<u8>>` (capacity =
  `notification_capacity`, default 1024) drained by the dispatcher.
  `notify` uses `try_send` and drops on full per the
  "Notification sink: bounded with drop" contract. *Not* the same
  channel as response frames; see §3b.
- `cancel: tokio_util::sync::CancellationToken` (or our own equivalent
  if we're avoiding the dependency).
- `request_id: Option<JsonRpcId>`.

`notify` builds a `RequestEnvelope::notification`, encodes it once, and
pushes it onto the outbound channel. Errors:

- `Err(FittingsError::Transport { ... })` if the bounded
  notification channel is closed, which happens only on
  dispatcher shutdown. This does
  **not** mean the peer is gone — it means the local serve loop
  is exiting. Peer-disconnect is detected asynchronously by the
  dispatcher's next `transport.send`. See "ctx.notify delivery
  contract" below for the full story.
- `Err(FittingsError::Internal { ... })` if encoding fails (params
  not serialisable). This should be rare; treating it as internal
  keeps the API total without a separate error type.

**Successful `Ok(())` does NOT confirm peer delivery.** It only
confirms the frame was queued for the dispatcher. This is
explicit; the original draft of this RFC implied otherwise and
was wrong on the architecture.

### 3. `Server::serve` plumbing

The existing outbound `mpsc::UnboundedSender<Vec<u8>>` is split into
two channels (see §3b): a bounded `notification_tx` (the
`NotificationSink`) and an unbounded `response_tx`. Per-request
worker construction now:

1. Builds a `ServiceContext` carrying:
   - a clone of the bounded `notification_tx` (for `ctx.notify`),
   - a fresh `CancellationToken`,
   - the request's `JsonRpcId`.
2. Stores the cancellation handle in an `in_flight: HashMap<JsonRpcId,
   CancellationToken>` shared with the receive arm of the select.
3. Calls `service.call(req, ctx).await` inside `catch_unwind`.
4. On worker completion, removes from `in_flight` and sends the
   response frame.

**Cancellation must NOT go through the request-worker semaphore.**
Today the receive arm calls `spawn_frame_handler` which does
`semaphore.acquire_owned().await` *before* decoding the frame
(`server/src/server.rs:107–126`). Under saturation, a cancellation
notification queued behind the long-running calls it is meant to
cancel would deadlock: the cancel can't be dispatched until a
worker frees a permit, and no worker frees a permit because none
of them have been told to cancel yet.

The new design:

1. The receive arm peeks at every inbound frame **before** acquiring
   a permit. We do a cheap `serde_json::from_slice::<RequestEnvelope>`
   and inspect `id`/`method`. This is the same work the worker
   would have done; we just do it on the receive task.
2. If `id.is_none()` (a notification) **and** `method` matches the
   configured cancellation method, we resolve the target id from
   `params` and signal the token in `in_flight` immediately, without
   touching the semaphore, without spawning a worker. Done.
3. All other notifications and all id-bearing requests fall through
   to the existing semaphore-gated worker spawn.

Effectively: **cancellation is a fast-path the dispatcher handles
itself.** The semaphore only exists to bound concurrent *handler*
work, and cancellation is not handler work.

```rust
// Server::new keeps its current shape; cancellation is configurable:
Server::new(service, transport)
    .with_cancellation_method("notifications/cancelled")
    .with_cancellation_id_extractor(|params| /* fn returning JsonRpcId */)
```

A reasonable default extractor reads `params.id` (LSP-ish) and a
named extractor reads `params.requestId` (MCP-ish). Either way, the
runtime owns the wiring — handlers just `select!` on `ctx.cancelled()`.

Trade-off: the receive arm now parses frame headers twice (once on
the dispatcher to check method, once in the worker to fully decode).
Acceptable cost; full decode of large param payloads still happens on
the worker side.

A defensive belt-and-braces alternative — running a separate
`cancellation_dispatcher` task that listens on its own channel — is
rejected as overkill. The single dispatcher task already serialises
inbound frames; peeking and routing in place is ~20 lines.

### 3a. Dispatcher MUST NOT block on the semaphore

Pi-review-2 finding 11. The dispatcher today does
`semaphore.acquire_owned().await` *inside* the `recv` branch of its
top-level `select!`. With a bounded outbound channel (see §
"Notification sink: bounded with drop"), this creates a deadlock:

1. all `max_in_flight` permits are held by running workers;
2. workers emit notifications until the bounded outbound channel
   fills;
3. workers then try to send their final response frame, but
   `mpsc::Sender::send` blocks because the channel is full;
4. the dispatcher is the only task that drains the outbound
   channel — but it is parked in `semaphore.acquire_owned()`
   waiting for a permit;
5. permits never release because the workers can't finish, and
   the channel never drains.

**Resolution: the dispatcher must never `.await` the semaphore on
the same task that drains the outbound channel.** Specifically:

1. Replace `spawn_frame_handler`'s `semaphore.acquire_owned().await`
   with a non-blocking `semaphore.try_acquire_owned()` plus a
   bounded backlog channel (call it `pending_workers: mpsc::Sender
   <(Frame, OwnedSemaphorePermit)>`, capacity = `max_in_flight`).
2. A small **worker-spawner task** owns the semaphore:
   `loop { permit = sem.acquire().await; frame = backlog.recv()
   .await; spawn(handle(frame, permit)) }`. This task does the
   awaiting; it never touches the outbound channel.
3. The dispatcher receive arm does `try_acquire`; on failure it
   pushes onto `pending_workers` (which is bounded but always
   drainable because the spawner is awaiting it). On success it
   spawns directly.
4. The dispatcher's top-level `select!` continues to include a
   `response_rx.recv()` arm that drains the outbound channel
   regardless of semaphore state. **This is the invariant: the
   dispatcher's drain loop is never gated by the semaphore.**

Equivalent simpler refactor (preferred unless we discover a need
for the extra task): collapse the worker model entirely to a
`JoinSet`-driven design:

- the dispatcher receives frames, decodes, optionally short-
  circuits (cancellation, parse error), and otherwise tries to
  spawn a worker;
- workers do **not** write responses to a channel — they return
  `(JsonRpcId, Result<Value, FittingsError>)` from the `JoinSet`;
- a third `select!` arm on `workers.join_next()` consumes
  completions and writes responses *directly* to the transport
  (or to the bounded outbound channel — either way, the writer
  task is separate from the spawn-side semaphore wait).
- the spawn-side `acquire_owned().await` is moved off the
  dispatcher task into the same JoinSet style as the MCP example
  in `fittings/examples/mcp-server/src/mcp.rs:587–705` already
  uses.

**Normative invariant (must appear as a server-loop test):** with
`notification_capacity = 1`, `max_in_flight = 1`, and a handler
that emits two notifications then completes, the server must not
deadlock. The current architecture, naively bound, does. The
refactor above breaks the cycle.

### 3b. One channel or two?

Pi-review-2 finding 11 also raises whether responses and
notifications share one channel. We commit to **two channels**:

- `response_tx: mpsc::Sender<Vec<u8>>`, **unbounded**. Responses
  are at most one per in-flight worker, so total queue depth
  is bounded by `max_in_flight`. Unbounded is safe and removes
  the responses-block-on-full failure mode entirely.
- `notification_tx: mpsc::Sender<Vec<u8>>`, **bounded** at
  `notification_capacity` (default 1024). Drop-on-full per the
  notification sink contract.

The dispatcher's `select!` polls *both* receive ends and writes
to the transport in arrival order between channels. Notifications
emitted before a response are still delivered before that
response (within a connection); fairness across requests is a
tokio-select coin flip — fine.

This supersedes the earlier wording ("the same channel is used for
response frames so the dispatcher preserves global order"). The
old wording was wrong: it created the deadlock pi-review-2 caught.

The validation rule (`notification_capacity ≥ max_in_flight`) is
no longer needed and is removed.

### 4. `Service` blanket impls and migration

The current single-arg form is used by `fn`-style handlers in
`fittings-macros` and by all current consumers. Migration:

- Update `Service` itself.
- Update `RouterService` / `MethodRouter::route` to take a context
  too (`route(method, params, metadata, ctx) -> ...`).
- **Macro signature: pick one shape, no overloading.** The macro's
  parser today (`fittings/crates/macros/src/parse.rs:193–217`) hard-
  requires `inputs.len() == 2` and rejects anything else. We change
  it to hard-require `inputs.len() == 3`:

  ```rust
  async fn name(&self, ctx: ServiceContext, params: P)
      -> Result<R, FittingsError>
  ```

  *Not* optional. *Not* arity-sniffed. Handlers that don't use the
  context write `_ctx: ServiceContext`. This is the same churn as
  any other breaking trait change and keeps the macro expander a
  hundred lines simpler than the optional-arg alternative.

  Generated server-side dispatch (`MethodRouter::route` impl produced
  by the macro) calls `self.name(ctx, deserialized_params).await`.

  Generated client-side stub (the macro also emits a typed client)
  takes a `&Client<C>` and the params; it does **not** take a
  `ServiceContext` because clients have no inbound context. Client
  stubs are unchanged in shape.

- **`MethodRouter` trait** changes to:

  ```rust
  async fn route(
      &self,
      method: &str,
      params: Value,
      metadata: Metadata,
      ctx: ServiceContext,
  ) -> Result<Value, FittingsError>;
  ```

  Hand-written `MethodRouter` impls migrate the same way as macro-
  generated ones.
- Provide a `From<Request> for (Request, ServiceContext::detached())`
  helper for tests, plus a `ServiceContext::detached()` constructor
  that drops notifications and is never cancelled. This keeps
  `Service` directly testable without spinning up a server.

Existing `Service` impls in tests will need a one-line signature
change. There are roughly six in-tree impls; that's acceptable churn.

### 5. Notification handlers (inbound, server-side)

JSON-RPC 2.0 says servers MUST NOT respond to inbound notifications.
The current server already drops the response when `request.id` is
`None` (`server.rs:248`). With `ServiceContext`:

- Inbound notification → `Request.id = None`,
  `ctx.request_id() = None`.
- The framework still calls `Service::call(req, ctx)`. The
  handler must produce a `Response` to satisfy the type, but
  its `id` and `result` are **discarded by the framework**.
  Idiomatic: handlers can construct `Response { id:
  JsonRpcId::Null, result: Value::Null, metadata: Default
  ::default() }` for notifications and forget about it.
- Handler `Err(_)` is logged at `tracing::warn!` and dropped
  on the wire (no spec-conformant alternative).
- Cancellation does not apply (no id to cancel against;
  `ctx.cancelled()` for a notification handler is wired to a
  token that fires only on connection shutdown).

A future minor revision could change `Service::call` to return
`Result<Option<Response>, _>` so notification handlers can return
`Ok(None)`, but that's a wider trait churn than warranted now.

### 6. Client side: receiving server-pushed notifications

This is the symmetric piece. Today the client loop only listens to the
transport when `pending` is non-empty (`client/src/lib.rs:150`) and
only knows how to decode `ResponseEnvelope`. To make rafaello's event
bus work, the client must:

1. Always read from the transport, even with no pending calls.
2. Distinguish a `ResponseEnvelope` (has `id`) from an inbound
   `RequestEnvelope` with no `id` (notification).
3. Hand notifications to a user-provided handler.

API addition:

```rust
impl<C> Client<C> {
    /// Register a notification sink. Inbound id-less frames are
    /// forwarded here. Replaces any previous sink.
    pub fn on_notification<F>(&self, handler: F)
    where
        F: Fn(String, Value) + Send + Sync + 'static;
}
```

**Execution model: spawn per notification, do not run inline.**

The earlier draft proposed running the handler on the client loop
itself. That makes the handler a head-of-line blocker for every
subsequent response. We change to: the client loop, on receiving a
notification frame, calls `tokio::spawn(handler(method, params))`
and immediately returns to its `select!`. Cost: one
`Arc<dyn Fn>` clone + one tokio task per notification. Acceptable
for rafaello's expected throughput (≪10k/s).

Failure semantics, normative:

- **Blocking.** A handler that takes a long time does not stall
  the client loop, because it runs in a spawned task. It can stall
  *itself* if the consumer registered a sequential queue, but
  that's the consumer's choice.
- **Panics.** Each handler invocation runs inside the spawned
  task. `tokio` will print a panic message and drop the task; the
  client loop is unaffected. The framework does not call
  `catch_unwind` on the handler — consumers who need panic
  isolation install it themselves.
- **Re-entrancy.** A handler MAY call back into `Client::call` /
  `Client::notify`. Because handlers run on spawned tasks, not
  on the client loop, there is no deadlock. The standard
  channel-based path delivers the new request to the loop.
- **Synchronous expensive work.** Discouraged. The handler's
  `Fn` signature is sync; consumers needing async should
  `tokio::spawn` further or hand the params to a `mpsc::Sender`
  the consumer drains elsewhere. Documented in the rustdoc.
- **No handler registered.** Inbound notifications are silently
  dropped after a `tracing::trace!`. Not an error.

### Client-side: server-originated *requests* arriving when not supported

The wire decoder will see an inbound `RequestEnvelope` *with* an
`id` and a method that the client never expected (because v1 of
fittings doesn't ship server→client requests). Policy:

1. The client responds with `ErrorEnvelope { code: -32601,
   message: "Method not found", data: None }` so a future-aware
   peer learns the client is not capable of this request.
2. A `tracing::warn!` records method + id.
3. Connection is **not** torn down. This is a soft failure, not
   a protocol violation by the peer.

This keeps the door open for server-originated requests in a
later RFC: when we add support, we just stop returning -32601 for
the methods we now handle.

## Smallest viable change vs. clean change

Two viable sequencings:

**A. One PR.** Land `ServiceContext`, the new `Service` signature,
in-flight tracking, and the cancellation method config together. Forces
a single migration; mcp-example collapses back to using `Server::serve`.

**B. Two PRs.**
1. Add a parallel `ServiceWithContext` trait (default-impl bridge from
   `Service`) and let the server prefer the new trait. Notifications
   work; cancellation deferred.
2. Replace `Service` with the new signature, delete the old trait,
   delete the bridge.

We recommend **A** because the consumer count is tiny and the bridge
trait would outlive its usefulness. The whole point of fittings is
that it's small enough to evolve without ceremony.

## Open questions

1. **CancellationToken type.** Do we adopt
   `tokio_util::sync::CancellationToken` or roll a 30-line equivalent?
   Adopting it means a new dependency on `tokio-util` for `core`.
   The MCP example already rolled its own; that suggests the
   inhouse version is fine.
2. **Backpressure.** *Resolved (was open).* See
   "Notification sink: bounded with drop" below.
3. **Per-request vs per-connection context.** Does the bus event-stream
   case want a context that survives a single request? Probably
   handled by the connection-scoped notify sink (a separate
   `ServerHandle::notify` that doesn't need a request id) — out of
   scope for v1, but the `ServiceContext` design must not foreclose it.
4. **Cancellation protocol naming.** Bake in MCP's
   `notifications/cancelled` as the default? Or LSP's `$/cancelRequest`?
   Keep it configurable, default to LSP since fittings markets itself
   as transport-agnostic.

## Cancellation response semantics (normative)

Settled here, referenced from the errors RFC. v1 rules:

1. **A request that has been cancelled gets no response on the wire.**
   The dispatcher tracks per-request cancellation state. When a
   worker completes, the dispatcher checks the token before
   serialising the response; if the token fired, the response
   (success *or* error) is dropped. This matches MCP semantics and
   is the only behaviour rafaello needs.
2. **Handlers should observe `ctx.cancelled()` and return promptly.**
   Recommended return: `Err(FittingsError::Cancelled { reason })`
   (variant added in the errors RFC). The variant exists so handlers
   can `?`-bubble cancellation through helper functions; the
   dispatcher then suppresses the response regardless of variant.
3. **Returning `Ok(_)` after cancellation is also fine** — the
   dispatcher still suppresses. We do not require handlers to
   return any specific error variant for correctness.
4. **`ctx.notify` after cancellation:** `notify` continues to
   succeed (still enqueues local frames; see "ctx.notify delivery
   contract" below) until the dispatcher actually shuts the worker
   down. There is a small window where late notifications can reach
   the peer for a request whose response will be suppressed. The
   peer must tolerate this — JSON-RPC notifications are fire-and-
   forget by spec, and MCP/LSP both already require clients to
   tolerate post-cancel notifications. We document this and move
   on; it is not worth the complexity to clamp.
5. **Inbound cancellation for an unknown id is silently dropped.**
   It is a benign race (cancel arrived after handler completed).
   Recorded at `tracing::trace!` only.
6. **Malformed cancellation payloads are dropped, not fatal.**
   Specifically, the dispatcher applies the configured
   `cancellation_id_extractor` and on any of:
   - `params` is missing or `null`;
   - the extractor's target field (`id` / `requestId`) is
     missing;
   - the target field is not a JSON-RPC id type (e.g. an object,
     array, boolean, or fractional number);
   it logs at `tracing::warn!` (with the raw payload truncated
   to 256 bytes) and drops the frame. The connection is **not**
   torn down; this is treated as peer noise, not a protocol
   violation.
7. **`"1"` (string) and `1` (number) are distinct ids.** The
   dispatcher uses `JsonRpcId`'s native equality. A peer that
   sent `id: 1` and then `notifications/cancelled` with
   `requestId: "1"` will see no cancellation; this is a peer
   bug. We do not normalise.
8. **Duplicate cancellation notifications for the same in-flight
   id are idempotent.** Triggering an already-fired token is a
   no-op (`CancellationToken::cancel` is idempotent by design).
   No log.

### Cancellation in batch requests

The current server processes JSON-RPC batches inside one worker
sequentially (`server/src/server.rs:168–219`). The revised design
must define how cancellation interacts with batches, since fittings
already supports them.

Rules:

1. **Cancellation notifications inside a batch are fast-pathed
   item-by-item.** When the dispatcher peels open a batch on
   the receive side (before semaphore acquisition), each
   notification item is checked against the cancellation
   method just like a top-level frame. Any matching items are
   dispatched immediately; non-cancellation items continue
   into the worker as before.
2. **Each id-bearing item in a batch is a separate `in_flight`
   entry.** The batch worker registers all ids before
   processing begins and removes them as each completes.
3. **Cancelling one item of a batch suppresses *that item's*
   response only.** Other items in the same batch still
   produce responses. The batch response array therefore may
   be shorter than the request array — this matches the
   "notification items produce no response" rule the spec
   already requires.
4. **If every item in a batch is cancelled (or every item is
   a notification), no batch response is emitted.** The
   current behaviour at `server.rs:211–213` already handles
   "no responses → emit nothing"; we keep it.
5. **Batches still occupy one semaphore permit total**, not
   one per item. The unit of concurrency is the inbound
   frame, and the existing test
   `batch_with_notifications_and_calls_returns_only_call_responses`
   captures the contract. Changing this is out of scope.
6. **A cancellation notification may target an id that is
   currently mid-batch.** This works exactly because the
   batch worker registered each item id in `in_flight`
   individually (rule 2). The mid-batch worker observes the
   token, drops the in-progress item's would-be response, and
   continues with the next batch item.

This preserves current batch behaviour for fittings consumers
that don't use cancellation, and gives consumers that do a
predictable per-item model.
6. **No LSP-style `RequestCancelled` error response.** Suppression
   is the chosen policy; it is not configurable in v1. If a
   non-MCP consumer ever needs the LSP behaviour, adding
   `Server::with_cancellation_response_policy(...)` is a
   non-breaking change later.

What the **client** sees for a cancelled call: its `pending`
oneshot is never resolved. The client API today exposes this
via `tokio::time::timeout` + drop on the future; we keep that.
Rafaello's client wrapper will translate "we sent a cancel and
then the call's future was dropped" into a domain `Cancelled`
result on its own; fittings does not need an in-band signal.

## Notification sink: bounded with drop

Resolves the backpressure question normatively.

- The outbound notification channel is a `tokio::sync::mpsc` with a
  bounded capacity. **Default capacity: 1024 frames.**
  Configurable via `Server::with_notification_capacity(n)`.
- `ctx.notify(method, params)` is **synchronous, non-blocking**.
  Internally it encodes the frame and `try_send`s. Three outcomes:
  - success → `Ok(())`.
  - channel full → drop the frame, increment a `notifications_dropped`
    metric, emit `tracing::warn!` (rate-limited per request id),
    return `Ok(())`. Rationale: notifications are advisory by spec;
    making `notify` fail or block forces every handler to write
    error-handling code for a condition the consumer cannot recover
    from anyway.
  - channel closed (dispatcher shutting down) → return
    `Err(FittingsError::Transport { ... })`. Handlers may bail.
- `notify` is therefore **lossy under load, never blocking, never
  back-pressuring the handler**. Streaming-tokens consumers must
  tolerate dropped intermediate frames. (In practice: emit a final
  "done" frame the consumer can verify.)
- **Responses use a separate, unbounded channel.** See §3b. The
  bounded sink applies to *notifications only*. An earlier draft
  shared one channel and risked a worker-blocks-on-full deadlock;
  pi-review-2 caught this. Two channels, polled in the same
  `select!`, sidestep the issue without losing ordering within
  notifications or within responses.

This contract is the API for `notify`. It does not change later
without an RFC.

## ctx.notify delivery contract

**Important:** `ctx.notify(...)` reports *local enqueue* status
only. It does **not** confirm peer delivery.

- `Ok(())` means the frame was encoded and `try_send`-ed onto
  the bounded notification channel that the dispatcher drains.
  This includes the drop-on-full case (see "Notification sink:
  bounded with drop"): a dropped frame still returns `Ok(())`.
- `Err(FittingsError::Internal { ... })` means encoding failed
  (params not serialisable). Handler bug; surface it.
- `Err(FittingsError::Transport { ... })` means the bounded
  notification channel is closed, which only happens on
  dispatcher shutdown (graceful EOF path). This is rare and
  informational; handlers may ignore.

**`Ok(())` does NOT mean the peer received the notification.**
Transport failure (broken pipe, peer crash) is discovered
asynchronously by the dispatcher's next `transport.send`, at
which point the dispatcher trips cancellation on every in-flight
worker and shuts the connection. Handlers can observe this via
`ctx.cancelled()`.

This is a softening of the original RFC text, which implied
`notify` could report "peer gone" synchronously. With the
mpsc-writer architecture it cannot — the writer task is the only
component that touches the transport, and it does so out of band.
Changing this would require either (a) making `notify` await the
transport write directly, which serialises all handlers behind
each other, or (b) a per-frame ack signal, which doubles channel
traffic. Neither is worth it for v1.

## Server-originated requests: v1 cut

**Out of scope for v1. Explicitly deferred. Concrete cut:**

- `ServiceContext` does NOT expose a `request(method, params)`
  method in v1. Reserved for a follow-up RFC.
- The server loop does NOT read response frames (it has no
  outstanding outbound requests, so there are none to correlate).
  No `pending: HashMap<JsonRpcId, oneshot::Sender>` on the server.
- The client loop, conversely, learns to *receive* notifications
  (this RFC) and to politely reject inbound id-bearing requests
  with `-32601` (this RFC, §"Client-side: server-originated
  requests arriving when not supported"). The reject behaviour is
  the v1 cut: it leaves the protocol forward-compatible for when
  we do ship server→client requests.
- Cancellation flows **client → server only** in v1. There is no
  symmetric mechanism for the server to cancel a request it never
  issued.

When this is revisited (likely once rafaello needs human-in-the-
loop confirmation prompts), the follow-up RFC will:

1. Add `ServiceContext::request(...) -> impl Future<Result>`.
2. Teach the server loop to track outstanding outbound requests
   and parse inbound response frames.
3. Teach the client to dispatch inbound id-bearing requests to a
   user-supplied handler, replacing the -32601 reject.
4. Decide whether bidirectional cancellation is in scope.

This RFC does not include scaffolding for that work. Doing so
would either pollute `ServiceContext` with an unused method or
make `Server::serve` carry plumbing it doesn't need yet. The
follow-up will add both at once, on demand.

## Future work (deferred)

- **Server→client requests.** Sampling/elicitation. Requires the server
  loop to read response frames and correlate them, mirroring the
  current client. Likely surfaces as
  `ServiceContext::request(method, params) -> impl Future<Result>`.
  v1 of `ServiceContext` reserves the method name but does not ship it.
- **Typed notification dispatch on the client.** A
  `NotificationRouter` analogous to `MethodRouter`.
- **Connection-scoped `ServerHandle`** for emitting notifications
  outside any handler (heartbeat, broadcast, eventbus push).

## Acceptance criteria

- `fittings/examples/mcp-server/src/mcp.rs::serve_stdio` is deleted in
  favour of `Server::new(service, transport).with_cancellation_method(
  "notifications/cancelled").serve()`.
- A new `mcp-server` test exercises a tool that emits two
  progress notifications and is then cancelled mid-flight; the test
  observes the notifications, the cancellation, and the absence of a
  response frame for the cancelled call.
- Rafaello can implement a streaming-tokens handler with no custom
  serve loop.
