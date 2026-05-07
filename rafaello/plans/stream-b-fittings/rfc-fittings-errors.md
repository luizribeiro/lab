# RFC: error handling

Status: draft (stream-b-fittings)
Owner: rafaello
Affects: `fittings-core` (`error.rs`, `message.rs`), `fittings-wire`
(`error_map.rs`), `fittings-server` (panic capture), `fittings-client`
(decode errors), `fittings-macros` (handler error propagation),
`fittings/examples/mcp-server`

## Why this RFC exists

The project owner flagged error handling as probably under-designed.
After reading the audit (see `notes.md`) the verdict is: the *wire*
side of error mapping is fine for the JSON-RPC pre-defined codes; the
*plugin author* side is missing several things, and `from_error_envelope`
actively destroys information that the wire faithfully transmitted.

The audit found seven distinct surfaces:

1. Handler `Result::Err(FittingsError)` returned through the trait.
2. Handler panics (caught via `catch_unwind`).
3. Inbound frame decode failures (parse error vs invalid request).
4. Outbound frame encode failures (almost never observed, currently
   silently swallowed).
5. Transport `send`/`recv` failures.
6. Domain errors that don't naturally fit `FittingsError`'s variants.
7. Client-side response-frame decode failures.

We need a coherent story across all seven. This RFC proposes one.

## Principles

1. **Information-preserving over the wire.** If a handler attaches
   `data` and a human message, the client gets them. We currently
   delete both for the well-known codes.
2. **Predictable mapping at exactly two seams.** Domain errors map to
   wire codes at the encode boundary; wire codes map to
   `FittingsError` at the decode boundary. No other place gets to
   re-interpret. Today `from_error_envelope` quietly normalises and
   loses data.
3. **Plugin authors write `?`, not match-rewrap pyramids.** Bridging
   `std::error::Error` should be a one-line `From` impl, not 20 lines
   per handler.
4. **Failure modes are observable.** Panics, transport drops, decode
   errors should reach a single tracing surface so an operator can
   tell what failed without reading the wire.
5. **Consistency with JSON-RPC 2.0.** We reserve the spec's reserved
   ranges and stop fighting them.

## Concrete proposal

### A. Reshape `FittingsError`

Today's variants conflate "framework-level" (parse/transport/internal)
with "JSON-RPC pre-defined" (invalid request, method not found, invalid
params) with "domain" (`Service(ServiceError)`).

Proposed:

```rust
#[derive(Debug, Clone, PartialEq, Error)]
pub enum FittingsError {
    /// Framework couldn't even parse the inbound bytes as JSON.
    #[error("parse error: {message}")]
    Parse { message: String },

    /// JSON parsed but didn't fit a JSON-RPC 2.0 request shape.
    #[error("invalid request: {message}")]
    InvalidRequest { message: String, data: Option<Value> },

    /// Method routed but the name is unknown.
    #[error("method not found: {method}")]
    MethodNotFound { method: String },

    /// Method routed but params didn't validate.
    #[error("invalid params: {message}")]
    InvalidParams { message: String, data: Option<Value> },

    /// Application-level error from a handler.
    /// `code` MUST be in the JSON-RPC server-error range
    /// (-32099..=-32000) OR the application-defined range
    /// (-32768..=-32100, plus the positive 1..=32767).
    #[error("{message} (code {code})")]
    Service {
        code: i32,
        message: String,
        data: Option<Value>,
    },

    /// Handler panic. Carries the panic payload's debug form when
    /// available. Always maps to internal-error on the wire, but
    /// distinguishable in-process for logging/metrics.
    #[error("handler panic: {message}")]
    Panic { message: String },

    /// I/O / framing error on the transport.
    #[error("transport: {message}")]
    Transport { message: String },

    /// Anything else the framework itself produced.
    #[error("internal: {message}")]
    Internal { message: String },
}
```

Key changes vs today:

- `Service` keeps `code/message/data` but its **valid code range is
  rewritten** (see B below).
- `InvalidRequest` and `InvalidParams` get a `data` slot.
- `MethodNotFound` carries the method name (it always existed —
  `error_map.rs` *receives* it and throws it away).
- `Panic` is its own variant (was hidden inside `Internal`).
- Constructor helpers stay (`FittingsError::parse_error`, etc.) for
  source compatibility.

### B. Wire-code policy

JSON-RPC 2.0 reserves:

- `-32700` parse error
- `-32600` invalid request
- `-32601` method not found
- `-32602` invalid params
- `-32603` internal error
- `-32099..=-32000` server-defined errors (open to applications)
- everything else outside `-32768..=-32000` is application-defined

Today we accept positive `1..=999` as the only valid plugin range.
That excludes the JSON-RPC server-error band entirely. New rule:

| Variant            | Outbound code            | Inbound recognises |
|--------------------|--------------------------|---------------------|
| `Parse`            | `-32700`                 | `-32700`            |
| `InvalidRequest`   | `-32600`                 | `-32600`            |
| `MethodNotFound`   | `-32601`                 | `-32601`            |
| `InvalidParams`    | `-32602`                 | `-32602`            |
| `Internal`         | `-32603`                 | `-32603`            |
| `Transport`        | `-32603` (with marker)   | n/a (wire side)     |
| `Panic`            | `-32603` (with marker)   | n/a (wire side)     |
| `Service { code }` | `code` if valid range    | any other valid code |

`ServiceError::is_valid_code_value` becomes:

```rust
const fn is_valid_application_code(code: i32) -> bool {
    // server-defined range
    (-32099..=-32000).contains(&code)
        // application range above the reserved cluster
        || (-32768..=-32100).contains(&code)
        // application range below the reserved cluster
        || (i32::MIN..=-32769).contains(&code)
        // positive application range
        || (1..=i32::MAX).contains(&code)
}
```

i.e. "anything except the five well-known codes and zero". This is the
spec's actual rule; we have been more restrictive without justification.

`Transport` and `Panic` mapping to `-32603` add a structured
`data: { "fittingsKind": "transport" | "panic", "detail": "..." }`
so the *peer* can tell them apart from a plain internal error,
and so logs on this side capture the detail. The peer's
`from_error_envelope` reads `fittingsKind` and rebuilds the right
variant.

### C. `from_error_envelope` stops dropping data

Today (`error_map.rs:64–96`):

```rust
ErrorEnvelope { code: METHOD_NOT_FOUND_CODE, .. } =>
    FittingsError::method_not_found(METHOD_NOT_FOUND_MESSAGE),
```

Proposed:

```rust
ErrorEnvelope { code: METHOD_NOT_FOUND_CODE, message, data } =>
    FittingsError::MethodNotFound {
        method: extract_method(&data).unwrap_or(message),
    },
ErrorEnvelope { code: INVALID_PARAMS_CODE, message, data } =>
    FittingsError::InvalidParams { message, data },
// etc.
```

The decoder preserves `message` and `data`, and uses our
`fittingsKind` data marker to distinguish `Transport`/`Panic` from
`Internal` when present.

This is a behaviour change visible to any consumer pattern-matching
on the *string* contents of the variant. We accept that — the
information was always supposed to be there.

### D. `?`-friendly bridging from arbitrary errors

Add a `IntoServiceError` trait and a blanket impl for any
`std::error::Error + Send + Sync + 'static`:

```rust
pub trait IntoServiceError {
    fn into_service_error(self, code: i32) -> FittingsError;
}

impl<E> IntoServiceError for E
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_service_error(self, code: i32) -> FittingsError {
        FittingsError::Service {
            code,
            message: self.to_string(),
            data: None,
        }
    }
}
```

So a handler that calls into `reqwest`, `sqlx`, etc. can write:

```rust
let row = db.fetch_one(&q).await.map_err(|e| e.into_service_error(2001))?;
```

And we add a small macro `service_err!(code, "...{}", arg)` for
the literal-message case. This is the smallest API that removes the
boilerplate without forcing every consumer onto a single error crate.

### E. Panic policy

Currently `Server::execute_request` does
`AssertUnwindSafe(call).catch_unwind()`. Keep that. Changes:

1. The panic payload's debug repr (`format!("{:?}", payload)`,
   downcast to `&str`/`String` first) becomes the
   `Panic { message }` field, not a fixed string.
2. Emit a `tracing::error!` at panic time with the request method,
   id, and panic payload. The log is the only place the operator
   gets to see it; the peer only ever sees `-32603` with the
   `fittingsKind: "panic"` marker.
3. Document that **handlers must not rely on panics for
   control flow**. Returning `Err(...)` is the contract.

### F. Encode-error policy

`Server::send_single_response` today silently swallows encode failures
(`if let Ok(encoded) = encode_response_line(&response)`, line 222).
A failed encode is a framework bug — the response was constructed by
the framework, so any failure is a serde-internal issue. Change to:

1. Log at `tracing::error!` with the response shape.
2. Send a synthesised `-32603` envelope built from a known-good
   constant, so the peer at least sees *something* with the right id.
3. Never panic.

### G. Transport-error policy

`Transport::send` failure on response delivery already aborts the
server (`server.rs:73–77`). Keep that. Add:

- `Transport::recv` failure that isn't graceful EOF returns from
  `serve`. Today this is correct.
- Transport errors raised *from inside a handler* (e.g. a handler
  calling `ctx.notify` after the peer dropped) propagate up as
  `Err(FittingsError::Transport { message })` and the handler may
  bail or ignore. The serve loop will discover the same failure
  on its next response write and shut down regardless, so handlers
  don't need to.

### H. Decode-error policy (client side)

`map_response_decode_error` (`client/src/lib.rs:212`) collapses both
parse and shape errors to a fixed string. Change to preserve the
underlying message:

```rust
WireDecodeError::Parse(message) => FittingsError::Parse { message },
WireDecodeError::InvalidRequest { message, .. } =>
    FittingsError::InvalidRequest { message, data: None },
```

This matches what we do server-side and gives consumers actual
debugging info.

### I. Middleware contract

Today's `Middleware::handle(req, next) -> Result<Response, _>` is fine
in shape, but the contract is undefined. Document:

- Middleware sees the handler's `Err` *after* it returns, but does
  *not* see panics — those are caught further out in
  `execute_request`. (We could move `catch_unwind` into the middleware
  chain, but that complicates the chain semantics; rejecting for
  simplicity.)
- Middleware MAY transform an error variant; if it does, it MUST NOT
  invent a `Service` code outside the valid range.
- Middleware MAY emit notifications via `ctx.notify` (logging, tracing).
- Middleware does NOT see decode errors. Decode errors are produced
  before any service is invoked.

### J. Inbound notifications: error policy

A handler invoked for a notification (id-less inbound) returning
`Err(_)` has nowhere to send the error. Today the framework silently
discards. Proposed: log at `tracing::warn!` with method and error,
keep discarding on the wire. There is no spec-conformant alternative.

## What this looks like for a plugin author

Before:

```rust
async fn call(&self, req: Request) -> Result<Response, FittingsError> {
    let row = db.fetch_one(&q).await
        .map_err(|e| FittingsError::internal(format!("{e}")))?;
    Ok(Response { id: req.id, result: json!({"ok": true}), metadata: Default::default() })
}
```

After:

```rust
async fn call(&self, req: Request, ctx: ServiceContext)
    -> Result<Response, FittingsError>
{
    let row = db.fetch_one(&q).await
        .map_err(|e| e.into_service_error(2001))?;
    ctx.notify("progress", json!({"step": 1}))?;
    Ok(Response { id: req.id, result: json!({"ok": true}), metadata: Default::default() })
}
```

The plugin author now:

- gets `?` for the common case;
- can attach a real domain code (2001) without it being silently
  rewritten to `-32603`;
- can emit notifications;
- does not have to think about panics, transport drops, or
  encoding — those are framework concerns.

## Migration

This RFC is mostly additive on the wire (adds `data` markers,
preserves message/data on decode) and breaking on the trait
(adds `ctx`, restructures `FittingsError` from tuple-variants to
struct-variants). Coordinate with the notifications RFC: both
land together in one PR series since they touch the same trait.

Code-search hits to update:

- `fittings/crates/macros/src/expand.rs` — handler invocation site.
- `fittings/crates/server/src/server.rs` — execute_request,
  send_single_response.
- `fittings/crates/wire/src/error_map.rs` — both directions.
- `fittings/crates/client/src/lib.rs` — decode error mapping.
- `fittings/examples/mcp-server/src/mcp.rs` — error construction
  sites; the custom serve loop disappears (per other RFC).
- All in-tree `Service`/`MethodRouter` impls.

Total change size: roughly 700 lines across the workspace, of which
~250 is tests.

## Open questions

1. **Struct-variant `FittingsError` vs keeping tuple variants and
   adding a sibling `ServiceErrorContext` struct.** Struct variants
   read better in match arms but break every `matches!` test. About
   forty test sites today. Tractable but noisy.
2. **Should we preserve `data` from `from_error_envelope` for the
   pre-defined codes, or only for `Service`?** The spec is silent.
   Recommendation: preserve everywhere. Information should not be
   destroyed in transit.
3. **Backtraces.** Do we capture `std::backtrace::Backtrace` for
   `Internal` and `Panic`? Cheap when `RUST_BACKTRACE=0` (just an
   atomic check), useful for debugging. Recommendation: yes, behind a
   feature flag `backtrace`, on by default.
4. **Should `IntoServiceError` take a `code` argument or pull it from
   a trait the source error implements?** Argument form is simpler
   and forces explicit thought at the call site. Trait form is
   ergonomic for crate authors who want one canonical mapping. v1 ships
   the argument form; trait form can come later non-breakingly.
5. **Cancellation as an error?** When `ctx.cancelled()` fires mid-call,
   does the handler return `Err(FittingsError::Cancelled)` or just
   stop emitting and return whatever partial result it has?
   Recommendation: a new variant `Cancelled { reason: Option<String> }`,
   and the server *suppresses* the response entirely when this variant
   surfaces (matching MCP semantics: cancelled requests do not get a
   response). This couples slightly with the notifications RFC; flagged
   there too.

## Acceptance criteria

- A handler can return `Err(some_io_error.into_service_error(2001))`
  and the peer receives `code: 2001, message: "...", data: null`.
- A handler returning `Err(FittingsError::InvalidParams { message,
  data: Some(json!({"field":"x"})) })` causes the peer's
  `Client::call` to resolve to `Err(FittingsError::InvalidParams {
  message, data: Some(json!({"field":"x"})) })` — message and data
  preserved.
- A handler panic produces `tracing::error!` server-side with the
  panic payload, and the peer sees `code: -32603,
  data: { "fittingsKind": "panic", "detail": "..." }`.
- A transport drop mid-call produces no response on the wire and a
  single `tracing::error!` server-side; in-flight workers are
  cancelled via the same token used by the notifications RFC.
- All five JSON-RPC pre-defined codes round-trip without information
  loss.
