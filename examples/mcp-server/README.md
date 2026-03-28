# mcp-server example

This example shows how to build an **MCP-style tool-calling server** on top of `fittings`.

## What this is

- `src/mcp.rs` contains an MCP-oriented service layer:
  - MCP request/response types (`initialize`, `tools/list`, `tools/call`)
  - a small tool registry
  - a `#[fittings::service]` trait with MCP wire method names via `#[fittings::method(name = ...)]`
- `src/main.rs` wires example tools (`echo`, `add`, `add_with_details`) and runs the service.

So this is intentionally a **library-like layer in the example itself** (not a core `fittings` crate), to demonstrate how MCP can be modeled on top of fittings primitives.

## Why there is JavaScript here

You’ll see:

- `package.json`
- `scripts/check-with-mcp-sdk.mjs`

This is **not** because the server is implemented in JS.

The server is still Rust. The JS script exists only to test interoperability against a **real MCP client implementation** (`@modelcontextprotocol/sdk`).

The example tools include `echo`, `add`, `add_with_details`, `long_running_demo` (used to demonstrate `notifications/cancelled`), and `progress_demo` (used to demonstrate `notifications/progress`).

That gives a stronger signal than testing with only our own Rust-side harnesses.

## Running the server

From repo root:

```bash
cargo run -p mcp-server -- serve
```

(Uses stdio transport.)

## Running Rust tests for this example

From repo root:

```bash
cargo test -p mcp-server
```

## Running real MCP client compatibility check

```bash
cd examples/mcp-server
npm install
npm run check:real-client
```

By default this script runs:

```bash
cargo run -q -p mcp-server -- serve
```

and then performs:

1. initialize
2. tools/list
3. tools/call (`echo`)
4. tools/call (`add`)
5. tools/call (`add_with_details`, text + structuredContent)

using the official MCP SDK client.

## Optional env overrides for the JS check

If you want to point at a different server command:

- `MCP_SERVER_COMMAND`
- `MCP_SERVER_ARGS`

Example:

```bash
MCP_SERVER_COMMAND=./target/debug/mcp-server MCP_SERVER_ARGS="serve" npm run check:real-client
```
