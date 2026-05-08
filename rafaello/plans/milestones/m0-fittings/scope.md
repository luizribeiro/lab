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
  cancellation).
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

- **W1.** `Request.id: Option<JsonRpcId>` distinguishes missing
  `id` field from explicit JSON `null`:
  - Missing `id` ⇒ `Request.id = None` ⇒ notification, no response.
  - Explicit `"id": null` ⇒ `Request.id = Some(JsonRpcId::Null)` ⇒
    request, enters `in_flight` keyed on `JsonRpcId::Null`, returns
    a response with `"id": null`. A second concurrent inbound
    `"id": null` request is rejected as a protocol-error duplicate
    per `rfc-fittings-notifications.md:137-145`.
  - `Response.id: JsonRpcId` (always present; for the explicit-null
    request path the response carries `JsonRpcId::Null`).
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
    JSON-RPC notification on the same connection. Returns local
    enqueue/encoding/channel-closed status only (channel-closed maps
    to `FittingsError::Transport`); **does not** prove peer delivery.
    Peer-gone is observed asynchronously via `peer.closed()` and
    pending-call drain (S3, K1) — never via a `Cancelled` from
    `notify`.
  - `peer() -> &PeerHandle` (or equivalent field) — connection-scoped
    handle so handlers can issue **outbound `peer.call`** requests
    back to the peer (not just notifications). Per
    `rfc-fittings-notifications.md:888-896`. Without this, `ctx.notify`
    only enables one direction of bidirectionality from inside a
    handler.
  - `cancelled() -> impl Future<Output = ()>` — resolves when the
    cancellation token fires.
  - `is_cancelled() -> bool` — non-blocking check.
  - `request_id() -> Option<&JsonRpcId>` — `None` for notification
    handlers, `Some(JsonRpcId::Null)` for explicit-null-id requests,
    `Some(_)` otherwise.
  Cheap to clone (`Arc`-shared inner state). For connection-scoped
  use **outside** any inbound handler (e.g. a startup task), callers
  obtain a `PeerHandle` via `Server::peer()` / `Client::peer()` (S1/K1).
- **C2.** `Service::call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError>`
  — breaking trait change. Old `call(&self, req: Request)` is gone.
- **C3.** `FittingsError::Cancelled { reason: Option<String> }` variant.
  No wire mapping (response is suppressed; see S6).
- **C4.** `Middleware::handle` receives `ServiceContext` (so
  middleware can also `notify` and observe cancellation).
- **C5.** `ServiceError.code: i32` validation accepts:
  - any positive code (`1..=i32::MAX`), and
  - the JSON-RPC server-defined band (`-32099..=-32000`), and
  - any negative code outside the reserved cluster
    (`-32768..=-32000`).
  Truly invalid (reserved or pre-defined-conflicting) codes serialise
  to `-32603 Internal` with `data: { "fittingsKind":
  "invalidServiceCode" }` per `rfc-fittings-errors.md:198-209`.
  Implementations MAY include extra diagnostic fields (e.g.
  `originalCode`); the acceptance test only requires
  `fittingsKind == "invalidServiceCode"` and tolerates additional
  diagnostic keys. If `originalCode` becomes a v1 hard requirement
  later, that's a Stream B RFC amendment, not a scope-only addition.

### Server layer — `fittings-server`

- **S1.** `Server::peer() -> PeerHandle` exposes the connection-scoped
  peer handle for use **outside any inbound handler** (e.g. tasks
  spawned at server startup that want to notify or call the peer
  proactively). Inside a handler, a `PeerHandle` is available via
  the existing `ServiceContext` plumbing — the same handle, just
  scoped to the handler's connection.
- **S2.** `PeerHandle::call(method, params).await -> Result<Value, FittingsError>`
  — server-initiated request to the peer, with response correlation.
  v1's only architectural consumer is core's renderer/peer call
  pattern (the renderer subprocess path itself is deferred to v2 per
  `decisions.md` row 29 — the *primitive* lands so it's available
  when v2 unblocks it). Acceptance criteria from
  `rfc-fittings-notifications.md:873-886`:
  - simultaneous calls in both directions on one fd correlate
    correctly via disjoint id namespaces (server-initiated and
    client-initiated; commits.md picks the prefix-or-shared-counter
    strategy);
  - dropping a `peer.call` future emits the configured cancellation
    notification and removes the pending slot in `pending_outbound`;
  - connection close resolves all pending outbound calls with
    `FittingsError::Transport` and resolves `peer.closed()`.
- **S3.** `PeerHandle::closed() -> impl Future<Output = ()>` — fires
  when the underlying transport tears down (graceful EOF or transport
  error). The authoritative signal that the peer is gone; `notify`
  intentionally does NOT report peer-gone synchronously (per
  `rfc-fittings-notifications.md:717-747`).
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
  a dedicated reader for the configured cancellation method fires
  the per-request token without competing for handler permits.
  Tested in S5's negative case (semaphore saturated, cancellation
  still observed).
- **S6.** Cancellation suppression has **two independent triggers**
  per `rfc-fittings-notifications.md:563-575` and `:591-593`:
  - Token-fired ⇒ response suppressed when the handler returns,
    regardless of the handler's return value.
  - Handler-returned `Err(FittingsError::Cancelled)` ⇒ response
    suppressed regardless of whether the token fired.
  Both paths are valid; handlers may "self-cancel" by returning
  `Err(Cancelled)` without the token firing (e.g. a deadline-exceeded
  case the handler decides on its own).

- **S7.** Cancellation method name and id-extractor are **configurable**
  on `Server`. Library defaults are LSP-style (`$/cancelRequest`,
  id field `id`) per `rfc-fittings-notifications.md:554-557` —
  fittings is transport- and protocol-agnostic, so the default
  cancellation shape is the LSP one. mcp-server explicitly
  configures the MCP convention (`notifications/cancelled`,
  id field `requestId`) at server-construction time. Tests cover
  both configurations where relevant.
- **S7.1.** Malformed cancellation payload (non-object, missing the
  configured id field, id-type mismatch — string id sent for a
  numeric in-flight key or vice versa) is logged at WARN and
  dropped. Does not kill the connection or affect other in-flight
  requests. Behaviour is identical regardless of which extractor
  (LSP `id` or MCP `requestId`) is configured.
- **S8.** Batch cancellation per `rfc-fittings-notifications.md:628-671`:
  the configured cancellation method references *individual request
  IDs*, not batch container IDs. A batched request whose component
  is cancelled has that component's response suppressed in the
  batch response; remaining components proceed. If every component
  in a batch is suppressed (cancelled or notification-only), no
  batch response is emitted at all.
- **S9.** The server's existing `Service` (passed to
  `Server::new(service, transport)`) **is** the inbound-request
  handler — there is no separate registration. A method the server's
  service does not implement returns `-32601 Method not found` per
  the existing dispatcher. The optional registration mechanism for
  inbound peer-originated requests lives only on the **client**
  side (K3) — the server already has its own `Service`, the client
  did not, hence the asymmetry.

### Client layer — `fittings-client`

- **K1.** `Client::peer() -> PeerHandle` symmetric to `Server::peer()`
  (S1). Exposes `notify`, `call`, and `closed` on the client side.
  (`call` already existed; `notify` and `closed` are new on this
  side.) Acceptance criteria mirror S2: dropped-future cancellation,
  close-drain, simultaneous bidirectional calls.
- **K2.** Inbound unsolicited *notifications* from server are
  dispatched to a registered handler `Fn(String, Value) + Send +
  Sync + 'static`, wrapped in `tokio::spawn(async move {
  handler(method, params); })` so handlers don't block the client
  read loop. **Synchronous** handler shape per
  `rfc-fittings-notifications.md:447-490`. If no handler registered,
  the notification is dropped silently. (m0 may *additionally* expose
  a `Client::dropped_notifications()` counter as an implementation
  convenience; this is an m0 addition, not an RFC requirement, and
  is not load-bearing for ratification.)
- **K3.** `Client::with_service(svc)` registers a service for
  inbound peer-originated *requests* from server (server-side
  `PeerHandle::call`). Without it, the client returns `-32601 Method
  not found`. Mirror of S9.

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
| `peerhandle_outside_handler.rs` | `Server::peer()` (S1) used by a startup task to call/notify the peer proactively, with no inbound request in flight. Mirror via `Client::peer()`. |
| `service_context_peer_call.rs` | Handler receives an inbound request, calls `ctx.peer().call(...)` to the peer while its own request is in flight, receives the peer response, and then returns its own response. Asserts that C1's in-handler peer access is load-bearing for bidirectionality, not just the outside-handler path. |
| `peerhandle_dropped_future_cancels.rs` | Dropping a `peer.call` future emits the configured cancellation notification on the wire and removes the slot in `pending_outbound`. |
| `peerhandle_close_drain.rs` | Closing the underlying transport resolves all pending `peer.call` futures with `FittingsError::Transport` and resolves `peer.closed()`. |
| `service_context_notify.rs` | Handler emits 5 notifications mid-request; client receives all 5 *before* the response. |
| `service_context_cancelled_by_token.rs` | Client sends `notifications/cancelled` for an in-flight request; handler observes `is_cancelled() == true`, returns `Err(Cancelled)`; client receives no response. |
| `service_context_cancelled_by_handler.rs` | Handler returns `Err(Cancelled)` *without* the cancellation token firing (e.g. handler-decided deadline); response is still suppressed. Asserts S6's two-independent-triggers rule. |
| `error_preservation_round_trip.rs` | **Table-driven across all five predefined codes** (`Parse`, `InvalidRequest`, `MethodNotFound`, `InvalidParams`, `Internal`): server returns `<variant> { message: "...", data: { ... } }`; client receives `message` and `data` byte-equal. |
| `error_marker_round_trip.rs` | `Transport` and `Panic` markers map to `-32603` outbound and decode back via the `fittingsKind` marker per `rfc-fittings-errors.md:223-262`. |
| `service_code_ranges.rs` | Valid positive (`42`), valid server-band (`-32050`), valid below-reserved (`-40000`) all round-trip without `invalidServiceCode` rewriting. |
| `id_null_explicit_request.rs` | Inbound `"id": null` request enters in-flight; handler runs; response `"id": null` returned. Distinct from a missing-id notification in the same connection. |
| `id_null_concurrent_rejected.rs` | A second concurrent `"id": null` inbound request is rejected per `rfc-fittings-notifications.md:137-145`. |
| `bounded_notify_drop.rs` | Handler floods notifications faster than transport flushes; bounded sink drops; `dropped_notifications()` increments; subsequent `peer.call` succeeds. |
| `cancellation_outside_semaphore.rs` | `with_max_in_flight(1)` saturated by a sleeping handler; second `notifications/cancelled` arrives; handler observes the token without waiting for a permit. |
| `batch_cancellation_partial_suppression.rs` | Send a 3-component batch; cancel one component mid-flight; only that response suppressed; remaining responses delivered. Plus a batch where every component is suppressed — no batch response emitted. |
| `id_namespace_isolation.rs` | Server-initiated and client-initiated `peer.call`s use disjoint id namespaces (commits.md picks the strategy). |

### Negative integration tests in `fittings/tests/`

| Test file | Asserts |
|-----------|---------|
| `malformed_cancellation.rs` | The configured cancellation method (LSP `$/cancelRequest` for the library default, MCP `notifications/cancelled` for the mcp-server configuration) with non-object params; missing the configured id field (`id` for LSP, `requestId` for MCP); id-type mismatch (string sent for numeric in-flight key, or vice versa). None kill the connection; all logged and dropped. Test runs against both extractor configurations. |
| `notification_handler_panic.rs` | Client-side notification handler panics on receipt; subsequent notifications still delivered; response correlation unaffected. |
| `inbound_request_no_service.rs` | Client receives a peer-originated request with no `with_service` registered; client returns `-32601`. (Server-side mirror is covered by the existing dispatcher tests for unknown methods on `Server::new(service, ...)`.) |
| `peer_gone_during_notify.rs` | Peer disconnects mid-stream of notifications; the dispatcher discovers the close, `peer.closed()` resolves, and pending `peer.call` futures resolve with `FittingsError::Transport`. **Does not** assert that a particular `ctx.notify` call synchronously fails — `notify` only reports local enqueue/encoding/channel-closed status (per `rfc-fittings-notifications.md:717-747`). |
| `invalid_service_code_marker.rs` | Handler returns `ServiceError { code: -32700 }` (a reserved predefined code, not a valid service code); outbound serialisation falls back to `-32603 Internal` with `data` containing `"fittingsKind": "invalidServiceCode"`. Test asserts the marker is present; additional diagnostic fields like `originalCode` are tolerated but not required (per `rfc-fittings-errors.md:198-209`). |

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
3. **Late notifications after `cancelled()` fires.** Handlers may
   still call `ctx.notify` after `cancelled()` resolves; per the RFC
   delivery contract, `notify` reports only local
   enqueue/encoding/channel-closed status (`FittingsError::Transport`
   on local-channel close). Peer-gone is observed asynchronously via
   `peer.closed()` and pending `peer.call` resolving with `Transport`,
   never via a `Cancelled` from `notify`. `peer_gone_during_notify.rs`
   asserts this contract.
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
granularity and surfaces an m0a/m0b split for owner approval as soon
as it becomes clear groups 1–3 cannot be kept independently green:

1. **Wire-layer ground work** (W1–W4, C5): `Request.id` /
   `Response.id` migration + error-data preservation +
   valid-code-range expansion + `invalidServiceCode` marker. No
   behavioural change for handlers; isolated; ~6–10 commits.
2. **`ServiceContext` + bounded notify + Service trait + macros**
   (C1–C4, S4, M1–M2, plus the connection-scoped accessors S1/K1):
   the new primitive plus the breaking trait and macro change. Lands
   together because everything compiles against the new shape;
   ~8–12 commits.
3. **Bidirectional `PeerHandle`** (S2–S3, S9, K1–K3 inbound services):
   server-initiated calls + client-side service registration +
   `peer.closed()` + dropped-future cancellation + close-drain;
   ~6–10 commits.
4. **Cancellation** (C3, S5–S7.1): cancellation token, two-trigger
   suppression rules, semaphore routing, malformed-payload handling,
   configurable extractor, batch cancellation; ~6–10 commits.
5. **`mcp-server` migration** (E1–E3): drops `serve_stdio` workaround,
   threads `ctx`, retains JS-SDK interop; ~3–5 commits.
6. **Transport regression + spawn verification** (T1, P1): tests
   only; ~2 commits.

Realistic total range after the round-1 review additions: **~30–50
commits, sequential**. The driver should surface an m0a (groups 1–2)
/ m0b (groups 3–6) split for owner approval **as soon as group 3
looks like it cannot land green on top of group 2 alone**, rather
than waiting for a fixed >40 threshold. The owner-ratification gate
for `commits.md` is the right place to make this call.

## Acceptance summary

m0 is done when:

- Every named test in the *Positive integration tests* and *Negative
  integration tests* matrices above is implemented and passes. Tests
  may split or merge during `commits.md` drafting as long as the
  named behaviours are all covered.
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
