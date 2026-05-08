# Pi review 2 — m0 fittings scope

Review target: `rafaello/plans/milestones/m0-fittings/scope.md` after
Claude's follow-up commits:

- `c64dba6 docs(rafaello-m0): align scope with stream-B RFCs per pi-review-1`
- `1b738d1 docs(rafaello-m0): finish negative-tests + internal-split alignment`

Verdict: **much improved, but not ratifiable yet**. The round-1 blockers
around explicit `id: null`, two-trigger cancellation, broad error
preservation, batch cancellation, and peer-handle acceptance coverage are
mostly fixed. Remaining issues are smaller, but still affect the public
API / protocol defaults and should be patched before owner ratification.

## Must-fix findings

### 1. `ServiceContext` still does not explicitly expose `PeerHandle`

`scope.md:94-99` says a `PeerHandle` is available inside handlers via
`ServiceContext` plumbing, but `scope.md:58-73` lists only
`notify`, `cancelled`, `is_cancelled`, and `request_id` on
`ServiceContext`.

The Stream-B RFC says `ServiceContext` gains a `peer: PeerHandle` so
handlers can issue outbound calls, not just notifications
(`rfc-fittings-notifications.md:888-896`). Without an explicit API,
implementers may land `ctx.notify(...)` but forget handler-side
`ctx.peer().call(...)`, leaving bidirectionality incomplete inside
services.

**Fix:** add `ServiceContext::peer() -> PeerHandle` (or equivalent
field/accessor) to C1, and add/extend a demo-bar test proving a handler
can call back to its peer via the context, not only via `Server::peer()`
outside a handler.

### 2. Cancellation default contradicts the RFC

`scope.md:143-149` says the default cancellation method/id extractor is
MCP-style `notifications/cancelled` + `requestId`, with LSP as an
override. The RFC says to keep cancellation configurable but default to
LSP because fittings is transport/protocol agnostic
(`rfc-fittings-notifications.md:554-557`). `scope.md:129-131` and
`scope.md:150-156` also hardcode `notifications/cancelled` in generic
server bullets.

m0 absolutely should configure MCP defaults in `examples/mcp-server`, but
that is different from the library default.

**Fix:** either:

- keep the RFC default: library defaults to `$/cancelRequest` + `id`,
  and mcp-server explicitly configures `notifications/cancelled` +
  `requestId`; or
- amend Stream-B / decisions before scope ratification if the owner now
  wants MCP as the fittings-wide default.

### 3. `Server::with_inbound_handler` is an invented API surface

`scope.md:162-166` adds `Server::with_inbound_handler(svc)` and says v1
doesn't ship a populated server-side inbound handler. That is not in the
Stream-B RFC. The current server shape is already `Server::new(service,
transport)`: the server's `Service` is its inbound request handler. The
RFC's optional registration mechanism is on the **client** side
(`Client::with_service`) for server-originated requests
(`rfc-fittings-notifications.md:813-823`, `:852-858`).

This also leaks into the demo bar: `scope.md:268` says the no-service
negative test mirrors onto the server via `with_inbound_handler`.

**Fix:** remove `Server::with_inbound_handler` from m0 scope unless a
new RFC explicitly defines a no-service server mode. Reframe S9 as:
normal server inbound requests go to the server's `Service`; only the
client has optional `with_service`, and unregistered client-side inbound
requests return `-32601`.

### 4. Invalid service-code fallback payload is stricter than the RFC

`scope.md:85-90` and `scope.md:270` require
`data: { "fittingsKind": "invalidServiceCode", "originalCode": <n> }`.
The errors RFC's canonical table only specifies
`{ "fittingsKind": "invalidServiceCode" }`
(`rfc-fittings-errors.md:198-209`). Round 1 explicitly asked scope to
align with that unless the RFC was amended.

Carrying `originalCode` may be a good idea, but making it a required
acceptance criterion creates spec drift.

**Fix:** either amend the RFC to require `originalCode`, or relax the
scope/test to require only `fittingsKind == "invalidServiceCode"` and
allow (but not require) extra diagnostic fields.

## Should-fix before commits.md

### 5. Client notification handler API drift: async vs sync

`scope.md:175-178` says inbound notifications dispatch to a registered
"async handler" and exposes `Client::dropped_notifications()`. The RFC
API is a synchronous `Fn(String, Value)` wrapped in
`tokio::spawn(async move { handler(method, params); })`, with no
framework `catch_unwind` and no normative drop counter for an
unregistered handler (`rfc-fittings-notifications.md:447-490`).

**Fix:** align K2 with the RFC's sync handler shape, or explicitly call
out an intentional API change and amend the RFC. If a dropped
notification counter is desired, mark it as an m0 addition rather than
implying it is RFC-mandated.

### 6. Risk #3 reintroduces a wrong `notify` error contract

`scope.md:313-317` says post-cancellation `notify` can return
`Err(Cancelled)` if the transport has torn down. The settled RFC
contract is: `notify` reports local enqueue/encoding/channel-closed
status only; channel-closed maps to `Transport`, while peer disconnect is
observed asynchronously via `closed()` / pending-call drain
(`rfc-fittings-notifications.md:717-747`).

**Fix:** rewrite the risk to say late notifications may enqueue or be
dropped locally; peer-gone is observed through `peer.closed()` and
pending-call `Transport`, not `Err(Cancelled)` from `notify`.

### 7. Numeric test counts are brittle

`scope.md:365-370` gates on "All 16 positive" and "All 5 negative".
Several rows are intentionally compound and may split/merge during
`commits.md` without changing semantic coverage.

**Fix:** make the acceptance summary point to the named demo-bar matrix
and required behaviours rather than hard numeric counts.

## Minor cleanup

- `scope.md:16-18` still describes the notifications RFC as including a
  "server-originated requests v1 cut" even though that section is
  superseded by bidirectional `PeerHandle`.
- `scope.md:76-77` says "see C7/C8", but there are no C7/C8 bullets.
- Numbering uses S7.1, while the internal split references S8.1
  (`scope.md:346`). Pick one numbering scheme.

## Round-1 status

Resolved or sufficiently addressed:

- missing-id vs explicit `id: null`;
- `request_id() -> Option<&JsonRpcId>`;
- two-trigger `Cancelled` suppression;
- full predefined error preservation coverage;
- PeerHandle outside-handler, dropped-future, close-drain tests;
- batch cancellation test coverage;
- peer-gone no longer requiring synchronous `ctx.notify` failure.

Once the must-fixes above are patched, I expect the scope to be ready for
owner ratification and `commits.md` drafting.
