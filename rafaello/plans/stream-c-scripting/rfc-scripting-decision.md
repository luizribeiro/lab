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

## Scenario 3 — SWE-bench-style evals

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

