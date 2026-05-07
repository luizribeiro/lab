# notes — stream-c-scripting

Working notes accumulated while researching the embedded-scripting
question. The decision lives in `rfc-scripting-decision.md`; this
file records the inputs and the small observations that shaped it.

## Reference points

### pi (badlogic/pi-mono)

- Extensions are TypeScript modules loaded in-process via jiti
  (no compile step). `pi -e ./my.ts` or auto-discovered from
  `~/.pi/agent/extensions/*.ts` and `.pi/extensions/*.ts`.
- Extensions get an `ExtensionAPI` with `pi.on(event, …)`,
  `pi.registerTool`, `pi.registerCommand`, `pi.registerShortcut`,
  `pi.registerProvider`, `pi.registerFlag`. Hot-reload via `/reload`.
- Notable: pi explicitly says "Extensions run with your full
  system permissions" — there is no embedded sandbox. The answer
  pi gives is "trust the source, no isolation."
- For *tiny* customizations pi specifically does NOT use TS:
  - keybindings → `~/.pi/agent/keybindings.json` (declarative).
  - prompt templates → `*.md` with frontmatter, `/name` to invoke.
  - skills → `*.md` (Agent Skills spec).
  - shell aliases → a single `shellCommandPrefix` string in
    `settings.json`.
- So pi already answers "what's the no-script path for the small
  cases" by *not* funnelling them through TypeScript.

### Neovim

- LuaJIT in-process, runtime is part of the binary, hot-reload
  is `:source %` or `:Lazy reload`.
- Cold start with sensible plugin manager: ~30–80 ms.
- Lua is the configuration language: `init.lua`, keymaps, hooks,
  plugin specs. Subprocess plugins exist (LSP, formatters) but
  the *config plane* is Lua.
- Plugins are not sandboxed. Trust model is "you installed it."

## Constraints from rafaello design

- Project-scoped (`rfl init`), minimal core, secure-by-default.
- LLM is untrusted; user-authored config is trusted (same as nvim,
  same as pi, same as Claude Code).
- Plugins (subprocesses) sandboxed via lockin; capability flow is
  manifest → lock → policy.
- Bus is JSON-RPC; everything subscribable.
- Embedded scripts, if added, would NOT run lockin — the only
  isolation would be language-level (Luau `Lua::sandbox(true)`).

## mlua / Luau quick facts

- `mlua` with `feature = "luau"` embeds Roblox Luau (Lua 5.1
  syntax + types, no `os`, no `io`, no `package` by default).
- `Lua::sandbox(true)` freezes globals, makes `_G` per-thread,
  disables bytecode loading. This is real isolation against
  *trusted-but-buggy* code; it is not a security boundary against
  adversarial code that has access to host functions you exposed.
- Async via `mlua`'s tokio integration (coroutines yield to the
  Rust runtime). Reasonable performance — interpreter, no JIT,
  but startup is ~milliseconds and per-call overhead is in the
  microseconds for trivial closures.
- Adds ~1–2 MB to binary size in release.

## Cost intuitions

- Cold start budget rafaello wants: <100 ms.
  - JSON-RPC subprocess spawn on Linux: 5–15 ms minimum (fork+exec
    + interpreter init for a Python plugin: 30–80 ms; for a Rust
    plugin: 5–15 ms).
  - In-process Luau call: <1 ms first call, sub-µs steady state.
  - For 5–10 subscribed-at-startup plugins, the subprocess world
    is already 50–150 ms of fork-exec serialised, unless we
    parallelise eagerly. With eager parallel spawn we can hide it,
    but then memory cost is N processes rather than one.
- Per-keystroke budget: a keymap that opens an editor must run
  in <16 ms to feel instant.
  - In-process: trivial.
  - Subprocess round-trip: 1–3 ms on warm pipe, fine *if* the
    plugin process is already running. If it has to spawn on
    keystroke: not fine.

## What "customizability" means here

Three distinct user populations, with different needs:

1. **Casual user:** wants `Ctrl+G` rebound, `/explain` template,
   maybe an OpenTelemetry hook copy-pasted from a gist.
2. **Power user:** wants to write a small plugin (custom tool,
   custom renderer, custom command).
3. **Researcher / harness author:** wants to run rfl headless from
   another process — drive a SWE-bench eval, stack CaMeL on top,
   embed in a different UI.

The embedded-scripting decision matters most for (1). (2) and (3)
are both well-served by the subprocess plane regardless.

## Things I'm uncertain about

- Whether mlua + Luau realistically holds rafaello's <100ms cold
  start once a real config and a couple of plugins are loaded. No
  prototype yet; numbers above are intuitions.
- Whether the "renderer" plane is in-process or subprocess. Stream
  E is open. If renderers must be in-process for latency reasons,
  that drags the scripting plane in by the back door.
- Whether the agent loop itself is replaceable, and at what layer.
  If the loop is replaceable as a subprocess that owns the bus,
  we've sidestepped a big motivation for embedded scripting.
