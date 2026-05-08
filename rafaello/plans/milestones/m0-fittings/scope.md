# m0 — fittings v1 — scope

> **Status:** drafted by claude (orchestrator) for pi adversarial
> review. Pending owner ratification.

## Goal

Land the changes specified in the two ratified stream-B RFCs into
`fittings/` so that m1 onward can build on the new API surface. m0
is intentionally **self-contained**: no `rafaello-core` code lands;
the only out-of-tree consumer touched is
`fittings/examples/mcp-server`.

## Inputs

- `rafaello/plans/streams/b-fittings/rfc-fittings-notifications.md`
  (the bidirectional `PeerHandle`, `ServiceContext`, channel model,
  cancellation, server-originated requests v1 cut).
- `rafaello/plans/streams/b-fittings/rfc-fittings-errors.md`
  (predefined error preservation, `JsonRpcId` migration, code-range
  expansion, `Cancelled` variant, error-data round-trip).
- `rafaello/plans/streams/b-fittings/pi-review-1.md` and
  `pi-review-2.md` (round-2 must-fixes already folded into the RFCs).
- Current fittings source: `fittings/crates/{wire,core,server,client,transport,spawn,macros}/src/`
  and `fittings/examples/mcp-server/`.
- `rafaello/plans/decisions.md` rows 18, 22, 23 (load-bearing fittings
  calls).

## In scope

Items map to the RFCs. The driver picks `commits.md` granularity per
the *Internal split* section below.

### Wire layer — `fittings-wire`

- **W1.** `Request.id: Option<JsonRpcId>` (notifications carry
  `None`); `Response.id: JsonRpcId` (always present, never `Null` on
  successful responses). Define behaviour for inbound `id: null`
  per JSON-RPC 2.0 (treated as notification).
- **W2.** Predefined error variants (`Parse`, `InvalidRequest`,
  `MethodNotFound`, `InvalidParams`, `Internal`) carry
  `data: Option<Value>` and a typed `message: String`. Round-trip
  preservation is byte-equal on `data` and `message`.
- **W3.** Outbound `to_error_envelope` keeps `message` + `data` for
  predefined codes (the current implementation flattens to canonical
  strings — that is what changes).
- **W4.** Inbound `from_error_envelope` preserves `message` + `data`
  for predefined codes (currently discards).

### Core layer — `fittings-core`

- **C1.** `ServiceContext` struct accessible from handlers, exposing:
  - `notify(method, params) -> Result<(), FittingsError>` — fire-and-forget
    JSON-RPC notification on the same connection.
  - `cancelled() -> impl Future<Output = ()>` — resolves when the
    cancellation token fires.
  - `is_cancelled() -> bool` — non-blocking check.
  - `request_id() -> &JsonRpcId`.
  Cheap to clone (`Arc`-shared inner state).
- **C2.** `Service::call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError>`
  — breaking trait change. Old `call(&self, req: Request)` is gone.
- **C3.** `FittingsError::Cancelled { reason: Option<String> }` variant.
  No wire mapping (response is suppressed; see C7/C8).
- **C4.** `Middleware::handle` receives `ServiceContext` (so
  middleware can also `notify` and observe cancellation).
- **C5.** `ServiceError.code: i32` validation accepts the JSON-RPC
  server-defined range `(-32099)..=(-32000)` *in addition to* the
  existing `1..=999`. Out-of-range codes still serialise to `Internal`
  with `data: { original_code: <n> }` preserved.

### Server layer — `fittings-server`

- **S1.** Per-connection `PeerHandle` accessible from handlers via
  `ServiceContext::notify`. Handlers can fire-and-forget
  notifications mid-call.
- **S2.** `PeerHandle::call(method, params) -> Result<Value, FittingsError>`
  — server-initiated request to the peer, with response correlation.
  v1's only architectural consumer is core's renderer/peer call
  pattern (the renderer subprocess path itself is deferred to v2 per
  `decisions.md` row 29 — the *primitive* lands so it's available
  when v2 unblocks it).
- **S3.** `PeerHandle::closed() -> impl Future<Output = ()>` — fires
  when the underlying transport tears down (graceful EOF or transport
  error). Used by handlers to bail early.
- **S4.** Two-channel server loop:
  - **Response channel:** `mpsc::UnboundedSender<Vec<u8>>` for
    request responses (correctness > backpressure on the response
    side).
  - **Notification channel:** `mpsc::Sender<Vec<u8>>` with bounded
    capacity (default 1024), drop-on-full with a logged drop counter
    exposed at `Server::dropped_notifications()`.
  - The split prevents the dispatcher from blocking on the
    notification sink while responses are still in-flight.
- **S5.** Cancellation routed *outside* the request-worker semaphore:
  a dedicated `notifications/cancelled` reader fires the per-request
  token without competing for handler permits. Tested in S5's negative
  case (semaphore saturated, cancellation still observed).
- **S6.** Cancellation token semantics:
  - Token fires AND handler hasn't returned → response is suppressed.
  - Handler returns `Err(Cancelled)` AND token has fired → response
    is suppressed (idempotent).
  - Handler returns `Err(Cancelled)` AND token has NOT fired →
    dispatcher rejects with `FittingsError::Internal { data: { reason:
    "handler returned Cancelled without token firing" }}`. Handlers
    are not allowed to fake cancellation.
- **S7.** Malformed `notifications/cancelled` payload (non-object,
  missing `id`, id-type mismatch) is logged at WARN and dropped.
  Does not kill the connection or affect other in-flight requests.
- **S8.** Batch cancellation: `notifications/cancelled` references
  *individual request IDs*, not batch container IDs. A batched
  request whose component is cancelled has that component suppressed
  in the batch response; remaining components proceed.
- **S9.** Inbound peer-originated *request* (with `id`) when no
  inbound service is registered → `-32601 Method not found`.
  Registration goes via a new `Server::with_inbound_handler(svc)`
  builder. v1 doesn't ship a populated handler — the registration
  mechanism exists so v2 sampling/elicitation slot in cleanly.

### Client layer — `fittings-client`

- **K1.** Symmetric `PeerHandle` on the client: `notify` and `call`.
  (`call` was already there; `notify` is new.)
- **K2.** Inbound unsolicited *notifications* from server are
  dispatched to a registered async handler, run via `tokio::spawn`
  (not on the client read loop). If no handler registered, dropped
  with a counter exposed at `Client::dropped_notifications()`.
- **K3.** Inbound peer-originated *requests* from server (server-side
  `PeerHandle::call`) are dispatched to a `Service` registered via
  `Client::with_service(svc)`. If unregistered, `-32601`. Mirror of
  S9.

### Transport layer — `fittings-transport`

- **T1.** Already symmetric (stdio + tcp). No code changes expected.
  m0 commits.md should include a regression test confirming both
  transports carry bidirectional traffic without ordering bugs.

### Spawn layer — `fittings-spawn`

- **P1.** `SubprocessConnector` already wires the spawned child's
  stdio into a bidirectional `PeerHandle`; verify against the new
  shape and update its tests if needed. No public API change.

### Macros — `fittings-macros`

- **M1.** `#[fittings::method]` and `#[fittings::service]` generate
  handlers with the new `(self, ctx: ServiceContext, params: P)`
  signature.
- **M2.** Hard cut: macros do *not* support the old `(self, params: P)`
  shape. Downstream code recompiles against the new API. Compiler
  errors are the migration aid.

### Example — `fittings/examples/mcp-server`

- **E1.** Replace the workaround in `serve_stdio` (the custom loop
  draining `Vec<ServerNotification>` post-tool-call, plus the
  `Arc<Mutex<Vec<ServerNotification>>>` smuggled through
  `ToolCallContext`) with the new `ServiceContext::notify` API. The
  function shrinks substantially.
- **E2.** Existing `notifications/progress` and
  `notifications/cancelled` flows continue to work end-to-end through
  the new APIs.
- **E3.** JS-SDK interop (`scripts/check-with-mcp-sdk.mjs`) passes
  against the rebuilt server. Fittings-interop with the official MCP
  SDK is the canonical "the wire is right" check.

## Out of scope

- **Server-originated requests beyond `PeerHandle::call`.** Sampling,
  elicitation, structured progress prompts, and any other higher-level
  protocol layered on top: callers can build them with `peer.call`,
  but m0 doesn't define a v1 protocol for them. (`decisions.md` row 22
  explicitly carves this.)
- **Backwards compatibility with the pre-m0 `Service` trait shape.**
  m0 is a major version bump.
- **Anything rafaello-specific.** No `rafaello-core` code, no plugin
  manifest fields, no bus broker. Only consumer touched is
  `mcp-server`.
- **Streaming convenience helpers on top of `notify`.** No
  `Notifier::stream_chunks(...)`, no async-stream adapter. m1+ layers
  ergonomics on the primitive.
- **Reshaping the JSON-RPC error code allocation policy.** m0
  expands the *accepted* range; deciding how plugin authors should
  pick codes is a separate doc.
- **Patching Stream F manifest drift.** m1 retrospective territory.

## Demo bar

### Positive integration tests in `fittings/tests/`

| Test file | Exercises |
|-----------|-----------|
| `peerhandle_bidirectional.rs` | Server-initiated `peer.call` → client responds; client-initiated `peer.call` → server responds; both within one connection. |
| `service_context_notify.rs` | Handler emits 5 notifications mid-request; client receives all 5 *before* the response. |
| `service_context_cancelled.rs` | Client sends `notifications/cancelled` for an in-flight request; handler observes `is_cancelled() == true`, returns `Err(Cancelled)`; client receives no response. |
| `error_preservation_round_trip.rs` | Server returns `MethodNotFound { message: "...", data: { method: "foo" } }`; client receives both fields byte-equal. |
| `bounded_notify_drop.rs` | Handler floods notifications faster than transport flushes; bounded sink drops; `dropped_notifications()` increments; subsequent `peer.call` succeeds. |
| `cancellation_outside_semaphore.rs` | `with_max_in_flight(1)` saturated by a sleeping handler; second `notifications/cancelled` arrives; handler observes the token without waiting for a permit. |
| `id_namespace_isolation.rs` | Server-initiated and client-initiated `peer.call`s use disjoint id namespaces (commits.md picks the strategy). |

### Negative integration tests in `fittings/tests/`

| Test file | Asserts |
|-----------|---------|
| `malformed_cancellation.rs` | `notifications/cancelled` with non-object params; with no `id`; with id-type mismatch — none kill the connection; all logged and dropped. |
| `notification_handler_panic.rs` | Client-side notification handler panics on receipt; subsequent notifications still delivered; response correlation unaffected. |
| `cancelled_without_token.rs` | Handler returns `Err(Cancelled)` without the token having fired; dispatcher rejects with `Internal`. |
| `inbound_request_no_service.rs` | Client receives a peer-originated request with no `with_service` registered; client returns `-32601`. Mirror on server (S9) covers `with_inbound_handler` not registered. |
| `peer_gone_during_notify.rs` | Peer disconnects mid-stream of notifications; `ctx.notify` returns `Err`; `closed()` resolves; handler exits cleanly. |
| `id_null_treated_as_notification.rs` | Inbound request with `"id": null` is treated as a notification per JSON-RPC 2.0 (no response). |
| `out_of_range_code_preserves_original.rs` | Handler returns `ServiceError { code: 10_000 }`; outbound serialisation falls back to `-32603 Internal` with `data: { original_code: 10000 }`. |

### MCP example interop

- `cargo run -p mcp-server -- serve` exchanges:
  1. one `tools/call` that emits `notifications/progress` mid-call
     (handler uses `ctx.notify`);
  2. one `tools/call` that the client cancels via
     `notifications/cancelled` mid-flight; handler observes the token;
     response is suppressed.
- The `serve_stdio` workaround (custom loop draining notifications)
  is gone — replaced by `Server::serve(...)` plus handlers using
  `ctx`.
- `cd fittings/examples/mcp-server && npm run check:real-client`
  passes against the rebuilt server.

### Manual validation in `manual-validation.md`

The driver runs and captures:
- `cargo run -p mcp-server -- serve` from one tmux pane and the JS
  interop driver from another; observes one progress notification +
  one cancelled call as described above.
- The full m0 negative-test suite (no test hangs the harness past
  30s).
- `cargo build -p fittings` clean — no `target/pre-commit` artefacts
  in git.
- `nix develop .#fittings --command cargo test --workspace` green
  on Linux + macOS (CI rerun is sufficient evidence for the macOS
  side).

## Risks

1. **Macro signature change ripples wide.** Every existing
   `#[fittings::service]` consumer must add `_ctx: ServiceContext`.
   The `mcp-server` example is the only one whose handlers actually
   exchange notifications/cancellation; the rest get the trivial
   compile-error fixup.
2. **`PeerHandle::call` server-side correlation under contention.**
   The dispatcher must correlate response IDs to in-flight
   server-initiated requests without colliding with client-initiated
   request IDs. `commits.md` picks one strategy (likely a string
   prefix `s_` / `c_` at the wire layer, or a shared atomic counter
   with disjoint ranges) and documents the choice.
3. **Cancellation race vs `ctx.notify`.** After `cancelled()` fires,
   may the handler still call `notify`? Decision (subject to pi
   review): yes, but the call can return `Err(Cancelled)` if the
   transport has already torn down. `peer_gone_during_notify.rs`
   covers the case.
4. **JS-SDK interop.** The MCP SDK has its own expectations; if the
   new `data` preservation breaks any string-equality tests on the SDK
   side, that's a real fix. m0 retrospective surfaces if this
   happens.
5. **Hard breaking change to a published library.** `fittings`
   doesn't yet have external consumers we're aware of, but the
   m0 retrospective should confirm before m1 starts.

## Internal split (driver guidance for `commits.md`)

Per `milestones/README.md`, m0 may split internally by RFC area.
Suggested grouping for `commits.md`; the driver picks final
granularity:

1. **Wire-layer ground work** (W1–W4, C5): `JsonRpcId` migration +
   error-data preservation + code-range expansion. No behavioural
   change for handlers; isolated; ~5–8 commits.
2. **`ServiceContext` + bounded notify + Service trait + macros**
   (C1–C4, S1, S4, M1–M2): the new primitive plus the breaking trait
   and macro change. Lands together because everything compiles
   against the new shape; ~6–10 commits.
3. **Bidirectional `PeerHandle`** (S2–S3, K1, K3, S9): server-initiated
   calls + client-side service registration; ~4–6 commits.
4. **Cancellation** (C3, S5–S8, K2): cancellation token, suppression
   rules, semaphore routing, malformed-payload handling, batch
   cancellation. Ties everything together; ~5–8 commits.
5. **`mcp-server` migration** (E1–E3): drops `serve_stdio` workaround,
   threads `ctx`, retains JS-SDK interop; ~3–5 commits.
6. **Transport regression + spawn verification** (T1, P1): tests
   only; ~2 commits.

Total range: ~25–40 commits, sequential. If `commits.md` finds the
total >40 commits, m0 splits into m0a (groups 1–2) and m0b (groups
3–6). The driver decides during `commits.md` drafting and surfaces
the call to the owner before pi review.

## Acceptance summary

m0 is done when:

- All 7 positive integration tests pass.
- All 7 negative integration tests pass.
- `cargo test -p fittings -p fittings-{wire,core,server,client,transport,spawn,macros,testkit} -p mcp-server`
  is green on Linux + macOS.
- The JS-SDK interop check passes against the rebuilt server.
- `mcp-server/src/serve_stdio` no longer contains the manual
  notification-draining loop.
- `manual-validation.md` records the items in the *Manual validation*
  section above.
- `retrospective.md` is written, with any drift surfaced during
  implementation landing in `overview.md` / `decisions.md` / stream
  RFCs as deltas.
