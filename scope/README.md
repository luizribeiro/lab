# scope

Non-interactive CLI "web browser" for AI agents. A single Rust binary that
exposes two commands — `scope read <url>` and `scope search <query>` — and
prints the result to stdout as Markdown (or JSON). URLs are routed to
"reader" backends and searches to "search provider" backends; both kinds
of backend are pluggable via a small subprocess JSON protocol
(`scope-json-v1`) so new sites and engines can be added without recompiling.

Built-ins:

- a generic HTML reader that fetches any `http`/`https` URL or reads a
  local `file://` HTML file and converts the page to Markdown
- a DuckDuckGo search provider

## Install

Prebuilt binary (macOS arm64/x86_64, Linux arm64/x86_64):

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/luizribeiro/lab/releases/latest/download/scope-installer.sh | sh
```

Or build from source:

```
nix build .#scope
# or
cargo build --release --manifest-path scope/Cargo.toml
```

The binary lands at `scope/target/release/scope`.

## Usage

```
scope [--config PATH] [--format markdown|json] read [--reader NAME] <url>
scope [--config PATH] [--format markdown|json] search [--provider NAME] [--limit N] <query>
```

Markdown is written to stdout and errors to stderr. The process exits
non-zero on failure.

### `scope read`

```
$ scope read https://example.com/
# Example Domain

Source: <https://example.com/>

This domain is for use in illustrative examples in documents...
```

`--reader NAME` forces a specific reader instead of letting the registry
pick one by route match.

### `scope search`

```
$ scope search --limit 2 'rust async'
# Search results for `rust async`

1. [Asynchronous Programming in Rust](https://rust-lang.github.io/async-book/)
   The async book...

2. [Tokio - An asynchronous Rust runtime](https://tokio.rs/)
   Tokio is an asynchronous runtime...
```

`--provider NAME` overrides the configured default search provider.
`--limit N` caps the number of results.

`--format json` emits the underlying `ReadOutput` / `SearchOutput`
struct as pretty-printed JSON instead of Markdown.

## Configuration

`scope` looks for a TOML config at `$XDG_CONFIG_HOME/scope/config.toml`
(falling back to `~/.config/scope/config.toml`). Pass `--config PATH` to
use a specific file. Unknown fields are rejected.

A complete example:

```toml
default_search_provider = "duckduckgo"

[http]
timeout_secs = 20
max_body_bytes = 5_000_000
user_agent = "scope/0.1"

[[readers]]
name = "wikipedia"
command = ["python3", "/path/to/wiki_reader.py"]
protocol = "scope-json-v1"
priority = 100
routes = [
    { host_suffix = "wikipedia.org" },
]

[[search_providers]]
name = "kagi"
command = ["python3", "/path/to/kagi_search.py"]
protocol = "scope-json-v1"
```

Reader `routes` entries support `scheme`, `host`, `host_suffix`, and
`path_prefix` (any subset). The reader registry picks the highest
`priority` whose route matches; ties break on the most specific route.
The built-in HTML reader acts as the fallback when nothing else matches.

## Plugin protocol: `scope-json-v1`

External readers and search providers are spawned as subprocesses. For
each request, `scope` writes a single JSON object to the plugin's stdin,
closes stdin, and reads a single JSON object from stdout. Anything
written to stderr is captured and surfaced on error. A non-zero exit
status or malformed response is treated as a failure. Every message
carries `"schema_version": 1`.

### Reader

Request:

```json
{
  "schema_version": 1,
  "kind": "read",
  "url": "https://example.com/page",
  "options": { "timeout_secs": 20 }
}
```

Success response:

```json
{
  "schema_version": 1,
  "ok": true,
  "title": "Example Page",
  "url": "https://example.com/page",
  "markdown": "# Example\n\n..."
}
```

### Search

Request:

```json
{
  "schema_version": 1,
  "kind": "search",
  "query": "rust async",
  "limit": 10
}
```

Success response:

```json
{
  "schema_version": 1,
  "ok": true,
  "results": [
    { "title": "...", "url": "https://...", "snippet": "..." }
  ]
}
```

### Errors

A plugin may signal failure with:

```json
{ "schema_version": 1, "ok": false, "error": "human readable message" }
```

## Python plugin examples

`scope/examples/` ships two minimal plugins and a sample config:

- `reader_plugin.py` — reads a request from stdin and returns a static
  Markdown document
- `search_plugin.py` — synthesizes `limit` fake results for the query
- `plugins.toml` — registers both with `scope`

Run scope against them with:

```
scope --config scope/examples/plugins.toml read https://example.com/
scope --config scope/examples/plugins.toml search --provider example 'hello'
```

Any language works — a plugin is just a process that reads one JSON
object from stdin and writes one JSON object to stdout.

## Embedding API

`scope` is also a Rust library. Build a `Scope` from a `Config` and use
its registries directly:

```rust
use scope::{Config, Scope, ReadRequest, ReadOptions};
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let scope = Scope::from_config(&Config::default())?;
    let url = Url::parse("https://example.com/")?;
    let reader = scope.readers.pick(&url, None)?;
    let output = reader
        .read(ReadRequest {
            url: url.to_string(),
            options: ReadOptions::default(),
        })
        .await?;
    println!("{}", output.markdown);
    Ok(())
}
```

`Scope::from_config` registers the built-in HTML reader and DuckDuckGo
search provider, plus any `[[readers]]` / `[[search_providers]]` from
the config. You can also register your own `Reader` / `SearchProvider`
implementations on the registries.
