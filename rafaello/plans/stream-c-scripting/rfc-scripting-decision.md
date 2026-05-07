# RFC — embedded scripting in rafaello v1

**Stream:** C (scripting)
**Status:** draft, single-author research
**Author:** stream-c agent

## TL;DR

**Recommendation: do NOT include an embedded scripting language in
rafaello v1.** Customisation is served by three planes:

1. **Declarative config** (TOML) for keymaps, prompt templates,
   hook→event wiring, statusline composition, theme.
2. **Subprocess plugins** speaking JSON-RPC over the bus, sandboxed
   by lockin from a manifest, for anything that needs code.
3. **Headless `rfl` driven over the bus** by external processes for
   evals, alternate frontends, and loop replacement.

The embedded plane (Luau via `mlua`) is deferred to v2 and only
revisited if real usage shows the declarative plane cannot hold the
"tiny customization" cases without forcing users to write a plugin
crate.

The single strongest argument: every motivation for the embedded
plane (keymaps, hooks, templates, statusline) collapses into
*declarative configuration that core already has to parse anyway*,
and the cases that are not declarative — custom tools, custom
renderers, the agent loop — are exactly the cases where lockin
isolation pays for itself. Adding Luau is therefore solving a
problem that doesn't exist (declarative cases) and *under*-solving
a problem that does (the trusted-config-vs-untrusted-plugin split is
clearer with one runtime model than two).

The rest of this document walks four UX scenarios in both worlds,
then tallies cost/benefit, then specifies the v1 declarative surface
that the recommendation depends on.

## Scenario 1 — Snappy CLI

UX target: cold start under ~100 ms, sub-frame keystroke latency,
popups/statusline/command-palette feel native.

### With embedded Luau

On startup `rfl` boots the Tokio runtime, opens the session DB,
constructs a `Lua` instance with `mlua` + `luau` feature, calls
`Lua::sandbox(true)`, and `dofile`s `~/.config/rfl/init.luau` plus
the project's `.rfl/init.luau` if present. The init script
registers keymaps, hook callbacks, statusline segments, and any
`/command` definitions by calling functions on a `rfl` global.

Cost: Luau interpreter init is sub-millisecond; the script reads
config and registers tables of callbacks — tens of microseconds for
a typical user file. Per-keystroke a keymap dispatch is a Lua
function call from Rust — sub-microsecond. **Cold start is fine,
keystroke is fine.** The cost is binary size (~1–2 MB), one more
runtime to crash-handle, and one more attack surface (sandboxed,
but exposed Rust functions can still be misused — see CaMeL
discussion).

### Without embedded Luau

`rfl` parses `~/.config/rfl/config.toml` and `.rfl/config.toml`
into a typed config struct. Keymaps are TOML tables. Statusline is
a TOML array of segments referencing built-in or
plugin-contributed segment IDs. Templates are `*.md` files in
`prompts/`. Hooks are TOML stanzas mapping bus topics to plugin
methods.

```toml
# ~/.config/rfl/config.toml
[keymaps]
"ctrl+g"   = "core.message.edit_in_editor"
"ctrl+l"   = "core.history.scroll_to_latest"
"alt+ret"  = "core.input.submit_with_no_confirm"

[statusline]
left  = ["core.session.name", "core.model.id"]
right = ["core.tokens.in_minute", "core.cost.session"]

[[hooks]]
on    = "model.token"
plugin = "otel"
method = "on_token"
```

Cost: TOML parse is sub-millisecond. Keystroke dispatch is a
hashmap lookup of action ID → built-in handler or plugin handler.
For built-in actions there is zero IPC. For plugin handlers there
is one bus dispatch (in-process channel send to the plugin
multiplexer; the plugin process itself is already running because
it was eagerly spawned at startup, or lazily on first matching
event per Stream F). **Cold start is faster** than the Luau case
(no interpreter, no script execution); keystroke latency is the
same in practice for the common case (built-in actions) and one
short pipe write for the plugin case.

The only thing the user cannot do without code is *compute* a
keymap or statusline segment. That is exactly the case where they
should write a plugin — i.e., where being forced down the
subprocess path is correct.

**Verdict on Scenario 1: subprocess-only wins.** It is faster cold,
identical hot for the common case, and forecloses a class of
"wrote a script that imports the world" footguns.

## Scenario 2 — CaMeL as a v1 plugin

CaMeL (arXiv:2503.18813) splits the agent into a **planner LLM**
that produces a structured plan over capability-tagged values, and
a **quoted LLM** that handles untrusted strings. Each tool call is
gated by a capability check: the plan must justify why the
arguments' provenance is acceptable.

To implement CaMeL on rafaello v1, the primitives needed are:

- Bus events for *every* tool call attempt, *before* dispatch
  (`tool.call.requested`), with the right to veto.
- A way to register an alternative "agent loop" — i.e., the thing
  that converts model output into tool calls is replaceable.
- A way to run a second model concurrently (the planner) without
  fighting the first for streaming.
- Provenance/data-flow tracking attached to bus messages: a tool
  result carries metadata indicating which inputs (and which
  upstream tool calls) it derived from.

Two of these — `tool.call.requested` with veto, and
provenance metadata on bus payloads — are bus/manifest design
decisions, independent of scripting language. The interesting one
is loop replacement.

### With embedded Luau

The "default loop" is implemented in Rust. CaMeL author writes a
Luau plugin that calls `rfl.replace_loop(my_loop_fn)`. The Luau
function is awaited per turn, gets `state` and `inbound_event`,
returns `actions`. Internal calls to `rfl.model.complete()` go
through the bus or directly through Rust. Cost: one Luau function
per turn — cheap. Hot-reload: trivial.

Problem: a CaMeL implementation needs *real* code — JSON Schema
constraint solving, AST manipulation of the planner output,
provenance graphs. That is hundreds of lines of non-trivial
logic. Writing it in sandboxed Luau is possible but unpleasant
(no `package`, no `io`, careful about escapes). At which point
the question becomes: why not write it in Rust as a real plugin?

### Without embedded Luau

CaMeL author writes a subprocess plugin in their language of
choice — Python with `pydantic`, TypeScript with `zod`, Rust with
`serde`. The plugin's manifest declares:

- `subscribes_to = ["agent.turn.start", "tool.call.requested",
  "model.response"]`
- `publishes  = ["agent.turn.action", "tool.call.veto",
  "tool.call.replace"]`
- `replaces_loop = true` (a manifest flag that asks the user, at
  install time, to grant exclusive ownership of the agent-loop
  events; rfl's built-in loop unsubscribes when granted).

Loop replacement is therefore a **manifest capability**, not an
in-process API. The bus already routes events; the plugin owns the
turn. The plugin can spawn child plugins (the quoted-LLM model
provider) by routing through the bus, or by hosting its own model
client with a network grant.

Cost: per-turn JSON-RPC round trips. A CaMeL turn already involves
2× model calls and probably 1–10 tool calls — model latency is in
the seconds. Adding a few sub-millisecond pipe round-trips is
noise.

**Verdict on Scenario 2: subprocess plugin is at minimum as good
and arguably better.** "Better" because:

- The CaMeL plugin can use mature libraries (`pydantic`,
  `instructor`, `dspy`) rather than re-implementing them in Luau.
- The provenance graph and policy engine are exactly the kind of
  code that benefits from being in a real language with a real
  type system, not a 2 MB sandboxed scripting interpreter.
- "Loop replacement" becomes a manifest capability rfl can audit
  and lock, instead of an in-process function pointer that the
  config user can transparently rebind. This is a security win.

The two prerequisites for CaMeL — pre-dispatch tool veto events
and provenance metadata — are required from Stream A regardless of
the scripting decision, so the scripting decision does not gate
CaMeL.

## Scenario 4 — Tiny customizations

This is the scenario where subprocess overhead is highest relative
to the value delivered, and where the embedded plane has its
strongest case. Three concrete tasks:

### 4a. A 3-line keymap: `Ctrl+G` opens an editor for the current message

#### With Luau

```lua
-- ~/.config/rfl/init.luau
rfl.keymap("ctrl+g", function(ctx)
  ctx.input.edit_in_editor()
end)
```

Three lines. Hot-reload via `:reload`. Cost: negligible.

#### Without Luau

```toml
# ~/.config/rfl/config.toml
[keymaps]
"ctrl+g" = "core.input.edit_in_editor"
```

One line. The action `core.input.edit_in_editor` is a built-in,
exactly because "open the current message in $EDITOR" is the kind
of thing that ships with the agent. The user is *picking* an
action from the available action namespace, not authoring one.

This is the right primitive. Neovim's `vim.keymap.set` exists
because the action set is too large and too dynamic to enumerate
declaratively, but rafaello's action set is small (probably under
200 actions in v1) and its dynamism is provided by plugins, which
*also* register actions by ID into the same namespace.

What if the user wants to compose actions? "Ctrl+G saves a
checkpoint and *then* opens the editor." Two options:

```toml
[keymaps]
"ctrl+g" = ["core.session.checkpoint", "core.input.edit_in_editor"]
```

A bare array means "run in sequence." This handles 95% of
compositions. For anything more elaborate, the user writes a
plugin — which is also where they should be writing it for
auditability.

**Verdict 4a: declarative wins on every axis** including line
count.

### 4b. Hook re-emitting `model.token` to OpenTelemetry

#### With Luau

```lua
-- ~/.config/rfl/init.luau
local otel = require "rfl.otel"
local tracer = otel.tracer("rfl")

rfl.on("model.token", function(ev)
  tracer:span("model.token", {
    session = ev.session_id,
    tokens  = ev.count,
  })
end)
```

This requires `rfl.otel` to be a host-provided Lua module — i.e.,
rfl ships with OpenTelemetry built into the binary and exposes it
to Luau. That's a substantial built-in dependency. Alternative:
the user vendors a pure-Lua OTLP client. Painful and slow.

#### Without Luau

The user installs an `otel` plugin (or writes a tiny one in
~30 lines of Python or Go) and wires it in:

```toml
# .rfl/config.toml
[[plugins]]
source = "github:luizribeiro/rfl-otel@0.1"

[plugins.otel.config]
endpoint = "http://localhost:4318"
service  = "rfl"

[[hooks]]
on     = "model.token"
plugin = "otel"
method = "emit_span"
```

Cost: per-token bus dispatch. Token streaming is high-frequency
(hundreds per second peak), so we should think about this.

Per-token cost: in-process channel send + JSON serialise + pipe
write. On Linux a 200-byte JSON message over a pipe is
~5–20 µs. At 500 tokens/s that's 2.5–10 ms/s of CPU — fine. We
can also batch: rfl can offer `subscribe(topic, batch={ms:50})`
in the manifest, so the otel plugin gets one message with 25
tokens every 50 ms. Then it's microseconds.

The mild downside: the user has to install a plugin to do what
five lines of Lua could do with a host-exposed `rfl.otel`. The
upside: that plugin is sandboxed, its network grants are
auditable, the OTel code is testable in isolation, and it's
useful to other rfl users as a shareable artifact.

**Verdict 4b: subprocess wins on isolation and reusability,
loses slightly on first-time-setup friction.** The friction is
mitigated by `rfl-otel` being one `rfl install` away, which is
how good plugin ecosystems work anyway.

### 4c. One-line prompt template: `/explain` expands to a fixed system prompt

#### With Luau

```lua
rfl.command("/explain", function(args, ctx)
  ctx.session.append_system(
    "Explain the following at the level of a senior engineer."
  )
  ctx.session.append_user(args)
end)
```

#### Without Luau

```markdown
<!-- ~/.config/rfl/prompts/explain.md -->
---
description: Explain code at senior-engineer level
---
Explain the following at the level of a senior engineer.

$ARGUMENTS
```

Pi already does this. The filename becomes the command name. No
language at all. **Strictly better than Lua** because it's
copy-paste-shareable, version-controllable as plain prose, and
diffable.

**Verdict 4c: declarative wins decisively.**

### Aggregate verdict on Scenario 4

Across the three sub-cases the declarative plane is *better*, not
merely adequate. Pi's evidence supports this: pi has TypeScript
extensions but explicitly chose JSON for keybindings and Markdown
for prompts and skills, because those are not coding tasks.
Rafaello can simply skip the TypeScript-equivalent layer.



UX target: an external harness drives `rfl` headless, one instance
per task, possibly tens or hundreds in parallel, each inside its
own dev VM.

### With embedded Luau

The harness has nothing to do with Luau directly. It either:

- spawns `rfl --headless --task task.json` and reads JSON from
  stdout, or
- connects to `rfl daemon` over the bus.

Whether `rfl` has Luau embedded is irrelevant to the harness; it
might be relevant only because the *evaluated agent's*
configuration could include a Luau init file. In which case the
Luau cold-start cost (sub-millisecond) is amortised over the
multi-minute SWE task.

### Without embedded Luau

Identical from the harness's perspective. The agent's
configuration is TOML; the harness can template a TOML config per
task ("for this task, allow writes to `/repo/`, allow these
hosts") and pass it via `--config` or the daemon's `session.open`
RPC.

The harness itself is a perfect demonstration of the third plane:
it is rfl's *parent* over the bus. If the rfl bus is well
specified (Stream B), the harness is a small amount of code in
any language. Luau adds nothing here.

**Verdict on Scenario 3: tied; embedded language is irrelevant.**
The eval scenario is entirely about bus quality, headless-mode
quality, and configuration ergonomics for templated runs. None of
those are language questions.

The mild push *toward* declarative-only: a TOML config is trivially
templatable from the harness language (`format!`, f-strings,
`tera`). A Luau init file is also templatable but with worse
ergonomics (string concatenation into another language is
fiddly). One-runtime-fewer is also one-fewer-thing-to-version
across the eval matrix.



The pushback the project owner raised — "if everything talks the
bus, any language can extend rafaello" — is correct, but it
collapses two questions into one:

1. *What language do plugins (capability-bearing extensions) write
   in?* Answer: any language that can speak JSON-RPC. Already
   settled by the bus design.
2. *What does the user write to bend rafaello to their taste
   without authoring a plugin?* This is the open question.

Neovim answers (2) with Lua. pi answers (2) with TypeScript for
extensions but with **plain files** (`keybindings.json`,
`*.md` prompts, `*.md` skills, `settings.json` shell prefix) for
the truly small cases. Pi is closer to the right answer for
rafaello, because rafaello's value proposition is "secure by
default, very few footguns" and a Turing-complete in-process plane
is by definition a footgun surface — even a sandboxed one.

The third plane is **headless rfl driven from outside**: an eval
harness, an alternate frontend, a CaMeL supervisor. This is also
the bus, but it inverts the relationship: external code drives
rfl, not vice versa. We need to be sure this works for v1
regardless of the embedded-language question.

