# m0-fittings — commits

> **Status:** drafted by claude (orchestrator) for pi adversarial
> review. Pending owner ratification before per-commit agent work
> begins.

Ordered commit list for m0, derived from `scope.md`. Each commit is
one logical idea; tests land with the code that exercises them per
`~/.claude/CLAUDE.md`. Commits land sequentially on per-commit
branches `agents/m0/c<NN>` rebased onto `rafaello-v0.1`, no merge
commits, no force pushes.

## Conventions

- Subject line follows the repo style: `<type>(<scope>): <imperative>`.
  Scopes used here: `fittings-wire`, `fittings-core`, `fittings-server`,
  `fittings-client`, `fittings-macros`, `fittings-spawn`,
  `fittings-transport`, `mcp-server`, `fittings`.
- "Acceptance" lists the new tests + the existing tests that must
  still pass. Pre-commit hooks (rustfmt + clippy + test suites) gate
  every commit.
- "Depends on" cites the lowest commit number whose code or types
  this commit references. A commit may only land after every
  declared dependency has landed on `rafaello-v0.1`.
- Test files live under `fittings/tests/` unless otherwise noted.

## m0a / m0b split point

Per scope §"Internal split", the driver should propose an **m0a / m0b
split for owner approval as soon as Group 3 looks like it cannot
land green on top of Group 2 alone**. Suggested cut: m0a = Groups 1
+ 2 (commits c01–c19); m0b = Groups 3–6 (c20–c41). Driver decides
mid-flight; default is "ship as one milestone."

---

## Group 1 — Wire-layer ground work (W1–W4, C5)

### c01 — feat(fittings-wire): make Request.id Option<JsonRpcId>

- **What.** Migrate `Request.id: JsonRpcId` to `Option<JsonRpcId>`.
  `Response.id: JsonRpcId` stays non-optional (carries
  `JsonRpcId::Null` for explicit-null-id responses).
- **Why.** scope §W1; rfc-fittings-notifications.md:124-145.
- **Depends on.** baseline.
- **Acceptance.** New `tests/wire_envelope_id_shape.rs`: `{}`
  decodes to `id = None`; `{"id":null}` decodes to
  `id = Some(JsonRpcId::Null)`; `Response` always emits `id` field
  (even when `Null`). Existing wire serde tests still pass.

### c02 — feat(fittings-wire): predefined error variants gain data field

- **What.** Add `data: Option<Value>` and a typed `message: String`
  to `Parse`, `InvalidRequest`, `MethodNotFound`, `InvalidParams`,
  `Internal` variants of the wire error enum. Outbound encoders
  serialise both fields when present.
- **Why.** scope §W2; rfc-fittings-errors.md:192-209.
- **Depends on.** c01.
- **Acceptance.** New `tests/wire_predefined_error_data_round_trip.rs`
  table-driven across all five variants asserts `data` and `message`
  byte-equal after encode + decode.

### c03 — feat(fittings-wire): outbound encoder preserves data for predefined codes

- **What.** Update `to_error_envelope` to keep `message` + `data`
  for predefined codes. Currently flattens to canonical strings.
- **Why.** scope §W3.
- **Depends on.** c02.
- **Acceptance.** Extends c02's table-driven test to include the
  `to_error_envelope` path explicitly. Adds
  `tests/transport_panic_marker_round_trip.rs` for `Transport` and
  `Panic` markers mapping to `-32603` with `data.fittingsKind`.

### c04 — feat(fittings-wire): inbound decoder preserves message + data for predefined codes

- **What.** Update `from_error_envelope` to retain `message` and
  `data` instead of discarding for predefined codes.
- **Why.** scope §W4.
- **Depends on.** c02.
- **Acceptance.** Adds the inbound half of the c02/c03 round-trip
  test (decode an error envelope round-trips through the typed
  `WireError` and back).

### c05 — feat(fittings-core): widen ServiceError code validation + invalidServiceCode marker

- **What.** Replace the `1..=999` validator with: any positive code
  (`1..=i32::MAX`), the JSON-RPC server band (`-32099..=-32000`),
  and any negative code outside the reserved cluster
  (`-32768..=-32000`). Truly invalid codes serialise to `-32603
  Internal` with `data.fittingsKind = "invalidServiceCode"` (per
  the RFC; additional diagnostic fields like `originalCode` are
  allowed but not required).
- **Why.** scope §C5; rfc-fittings-errors.md:163-209.
- **Depends on.** c02.
- **Acceptance.** New `tests/service_code_ranges.rs` covers valid
  positive, valid server-band, valid below-reserved. New
  `tests/invalid_service_code_marker.rs` covers a reserved code
  (`-32700`) producing the marker; asserts only `fittingsKind ==
  "invalidServiceCode"` (tolerates extra fields).

---

## Group 2 — `ServiceContext` + bounded notify + Service trait + macros (C1–C4, S1, S4, M1–M2)

### c06 — feat(fittings-core): introduce FittingsError::Cancelled variant

- **What.** Add `FittingsError::Cancelled { reason: Option<String> }`.
  No wire mapping; explicit non-suppression-trigger return value
  for handlers.
- **Why.** scope §C3.
- **Depends on.** baseline (used by ServiceContext later).
- **Acceptance.** Variant compiles; existing error-mapping tests
  still pass; new unit test asserts `Cancelled` does not have a
  numeric wire code.

### c07 — feat(fittings-core): introduce ServiceContext type (notify + cancellation token + request_id)

- **What.** Add `ServiceContext` struct in `fittings-core` exposing
  `notify(method, params) -> Result<(), FittingsError>`,
  `cancelled() -> impl Future<Output = ()>`, `is_cancelled() -> bool`,
  `request_id() -> Option<&JsonRpcId>`. `peer()` is added in c10
  once `PeerHandle` exists. Cheap to clone.
- **Why.** scope §C1.
- **Depends on.** c01, c06.
- **Acceptance.** Unit tests for the public API (token semantics,
  `request_id` for various id shapes, clonability).

### c08 — feat(fittings-core): change Service trait to take ServiceContext

- **What.** Update `Service::call(&self, req: Request, ctx:
  ServiceContext) -> Result<Response, FittingsError>`. Breaking
  trait change.
- **Why.** scope §C2.
- **Depends on.** c07.
- **Acceptance.** `fittings-core` still compiles; downstream
  workspace crates (`-server`, `-client`, `-macros`, examples)
  don't yet — that's c09–c14's job. This commit may include a
  `compile_fail` doctest demonstrating the trait shape.

### c09 — feat(fittings-server): split server loop into bounded notification channel

- **What.** Two-channel server loop: response channel
  (`mpsc::UnboundedSender<Vec<u8>>`) for request responses,
  notification channel (`mpsc::Sender<Vec<u8>>` with bounded
  capacity, default 1024) for handler-emitted notifications.
  Drop-on-full with a `Server::dropped_notifications()` counter.
  The split prevents the dispatcher from blocking on the
  notification sink while responses are still in-flight.
- **Why.** scope §S4.
- **Depends on.** c08.
- **Acceptance.** New `tests/bounded_notify_drop.rs` floods notifications faster
  than the transport flushes; counter increments; subsequent
  request responses succeed.

### c10 — feat(fittings-server): introduce PeerHandle (notify-only) + Server::peer() accessor

- **What.** Define `PeerHandle` in `fittings-core` with `notify`
  initially (call comes in c14, closed comes in c16). Add
  `Server::peer() -> PeerHandle` returning the connection-scoped
  handle. Hook the bounded notification channel from c09 to
  `PeerHandle::notify`.
- **Why.** scope §S1, partial S2/S3.
- **Depends on.** c09.
- **Acceptance.** Unit test for `Server::peer().notify(...)` from
  outside any handler.

### c11 — feat(fittings-core): add ctx.peer() returning PeerHandle

- **What.** Plumb `PeerHandle` through `ServiceContext` so
  `ctx.peer()` returns the connection-scoped handle inside a
  handler.
- **Why.** scope §C1 (peer accessor).
- **Depends on.** c07, c10.
- **Acceptance.** New `tests/service_context_peer_call.rs`
  *stubbed*: handler calls `ctx.peer().notify(...)` mid-request
  and the test asserts the peer received it; `ctx.peer().call(...)`
  test lights up in c14.

### c12 — feat(fittings-core): update Middleware trait to accept ServiceContext

- **What.** `Middleware::handle(&self, req: Request, ctx:
  ServiceContext, next: ...) -> ...`. The `ctx` flows through
  middleware so middleware can `notify`/observe cancellation.
- **Why.** scope §C4.
- **Depends on.** c07, c08.
- **Acceptance.** Existing middleware tests recompile with the
  new signature and still pass; new test asserts middleware can
  emit a notification from `handle`.

### c13 — feat(fittings-macros): generate (self, ctx, params) handler signature

- **What.** Update `#[fittings::method]` and `#[fittings::service]`
  to produce `(self, ctx: ServiceContext, params: P)`. Hard cut —
  no support for the old signature.
- **Why.** scope §M1, §M2.
- **Depends on.** c08.
- **Acceptance.** Macro UI tests expect the new shape; existing
  macro UI tests for the old shape are deleted (with comment in
  commit body explaining the breaking cut).

### c14 — chore(fittings): update hello-{api,service,client} examples to new signature

- **What.** Mechanical migration: each `#[fittings::method]` consumer
  in `fittings/examples/` adds `_ctx: ServiceContext`. No new
  behaviour; just compile-error fixups.
- **Why.** Keeps the workspace green after c08+c13.
- **Depends on.** c13.
- **Acceptance.** `cargo build --workspace` green; existing example
  tests still pass.

### c15 — feat(fittings-server): wire ctx.notify through the server loop

- **What.** Connect `ServiceContext::notify` (c07) to the
  bounded-notification channel (c09). Dispatcher carries the
  per-request `ServiceContext` to the handler.
- **Why.** scope §C1, §S4 integration.
- **Depends on.** c07, c08, c09.
- **Acceptance.** New `tests/service_context_notify.rs`: handler
  emits 5 notifications mid-request; client receives all 5
  *before* the response.

---

## Group 3 — Bidirectional `PeerHandle` (S2–S3, S9, K1–K3)

### c16 — feat(fittings-core): id-namespace strategy for server-vs-client originated requests

- **What.** Define a namespacing strategy so server-initiated and
  client-initiated `peer.call`s don't collide on response
  correlation. Recommended: shared `AtomicU64` per direction with
  a 1-bit prefix in the id encoding (string-id `s_<n>`/`c_<n>`).
  Document the choice in `fittings-core` doc comment.
- **Why.** scope §S2 acceptance + risk #2.
- **Depends on.** c01.
- **Acceptance.** New `tests/id_namespace_isolation.rs`: 100
  concurrent `peer.call`s in each direction; no id collision; all
  correlate correctly.

### c17 — feat(fittings-server): PeerHandle::call (server-initiated request)

- **What.** Add `PeerHandle::call(method, params).await ->
  Result<Value, FittingsError>` on the server side, implementing
  the id-namespacing from c16. Maintains a `pending_outbound`
  map keyed by id; resolves on matching response.
- **Why.** scope §S2.
- **Depends on.** c16, c10.
- **Acceptance.** New `tests/peerhandle_bidirectional.rs`: server
  initiates `peer.call`, client (with a registered `Service` from
  c19) responds, server gets the result.

### c18 — feat(fittings-client): symmetric PeerHandle (notify + call) + Client::peer()

- **What.** Add `Client::peer() -> PeerHandle` symmetric to S1's
  `Server::peer()`. Wire the bounded-notification channel on the
  client side too. `notify` is new on the client; `call` already
  existed and is preserved.
- **Why.** scope §K1.
- **Depends on.** c17.
- **Acceptance.** Extend `peerhandle_bidirectional.rs` with
  client-initiated `peer.call`; new `tests/peerhandle_outside_handler.rs`
  exercises both `Server::peer()` and `Client::peer()` from
  startup tasks.

### c19 — feat(fittings-client): Client::with_service for inbound peer-originated requests

- **What.** `Client::with_service(svc)` registers a service for
  inbound peer-originated requests (the server uses
  `PeerHandle::call`). Without it, the client returns
  `-32601 Method not found`.
- **Why.** scope §K3.
- **Depends on.** c17.
- **Acceptance.** New `tests/inbound_request_no_service.rs`: peer
  request to a client with no `with_service` returns `-32601`.

### c20 — feat(fittings-server): PeerHandle::closed() lifecycle observation

- **What.** Add `PeerHandle::closed() -> impl Future<Output = ()>`
  on both server and client. Fires when the underlying transport
  tears down (graceful EOF or transport error).
- **Why.** scope §S3.
- **Depends on.** c17, c18.
- **Acceptance.** New `tests/peerhandle_close_drain.rs`: closing
  the underlying transport resolves all pending `peer.call`
  futures with `FittingsError::Transport` and resolves
  `peer.closed()`.

### c21 — feat(fittings-server): PeerHandle::call dropped-future cancellation

- **What.** When a `peer.call` future is dropped, the handle emits
  the configured cancellation method (LSP default
  `$/cancelRequest`) on the wire and removes its slot in
  `pending_outbound`.
- **Why.** scope §S2 acceptance criterion 2.
- **Depends on.** c17, c20.
- **Acceptance.** New `tests/peerhandle_dropped_future_cancels.rs`:
  spawn a `peer.call`, drop the future before the response,
  observe the cancellation notification on the wire and the
  pending slot vacated.

### c22 — feat(fittings-client): inbound notification handler (sync Fn)

- **What.** `Client::with_notification_handler(Fn(String, Value) +
  Send + Sync + 'static)`. Wrapped in `tokio::spawn(async move {
  handler(method, params); })`. If unregistered, drops silently.
- **Why.** scope §K2.
- **Depends on.** c18.
- **Acceptance.** New `tests/notification_handler_panic.rs`:
  panic in handler doesn't kill subsequent notifications, doesn't
  affect response correlation.

### c23 — feat(fittings-core): full ServiceContext::peer().call test coverage

- **What.** No new behaviour; promote the c11 stub to a full test
  asserting in-handler `ctx.peer().call(...)` works mid-handler.
- **Why.** scope demo bar `service_context_peer_call.rs`; pi
  round-3 must-fix.
- **Depends on.** c11, c17.
- **Acceptance.** New `tests/service_context_peer_call.rs`:
  handler receives an inbound request, calls `ctx.peer().call(...)`
  to the peer mid-flight, gets the response, returns its own
  response. Tests both directions.

---

## Group 4 — Cancellation (C3 already in c06; S5–S8, S7.1)

### c24 — feat(fittings-core): cancellation token plumbing through ServiceContext

- **What.** Per-request cancellation token, implemented via
  `tokio_util::sync::CancellationToken` (or equivalent). Wired
  through `ServiceContext::cancelled()` and `is_cancelled()`. Token
  is fired by Group 4's later commits.
- **Why.** scope §C1, §S6.
- **Depends on.** c07.
- **Acceptance.** Unit test: `ctx.cancelled()` resolves when the
  token is fired; `is_cancelled()` flips synchronously.

### c25 — feat(fittings-server): configurable cancellation method + extractor

- **What.** Add `Server::with_cancellation(method: &str,
  id_field: &str)` configuration. Library default is LSP
  (`$/cancelRequest`, id field `id`); MCP is configured by
  callers. The dispatcher's cancellation reader uses the
  configured pair.
- **Why.** scope §S7.
- **Depends on.** c08.
- **Acceptance.** Unit test: server configured with LSP defaults
  fires token on `$/cancelRequest`; server configured with MCP
  fires token on `notifications/cancelled`.

### c26 — feat(fittings-server): route cancellation reader outside the request semaphore

- **What.** Dedicated cancellation reader in the server loop fires
  the per-request token without competing for handler permits.
- **Why.** scope §S5.
- **Depends on.** c24, c25.
- **Acceptance.** New `tests/cancellation_outside_semaphore.rs`:
  `with_max_in_flight(1)` saturated by a sleeping handler; second
  request's cancellation arrives; saturated handler observes the
  token without waiting for a permit.

### c27 — feat(fittings-server): two-trigger Cancelled response suppression

- **What.** Implement S6's two-trigger rule. Token-fired ⇒ response
  suppressed when handler returns. Handler-returned `Err(Cancelled)`
  ⇒ response suppressed regardless of whether the token fired.
  Both paths idempotent.
- **Why.** scope §S6.
- **Depends on.** c06, c24.
- **Acceptance.** Two new tests:
  `service_context_cancelled_by_token.rs` (client cancellation
  fires token; handler observes; returns `Err(Cancelled)`; no
  response). `service_context_cancelled_by_handler.rs` (handler
  returns `Err(Cancelled)` without the token firing; no response).

### c28 — feat(fittings-server): malformed cancellation payload handling

- **What.** Cancellation payloads with non-object params, missing
  the configured id field, or id-type mismatch are logged at WARN
  and dropped.
- **Why.** scope §S7.1.
- **Depends on.** c25.
- **Acceptance.** New `tests/malformed_cancellation.rs` covers
  three payload shapes against both LSP-default and MCP-override
  configurations.

### c29 — feat(fittings-server): batch cancellation per-item suppression

- **What.** Cancellation references individual request IDs, not
  batch container IDs. Suppress only the cancelled component's
  response in the batch response. If every component is suppressed,
  no batch response is emitted.
- **Why.** scope §S8; rfc-fittings-notifications.md:628-671.
- **Depends on.** c27.
- **Acceptance.** New `tests/batch_cancellation_partial_suppression.rs`:
  3-component batch, cancel one mid-flight; only that response
  suppressed. Plus an all-cancelled-components case → no batch
  response emitted.

### c30 — feat(fittings-wire,fittings-server): id_null_explicit_request semantics

- **What.** Inbound `"id": null` enters `in_flight` keyed on
  `JsonRpcId::Null`; handler runs; response carries
  `"id": null`. A second concurrent `"id": null` request is
  rejected as a protocol-error duplicate.
- **Why.** scope §W1 runtime semantics.
- **Depends on.** c01.
- **Acceptance.** New `tests/id_null_explicit_request.rs` and
  `tests/id_null_concurrent_rejected.rs`.

### c31 — feat(fittings-server): peer-gone observed via closed/Transport, not notify

- **What.** No new code; this commit lands the test asserting the
  RFC delivery contract.
- **Why.** scope §S3 / risk #3 / pi round-2 finding 6.
- **Depends on.** c20.
- **Acceptance.** New `tests/peer_gone_during_notify.rs`: peer
  disconnects mid-stream; `peer.closed()` resolves; pending
  `peer.call` futures resolve with `FittingsError::Transport`.
  Asserts that `ctx.notify` does **not** synchronously fail with
  Cancelled — the contract is local-status-only.

---

## Group 5 — `mcp-server` migration (E1–E3)

### c32 — refactor(mcp-server): replace serve_stdio workaround with Server::serve

- **What.** Drop the custom serve_stdio loop that drained
  `Vec<ServerNotification>` post-tool-call. Use the new
  `Server::serve(...)` plus handlers using `ctx.notify`.
- **Why.** scope §E1.
- **Depends on.** c15.
- **Acceptance.** `cargo run -p mcp-server -- serve` exchanges one
  `tools/call` emitting `notifications/progress` mid-call; existing
  Rust-side tests for `mcp-server` continue to pass.

### c33 — feat(mcp-server): configure MCP cancellation extractor explicitly

- **What.** `mcp-server`'s `Server` builder calls
  `with_cancellation("notifications/cancelled", "requestId")` so
  it works with the MCP SDK without relying on library defaults
  (which are LSP).
- **Why.** scope §S7 + §E2.
- **Depends on.** c25, c32.
- **Acceptance.** New `tests/mcp_server_cancellation_interop.rs`:
  cancel an in-flight `tools/call` via `notifications/cancelled`;
  handler observes the token; response suppressed.

### c34 — refactor(mcp-server): handlers use ctx.cancelled instead of custom CancellationToken

- **What.** Drop the `Arc<Mutex<Vec<ServerNotification>>>`-style
  hack in `ToolCallContext`; handlers consume `ctx.cancelled()`
  / `ctx.is_cancelled()` directly.
- **Why.** scope §E2.
- **Depends on.** c24, c33.
- **Acceptance.** Existing `long_running_demo` and
  `progress_demo` examples continue to work end-to-end via the
  new APIs; existing JS-SDK interop tests still pass.

### c35 — test(mcp-server): JS-SDK interop check passes against rebuilt server

- **What.** Re-run `npm run check:real-client` (the
  `scripts/check-with-mcp-sdk.mjs` driver) against the rebuilt
  server; ensure the existing test still passes after Groups 1–4
  have changed the wire shape.
- **Why.** scope §E3.
- **Depends on.** c32, c33, c34.
- **Acceptance.** `npm run check:real-client` exits 0; manual
  validation captured in `manual-validation.md` (commit body
  notes the expected output).

---

## Group 6 — Transport regression + spawn verification (T1, P1)

### c36 — test(fittings-transport): bidirectional traffic regression on stdio + tcp

- **What.** Tests-only commit. Confirm both stdio and tcp
  transports carry simultaneous bidirectional traffic without
  ordering bugs after Group 3's bidirectional `PeerHandle` lands.
- **Why.** scope §T1.
- **Depends on.** c20.
- **Acceptance.** New `tests/transport_bidirectional_regression.rs`
  for each transport: simultaneous `peer.call`s in both directions;
  100 calls each side; all correlate; no orphan responses.

### c37 — test(fittings-spawn): SubprocessConnector wires PeerHandle correctly

- **What.** Tests-only commit. Verify `SubprocessConnector` from
  `fittings-spawn` wires the spawned child's stdio into a
  bidirectional `PeerHandle` after Group 2's API changes.
- **Why.** scope §P1.
- **Depends on.** c10.
- **Acceptance.** New `tests/spawn_peerhandle_round_trip.rs`:
  spawn a child with a hand-rolled echo service; parent
  `peer.call`s the child; child responds; parent
  `peer.notify`s; child receives.

---

## Acceptance for the milestone as a whole

Beyond per-commit acceptance, m0 lands when:

- `cargo test --workspace` is green from `fittings/`.
- `npm run check:real-client` from `fittings/examples/mcp-server/`
  exits 0.
- `mcp-server/src/serve_stdio` no longer contains the manual
  notification-draining loop.
- `manual-validation.md` records the items in scope §"Manual
  validation".
- `retrospective.md` is written after the last commit; any drift
  surfaced during implementation lands in `overview.md` /
  `decisions.md` / stream RFCs as deltas.

## Open items for pi review

- **Commit count and split.** Current draft is 37 commits; range
  in scope is 30–50. Is anything missing? Is anything over-split?
- **Group ordering.** Within Group 4, c30 ("`id_null_explicit_request`
  semantics") might fit better in Group 1 since it's a wire-level
  concern — but the test depends on the dispatcher routing requests
  with id=Null through to the handler. Pi: which group?
- **c16 id-namespace strategy.** I picked the
  string-prefix-`s_`/`c_` shape and an `AtomicU64` per direction.
  Pi: is this sane, or is the shared-counter-with-disjoint-ranges
  shape better?
- **Test consolidation.** Several commits create one new test
  file each. If `commits.md` ratification surfaces concern about
  commit count, some tests can be merged (e.g. `id_null_explicit`
  + `id_null_concurrent` into one file). Pi's call.
- **m0a/m0b split.** Current cut is c01–c19 (m0a) vs c20–c37 (m0b).
  Driver should surface the split after c19 lands if c20–c23
  cannot land green on top of c19 alone. Pi: is the cut at c19 the
  right one, or earlier/later?
