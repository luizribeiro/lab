# rafaello

Minimal, customizable coding agent. Binary name: `rfl`.

> **Status:** Scaffolding only. Architecture is being designed in
> [`plans/`](./plans/); no implementation has landed yet. The current
> binary prints a placeholder and exits.

The shape we're aiming at:

- **Project-scoped.** `rfl init` anchors the agent to a directory.
  Without an init'd project, `rfl` runs as a tool-less LLM.
- **Minimal core in Rust.** The agent core exposes primitives —
  provider, tool dispatch, event bus, session store — and little else.
- **Plugins as the unit of capability.** Tools, providers, renderers,
  and most user-visible features ship as plugins, gated through a
  manifest + lock + sandbox flow so the LLM cannot grant itself new
  capabilities.
- **Multiple frontends over one bus.** The default TUI is one client;
  a daemon mode lets other frontends (web, email, IDE) attach over
  JSON-RPC.

Concrete designs in flight:

- Stream A — security model
- Stream B — fittings RFC (notifications, error handling)
- Stream C — embedded scripting question
- Stream E — renderer / chat-history-entry model
- Stream F — plugin manifest schema + lazy-loading

See [`plans/`](./plans/) for each stream's brief and current notes.

m5a adds rfl-openai (OpenAI-compatible provider plugin)
