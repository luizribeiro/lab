# Pi review 1 — m0 fittings scope

Review target: `rafaello/plans/milestones/m0-fittings/scope.md` as
introduced by `0a3995d docs(rafaello-m0): draft m0-fittings scope`.

Verdict: **do not ratify as-is**. The scope is well structured and the
self-contained `fittings/` boundary is right, but several bullets now
contradict the Stream-B RFCs / ratified decision rows on load-bearing
wire semantics. If implemented literally, m0 would ship the wrong
`JsonRpcId`, cancellation, and error-code behaviours.

## Summary of blocking findings

1. **`id: null` semantics are wrong in the scope.** The scope treats
   explicit JSON null ids as notifications; Stream B and decision row 18
   require distinguishing missing id (`None`, notification) from
   explicit `id: null` (`Some(JsonRpcId::Null)`, request).
2. **`Cancelled` without a fired token is reversed.** The scope rejects
   this as `Internal`; the notifications RFC deliberately allows it and
   suppresses the response.
3. **Service error-code policy is stale.** The scope keeps `1..=999`,
   makes `10_000` invalid, and expects `original_code`; the errors RFC
   allows the full positive range and uses the `invalidServiceCode`
   marker for truly invalid codes.
4. **Bidirectional `PeerHandle` is underspecified.** The scope conflates
   `ServiceContext::notify` with a per-connection peer handle and omits
   several RFC acceptance criteria: outside-handler calls, dropping a
   call future emits cancellation, and connection close drains pending
   calls.
5. **Acceptance tests do not cover all information-preserving error
   paths.** One `MethodNotFound` round trip is not enough for the RFC's
   predefined-error preservation and `fittingsKind` marker contract.

## Blocking findings in detail

### 1. `id: null` must remain an id-bearing request, not a notification

Scope citations:

- `scope.md:36-39` says inbound `id: null` is “treated as
  notification”.
- `scope.md:58` exposes `ServiceContext::request_id() -> &JsonRpcId`.
- `scope.md:210` adds `id_null_treated_as_notification.rs` expecting no
  response.

This conflicts with the revised Stream-B model:

- `rfc-fittings-notifications.md:99-101` has
  `request_id() -> Option<&JsonRpcId>`.
- `rfc-fittings-notifications.md:124-135` says
  `Request.id: Option<JsonRpcId>` distinguishes missing id from explicit
  null.
- `rfc-fittings-notifications.md:137-145` says an inbound request with
  `"id": null` is a request, enters `in_flight` as
  `JsonRpcId::Null`, and returns a response with `id: null` unless it is
  a protocol-error duplicate.
- `decisions.md` row 18 ratifies `Request.id: Option<JsonRpcId>` / two
  channels as the fittings v1 model.

Recommended fix:

- Change W1 to: missing `id` ⇒ notification / `Request.id = None`;
  explicit `"id": null` ⇒ `Request.id = Some(JsonRpcId::Null)`.
- Change C1 to `request_id() -> Option<&JsonRpcId>`.
- Replace `id_null_treated_as_notification.rs` with tests for:
  1. explicit `id: null` receives a response with `id: null`;
  2. a true id-less notification receives no response;
  3. a second concurrent `id: null` request is rejected per the RFC.

### 2. `FittingsError::Cancelled` without a fired token is allowed, not an internal error

Scope citations:

- `scope.md:98-105` says `Err(Cancelled)` without a fired token is
  rejected as `Internal` and handlers “are not allowed to fake
  cancellation”.
- `scope.md:207` makes that a negative test.

This directly contradicts the normative cancellation rules:

- `rfc-fittings-notifications.md:563-575` says token-fired and
  handler-returned `Cancelled` are two independent suppression triggers.
- `rfc-fittings-notifications.md:591-593` explicitly says handlers may
  return `Cancelled` without the token firing and that this is not a bug.

Recommended fix:

- Rewrite S6 so `Err(Cancelled)` always suppresses the response whether
  or not the token fired.
- Delete or invert `cancelled_without_token.rs`: it should assert no
  response and no `Internal` error for the local-cancel path.
- If the owner wants the stricter policy, amend the Stream-B RFC first;
  scope.md should not silently reverse it.

### 3. Error-code validation and invalid-code data are stale

Scope citations:

- `scope.md:66-69` accepts `-32099..=-32000` plus existing `1..=999`.
- `scope.md:211` says `ServiceError { code: 10_000 }` falls back to
  `-32603` with `data.original_code`.

The errors RFC says:

- `rfc-fittings-errors.md:85-90` allows the JSON-RPC server band and
  any code outside the reserved `-32768..=-32000` cluster, including
  `1..=i32::MAX`.
- `rfc-fittings-errors.md:163-179` makes `10_000` valid.
- `rfc-fittings-errors.md:198-209` maps truly invalid service codes to
  `-32603` with `data: { "fittingsKind": "invalidServiceCode" }`, not
  `original_code`.

Recommended fix:

- Change C5 to the RFC’s full valid-code predicate.
- Replace the invalid-code test with reserved/invalid values such as
  `0`, a predefined code, or a reserved future-predefined negative code.
- Align the expected fallback payload with `invalidServiceCode` unless
  the RFC is deliberately amended.

### 4. `PeerHandle` API and acceptance criteria need to be explicit

Scope citations:

- `scope.md:73-75` says a per-connection `PeerHandle` is accessible from
  handlers “via `ServiceContext::notify`”, which is only the per-request
  notification helper.
- `scope.md:76-84` lists `PeerHandle::call` and `closed`, but not how
  callers obtain a peer handle outside an inbound request.
- `scope.md:193-199` tests bidirectional calls and id isolation, but not
  the other RFC acceptance criteria.

The notifications RFC says:

- `rfc-fittings-notifications.md:777-780` explicitly says
  `ServiceContext::notify` does not cover connection-scoped bus use
  cases; the unified `PeerHandle` does.
- `rfc-fittings-notifications.md:807-823` sketches `Server::peer()`,
  `Client::peer()`, and `Client::with_service(...)`.
- `rfc-fittings-notifications.md:859-870` requires outbound call
  cancellation on dropped futures and close-drain behaviour.
- `rfc-fittings-notifications.md:873-886` lists acceptance criteria for
  outside-handler `peer.call`, simultaneous calls, dropped-future
  cancellation, and draining `pending`/`pending_outbound` with
  `Transport` errors.

Recommended fix:

- Add explicit in-scope bullets for `Server::peer()`, `Client::peer()`,
  and `ServiceContext` carrying or exposing a `PeerHandle`.
- Add tests for:
  - `peer.call` outside any inbound handler;
  - dropping a `peer.call` future emits the configured cancellation
    notification and removes the pending slot;
  - connection close resolves all pending outbound calls with
    `Transport` and resolves `peer.closed()`.

### 5. Error-preservation tests are too narrow

Scope citation:

- `scope.md:196` only tests `MethodNotFound { message, data }`.

The errors RFC requires preservation across all predefined variants and
special marker handling:

- `rfc-fittings-errors.md:192-209` defines outbound preservation for
  `Parse`, `InvalidRequest`, `MethodNotFound`, `InvalidParams`,
  `Internal`, plus `Transport`/`Panic` markers.
- `rfc-fittings-errors.md:223-262` defines inbound preservation and
  `fittingsKind` decoding.

Recommended fix:

- Expand `error_preservation_round_trip.rs` into a table-driven test
  over all five predefined codes, checking both `message` and exact
  `data` preservation.
- Add explicit marker tests for `Transport` and `Panic` mapping to
  `-32603` and decoding back via `fittingsKind`.
- Add service-code tests for valid positive, valid server-band, valid
  below-reserved, and invalid/reserved codes.

## High-priority non-blocking findings

### 6. Batch cancellation is in scope but lacks a demo-bar test

`scope.md:109-112` correctly includes per-item batch cancellation, but
there is no test for it in `scope.md:191-211`. Add a dedicated
`batch_cancellation_partial_suppression.rs` covering: cancel one item in
a batch, suppress only that response, continue the remaining items, and
emit no batch response when every item is suppressed or notification-only
(per `rfc-fittings-notifications.md:628-671`).

### 7. `peer_gone_during_notify.rs` overpromises synchronous peer-gone detection

`scope.md:209` expects `ctx.notify` to return `Err` when the peer
disconnects mid-stream. The RFC’s delivery contract says `notify` only
reports local enqueue/encoding/channel-closed status; successful enqueue
does not prove peer delivery, and transport failure is discovered later
by the dispatcher (`rfc-fittings-notifications.md:717-747`).

Recommended fix: rewrite the test to assert the reliable contract:
connection close eventually fires cancellation / `peer.closed()` and
pending calls resolve with `Transport`; do not require a particular
`ctx.notify` call to synchronously fail on peer disconnect.

### 8. Cancellation payload naming is too MCP-specific in one place and too generic in another

`scope.md:94-107` hardcodes `notifications/cancelled` and says malformed
payloads are missing `id`. The RFC keeps the cancellation method and id
extractor configurable, with MCP-style `requestId` and LSP-style `id`
called out separately. Because m0’s example is MCP, the scope should be
explicit about which extractor is used in each test and should include
string-vs-number id mismatch coverage.

## Scope-size note

The internal split is honest, but after adding the missing PeerHandle,
error-preservation, and batch-cancellation coverage, 25–40 sequential
commits may be optimistic. I would keep the proposed split rule, but ask
`commits.md` to surface an m0a/m0b split for owner approval earlier than
“only if >40 commits” if groups 1–3 cannot be kept independently green.

## Ratification bar

Before owner ratification, update `scope.md` so it is semantically
aligned with Stream B on:

- missing id vs explicit `id: null`;
- `ServiceContext::request_id() -> Option<&JsonRpcId>`;
- `Cancelled` suppression semantics;
- valid service-code ranges and invalid-code marker data;
- explicit `PeerHandle` acquisition and lifecycle behaviour;
- complete predefined-error preservation coverage.
