# mcp-server example

An MCP-style tool-calling server built with [`mcpfit`](../../).

## What this is

`src/main.rs` defines five `#[tool]` functions and registers them with a
`Server`:

- `echo` — text-only return
- `add` — scalar (`f64`) text-only return
- `add_with_details` — `Structured<T>` with structured + text content
- `long_running_demo` — uses `cx.check_cancelled()?` for `notifications/cancelled`
- `progress_demo` — emits `notifications/progress` via the `Cx` builder

The whole binary is ~100 lines because all the protocol plumbing lives in
`mcpfit`.

## Why there is JavaScript here

`package.json` and `scripts/check-with-mcp-sdk.mjs` exist only to test
interoperability against a real MCP client implementation
(`@modelcontextprotocol/sdk`). The server is still Rust.

## Running the server

From the repo root:

```bash
cargo run -p mcp-server -- serve
```

(Uses stdio transport.)

## Running Rust tests

From the repo root:

```bash
cargo test -p mcp-server
```

This runs both unit tests and a `stdio_e2e` integration suite that spawns the
binary and exercises real MCP stdio sessions.

## Running the real-MCP-client compatibility check

```bash
cd mcpfit/example
npm install
npm run check:real-client
```

By default this runs `cargo run -q -p mcp-server -- serve` and then performs:

1. initialize
2. tools/list
3. tools/call (`echo`)
4. tools/call (`add`)
5. tools/call (`add_with_details`, text + structuredContent)

using the official MCP SDK client.

## Optional env overrides for the JS check

- `MCP_SERVER_COMMAND` — server binary path
- `MCP_SERVER_ARGS` — args passed to it

```bash
MCP_SERVER_COMMAND=./target/debug/mcp-server MCP_SERVER_ARGS="serve" \
  npm run check:real-client
```
