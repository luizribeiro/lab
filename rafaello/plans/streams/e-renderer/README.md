# Stream E — Renderer / chat-history-entry model

## The question

What is the data model for chat-history entries and renderers, such
that:

- Multiple frontends (TUI, web, email, IDE) can render the same
  entries without duplicating logic.
- Plugins can register custom entry kinds and ship renderers for
  them.
- Terminal scrollback compatibility is preserved (we cannot re-render
  past entries; once written, they stay).
- The model is expressible in fittings/OpenRPC types (entries flow
  over the bus).

## Sketch to refine

Each entry has `kind`, `payload`, `metadata`. Built-in kinds: `text`,
`heading`, `code_block`, `tool_call`, `tool_result`, `thinking`,
`image`, `error`. Plugins can register additional kinds.

Renderers are functions `(kind, payload) -> RenderTree`, where
`RenderTree` is a small ADT (`Text`, `Heading`, `CodeBlock`, `List`,
`Container { children }`, …). Frontends consume the tree and draw
it however they want — TUI uses ratatui, web emits HTML, email emits
HTML or plain text, etc.

## Open questions

- What's the minimum useful set of `RenderTree` variants for v1?
- How does a frontend communicate its capabilities (color depth,
  width, image support) so renderers can adapt?
- How do plugins ship renderers? In-process registration vs.
  subprocess that returns a tree per request.
- Streaming: entries that grow over time (a model's reply being
  generated). How does scrollback see partial then final state?
- Fallback rendering for unknown `kind` values (forward-compat with
  newer plugins).

## Deliverables

- `rfc-renderer-model.md` — full schema with examples, fittings/
  OpenRPC schema definitions, and the streaming/scrollback semantics.

## Inputs

- pi's session-format docs at `/tmp/pi-mono/packages/coding-agent/docs/session-format.md`
  (if present) for reference on how a similar model partitions its
  history.
- ratatui's widget model for what TUI rendering naturally consumes.
- Conversation history.
