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
  Designed-around, not designed-in. See "Future work".
- Per-handler back-pressure on notification volume.
- Ordering guarantees stronger than "notifications observed by the
  framework in order N are written to the transport in order N".

## Today

```rust
#[async_trait]
pub trait Service: Send + Sync {
    async fn call(&self, req: Request) -> Result<Response, FittingsError>;
}
```

`Server::serve` owns a private `mpsc::UnboundedSender<Vec<u8>>` that
only its workers reach (`server/src/server.rs:49`). One worker produces
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

Internally `ServiceContextInner` holds:

- `notifier: NotificationSink` — an `mpsc::UnboundedSender<Vec<u8>>`
  pointing at the same outbound channel `Server::serve` already drains.
- `cancel: tokio_util::sync::CancellationToken` (or our own equivalent
  if we're avoiding the dependency).
- `request_id: Option<JsonRpcId>`.

`notify` builds a `RequestEnvelope::notification`, encodes it once, and
pushes it onto the outbound channel. Errors:

- `Err(FittingsError::transport(...))` if the channel is closed
  (peer gone). Handlers can ignore-or-bail at their discretion.
- `Err(FittingsError::internal(...))` if encoding fails (params not
  serialisable). This should be rare; treating it as internal keeps
  the API total without a separate error type.

### 3. `Server::serve` plumbing

The existing outbound `mpsc::UnboundedSender<Vec<u8>>` becomes the
shared `NotificationSink`. Per-request worker construction now:

1. Builds a `ServiceContext` carrying:
   - a clone of the response sender (for notifications),
   - a fresh `CancellationToken`,
   - the request's `JsonRpcId`.
2. Stores the cancellation handle in an `in_flight: HashMap<JsonRpcId,
   CancellationToken>` shared with the receive arm of the select.
3. Calls `service.call(req, ctx).await` inside `catch_unwind`.
4. On worker completion, removes from `in_flight` and sends the
   response frame.

The receive arm gains one extra branch: when an inbound notification
arrives whose method matches a configured **cancellation method**
(default: `"$/cancelRequest"`, MCP override `"notifications/cancelled"`),
look up the `id` in `in_flight` and trigger the token.

```rust
// Server::new keeps its current shape; cancellation is configurable:
Server::new(service, transport)
    .with_cancellation_method("notifications/cancelled")
    .with_cancellation_id_extractor(|params| /* fn returning JsonRpcId */)
```

A reasonable default extractor reads `params.id` (LSP-ish) and a
named extractor reads `params.requestId` (MCP-ish). Either way, the
runtime owns the wiring — handlers just `select!` on `ctx.cancelled()`.

### 4. `Service` blanket impls and migration

The current single-arg form is used by `fn`-style handlers in
`fittings-macros` and by all current consumers. Migration:

- Update `Service` itself.
- Update `RouterService` / `MethodRouter::route` to take a context
  too (`route(method, params, metadata, ctx) -> ...`).
- The fittings-macros expansion gets a per-method choice: handlers
  that don't need it accept `_ctx: ServiceContext`; handlers that do
  declare it explicitly. We can do this with a function-arity check
  in the macro, but the simpler path is: **always** pass `ctx`, and
  let the user prefix with `_` if unused.
- Provide a `From<Request> for (Request, ServiceContext::detached())`
  helper for tests, plus a `ServiceContext::detached()` constructor
  that drops notifications and is never cancelled. This keeps
  `Service` directly testable without spinning up a server.

Existing `Service` impls in tests will need a one-line signature
change. There are roughly six in-tree impls; that's acceptable churn.

### 5. Notification handlers (inbound, server-side)

JSON-RPC 2.0 says servers MUST NOT respond to inbound notifications.
The current server already drops the response when `request.id` is
`None` (`server.rs:248`). With `ServiceContext`, an inbound
notification still goes through `Service::call`, but `request_id`
is `None` and any returned `Response.result` is dropped. Errors
returned from the handler are logged (the framework already
discards them today; we can keep that behaviour and emit a `tracing`
event). Cancellation does not apply (no id to cancel against).

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
impl<C> Client<C> { ... }

impl<C> Client<C> {
    pub fn on_notification<F>(&mut self, handler: F)
    where
        F: Fn(&str, Value) + Send + Sync + 'static;
}
```

The handler runs on the client loop; if a consumer wants async work,
it spawns. v1 keeps it simple — no per-method routing in the client,
no typed dispatch. Rafaello layers its own router on top.

This is the smallest change that unblocks bidirectional notification
flow. Server-originated *requests* are deferred (see Future work).

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
2. **Backpressure.** If a handler emits 10k progress events while the
   transport is slow, the unbounded mpsc grows without bound. Should
   `notify` become `async fn notify` with a bounded channel? Probably
   yes for v1. Default cap of 1024 with `try_send` semantics on
   overflow (drop with a `tracing::warn!` and a metric, since
   notifications are by definition lossy).
3. **Per-request vs per-connection context.** Does the bus event-stream
   case want a context that survives a single request? Probably
   handled by the connection-scoped notify sink (a separate
   `ServerHandle::notify` that doesn't need a request id) — out of
   scope for v1, but the `ServiceContext` design must not foreclose it.
4. **Cancellation protocol naming.** Bake in MCP's
   `notifications/cancelled` as the default? Or LSP's `$/cancelRequest`?
   Keep it configurable, default to LSP since fittings markets itself
   as transport-agnostic.

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
