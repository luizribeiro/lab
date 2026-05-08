# m0-fittings — commits

> **Status:** converged after three pi review rounds
> (`commits-pi-review-1.md`, `commits-pi-review-2.md`,
> `commits-pi-review-3.md`). Pi recommended ratification after
> the round-3 traceability fixes landed. Pending owner ratification
> at commits granularity, after which Phase 3 per-commit agent work
> begins on `rafaello-v0.1`.

Ordered commit list for m0, derived from `scope.md`. Each commit is
one logical idea **and leaves the workspace green** — pre-commit
hooks (rustfmt + clippy + test suites) gate every commit; intermediate
non-green states are not allowed. Commits land sequentially on
per-commit branches `agents/m0/c<NN>` rebased onto `rafaello-v0.1`,
no merge commits, no force pushes. Tests land with the code that
exercises them per `~/.claude/CLAUDE.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes: `fittings-wire`,
  `fittings-core`, `fittings-server`, `fittings-client`,
  `fittings-macros`, `fittings-spawn`, `fittings-transport`,
  `mcp-server`, `fittings` (cross-crate).
- "Acceptance" lists new tests + the pre-commit invariants the commit
  must keep green.
- "Depends on" cites the *lowest* commit number whose code or types
  this commit references. A commit only lands after every declared
  dependency has landed on `rafaello-v0.1`.
- Test files live under `fittings/tests/` unless otherwise noted.

## m0a / m0b split point

Pi round-1 pointed out the previous draft's split at c19 wasn't a
clean stopping point. Two options:

- **Default**: ship m0 as one milestone (no internal split).
- **If the driver needs to ship a partial v0.1 of fittings before
  Group 3 is done**: the cleanest stopping point is **after c10**
  (the API/notify cutover is green, no bidirectional `peer.call`
  yet). m0b would then cover bidirectional calls + cancellation +
  mcp-server migration + transport/spawn. This sub-milestone has
  no external consumer in v1 — fittings v0.x at c10 is only useful
  as a checkpoint for review.

Driver decides during implementation. Default: no split.

---

## Group 1 — Wire / error-shape ground work (W1–W4, C5)

### c01 — feat(fittings-core): Request.id Option<JsonRpcId>, Response.id JsonRpcId

- **What.** `RequestEnvelope.id` in `fittings-wire` is already
  `Option<JsonRpcId>`; the lossy bit is `fittings-core::message::Request`
  (currently `id: String`). Migrate the core `Request.id` to
  `Option<JsonRpcId>` and `Response.id` to `JsonRpcId`. Update the
  dispatcher's wire→core conversion to preserve the distinction:
  missing wire id → `None` (notification); wire `id: null` →
  `Some(JsonRpcId::Null)` (request); wire `id: <T>` →
  `Some(<T>)`.
- **Why.** scope §W1; rfc-fittings-notifications.md:124-145.
- **Depends on.** baseline.
- **Acceptance.** `tests/core_request_id_shape.rs` covers the three
  decode paths. Existing dispatcher tests still pass with id-shape
  fixups.

### c02 — feat(fittings-core): FittingsError predefined variants gain data field + Panic variant

- **What.** `FittingsError::{Parse, InvalidRequest, MethodNotFound,
  InvalidParams, Internal}` gain `data: Option<Value>` and a typed
  `message: String`. New `FittingsError::Panic { message: String }`
  variant for handler-panic propagation; outbound mapping in c04
  emits `data.fittingsKind = "panic"`. **Existing single-argument
  constructors (`FittingsError::method_not_found(msg)`, etc.) are
  preserved and set `data: None`** so the cutover doesn't churn
  call sites that don't have a payload to attach; data-bearing
  constructors are added alongside (`method_not_found_with_data`)
  or callers may directly construct the variant.
- **Why.** scope §W2; rfc-fittings-errors.md:192-209. Pi review-1
  finding 7: this is `fittings-core` work, not `fittings-wire`.
  Pi review-2 finding 4: `Panic` variant must be explicit so c04's
  marker round-trip can prove it. Pi review-2 finding 8: keep
  one-arg constructors compatible.
- **Depends on.** baseline.
- **Acceptance.** `tests/core_predefined_error_data.rs` covers the
  five predefined variants table-driven (construction + read of
  `data` and `message`) and the new `Panic { message }` variant
  (construction + read of `message`; `Panic` itself has no `data`
  field — its wire-side `fittingsKind = "panic"` marker is produced
  by the codec in c04). Existing one-arg constructor call sites
  keep compiling unchanged.

### c03 — feat(fittings-wire): error_map preserves data field on outbound encode

- **What.** Update `fittings-wire::error_map::to_error_envelope` to
  carry `data` and the typed `message` for predefined codes (current
  impl flattens to canonical strings).
- **Why.** scope §W3.
- **Depends on.** c02.
- **Acceptance.** `tests/wire_outbound_error_round_trip.rs`
  table-driven across the five variants asserts `data` byte-equal
  + `message` preserved through `to_error_envelope`.

### c04 — feat(fittings-wire): error_map preserves data field on inbound decode + Transport/Panic markers

- **What.** Update `fittings-wire::error_map::from_error_envelope`
  (the inbound mapping; pi review-2 minor note caught the wrong
  function name in the previous draft) to carry `data` and `message`
  back for predefined codes. Plus `Transport` / `Panic` marker
  encoding+decoding per rfc-fittings-errors.md:223-262: outbound
  emits `data.fittingsKind = "transport"` for Transport,
  `data.fittingsKind = "panic"` for Panic; inbound decodes the
  marker back to the typed variant. (The actual dispatcher mapping
  of handler panics to `FittingsError::Panic` lives in c08.)
- **Why.** scope §W4 + marker preservation.
- **Depends on.** c02, c03.
- **Acceptance.** Two test files implementing the scope demo bar:
  - `tests/wire_inbound_error_round_trip.rs` covers the five
    predefined variants byte-equal round-trip through
    `to_error_envelope` → `from_error_envelope`.
  - `tests/error_marker_round_trip.rs` (the scope-named test)
    covers the Transport + Panic marker round-trip; constructs
    `FittingsError::Panic` and `FittingsError::Transport` directly
    (no dispatcher needed yet).

### c05 — feat(fittings-core): widen ServiceError code validation + invalidServiceCode marker

- **What.** Replace `1..=999` validator with: any positive code
  (`1..=i32::MAX`); the JSON-RPC server band (`-32099..=-32000`);
  any negative code outside the reserved cluster (`-32768..=-32000`)
  — including above-reserved-negative (e.g. `-31999`, `-1`). Truly
  invalid codes (reserved cluster, conflicting with predefined,
  `0`) serialise to `-32603 Internal` with
  `data.fittingsKind = "invalidServiceCode"` per rfc-fittings-errors.md:198-209.
- **Why.** scope §C5. Pi review-1 finding 8: also exercise
  above-reserved-negative band and code `0` as invalid.
- **Depends on.** c02.
- **Acceptance.** `tests/service_code_ranges.rs` covers valid
  positive (`42`), valid server-band (`-32050`), valid below-reserved
  (`-40000`), valid above-reserved-negative (`-31999`).
  `tests/invalid_service_code_marker.rs` covers code `0`, code
  `-32700` (reserved), and a code conflicting with a predefined
  variant; all produce `fittingsKind == "invalidServiceCode"` and
  tolerate (but don't require) extra diagnostic fields.

---

## Group 2 — `ServiceContext` + bounded notify + Service trait + macros (C1–C4, S1, S4, M1–M2)

### c06 — feat(fittings-core): introduce FittingsError::Cancelled variant

- **What.** Add `FittingsError::Cancelled { reason: Option<String> }`.
  No wire mapping. Will become a handler-returned response-suppression
  trigger once the server implements S6 (c22).
- **Why.** scope §C3.
- **Depends on.** baseline.
- **Acceptance.** Variant compiles; existing error-mapping unit
  tests still pass; new unit test confirms it has no wire code
  mapping.

### c07 — feat(fittings-core): introduce ServiceContext + PeerHandle types (with cancellation token)

- **What.** Add `ServiceContext` and `PeerHandle` types in
  `fittings-core`. Definition only (callers wired in c08+):
  - `ServiceContext`: `notify(method, params)`, `cancelled()`,
    `is_cancelled()`, `request_id() -> Option<&JsonRpcId>`,
    `peer() -> &PeerHandle`. Cheap to clone (`Arc`-shared inner
    state). Per-request cancellation token implemented here via
    `tokio_util::sync::CancellationToken`.
  - `PeerHandle`: opaque handle exposing `notify` for now (`call`
    and `closed` lit up in Group 3). Its inner channel is wired in
    c08.
- **Why.** scope §C1. Pi review-1 finding 6: cancellation token impl
  lives here, not duplicated later.
- **Depends on.** c06.
- **Acceptance.** Unit tests for `cancelled()`/`is_cancelled()`
  semantics on a freestanding token; `request_id()` returns the
  configured value; cloning preserves shared state.

### c08 — feat(fittings): API cutover — Service trait, Middleware, macros, server dispatcher, all examples

- **What.** Single workspace-wide cutover commit. All breaking changes
  land together so the workspace stays green per the conventions
  above. Includes:
  - `fittings-core::Service::call(&self, req: Request, ctx: ServiceContext)
    -> Result<Response, FittingsError>` (was: `call(&self, req: Request)`).
  - `fittings-core::Middleware::handle` accepts `ServiceContext`
    and threads it to the inner handler.
  - `fittings-server` dispatcher constructs a `ServiceContext`
    per request: populates `request_id`, allocates a per-request
    cancellation token, wires `notify` to a basic notification
    channel (refined to bounded-with-drop in c09).
  - `fittings-macros` `#[fittings::method]` and
    `#[fittings::service]` generate `(self, ctx: ServiceContext, params: P)`.
    Old shape removed.
  - `fittings/examples/{hello-api,hello-service,hello-client}` and
    `fittings/examples/mcp-server` migrated to the new signatures.
    `mcp-server` participates in the cutover because it is a macro
    consumer; its existing `serve_stdio` workaround (the custom
    notification-draining loop and `ToolCallContext` shim) is
    *retained* here — that workaround drops in c26, separately.
  - **Server panic mapping**: the dispatcher's `catch_unwind` path
    is updated to map worker panics to `FittingsError::Panic { message }`
    (carrying the panic's payload string when extractable) instead
    of the previous flat `Internal("request handler panicked")`.
    This proves the c04 marker round-trip end-to-end.
- **Why.** scope §C1, §C2, §C4, §M1, §M2. Pi review-1 findings 1
  and 4: this is the only way to keep the workspace green per
  commit, and `mcp-server` must be in the cutover (not deferred).
  Pi review-2 finding 4: server panic mapping must be explicit and
  land here (the dispatcher cutover) so c04's marker test works
  end-to-end.
- **Depends on.** c01, c02, c07.
- **Acceptance.** `cargo test --workspace` from `fittings/` is
  green. `npm run check:real-client` from
  `fittings/examples/mcp-server` still passes (the wire shape is
  the same; only the in-process API moved). New
  `tests/service_context_basic.rs` asserts a handler can read its
  `ctx.request_id()` and `ctx.is_cancelled() == false`. New
  `tests/error_preservation_round_trip.rs` (the scope demo-bar
  test) table-driven across all five predefined variants: server
  returns `<variant> { message, data }`; client receives both
  byte-equal via the full dispatcher path. New
  `tests/handler_panic_maps_to_panic.rs` asserts a panicking
  handler surfaces as `FittingsError::Panic` on the client side
  via the `fittingsKind = "panic"` marker.

### c09 — feat(fittings-server): two-channel server loop (response unbounded, notification bounded with drop counter)

- **What.** Refine c08's basic notification channel into the
  scope-specified two-channel shape. Response channel:
  `mpsc::UnboundedSender<Vec<u8>>` (correctness > backpressure).
  Notification channel: `mpsc::Sender<Vec<u8>>` with bounded
  capacity (default 1024, configurable on `Server`), drop-on-full
  with a `Server::dropped_notifications()` counter. The split
  prevents the dispatcher from blocking on the notification sink
  while responses are still in-flight.
- **Why.** scope §S4.
- **Depends on.** c08.
- **Acceptance.**
  - `tests/service_context_notify.rs` (the scope demo-bar test):
    handler emits 5 notifications mid-request; client receives
    all 5 *before* the response. Asserts ordering via the
    non-drop path.
  - `tests/bounded_notify_drop.rs`: handler floods notifications
    faster than the transport flushes; bounded sink drops;
    counter increments; subsequent ordinary request/response
    traffic still succeeds (asserts the S4 contract: response
    channel never blocks on the notification sink). Bidirectional
    `peer.call` traffic post-flood is exercised in c14/c30 once
    the API exists.

### c10 — feat(fittings-server,fittings-client): Server::peer() / Client::peer() accessors expose PeerHandle (notify only)

- **What.** Add `Server::peer() -> PeerHandle` and `Client::peer() ->
  PeerHandle` accessors. `PeerHandle` wraps the connection-scoped
  notification sink so callers outside any inbound handler (e.g.
  startup tasks) can `peer.notify(...)`. `peer.call` and
  `peer.closed()` light up in Group 3.
- **Why.** scope §S1, §K1 (notify portion).
- **Depends on.** c09.
- **Acceptance.** `tests/peerhandle_outside_handler.rs` part one:
  a startup task on the server side calls `Server::peer().notify(...)`
  and a raw test-harness client (reading frames directly off the
  transport, not via `Client`) observes the notification. The
  client-side public `Client::peer().notify(...)` plus client-side
  observation of server-originated notifications via the registered
  notification handler is added in c19 (which is when K2's handler
  registration lands).

---

## Group 3 — Bidirectional `PeerHandle` + id-null semantics (S2–S3, S9, K2, K3)

### c11 — feat(fittings-core): id-namespace strategy for outbound peer.call (standalone allocator)

- **What.** A **standalone** `IdAllocator` module in `fittings-core`
  (no PeerHandle integration yet — that's c12 server-side and c14
  client-side). Both directions use a string prefix encoding
  (`s_<n>` / `c_<n>`) backed by an `AtomicU64` counter per direction.
  Document the invariant in the module doc comment: generated
  outbound ids cannot collide with the opposite side's generated
  namespace; duplicate inbound ids from a peer are still possible
  and are handled at the in-flight map (a separate concern).
- **Why.** scope §S2 acceptance + risk #2. Pi review-2 open-item
  answer: standalone allocator depends on c01 only; integration
  with PeerHandle internals lands in c12/c14.
- **Depends on.** c01.
- **Acceptance.** Unit tests: `IdAllocator::next()` returns
  monotonically increasing prefixed ids; allocators with different
  prefixes cannot produce colliding ids.

### c12 — feat(fittings-server): PeerHandle::call (server-initiated)

- **What.** Implement `PeerHandle::call(method, params).await ->
  Result<Value, FittingsError>` on the server side. Maintains a
  `pending_outbound` map keyed by the prefixed id (c11); resolves
  on matching response. Inbound responses to those ids are routed
  to `pending_outbound` instead of being dispatched as requests.
- **Why.** scope §S2.
- **Depends on.** c10, c11.
- **Acceptance.** `tests/peerhandle_server_initiated_call.rs`
  uses a hand-rolled echo client that recognises `s_<n>` ids and
  responds; server's `peer.call` resolves with the result.

### c13 — feat(fittings-client): Client::with_service for inbound peer-originated requests

- **What.** `Client::with_service(svc)` registers a `Service` that
  handles inbound peer-originated requests (server using
  `PeerHandle::call`). Without it, the client returns
  `-32601 Method not found`.
- **Why.** scope §K3.
- **Depends on.** c08.
- **Acceptance.** `tests/inbound_request_no_service.rs`: client
  with no `with_service` returns `-32601`.
  `tests/inbound_request_with_service.rs`: client with `with_service`
  routes the request to the registered handler.

### c14 — feat(fittings-client): Client::peer().call for client-initiated outbound calls

- **What.** Add the symmetric `peer.call` on the client side using
  the c11 allocator (prefix `c_<n>`). Mirrors c12 in shape.
- **Why.** scope §K1.
- **Depends on.** c11, c12, c13.
- **Acceptance.** Three tests, all required by scope demo bar:
  - `tests/peerhandle_bidirectional.rs`: server initiates
    `peer.call`, client (with `with_service`) responds; client
    initiates `peer.call`, server's registered `Service` responds;
    both within one connection.
  - `tests/id_namespace_isolation.rs`: 100 concurrent `peer.call`s
    in each direction on one connection; all responses correlate;
    no id collision between server-initiated (`s_<n>`) and
    client-initiated (`c_<n>`) namespaces.
  - Extends `tests/bounded_notify_drop.rs` (originally landed in
    c09) with the scope-required post-flood assertion: after the
    bounded notification sink has dropped frames, a subsequent
    `peer.call` succeeds. This closes the scope row that was
    deferred from c09 because `peer.call` didn't yet exist.

### c15 — feat(fittings-core): full ctx.peer() handler-side test

- **What.** No new code; promotes c10's stub by exercising
  `ctx.peer().notify(...)` AND `ctx.peer().call(...)` from inside
  a handler.
- **Why.** scope demo `service_context_peer_call.rs`; pi
  round-3 must-fix on scope.
- **Depends on.** c10, c12, c13, c14.
- **Acceptance.** `tests/service_context_peer_call.rs`: handler
  receives an inbound request, calls `ctx.peer().call(...)` to
  the peer mid-flight, gets the response, returns its own response.
  Tests both directions.

### c16 — feat(fittings): PeerHandle::closed() lifecycle observation

- **What.** Add `PeerHandle::closed() -> impl Future<Output = ()>`
  on both server and client. Fires when the underlying transport
  tears down (graceful EOF or transport error). Pending `peer.call`
  futures resolve with `FittingsError::Transport` on close.
- **Why.** scope §S3.
- **Depends on.** c12, c14.
- **Acceptance.** `tests/peerhandle_close_drain.rs`: closing the
  underlying transport resolves all pending `peer.call` futures
  with `FittingsError::Transport` and resolves `peer.closed()` on
  both sides.

### c17 — feat(fittings-server): cancellation method + extractor configuration

- **What.** Add `Server::with_cancellation(method: &str, id_field:
  &str)` builder and the corresponding default. Library default is
  LSP (`$/cancelRequest`, id field `id`); MCP override is set
  explicitly by callers (c27). The dispatcher routes cancellation
  notifications to a per-request token (c07); this commit only
  *configures* which method/extractor the dispatcher listens for.
  Actual token-firing logic is c21.
- **Why.** scope §S7. Pi review-1 finding 3: dropped-future
  cancellation in c18 needs the configured method to exist first.
- **Depends on.** c08.
- **Acceptance.** Unit test: server constructed with default
  config has cancellation_method == `$/cancelRequest`; with MCP
  override has `notifications/cancelled` + `requestId`. (Token
  firing is exercised in c21.)

### c18 — feat(fittings-server): PeerHandle::call dropped-future cancellation

- **What.** When a `peer.call` future is dropped before resolving,
  the handle emits the configured cancellation method (c17) on the
  wire, naming the dropped call's id, and removes its slot in
  `pending_outbound`.
- **Why.** scope §S2 acceptance criterion 2.
- **Depends on.** c12, c14, c16, c17.
- **Acceptance.** `tests/peerhandle_dropped_future_cancels.rs`:
  spawn a `peer.call`, drop the future before the response, observe
  the cancellation notification on the wire (LSP default), pending
  slot vacated. Repeats with MCP override (`notifications/cancelled`
  + `requestId`).

### c19 — feat(fittings-client): inbound notification handler (sync Fn) + client-side outside-handler notify

- **What.** `Client::with_notification_handler(Fn(String, Value) +
  Send + Sync + 'static)`. Wrapped in `tokio::spawn(async move {
  handler(method, params); })`. If unregistered, dropped silently.
  m0 may additionally expose a `Client::dropped_notifications()`
  counter (an implementation convenience, not RFC-mandated). With
  this commit, the client now has the public-API path for observing
  server-originated notifications, completing the K1/K2 surface.
- **Why.** scope §K2. Pi review-2 finding 3: c10 needed a raw
  harness for the outside-handler notify test because K2's
  notification handler didn't exist yet; this commit adds the
  public-API client observation.
- **Depends on.** c10.
- **Acceptance.**
  - `tests/notification_handler_panic.rs`: panic in handler doesn't
    kill subsequent notifications; doesn't affect response
    correlation.
  - Extends `peerhandle_outside_handler.rs` (from c10) with the
    client-side public path: a startup task on the client calls
    `Client::peer().notify(...)`, server observes; server's startup
    task calls `Server::peer().notify(...)`, client's registered
    notification handler observes. Both directions through the
    public API.

### c20 — feat(fittings-server): id_null_explicit_request runtime semantics

- **What.** Inbound `"id": null` enters the in-flight map keyed on
  `JsonRpcId::Null`; handler runs; response carries `"id": null`.
  A second concurrent `"id": null` inbound request is rejected as
  a protocol-error duplicate per
  rfc-fittings-notifications.md:137-145.
- **Why.** scope §W1 runtime semantics. Pi review-1 open-item:
  moved earlier from Group 4 because cancellation/in-flight tracking
  in Group 4 depends on Null-id keying being correct.
- **Depends on.** c01, c08.
- **Acceptance.** `tests/id_null_explicit_request.rs` (request gets
  response with id null). `tests/id_null_concurrent_rejected.rs`
  (second concurrent null-id request rejected).

---

## Group 4 — Cancellation routing + suppression + batch (S5–S6, S7.1, S8)

### c21 — feat(fittings-server): cancellation reader routed outside the request semaphore

- **What.** A dedicated reader for the configured cancellation
  method (c17) fires the matching per-request token (c07) without
  competing for handler permits.
- **Why.** scope §S5.
- **Depends on.** c17, c20.
- **Acceptance.** `tests/cancellation_outside_semaphore.rs`:
  `with_max_in_flight(1)` saturated by a sleeping handler;
  cancellation notification arrives; the saturated handler observes
  the token without waiting for a permit.

### c22 — feat(fittings-server): two-trigger Cancelled response suppression

- **What.** S6's two-trigger rule: token-fired ⇒ response
  suppressed when handler returns; handler-returned `Err(Cancelled)`
  ⇒ response suppressed regardless of token state. Both paths
  idempotent.
- **Why.** scope §S6.
- **Depends on.** c06, c21.
- **Acceptance.** Two new tests:
  `service_context_cancelled_by_token.rs` (client cancellation fires
  token; handler returns `Err(Cancelled)`; no response).
  `service_context_cancelled_by_handler.rs` (handler returns
  `Err(Cancelled)` without the token firing; no response).

### c23 — feat(fittings-server): malformed cancellation payload handling

- **What.** Cancellation payloads with non-object params, missing
  the configured id field, or id-type mismatch (string for numeric,
  vice versa) are logged at WARN and dropped. Doesn't kill the
  connection or affect other in-flight requests. Behaviour identical
  for LSP and MCP extractor configurations.
- **Why.** scope §S7.1.
- **Depends on.** c21.
- **Acceptance.** `tests/malformed_cancellation.rs` table-driven
  across LSP-default and MCP-override configurations and three
  malformed payload shapes.

### c24 — feat(fittings-server): batch cancellation per-item suppression

- **What.** Cancellation references *individual request IDs*, not
  batch container IDs. A batched request whose component is
  cancelled has that component's response suppressed; remaining
  components proceed. If every component in a batch is suppressed
  (cancelled or notification-only), no batch response is emitted
  per rfc-fittings-notifications.md:628-671.
- **Why.** scope §S8.
- **Depends on.** c22.
- **Acceptance.** `tests/batch_cancellation_partial_suppression.rs`:
  3-component batch; cancel one mid-flight; only that response
  suppressed; remaining responses delivered. Plus an
  all-cancelled-components case → no batch response emitted.

### c25 — test(fittings): peer-gone observed via closed/Transport, not notify

- **What.** Tests-only commit. Asserts the RFC delivery contract
  per rfc-fittings-notifications.md:717-747: peer disconnect mid-
  notification stream causes `peer.closed()` to resolve and pending
  `peer.call` futures to resolve with `FittingsError::Transport`.
  `ctx.notify` does **not** synchronously fail with `Cancelled`
  on peer-gone — `notify` only reports local
  enqueue/encoding/channel-closed status.
- **Why.** scope risk #3, pi review-2 finding 6 on scope.
- **Depends on.** c16, c19.
- **Acceptance.** `tests/peer_gone_during_notify.rs`.

---

## Group 5 — `mcp-server` post-cutover refinements (E1–E3)

### c26 — refactor(mcp-server): replace serve_stdio workaround with Server::serve + ctx.notify

- **What.** Drop the custom `serve_stdio` loop that drained
  `Vec<ServerNotification>` post-tool-call; use `Server::serve(...)`
  + handlers using `ctx.notify`. The
  `Arc<Mutex<Vec<ServerNotification>>>` shim in `ToolCallContext`
  is removed.
- **Why.** scope §E1. (`mcp-server` was already migrated to the
  ServiceContext signature in c08; this commit removes the
  workaround that used to compensate for the missing
  `ServiceContext::notify`.)
- **Depends on.** c09.
- **Acceptance.** `cargo test -p mcp-server` green. Existing
  `progress_demo` example exercises one `notifications/progress`
  mid-call.

### c27 — feat(mcp-server): configure MCP cancellation extractor explicitly

- **What.** `mcp-server`'s `Server` builder calls
  `with_cancellation("notifications/cancelled", "requestId")` so
  it works with the MCP SDK without relying on library defaults
  (which are LSP).
- **Why.** scope §E2.
- **Depends on.** c17, c26.
- **Acceptance.** `tests/mcp_server_cancellation_config.rs`:
  asserts the configured method/extractor.

### c28 — refactor(mcp-server): handlers use ctx.cancelled instead of custom CancellationToken

- **What.** Drop the custom `CancellationToken` plumbing in
  `ToolCallContext`; handlers consume `ctx.cancelled()` /
  `ctx.is_cancelled()` directly. End-to-end cancellation works
  through the new APIs after c22 (suppression) and c27 (extractor).
- **Why.** scope §E2.
- **Depends on.** c22, c27.
- **Acceptance.** `tests/mcp_server_cancellation_interop.rs`:
  cancel an in-flight `tools/call` via `notifications/cancelled`
  + `requestId`; handler observes the token; response suppressed.
  `long_running_demo` example continues to work.

### c29 — test(mcp-server): JS-SDK interop check passes against rebuilt server

- **What.** Re-run `npm run check:real-client` (the
  `scripts/check-with-mcp-sdk.mjs` driver) against the rebuilt
  server; confirm the wire shape changes from Groups 1–4 didn't
  regress interop. Records the JS-SDK interop output snippet in
  the commit body. (The full milestone-level `manual-validation.md`
  artefact is written at c32 after Group 6 lands.)
- **Why.** scope §E3.
- **Depends on.** c26, c27, c28.
- **Acceptance.** `npm run check:real-client` exits 0; output
  matches the recorded baseline.

---

## Group 6 — Transport regression + spawn verification (T1, P1)

### c30 — test(fittings-transport): bidirectional traffic regression on stdio + tcp

- **What.** Tests-only commit confirming both stdio and tcp
  transports carry simultaneous bidirectional traffic without
  ordering bugs after Group 3's bidirectional `PeerHandle` lands.
- **Why.** scope §T1.
- **Depends on.** c14, c16.
- **Acceptance.** `tests/transport_bidirectional_regression.rs`
  for each transport: simultaneous `peer.call`s in both directions;
  100 calls each side; all correlate; no orphan responses.

### c31 — test(fittings-spawn): SubprocessConnector wires PeerHandle correctly

- **What.** Tests-only commit verifying `SubprocessConnector`
  wires the spawned child's stdio into a bidirectional `PeerHandle`
  after Group 2's API changes + Group 3's call/with_service work.
- **Why.** scope §P1. Pi review-1 finding on c37: needed the full
  bidirectional API, not just notify, so the dependency now
  references c14/c16 not c10.
- **Depends on.** c14, c16.
- **Acceptance.** `tests/spawn_peerhandle_round_trip.rs`: spawn a
  child with a hand-rolled echo service; parent `peer.call`s the
  child; child responds; parent `peer.notify`s; child receives.

### c32 — docs(fittings): write manual-validation.md for m0

- **What.** Write `rafaello/plans/milestones/m0-fittings/manual-validation.md`
  recording the items in scope §"Manual validation in
  manual-validation.md": the tmux + JS interop driver session
  with one progress notification + one cancelled call captured;
  the full m0 test suite running clean (no test hangs past 30s);
  `cargo build -p fittings` clean (no `target/pre-commit`
  artefacts in git); `nix develop .#fittings --command cargo test
  --workspace` green on Linux. The macOS leg is delegated to CI
  per scope.
- **Why.** scope §"Manual validation"; pi review-2 finding 5: the
  manual-validation artefact is a milestone-level deliverable, not
  a per-commit one, and must land after Group 6.
- **Depends on.** c30, c31.
- **Acceptance.** `manual-validation.md` exists, captures the
  required evidence, and is committed alongside any minor
  tooling/CI/Nix follow-ups discovered while exercising it.

---

## Acceptance for the milestone as a whole

Beyond per-commit acceptance, m0 lands when:

- Every named test in `scope.md`'s positive + negative test matrices
  is implemented and passes.
- `cargo test --workspace` from `fittings/` is green.
- `npm run check:real-client` from
  `fittings/examples/mcp-server/` exits 0.
- `mcp-server/src/serve_stdio` no longer contains the manual
  notification-draining loop or the `Arc<Mutex<Vec<ServerNotification>>>`
  shim.
- `manual-validation.md` records the items in `scope.md` §"Manual
  validation".
- `retrospective.md` is written after the last commit; any drift
  surfaced during implementation lands in `overview.md` /
  `decisions.md` / stream RFCs as deltas.

## What changed from the first draft

Round-2 pi review (`commits-pi-review-2.md`) prompted these revisions:

- **c02 adds `FittingsError::Panic` variant** explicitly so c04's
  marker round-trip can prove the contract; c08 (dispatcher cutover)
  maps handler panics to `Panic`. Pi-2 finding 4.
- **c02 keeps existing one-arg constructors** (`method_not_found(msg)`)
  setting `data: None`, with data-bearing constructors added. Pi-2
  finding 8.
- **c08 acceptance includes `error_preservation_round_trip.rs`** —
  the full server→client end-to-end round-trip for predefined
  variants — and `handler_panic_maps_to_panic.rs`. Pi-2 finding 6.
- **c09 acceptance includes `service_context_notify.rs`** (the
  scope demo bar's 5-notifications-mid-request ordering test),
  and the bounded-drop test no longer references `peer.call` at
  this commit (`peer.call` traffic post-flood is exercised in
  c14/c30). Pi-2 findings 1+2.
- **c10 acceptance restricted to `Server::peer().notify` + raw
  harness**; client-side public observation moves to c19. Pi-2
  finding 3.
- **c11 explicitly standalone allocator** with c01-only dependency;
  PeerHandle integration in c12/c14. Pi-2 open-item.
- **c19 acceptance extended** with the client-side public path
  (`Client::peer().notify` + client-handler observation), now
  that the client-side notification handler exists. Pi-2 finding 3.
- **c29 records JS-SDK output only**; the milestone-level
  `manual-validation.md` is c32 (new). Pi-2 finding 5.
- **New c32** writes the milestone-level `manual-validation.md`
  after Group 6. Pi-2 finding 5.
- **Stale internal references corrected**: c06 cites c22 (not c23);
  c17 cites c21 (not c20); peer-gone-during-notify reference fixed
  to c25 (not c30).

Round-1 pi review (`commits-pi-review-1.md`) prompted these revisions:

- **API cutover consolidated** into c08 (was c08/c09/c12/c13/c14
  in the previous draft) so the workspace stays green per commit.
  Includes `mcp-server` migration to the new signature; the
  `serve_stdio` workaround is retained at c08 and dropped at c26.
- **Wire-error retargeted to fittings-core** (c02/c03/c04 split):
  `FittingsError` variants live in `fittings-core`; codec mapping
  is in `fittings-wire::error_map`. The previous draft wrongly
  attributed the variant change to `fittings-wire`.
- **c01 clarified** to target `fittings-core::message::Request`
  (the actual source of the lossy id conversion); `fittings-wire`'s
  envelope was already correct.
- **`id_null_explicit_request` semantics moved from Group 4 to c20**
  (Group 3) because cancellation/in-flight tracking in Group 4
  depends on Null-id keying being correct.
- **Cancellation configuration (c17) precedes dropped-future
  cancellation (c18)** so the dropped-future commit can reference
  the configured method.
- **Token plumbing consolidated in c07**; c07 ships with
  `tokio_util::sync::CancellationToken` impl and unit tests on token
  semantics. Group 4's c21/c22 only connect the cancellation reader
  to existing tokens.
- **c11 (id-namespace strategy) acceptance** is a unit test of
  the allocator, not a full bidirectional `peer.call` test (which
  is c14).
- **c12 acceptance** uses a hand-rolled responder, not a registered
  `Service`. Full `Service`-backed bidirectional test is c14 (after
  `with_service` lands at c13).
- **Stale internal references** corrected throughout (c10/c11
  pointers, c26 wording, m0b numeric range).
- **Service code test (c05)** widened to include the
  above-reserved-negative band and code `0` as invalid, per pi's
  reading of scope §C5.
- **c30 (`peer_gone_during_notify`)** depends on c19 + c16 for the
  full delivery contract test (was previously c20-only).
- **m0a/m0b split point** rewritten: the only clean stopping
  point inside m0 is after c10 (API+notify cutover, no
  bidirectional). Default is to ship m0 as one milestone.

## Open items resolved by pi rounds 1–2

All open questions from earlier drafts have been answered by pi:

- **c08 size**: consolidated cutover is the right trade-off (pi-2
  resolved).
- **c20 placement**: pi recommended moving earlier but said it isn't
  blocking if dependencies are correct. Left in current position
  (Group 3) since deps are valid (`c01, c08`); future driver may
  reshuffle.
- **c11 dependency**: standalone allocator depends on c01 only;
  documented in the c11 entry.
- **c25 placement**: pi noted it semantically belongs in Group 3
  but said placement is not blocking. Left in Group 4 since deps
  (`c16, c19`) are valid.
- **m0a/m0b cut**: only clean stopping point is after c10 (default:
  no split).
