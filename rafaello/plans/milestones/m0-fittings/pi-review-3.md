# Pi review 3 — m0 fittings scope sign-off review

Review target: `rafaello/plans/milestones/m0-fittings/scope.md` after
`c67d290 docs(rafaello-m0): apply pi-review-2 must-fixes + cleanups`.

Verdict: **sign-off pending one small scope clarification**. Claude has
addressed the substantive round-2 must-fixes: `ServiceContext` now
exposes peer access, cancellation defaults are LSP-style with MCP
configured explicitly by `mcp-server`, the invented server-side
`with_inbound_handler` API is gone, `invalidServiceCode` matches the RFC
marker contract, K2 uses the sync notification-handler shape, and the
acceptance summary no longer depends on brittle raw test counts.

## Remaining must-fix before owner ratification

### 1. Demo bar must explicitly cover in-handler `ctx.peer().call(...)`

`scope.md:58-76` now correctly adds `ServiceContext::peer()` (or an
equivalent field) so handlers can issue outbound `peer.call` requests
back to the peer. However, the demo-bar matrix still does not explicitly
name a test for that path:

- `peerhandle_bidirectional.rs` (`scope.md:245`) covers calls in both
  directions, but does not say one call originates from inside a handler
  via `ctx.peer()`.
- `peerhandle_outside_handler.rs` (`scope.md:246`) explicitly covers
  `Server::peer()` / `Client::peer()` outside any inbound request.

That leaves the new `ServiceContext::peer()` API present in prose but not
load-bearing in acceptance criteria. A future implementer could satisfy
outside-handler bidirectionality while accidentally leaving handler-side
`ctx.peer().call(...)` untested or broken.

**Required fix:** add a named row such as
`service_context_peer_call.rs`, or extend `peerhandle_bidirectional.rs`,
to assert: handler receives an inbound request, calls
`ctx.peer().call(...)` to the peer while the inbound request is in
flight, receives the peer response, and then returns its own response.
This is enough for sign-off.

## Editorial cleanup (non-blocking, but do it with the same patch)

`scope.md:143-149` now correctly says the library default cancellation
shape is LSP (`$/cancelRequest` + `id`) and MCP is explicitly configured
by `mcp-server`. A few generic bullets still say `notifications/cancelled`
as if it were the universal cancellation method:

- `scope.md:129-131` (`S5`) says a dedicated `notifications/cancelled`
  reader.
- `scope.md:155-156` (`S8`) says `notifications/cancelled` references
  individual IDs.
- The malformed-cancellation test description still starts from
  `notifications/cancelled` while also claiming to cover LSP.

Recommended wording: use “the configured cancellation method” in generic
server/test bullets, and reserve `notifications/cancelled` for the
MCP-specific example/configuration.

## Round-2 status

Resolved:

- `ServiceContext` now exposes peer access in C1.
- Cancellation library default now matches the RFC (LSP), with MCP
  configured explicitly by `mcp-server`.
- `Server::with_inbound_handler` was removed; server inbound handling is
  the existing `Server::new(service, transport)` service.
- `invalidServiceCode` no longer requires `originalCode` beyond the RFC.
- K2 sync handler shape matches the RFC.
- `notify` no longer reports peer-gone/`Cancelled` synchronously.
- Acceptance summary is behaviour/name based, not raw count based.

After the single in-handler peer-call acceptance row is added, I would
sign off on the scope for owner ratification and `commits.md` drafting.
