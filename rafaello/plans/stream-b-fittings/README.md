# Stream B — fittings RFC

## The question

What changes does fittings need so that rafaello can be built on top
of it, and what other gaps surface during the audit?

Identified gaps (from the design conversation):

1. **Outbound notifications.** `fittings-wire` understands JSON-RPC
   notifications (`RequestEnvelope::notification()`), and the server
   correctly drops inbound id-less requests, but the `Service` trait
   is `call(Request) -> Response` — there is no API for a handler to
   push a notification asynchronously back to the peer. We need this
   for streaming tokens, tool progress, and the rafaello event bus.
2. **Error handling.** The project owner flagged this as probably
   under-designed. Audit the current story: how do handler errors,
   transport errors, decoding errors, and panics flow through? What
   does a plugin author actually catch and translate?

Likely additional surface to design:

- A `ServiceContext` (or equivalent) accessible from a handler that
  exposes `notify(method, params)` and probably a request-cancellation
  signal.
- Bidirectional peer model: server-side handlers may want to issue
  *requests* to the client (sampling/elicitation), not just
  notifications. Decide whether that lands now or later.
- Structured error mapping: fittings codes vs. JSON-RPC codes vs.
  domain errors plugins raise.

## Deliverables

- `rfc-fittings-notifications.md` — the smallest, cleanest change
  to fittings adding outbound notifications + ServiceContext.
- `rfc-fittings-errors.md` — error-handling design covering all the
  failure surfaces above.
- After human review, the RFC content is filed as a GitHub issue on
  `luizribeiro/lab` (the human will do this; the stream ends at the
  RFC).

## Inputs

- `fittings/crates/*/src/` — current source.
- `fittings/examples/mcp-server/` — concrete consumer that already
  hit notifications limits.
- JSON-RPC 2.0 spec for notification semantics.
