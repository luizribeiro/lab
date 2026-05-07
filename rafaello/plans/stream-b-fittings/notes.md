# notes — stream-b-fittings

Running findings from auditing fittings as it stands today.

## Where things live

- `fittings-wire` — JSON-RPC envelopes, codec, error-code mapping. Already
  models notifications correctly: `RequestEnvelope::notification` produces
  an id-less envelope, and the decoder accepts inbound id-less requests.
- `fittings-core` — `Service`, `Transport`, `Middleware`, `FittingsError`,
  `Request`/`Response`/`ServiceError` types. This is the abstraction layer
  every other crate plugs into.
- `fittings-server` — owns the per-connection serve loop. `Server::serve`
  reads frames, spawns workers under a semaphore, and writes responses.
  This is where outbound notifications would have to land but currently
  cannot.
- `fittings-client` — owns a client serve loop with pending-call
  correlation. Does *not* listen for inbound server-pushed notifications.
- `fittings-transport` — stdio + tcp framing. Symmetric, no awareness of
  request/response direction.
- `fittings/examples/mcp-server` — already needed notifications and
  cancellation, and worked around the missing API by bypassing
  `Server::serve` and writing a custom loop. This is the most useful piece
  of evidence we have about the gap.

## Outbound notifications — what's actually missing

The `Service` trait is `async fn call(Request) -> Result<Response, _>`.
It returns at most one response. There is no second channel for the
handler to push frames at the peer.

`Server::handle_frame` builds exactly one `ResponseEnvelope` per inbound
request and sends it over the response mpsc. The mpsc is private to
`serve` — handlers don't see it.

**Concrete proof this hurts:** `mcp-server/src/mcp.rs::serve_stdio`
re-implements the entire serve loop (lines 587–705) so it can:

1. Drain a buffered `Vec<ServerNotification>` from the service after
   every request, and on a 25 ms tick for in-flight requests
   (`send_pending_notifications`).
2. Hand each tool handler a `ToolCallContext` that owns
   `Arc<Mutex<Vec<ServerNotification>>>` and a `CancellationToken`, then
   read those out-of-band.
3. Recognise `notifications/cancelled` inbound and cancel the matching
   in-flight worker.

So today, *any* fittings consumer that wants notifications has to ditch
`Server::serve` and re-write the loop. Rafaello will need streaming
tokens, tool progress, and event-bus delivery — three independent
streams of notifications. That is unworkable on top of the current API.

## Cancellation — also missing

There is no cancellation signal on `Request`. `Service::call` returns a
plain future. `Server::serve` will keep a worker running even after the
peer has gone away or sent `notifications/cancelled`. The MCP example
papers over this with its own `CancellationToken`. Cancellation belongs
in the same context object as `notify`, since both are
per-request-handler facilities.

## Server→client requests (sampling/elicitation)

A server-side handler may want to *ask* the client a question
(MCP sampling, elicitation, future rafaello human-in-the-loop). That is
a request the server originates and a response it consumes. The current
client-loop assumes it is the only originator of requests; the server
loop never reads response frames at all (it only sends them). Adding
this is bigger than v1 of notifications and should be deferred — but
the v1 design of `ServiceContext` should not box itself out of it.

## Error handling — what I see

### What works
- JSON-RPC pre-defined codes (-32700, -32600, -32601, -32602, -32603)
  are mapped both directions in `fittings-wire/src/error_map.rs`.
- `Server::execute_request` wraps handler futures in `catch_unwind` and
  produces an internal-error response on panic. The response loop then
  drains in-flight before returning.
- `Server::serve` distinguishes graceful EOF from a transport error and
  drains in-flight work before returning in the EOF case.
- A failed `transport.send` for a response frame causes a global
  shutdown (workers aborted, Err returned). Reasonable.

### What's wrong or under-designed

1. **`from_error_envelope` discards `message` and `data`** for every
   well-known code. A method-not-found error reaches the caller as
   `MethodNotFound("Method not found")` — the actual method name from
   `data` and the human message are gone. This destroys debugging
   information for plugin authors. See `error_map.rs:65–96`.

2. **The `ServiceError` code range (1..=999) excludes the JSON-RPC
   server-defined range (-32000..-32099).** Plugins can't return any of
   the codes the JSON-RPC spec actually reserves for application use.
   And any out-of-range code silently becomes `-32603 Internal error`
   with `data` dropped. See `error_map.rs:45–58` + `message.rs:39–48`.

3. **`Transport` and `Internal` collapse to the same wire code** with
   no `data`. A plugin author can't tell a transport-level failure on
   the *peer* from a logic bug, because both arrive as
   `FittingsError::Internal("Internal error")`.

4. **Panics → `Internal("request handler panicked")` only on the
   server side.** The literal panic message is dropped. There is no
   structured marker (e.g. `data: {"kind":"panic"}`) the client could
   key off.

5. **Decode errors lose context.** `WireDecodeError::Parse(message)`
   becomes `parse_error(message)` server-side — fine — but on the
   client side `map_response_decode_error` flattens it to a
   hard-coded string, losing the original.

6. **No mapping from arbitrary `std::error::Error` to FittingsError.**
   Plugin authors today either match-and-rewrap manually for every
   call site, or `FittingsError::internal(format!("{e}"))` and lose the
   chain. There is no `?`-friendly bridge, no `From` impl story.

7. **Middleware exists but no composition or ordering rules.** A
   `Middleware::handle` is defined, but there is no `ServiceStack`,
   no documented contract for whether middleware sees errors before or
   after panic-catching, and no story for emitting notifications from
   middleware (logging, tracing).

8. **No request-level error metadata.** `ServiceError::data` is the
   only place to put structured info; there is no attached cause chain
   or backtrace path. For rafaello a tool failure needs to surface
   "which tool, with what args, failed how".

## Smaller things worth recording

- `Request.id` is forced through `JsonRpcId::Display`, so a numeric or
  null id arrives at the handler as a string. Round-trips work because
  `Server::execute_request` reuses the original `JsonRpcId` from the
  envelope, not the stringified form. But handlers can't distinguish
  numeric vs string ids — fine for now, worth flagging.
- `Request.metadata` is intentionally not serialised (`message.rs:13`).
  Good. We can use it for context plumbing without polluting the wire.
- The client loop drops inbound frames when `pending` is empty
  (`client/src/lib.rs:150` — `recv_result = transport.recv(), if
  !pending.is_empty()`). That means a client today *cannot* receive
  unsolicited notifications from a server even if the server learned
  to send them. Both ends need work for inbound-on-client to function.
