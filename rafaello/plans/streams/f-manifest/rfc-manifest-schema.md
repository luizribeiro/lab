# RFC: rafaello plugin manifest schema

Status: historical-as-of-2026-05-07. The body below is the
round-2 design; **the live v1 schema diverges in several
ratified ways**. m1 retrospective added this status banner;
the body is unchanged. The live schema is the union of:

- this RFC body (the historical baseline), MINUS
- the deferrals in `decisions.md` rows 26 (helper plugins to
  v2), 30 (no `runtime` field), 31 (`[rpc]` block dropped;
  `openrpc.json` sibling required), 32 (compiler emits lockin
  structured `CompiledPlugin` plan applied via lockin's Rust API at spawn time, not `lockin.toml`), PLUS
- the §15.1 normative-delta items in `overview.md` (the
  `[provides]` block, the `[provides.tool.<n>]` table with
  `sinks` / `grant_match` / `always_confirm`,
  install-time validation rules), MINUS
- §11 open question 1 (scoped bundles flatten at compile time,
  per `decisions.md` row 17), and §11 open question 7 (eager
  handshake is fail-closed in v1, per `decisions.md` row 24 —
  no `load.eager_failure` knob).

Concretely:

| RFC text in the body | Live v1 position |
|----------------------|------------------|
| §2 `runtime = "subprocess"` example field | DROPPED — `decisions.md` row 30; m1 manifest parser rejects it. |
| §3 `[rpc]` block (`openrpc` field + inline `[[rpc.methods]]`) | DROPPED — `decisions.md` row 31; m1 rejects `[rpc]` and requires an `openrpc.json` sibling at the manifest's parent dir. |
| §4 `bus.publishes` examples without `[provides]` | EXTENDED — see `overview.md` §15.1: tools / provider / sinks / grant_match / always_confirm live in a top-level `[provides]` block in v1. |
| §6 `[[renderers]]` non-built-in kinds | EXTENDED — must use the Stream E `<vendor>:<kind>` prefix grammar; built-in kinds (`text`, `code_block`, etc.) are reserved per `overview.md` §11. |
| §7.3 boot-sequence `eager` knob references | NARROWED — fail-closed only, no `load.eager_failure` field per `decisions.md` row 24. |
| §8 "writes `lockin.toml`" | NARROWED — m1 emits a structured `CompiledPlugin` plan against the lockin Rust API per `decisions.md` row 32; m2 applies it. |
| §9.x worked examples with `runtime` / `[rpc]` / `${secret:...}` | OBSOLETE — m1's fixtures are inline / programmatic (TempDir-based) inside the test files under `rafaello/crates/rafaello-core/tests/`; no `tests/fixtures/` directory exists or needs to. |
| §11 open question 1 (scoped bundles in v1) | RESOLVED — accept + flatten at compile time per `decisions.md` row 17. |

Stream F drift items 1–4 from `overview.md` §15.1 are
authoritative for the live v1 schema. m1 implementation
(`rafaello/crates/rafaello-core/`) follows the live schema, not
the body of this RFC. Future revisions to the body are
deferred to a Stream F post-v1 cleanup pass.

---

(Body below preserved as the round-2 design baseline.)

## 1. Three-artifact pipeline

```
plugin author writes        rfl install --review writes        runtime derives
+----------------+          +-------------------+              +----------------+
| rafaello.toml  | -------> | rafaello.lock     | -----------> | lockin.toml    |
| (request)      |  user    | (grant + digest)  |  spawn time  | (enforcement)  |
+----------------+  edits   +-------------------+              +----------------+
```

The manifest is the plugin author's **request**. The lock is the
user's **grant**, plus a content digest computed by rafaello. The
lockin policy is **derived** from the lock at spawn time and never
hand-edited. The agent process and the LLM never write the lock;
only `rfl install` / `rfl grant` do (Stream A).

This RFC defines the manifest. The lock is a near-clone of the
manifest with `granted = true|false` per capability and a
`digest = "sha256:..."` for the plugin payload; `rfl install`
copies the manifest, presents diffs to the user, and writes the
result.

## 2. Top-level shape

The manifest file is `rafaello.toml`, sitting at the root of a
plugin's package. One file per plugin.

```toml
schema = 1                                    # manifest format version
name   = "rust-tools"                         # plugin name (unique within source)
version = "0.4.2"                             # semver
runtime = "subprocess"                        # backend: "subprocess" (v1) | "capsa" (v2)
entry   = "bin/rust-tools"                    # path inside the plugin package

description = "Cargo + rustc helpers"
authors     = ["luiz@example.com"]
license     = "MIT"
homepage    = "https://example.com/rust-tools"

# rafaello-version compatibility (semver req)
rafaello = ">=0.1, <0.2"
```

Identity in the lock is `<source>:<name>@<version>` plus
`digest = "sha256:..."`. The manifest declares `name` and
`version`; `source` and `digest` are added by rafaello at install
time. Plugins cannot self-attest a digest.

`runtime` exists so that v2 swaps `"subprocess"` → `"capsa"` with
no other manifest changes. The schema below is identical for both;
only the backend that consumes the lock differs.

## 3. JSON-RPC surface

```toml
[rpc]
# OpenRPC document, inline or by reference. fittings already supports
# this; we reuse it verbatim.
openrpc = "openrpc.json"                      # file path inside package

# Or, for tiny plugins, declare methods directly:
[[rpc.methods]]
name        = "rust.format"
summary     = "Format a Rust source file"
params      = { schema = "schemas/format-params.json" }
result      = { schema = "schemas/format-result.json" }
idempotent  = true
```

Either `rpc.openrpc` (preferred — single source of truth for
fittings clients) **or** an inline `[[rpc.methods]]` array, never
both. rafaello validates schemas at install time; mismatches fail
`rfl install`.

`idempotent = true` is a hint for the agent loop's retry/cache
layer; it does not affect sandboxing.

## 4. Bus topics

```toml
[bus]
subscribes = [
  "core.session.started",
  "core.fs.changed",                          # plugin filters payload by extension
]
publishes = [
  "plugin.<topic-id>.rust.diagnostics",
  "plugin.<topic-id>.rust.format.completed",
]
```

Topic strings follow Stream A's broker grammar — dot-separated
lowercase segments (`segment := [a-z0-9_-]+`), with `*` and
`**` as whole-segment wildcards in subscribe patterns only
(see security RFC §5.1 / `overview.md` §4.2). The earlier
`namespace.event[:filter]` notation in this section was
**stale**: `:` is illegal inside a topic segment, and
payload-level filtering is the plugin's job, not the broker's.

Publish authority follows the four-namespace model in
security RFC §5.2 / `overview.md` §4.3:

- `core.*` is core-published only — a plugin cannot list any
  `core.*` topic under `publishes`. `rfl install` rejects
  manifests that try.
- `provider.<provider-id>.*` is publishable only by the
  plugin whose `[provides] provider = "<provider-id>"`
  matches the segment.
- `plugin.<topic-id>.*` is publishable by the plugin whose
  hashed canonical id matches `<topic-id>`. `rfl install`
  computes and validates the topic-id.
- `frontend.<attach-id>.*` is publishable by frontends only,
  not plugins; `rfl install` rejects plugin manifests that
  list these topics under `publishes`.

## 5. Capabilities

This block maps **directly** onto lockin's TOML; the field names
are identical so compilation is mechanical (see §8).

```toml
[capabilities.filesystem]
read_paths  = ["${project}/Cargo.toml", "${project}/Cargo.lock"]
read_dirs   = ["${project}/src", "${home}/.cargo/registry"]
write_dirs  = ["${project}/target"]
exec_paths  = ["/usr/bin/rustc", "/usr/bin/cargo"]
exec_dirs   = []                              # rare; require justification

[capabilities.network]
mode        = "proxy"
allow_hosts = ["crates.io", "*.crates.io", "static.crates.io"]

[capabilities.env]
pass = ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"]
set  = { CARGO_TERM_COLOR = "always" }

[capabilities.limits]
max_cpu_time   = 300
max_open_files = 1024
```

### Variable substitution

A small, fixed set of placeholders is expanded at lock-derivation
time, not at manifest-parse time:

| Placeholder    | Resolves to                                       |
|----------------|---------------------------------------------------|
| `${project}`   | The directory `rfl init` was run in.              |
| `${home}`      | `$HOME` of the rafaello user.                     |
| `${plugin}`    | The plugin's installed package directory.         |
| `${cache}`     | `${home}/.cache/rafaello/<plugin-name>`           |
| `${state}`     | `${home}/.local/state/rafaello/<plugin-name>`     |

No arbitrary env-var interpolation, no command substitution. The
list is closed; adding to it is a manifest schema bump.

### Capability scoping

`[capabilities]` may be split into **scoped sub-tables** so that a
plugin author can declare different capability bundles per-method
or per-trigger. Granting is per-bundle:

```toml
[capabilities.default.filesystem]
read_dirs = ["${project}/src"]

[capabilities."rust.format".filesystem]
read_dirs  = ["${project}/src"]
write_dirs = ["${project}/src"]               # only when formatting
exec_paths = ["/usr/bin/rustfmt"]
```

The `default` bundle applies to any RPC method or bus subscription
not otherwise scoped. Method-scoped bundles **add to** the default
(union semantics) — they do not replace it. Scoping lets the user
grant `rust.lint` but withhold `rust.format`'s write authority,
without rejecting the whole plugin.

## 6. Renderer registrations

```toml
[[renderers]]
kind     = "rust.diagnostic"                  # Stream E entry kind
priority = 100                                # higher wins on conflict
method   = "render.rust.diagnostic"           # RPC method to invoke
```

A renderer plugin declares the `kind` values it can render; the
agent loop's renderer registry consults these at boot. See Stream E
for the entry/kind model.

## 7. Lazy-loading

### 7.1 Why not "required vs optional"

A binary cut blurs three different things: **what must be
available before the loop starts**, **what's worth paying for at
startup**, and **what should only ever load when needed**. A
plugin that registers a renderer for `rust.diagnostic` does not
need to be running until a `rust.diagnostic` entry exists; eager
loading wastes a process and an `rfl install`-reviewed sandbox per
session. Conversely, a provider plugin must be live before the
loop accepts a turn, or the user's first prompt fails.

The neovim ecosystem converged on a small set of triggers
(lazy.nvim's `event` / `cmd` / `ft` / `keys`, packer's `cond`,
mini.deps's `later`). rafaello adopts a parallel small set, sized
to what the agent loop actually exposes.

### 7.2 Triggers supported in v1

| Trigger    | Fires when                                            | Manifest field                  |
|------------|-------------------------------------------------------|---------------------------------|
| `eager`    | Boot, before the loop accepts user input.             | `load = "eager"`                |
| `boot`     | Boot, but in parallel with first input.               | `load = "boot"`                 |
| `event`    | A bus topic the plugin subscribes to fires.           | `load.event = ["fs.changed"]`   |
| `command`  | An RPC method the plugin exposes is dispatched.       | `load.command = ["rust.format"]`|
| `kind`     | A renderer kind it provides is requested.             | `load.kind = ["rust.diagnostic"]` |
| `manual`   | User invokes `rfl plugin start <name>`.               | `load = "manual"`               |

Five triggers (plus `manual`), mapping cleanly onto the
event/command/kind axes the agent loop already exposes. **Not** in
v1: file-pattern (`ft`) and keymap (`keys`) — rafaello has no
analogue of a buffer or a keymap; a `ft`-style trigger would have
to be reinvented as "project type detected" and we'd rather wait
until that abstraction exists in core.

### 7.3 Boot sequence

```
t=0   rfl starts → reads rafaello.lock
t=1   spawn all `eager` plugins; wait for handshake (RPC ready)
       └── providers, the security guard plugin, anything with
           grants-checked-at-startup business
t=2   loop accepts user input
       │
       ├── in parallel: spawn `boot` plugins; register their
       │   methods/renderers as soon as ready
       └── lazy plugins remain unspawned, but their manifest
           entries (RPC names, renderer kinds, subscribed events)
           are *registered* in the routing tables as stubs
t=3   first event/command/kind hits a stub → spawn the plugin,
       hold the dispatch until handshake completes, then forward
```

Stubs are critical: the routing tables know about every installed
plugin's surface even before the plugin process exists. The agent
loop never says "no such method"; it says "spawning, please
wait" — which the user perceives as a one-time cold-start hit per
plugin per session.

A plugin with `load = "eager"` blocks the loop. We expect this to
be rare: providers, the security guard, possibly a bus auditor.
Most plugins should be `load = "boot"` or lazy.

### 7.4 Mixing triggers

`load` may be either a string (`"eager"` / `"boot"` / `"manual"`)
or a table with one or more of `event` / `command` / `kind`. If a
table is given, the plugin loads on **first** matching trigger
(union, OR semantics). A scalar shorthand `load = "lazy"` is
sugar for `{ event = [<all subscribed>], command = [<all
methods>], kind = [<all renderers>] }` — load on first access of
anything the manifest declares.

## 8. Compilation to lockin policy

Mechanical, deterministic, no policy decisions at spawn time:
`rfl` reads the lock, takes the union of `default` and any
currently-active scoped capability bundles, expands placeholders,
and writes `lockin.toml`.

### 8.1 Worked example

Manifest excerpt (the `rust-tools` plugin from §5):

```toml
[capabilities.default.filesystem]
read_paths = ["${project}/Cargo.toml", "${project}/Cargo.lock"]
read_dirs  = ["${project}/src", "${home}/.cargo/registry"]
write_dirs = ["${project}/target"]
exec_paths = ["/usr/bin/rustc", "/usr/bin/cargo"]

[capabilities.default.network]
mode        = "proxy"
allow_hosts = ["crates.io", "*.crates.io", "static.crates.io"]

[capabilities.default.env]
pass = ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"]
set  = { CARGO_TERM_COLOR = "always" }

[capabilities.default.limits]
max_cpu_time   = 300
max_open_files = 1024
```

After `rfl install` (user grants everything; `${project}` =
`/home/luiz/work/widget`, `${home}` = `/home/luiz`), the derived
`lockin.toml`:

```toml
command = ["/home/luiz/.local/share/rafaello/plugins/rust-tools-0.4.2/bin/rust-tools"]

[sandbox.network]
mode        = "proxy"
allow_hosts = ["crates.io", "*.crates.io", "static.crates.io"]

[filesystem]
read_paths = [
  "/home/luiz/work/widget/Cargo.toml",
  "/home/luiz/work/widget/Cargo.lock",
]
read_dirs  = [
  "/home/luiz/work/widget/src",
  "/home/luiz/.cargo/registry",
]
write_dirs = ["/home/luiz/work/widget/target"]
exec_paths = ["/usr/bin/rustc", "/usr/bin/cargo"]

[env]
pass = ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"]
set  = { CARGO_TERM_COLOR = "always" }

[limits]
max_cpu_time   = 300
max_open_files = 1024
```

### 8.2 Compilation rules

1. `command[0]` = `${plugin}/<entry>`, resolved absolute; the JSON-RPC
   socket fd is passed via `inherit_fd` (lockin Rust API only;
   not exposed in TOML, so the spawning code uses the Rust API).
2. Placeholders expand to absolute paths.
3. `[capabilities.<bundle>.filesystem|network|env|limits]` →
   `[filesystem|sandbox.network|env|limits]` 1:1, with the bundle
   union taken first.
4. Bus topics, RPC method names, renderer kinds do **not** appear
   in `lockin.toml`; they are enforced inside rafaello core (the
   bus router gates publish/subscribe; the RPC dispatcher gates
   method calls). Lockin enforces only OS-level isolation.
5. Resource limits get conservative defaults (5-minute CPU, 1024
   FDs) if the manifest omits them — never unbounded.

### 8.3 Rafaello-specific extensions (no lockin equivalent)

- `[bus]` — enforced by the rafaello bus router, not OS sandbox.
- `[rpc]` — enforced by the rafaello dispatcher.
- `[[renderers]]` — enforced by the rafaello renderer registry.
- `[load]` — consumed by the supervisor, not the sandbox.
- `[capabilities.<method>]` scoped bundles — rafaello switches the
  effective lockin policy by re-spawning (v1) or by dynamic policy
  reload (future): in v1, scoped bundles flatten into the single
  spawn-time policy as a union, with method-level enforcement
  layered in core. The trade-off is documented in open questions.

## 9. Worked manifests

### 9.1 Tool plugin: `rust-tools`

```toml
schema = 1
name = "rust-tools"
version = "0.4.2"
runtime = "subprocess"
entry = "bin/rust-tools"
rafaello = ">=0.1, <0.2"

[rpc]
openrpc = "openrpc.json"

[bus]
subscribes = ["core.fs.changed"]              # plugin filters by extension in payload
publishes  = ["plugin.<topic-id>.rust.diagnostics"]

[load]
event   = ["core.fs.changed"]                 # payload-level filter applied after wake
command = ["rust.format", "rust.check"]

[capabilities.default.filesystem]
read_dirs  = ["${project}/src", "${home}/.cargo/registry"]
read_paths = ["${project}/Cargo.toml", "${project}/Cargo.lock"]
exec_paths = ["/usr/bin/rustc", "/usr/bin/cargo"]

[capabilities.default.network]
mode        = "proxy"
allow_hosts = ["crates.io", "*.crates.io"]

[capabilities."rust.format".filesystem]
write_dirs = ["${project}/src"]
exec_paths = ["/usr/bin/rustfmt"]

[capabilities.default.limits]
max_cpu_time = 300
```

### 9.2 Renderer plugin: `markdown-pretty`

```toml
schema = 1
name = "markdown-pretty"
version = "1.0.0"
runtime = "subprocess"
entry = "bin/md-renderer"

[rpc]
[[rpc.methods]]
name   = "render.markdown"
result = { schema = "schemas/render-result.json" }
params = { schema = "schemas/render-params.json" }

[[renderers]]
kind     = "markdown"
priority = 50
method   = "render.markdown"

[[renderers]]
kind     = "code.diff"
priority = 50
method   = "render.markdown"

[load]
kind = ["markdown", "code.diff"]

# No filesystem, network, env, or exec capabilities at all —
# this plugin is a pure function. It compiles to a deny-everything
# lockin policy with only the entry binary execable.
```

### 9.3 Provider plugin: `anthropic`

```toml
schema = 1
name = "anthropic"
version = "0.2.0"
runtime = "subprocess"
entry = "bin/anthropic-provider"

[rpc]
openrpc = "openrpc.json"

[provides]
provider = "anthropic"                              # provider-id used in topics

[bus]
# All provider publishes live under `provider.<provider-id>.*`
# per security RFC §5.2.
publishes  = [
  "provider.anthropic.tool_request",
  "provider.anthropic.assistant_message",
  "provider.anthropic.tokens",
  "provider.anthropic.cost",
]
subscribes = [
  "core.session.user_message",
  "core.session.tool_result",
  "core.session.confirm_reply",
]

# Providers must be live before the loop accepts the first turn.
load = "eager"

[capabilities.default.network]
mode        = "proxy"
allow_hosts = ["api.anthropic.com"]

[capabilities.default.env]
pass = ["PATH"]
set  = { ANTHROPIC_API_KEY = "${secret:anthropic_api_key}" }
# `${secret:...}` is the only manifest sigil that resolves against
# rafaello's keystore rather than the static placeholder table —
# see open question #4.

[capabilities.default.filesystem]
read_dirs = ["${state}"]
write_dirs = ["${state}"]

[capabilities.default.limits]
max_cpu_time = 0                              # 0 = no limit; provider runs for the session
```

## 10. Capsa-backend story

The v2/v3 vision is per-tool capsa VMs (thin Nix-built initramfs).
The schema above already contains everything a capsa backend
needs:

- `[capabilities.filesystem]` becomes the set of host paths bind-
  mounted into the guest (read-only or read-write per field).
- `[capabilities.network]` becomes the guest's network policy:
  `deny` = no NIC; `proxy` = NIC bridged to a host-side proxy with
  the same allow-list; `allow_all` = bridged unrestricted.
- `[capabilities.exec_*]` becomes the guest's PATH allowlist (or,
  more likely, the set of binaries baked into the Nix-built guest
  rootfs — at which point exec_paths becomes a *build-time* hint
  rather than a runtime grant).
- `[capabilities.env]` is honored verbatim by the guest init.
- `[capabilities.limits]` maps to vCPU/memory limits on the VM.
- `[bus]`, `[rpc]`, `[[renderers]]`, `[load]` are unchanged —
  they live in rafaello core, above the sandbox boundary.

Switching backends is a `runtime = "subprocess"` →
`runtime = "capsa"` flip in the manifest (and the matching grant
in the lock). **No new fields, no removed fields, no semantic
shift in existing fields.** The compiler-to-lockin in §8 is
replaced by a compiler-to-capsa-config; both consume the same
lock.

The one real difference is reachable defaults: a capsa guest does
not start with `/usr/lib`, `/etc/ld.so.cache`, or
`/etc/ssl/certs` reachable, because those are host paths. The
guest rootfs supplies its own. The manifest doesn't care; the
backend compiler decides what goes into the rootfs vs. what gets
bind-mounted.

## 11. Open questions

1. **Scoped bundles in v1.** §5 allows per-method capability
   bundles, but §8.2(5) flattens them into one spawn-time policy.
   Real per-method enforcement requires either re-spawning per
   call (latency hit) or a future lockin feature for in-process
   policy switching. Should v1 reject scoped bundles entirely and
   ship only `default`, or accept them with the documented
   union-flatten and revisit in v2?

2. **Project-type triggers.** lazy.nvim's `ft` is the most
   commonly used lazy trigger in practice. We dropped it because
   rafaello has no buffer concept. Worth designing a "project
   detected" event (e.g., `Cargo.toml` present at init time fires
   `project.kind:rust`) so the trigger surface can grow back
   without a schema bump?

3. **Manifest signing.** The lock records a content digest, but
   the manifest itself is unsigned. Do we want a `[signing]` block
   so plugin authors can sign manifests with a long-lived key, and
   `rfl install` can warn on key rotation across versions of the
   same plugin?

4. **Secrets sigil.** §9.3 used `${secret:anthropic_api_key}`
   without specifying the keystore. Is that a separate stream
   (Stream A's purview), or does this RFC need to nail it down?

5. **Bus topic ACL grammar — resolved.** Per `overview.md`
   §4.2 and security RFC §5.1, the canonical topic grammar
   forbids `:` and `/` inside segments and uses dot-separated
   pattern segments with `*` (one segment) and `**` (final,
   trailing-only) as the only wildcards. The earlier
   `fs.changed:**/*.rs` examples in §4 / §9.1 above have been
   rewritten to plain `core.fs.changed`; payload-level
   extension filtering is the plugin's responsibility, not the
   broker's.

6. **Renderer priority ties.** §6's `priority` is an integer; ties
   are unspecified. Stream E should pin tie-breaking (insertion
   order? plugin name lex order? user preference list?).

7. **Eager-plugin failure mode.** §7.3 has the loop block on
   eager-plugin handshake. What happens if the handshake never
   arrives — fail open (no plugin, loop runs anyway), fail closed
   (no loop until plugin or user override), or prompt? Default
   should probably be fail-closed-with-timeout, but worth
   confirming.
