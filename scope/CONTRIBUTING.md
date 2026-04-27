# Contributing to scope

## Development environment

The repo uses a [devenv](https://devenv.sh/)-managed Nix shell. With
[direnv](https://direnv.net/) installed, `cd` into the repo and the
shell is loaded automatically. Otherwise:

```
nix develop
```

This provides Rust, `cargo-dist`, `pandoc` (used by some example
plugins), and the other tools used by CI.

## Building

From the `scope/` directory:

```
cargo build --release
```

The binary lands at `scope/target/release/scope`.

Or via Nix:

```
nix build .#scope
```

## Running tests

```
cargo test
```

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

### Python examples

`examples/` ships minimal plugins and a sample config:

- `reader_plugin.py` — returns a static Markdown document
- `search_plugin.py` — synthesizes `limit` fake results
- `wikipedia_plugin.py` / `wikipedia_search_plugin.py` — real plugins
  using the Wikipedia REST API
- `plugins.toml` — registers them with `scope`

Run scope against them with:

```
scope --config examples/plugins.toml read https://example.com/
scope --config examples/plugins.toml search --provider example 'hello'
```

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

## Releasing

Releases are published by the unified `release.yml` workflow when a
scope tag is pushed:

```
git tag scope-vX.Y.Z
git push origin scope-vX.Y.Z
```

Bump `version` in `crates/scope/Cargo.toml` to match before tagging.
The workflow builds prebuilt binaries for macOS / Linux (x86_64 and
arm64) and pushes a Homebrew formula to `luizribeiro/homebrew-tap`.
