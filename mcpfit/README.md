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
