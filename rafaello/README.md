# rafaello

Minimal, customizable coding agent. Binary name: `rfl`.

Rafaello is a project-scoped coding agent built around a small Rust
core (provider, tool dispatch, event bus, session store) with
capabilities delivered as sandboxed plugins gated through a
manifest + lock + policy pipeline. The v1 scope cut — lockin
sandbox, four-namespace bus broker, core-mediated sink confirmation,
default ratatui TUI, the `rfl init / install / grant / revoke /
update / provider use / status / chat` CLI surface, bundled
`rfl-openai` default provider, built-in Rust renderers,
turn-by-turn entry persistence, lazy loading, and fittings v1 — is
defined in [`plans/overview.md` §16](./plans/overview.md). See
that document for what is in v1, what is deferred to v2, and what
is explicitly out of scope.

## Bootstrap

Five lines from a fresh project directory to a running chat
session:

```
cd ~/your/project
nix develop .#rafaello --impure --command rfl init
export LITELLM_API_KEY=…
nix develop .#rafaello --impure --command rfl install rfl-mailcat
nix develop .#rafaello --impure --command rfl chat
```

The first `nix develop` invocation may take several minutes the
first time (the rafaello toolchain is being fetched and built);
subsequent invocations are cached.

## Installation

Two supported install paths.

### Nix flake (recommended)

The lab flake exposes a `rafaello` dev shell that wires up the
`rfl` binary together with the sandbox helper `syd-pty` and the
provider/tool plugin set:

```
nix develop github:luizribeiro/lab#rafaello --impure
```

Inside the shell, `rfl`, `rfl-openai`, `rfl-mailcat`, and the
lockin sandbox binaries are on `PATH`, and the
`CARGO_BIN_EXE_syd-pty` environment variable points at the
sandbox helper that `rfl chat` invokes for tool calls.

`--impure` is required because rafaello reads `LITELLM_API_KEY`
(or `OPENAI_API_KEY` against a vanilla OpenAI deployment) from
the ambient environment at spawn time, per `plans/overview.md`
§17.

### Homebrew

For a Nix-free install on macOS or Linuxbrew, tap the rafaello
formula and `brew install`:

```
brew tap luizribeiro/rafaello
brew install rafaello
```

The Homebrew bottle ships `rfl` along with the same plugin set
and sandbox helper, with the `syd-pty` discovery path baked into
the formula at build time.

## Architecture at a glance

The agent is a Rust binary `rfl` plus a tree of plugin processes
connected by a bus broker. `rfl init` writes a project anchor
(`.rfl/`) into the working directory; `rfl install` resolves and
locks plugin manifests against the configured registry; `rfl
chat` spawns the default ratatui TUI as a local bus principal,
brings up the provider plugin, and runs tool calls through the
lockin sandbox under the policy that the manifest+lock
describes. The full picture — bus namespaces, manifest schema,
renderer model, fittings, lazy-loading triggers — lives in
[`plans/overview.md`](./plans/overview.md), with §16 calling out
exactly what v1 ships.

## Troubleshooting

**`rfl chat` exits immediately with a `syd-pty` discovery
error.** Make sure you're inside `nix develop .#rafaello
--impure` (which exports `CARGO_BIN_EXE_syd-pty`), or install
the m6-or-newer release that ships the lockin sandbox `syd-pty`
discovery fix. A bare `cargo run` from a checkout without that
environment variable cannot find the sandbox helper and the chat
session will refuse to start.

**`rfl install <plugin>` fails with a manifest verification
error.** Re-run `rfl install` after `rfl update` so the lock
sees the current manifest revision; if the failure persists, the
manifest is genuinely rejecting the plugin and the error text
identifies the offending field.

**The provider returns 401 on the first chat turn.** Check that
`LITELLM_API_KEY` (dev endpoint) or `OPENAI_API_KEY` (vanilla
OpenAI) is exported in the shell that runs `rfl chat`. Provider
plugins read the key from the env var that the lock declares in
`env.pass` at spawn time; there is no keystore in v1
(`plans/overview.md` §17 #5).

### Pre-m6 workaround

> **Use only against pre-m6 builds — m6+ does not need this.**

m5 and earlier releases shipped without the lockin sandbox
`syd-pty` discovery fix. If you are pinned to a pre-m6 revision
and cannot upgrade, export `CARGO_BIN_EXE_syd-pty` manually
before launching `rfl chat`:

```
export CARGO_BIN_EXE_syd-pty=$(which syd-pty)
rfl chat
```

m6 and later wire this into the `nix develop .#rafaello` shell
hook and the Homebrew formula, so on a current build the recipe
above is a no-op at best and a footgun at worst — it pins to
whatever `syd-pty` is first on `PATH`, which may disagree with
the version the rafaello shell was built against. Drop the
workaround as soon as you've updated.

## Further reading

- [`plans/overview.md`](./plans/overview.md) — full architecture
  brief; §16 is the v1 scope cut.
- [`plans/decisions.md`](./plans/decisions.md) — numbered
  decision log referenced from the overview.
- [`plans/streams/`](./plans/streams/) — per-stream RFCs
  (security model, fittings, renderer, manifest, scripting).
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — development setup,
  commit conventions, review flow.
