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

