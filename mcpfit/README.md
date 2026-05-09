# mcpfit

Macro-first MCP server framework on top of [`fittings`](../fittings).

The headline interface is the `#[tool]` proc macro. A public builder API sits
underneath for dynamic and manual tools.

## A complete server

```rust
use mcpfit::{Result, Server, tool};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize)]
struct AddArgs { a: f64, b: f64 }

/// Adds two numbers.
#[tool]
async fn add(args: AddArgs) -> Result<f64> {
    Ok(args.a + args.b)
}

#[tokio::main]
async fn main() -> Result<()> {
    Server::new("demo", env!("CARGO_PKG_VERSION"))
        .tool(add::TOOL)
        .run_entrypoint()
        .await
}
```

`#[tool]` rewrites the function into a module exposing `pub const TOOL: ToolSpec`.
The doc comment becomes the description; the argument type drives the JSON schema
via `schemars`.

## Handler shapes

Handlers are always `async`. Two accepted signatures:

```rust
async fn name(args: A) -> Result<R>
async fn name(args: A, cx: Cx) -> Result<R>
```

No-args tools use `args: ()`, which generates an empty-object schema.

## Return types

The return type controls the response shape — no runtime magic.

| Return                             | Response                                     |
|------------------------------------|----------------------------------------------|
| `String`, `&'static str`           | text-only                                    |
| `f64` / `i64` / `u64` / `bool` etc | text-only via `Display`                      |
| `Structured<T>`                    | structured content + compact-JSON text       |
| `ToolResponse`                     | full control: multi-content, `is_error`, ... |

```rust
#[derive(Serialize, JsonSchema, StructuredObject)]
struct AddOut { sum: f64 }

/// Adds with structured output.
#[tool]
async fn add_structured(args: AddArgs) -> Result<Structured<AddOut>> {
    Ok(Structured::new(AddOut { sum: args.a + args.b }))
}
```

## Cancellation and progress

`Cx` wraps the fittings `ServiceContext`:

```rust
/// Sleeps, with cooperative cancellation.
#[tool]
async fn nap(_args: (), cx: Cx) -> Result<&'static str> {
    cx.check_cancelled()?;
    cx.progress(1).total(1).message("done").emit();
    Ok("ok")
}
```

## Builder API (dynamic / manual tools)

`#[tool]` is sugar over `Tool`. Reach for the builder when tools are constructed
at runtime or you want to hand-tune the schema:

```rust
let add = Tool::new("add")
    .description("Adds two numbers")
    .input::<AddArgs>()
    .handler(|args: AddArgs, _cx: Cx| async move { Ok(args.a + args.b) });

Server::new("demo", env!("CARGO_PKG_VERSION")).tool(add).run_entrypoint().await
```

Schema variants are mutually exclusive: `.input::<T>()`,
`.input_schema(json!({...}))`, or `.input_with_schema::<T>(json!({...}))`.

## Runtime mutability, errors, transport

Tools are immutable by default. `Server::allow_runtime_registration()` exposes
client-callable `tools/register`; `Server::allow_dynamic_tools()` advertises
`tools.listChanged` without exposing client registration.

`Err(McpfitError::...)` becomes a JSON-RPC error; `Ok(ToolResponse::error(...))`
becomes a successful response with `isError: true`. The `String` / `Structured<T>`
shorthands always produce `isError: false`.

`Server::serve_stdio()` is the embedder/test entrypoint; `Server::run_entrypoint()`
adds the `FITTINGS` env + first-arg `serve` dance.

## Client

`mcpfit::Client` connects to an MCP server over any `fittings::Connector` and
exposes the protocol as typed Rust calls.

```rust
use mcpfit::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> mcpfit::Result<()> {
    let client = Client::spawn("./my-mcp-server").await?;
    for tool in client.list_tools().await? {
        println!("{}", tool.name);
    }
    let response = client.call_tool("add", json!({"a": 1, "b": 2})).await?;
    println!("{:?}", response.content);
    Ok(())
}
```

`Client::spawn` is sugar for `Client::connect(SubprocessConnector::new(cmd))` —
it inherits the `FITTINGS=1` env + first-arg `serve` injection from
`SubprocessConnector`, so the same binary that runs `Server::run_entrypoint()`
on the server side spawns cleanly.

### Handshake

`Client::connect` performs the full MCP handshake (`initialize` then
`notifications/initialized`) and returns a ready client. Use the explicit form
to inspect the `InitializeResult` or interleave setup work:

```rust
let client = Client::connect_uninitialized(connector).await?;
let info = client.initialize().await?;
// inspect info.capabilities, info.server_info, ...
client.initialized().await?;
```

### Tool calls

```rust
client.list_tools().await?;                      // Vec<ToolInfo>
client.call_tool_raw(name, args).await?;          // ToolResponse, isError passthrough
client.call_tool(name, args).await?;              // Err(ToolFailed) on isError: true
```

`args` is any `Serialize` value — typed structs work directly, no manual JSON
construction required. `call_tool` maps `isError: true` into
`McpfitError::ToolFailed(ToolResponse)`; `call_tool_raw` hands the response
back unchanged so callers can inspect tool-reported failures themselves.

### Progress and cancellation

Progress-enabled calls use a separate handle-style API. mcpfit generates a
progress token, injects it into `tools/call.params._meta.progressToken`, and
routes matching `notifications/progress` to a per-call channel:

```rust
let mut call = client
    .call_tool_with_progress("work", json!({"n": 100}))
    .start()
    .await?;

while let Some(event) = call.progress().recv().await {
    println!("{}/{:?}: {:?}", event.progress, event.total, event.message);
}

let response = call.await?;             // ToolResponse, errors on isError
// or, to abort early:
// call.cancel().await?;                // sends notifications/cancelled
```

The per-call progress channel is bounded; if a slow consumer can't keep up the
router drops events rather than blocking and bumps a missed counter exposed as
`call.missed_progress_count()`. Plain `call_tool` futures don't auto-cancel on
drop — use `ToolCallHandle::cancel` explicitly when you want the server
notified.

### Raw notifications

`client.notifications()` returns a `broadcast::Receiver<InboundNotification>`
delegating to the underlying fittings subscription — useful for
`notifications/tools/list_changed` and debugging.
