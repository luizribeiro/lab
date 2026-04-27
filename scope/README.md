# scope

Non-interactive CLI "web browser" for AI agents. Reads URLs and runs
searches, printing the result to stdout as Markdown. URLs are routed to
"reader" backends and searches to "search provider" backends; both kinds
of backend are pluggable via a small subprocess JSON protocol so new
sites and engines can be added without recompiling.

Built-ins:

- a generic HTML reader that fetches any `http`/`https` URL or reads a
  local `file://` HTML file and converts the page to Markdown
- a DuckDuckGo search provider

## Install

Homebrew (macOS, Linux):

```
brew install luizribeiro/tap/scope
```

Or grab a prebuilt binary from the [latest release][releases] (macOS
and Linux, x86_64 and arm64).

[releases]: https://github.com/luizribeiro/lab/releases?q=scope

## Usage

```
scope read <url>
scope search [--limit N] <query>
```

Markdown is written to stdout, errors to stderr. The process exits
non-zero on failure.

### `scope read`

```
$ scope read https://example.com/
# Example Domain

Source: <https://example.com/>

This domain is for use in illustrative examples in documents...
```

`scope read file:///path/to/page.html` works for local HTML files too.

`--provider NAME` forces a specific reader instead of letting the
registry pick one by route match.

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

### `scope providers`

Lists registered readers and search providers (built-in and configured).

## Configuration

`scope` looks for a TOML config at `$XDG_CONFIG_HOME/scope/config.toml`
(falling back to `~/.config/scope/config.toml`). Pass `--config PATH` to
use a specific file. Unknown fields are rejected.

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

## Plugins

External readers and search providers are spawned as subprocesses
speaking the `scope-json-v1` protocol: one JSON request on stdin, one
JSON response on stdout. Any language works.

```json
{ "schema_version": 1, "kind": "read", "url": "https://example.com/", "options": { "timeout_secs": 20 } }
```

```json
{ "schema_version": 1, "ok": true, "title": "Example", "url": "https://example.com/", "markdown": "# Example\n..." }
```

For the full protocol (search requests, error responses, search results
schema) and runnable Python examples, see
[`examples/`](./examples/) and
[CONTRIBUTING.md](./CONTRIBUTING.md#plugin-protocol-scope-json-v1).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup, building
from source, the embedding API, and the full plugin protocol reference.
