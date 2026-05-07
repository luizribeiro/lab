# Stream C — Embedded scripting question

## The question

Does rafaello need an embedded scripting language at all, or can the
event bus + subprocess plugin model carry the customization story
entirely?

The default candidate was Luau via `mlua`: default-deny sandbox,
async via Tokio coroutines, optional gradual types via Luau's offline
checker. The pushback: if everything talks to the bus over JSON-RPC,
any language can extend rafaello as a subprocess. Maybe the embedded
plane is unnecessary complexity.

## What to evaluate

Sketch rafaello's user-facing customization story under both
scenarios. Concrete UX targets the design needs to support:

- Snappy, good-looking CLI (tight cold start, low overhead per
  keystroke, clean popups/statusline/command-palette).
- CaMeL implementable as a plugin using only v1 primitives.
- Complex evals on top of rafaello (SWE-style benchmarks needing
  per-task dev VMs orchestrated externally).
- Tiny customizations: a keymap, a hook reacting to a single event,
  a one-line prompt template.

For each scenario, walk through:

- What does writing a 3-line keymap look like *with* Luau? *Without*?
- What does writing a hook that re-emits `model.token` to OpenTelemetry
  look like with vs without?
- What does the agent loop look like with vs without, including loop
  replacement?
- What does the renderer-customization story look like with vs without?

## Deliverables

- `rfc-scripting-decision.md` — recommendation and rationale.
  Explicit answer: yes or no embedded scripting in v1.
  - If yes: which engine and why, sandbox posture confirmed,
    minimum-viable Lua API surface.
  - If no: how each customization scenario is served. Specifically
    address the "tiny customization" scenario, where subprocess
    overhead is highest relative to value.

## Inputs

- pi (TypeScript-as-extensions) and Neovim (Lua-as-config) as the two
  baseline reference points.
- Conversation history.
- This is research only, no prototyping. A separate Stream D may
  follow with hands-on prototypes if the answer is "yes."
