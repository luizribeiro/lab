# RFC — Renderer & chat-history-entry model

Status: draft (stream E research artifact)
Owner: stream-e-renderer

## 1. Goals & non-goals

rafaello has many simultaneous frontends (default ratatui TUI, daemon
clients over JSON-RPC, web UI, IDE integrations, even an
email-as-UI). They all consume the same conversation event stream
(turns, tool calls, results, thinking, errors, plugin-defined kinds)
and render it to wildly different surfaces. We need:

- **One source of truth** for conversation events (the *entry*) so
  session storage, replay, and JSON-RPC clients all agree.
- **One pure render layer** that turns an entry into a structured
  description of its visual form (the *render tree*) — no draw
  calls, no widget references — so frontends can specialise the
  paint step.
- **Plugin extensibility** for both new entry kinds *and* their
  renderers, including out-of-process plugins.
- **Forward and backward compatibility** so old frontends survive new
  entry kinds and new render-tree variants.
- **Streaming compatibility with append-only surfaces** (terminal
  scrollback) where past output cannot be redrawn.

Non-goals: layout (frontends own that), theming (frontends own
colours/fonts; renderers only emit semantic intent), interaction
events back into the agent (covered in stream B fittings).

## 2. Top-level architecture

```
  ┌──── plugin renderers (in-proc / subprocess) ────┐
  │                                                 │
  Entry ──► registered Renderer(kind) ──► RenderTree ──► Frontend paint
                          ▲                                    │
                          └────── FrontendCapabilities ◄────────┘
```

Entries flow on the fittings event bus as JSON-RPC notifications
(`core.session.entry.appended`, `core.session.entry.patched`,
`core.session.entry.finalized`). Renderers are pure functions
`(kind, payload, capabilities) -> RenderTree`. The frontend is the
only side-effectful component.

## 3. The `Entry` schema

```jsonc
// JSON Schema (draft-2020-12), expressed in fittings conventions
{
  "$id": "rafaello://schema/entry/v1",
  "type": "object",
  "required": ["id", "kind", "payload", "metadata"],
  "properties": {
    "id":       { "type": "string" },              // ULID
    "parent":   { "type": ["string", "null"] },    // entry tree
    "kind":     { "type": "string" },              // e.g. "text"
    "schema":   { "type": "string" },              // payload schema URI
    "payload":  { "type": "object" },              // kind-specific
    "metadata": { "$ref": "#/$defs/EntryMeta" },
    "fallback": { "$ref": "#/$defs/Fallback" }     // see §6
  },
  "$defs": {
    "EntryMeta": {
      "type": "object",
      "required": ["created_at", "stream_state"],
      "properties": {
        "created_at":  { "type": "string", "format": "date-time" },
        "updated_at":  { "type": "string", "format": "date-time" },
        "author":      { "enum": ["user", "assistant", "tool",
                                   "system", "plugin"] },
        "plugin":      { "type": "string" },         // when author=plugin
        "stream_state":{ "enum": ["final", "open", "patch", "closed"] },
        "seq":         { "type": "integer" },        // patch ordering
        "tags":        { "type": "array", "items": { "type": "string" } }
      }
    },
    "Fallback": {
      "type": "object",
      "properties": {
        "text":     { "type": "string" },          // plain-text rep
        "markdown": { "type": "string" },          // optional richer
        "summary":  { "type": "string" }           // one-liner
      }
    }
  }
}
```

`schema` is a URI naming the payload's contract (e.g.
`rafaello://schema/payload/text/v1`,
`rafaello://schema/payload/tool_call/v1`,
`mermaid://schema/diagram/v1`). It lets frontends/renderers select a
renderer without conflating it with `kind`, which is the routing key.

### 3.1 Built-in kinds

| kind          | payload (sketch)                                   |
|---------------|----------------------------------------------------|
| `text`        | `{ "text": string, "markdown": bool }`             |
| `heading`     | `{ "text": string, "level": 1..6 }`                |
| `code_block`  | `{ "code": string, "lang": string? }`              |
| `tool_call`   | `{ "id": string, "name": string, "args": object, "status": "pending"|"running"|"ok"|"error" }` |
| `tool_result` | `{ "call_id": string, "ok": bool, "content": <render-tree>, "details": object? }` |
| `thinking`    | `{ "text": string }`                               |
| `image`       | `{ "uri": string, "mime": string, "alt": string, "bytes_b64": string? }` |
| `error`       | `{ "code": string, "message": string, "data": object? }` |

`tool_result.content` is *already* a render tree, so tools can
deliver structured output (a table, a list, an image) without
inventing kinds for each shape.

## 4. The `RenderTree` schema

`RenderTree` is a small, semantic ADT — *not* a layout tree. It
describes what the content *means*; the frontend chooses how to
paint it.

### 4.1 Variants for v1

Minimum useful set, kept deliberately small and primitive:

| variant      | shape                                                     | notes |
|--------------|-----------------------------------------------------------|-------|
| `Text`       | `{ text, emphasis?: "none"|"em"|"strong"|"dim"|"warn"|"err" }` | inline text with semantic emphasis (no colours) |
| `Heading`    | `{ level: 1..6, text }`                                   | block |
| `Code`       | `{ code, lang? }`                                         | block, monospace, no wrapping |
| `Inline`     | `{ children: RenderTree[] }`                              | run of inline nodes |
| `Block`      | `{ children: RenderTree[] }`                              | vertical stack of block nodes |
| `List`       | `{ ordered: bool, items: RenderTree[] }`                  | block |
| `KeyValue`   | `{ pairs: [{ key, value: RenderTree }] }`                 | for tool args, errors, headers |
| `Table`      | `{ headers: string[], rows: RenderTree[][] }`             | tabular tool output |
| `Divider`    | `{}`                                                      | horizontal rule |
| `Image`      | `{ uri, mime, alt, bytes_b64? }`                          | frontend may degrade |
| `Link`       | `{ href, child: RenderTree }`                             | inline; degrades to text |
| `Callout`    | `{ kind: "info"|"warn"|"error"|"success", child: Block }` | semantic boxed block |
| `Collapsed`  | `{ summary: RenderTree, detail: RenderTree, default_open: bool }` | for thinking, tool args, large outputs |
| `Raw`        | `{ format: "ansi"|"html"|"plain", body: string }`         | escape hatch — frontends may refuse |
| `Unknown`    | `{ kind, payload, fallback }`                             | see §6 |

Everything else (panels, sidebars, paginated views, blinking cursors)
belongs to the frontend, not the tree. Notably absent: any notion of
colour, width, fonts, or pixel positioning. Emphasis is *semantic*;
the frontend maps it to ANSI bold, CSS `<strong>`, or whatever fits.

### 4.2 JSON encoding (tagged)

```jsonc
// internally tagged per fittings convention
{ "node": "Text", "text": "hello", "emphasis": "strong" }
{ "node": "Block", "children": [ ... ] }
{ "node": "Unknown", "kind": "custom-plot",
  "payload": { ... }, "fallback": { "text": "..." } }
```

`node` (not `type`) avoids collision with payload `type` discriminators
inside e.g. `tool_call` content.

## 5. Frontend capabilities

A frontend advertises capabilities to the daemon at attach time via
JSON-RPC `frontend.hello`:

```jsonc
{
  "name": "rfl-tui",
  "version": "0.1.0",
  "render_tree_version": "1",          // see §8
  "capabilities": {
    "color":        "truecolor"|"256"|"16"|"none",
    "unicode":      "full"|"basic"|"ascii",
    "width":        120,                // current cells/columns
    "height":       40,                 // null for unbounded (web)
    "image":        ["png","jpeg","kitty","sixel","none"],
    "interactive":  true,               // can render Collapsed open/closed
    "scrollback":   "append-only"|"redrawable",
    "raw":          ["ansi"],           // accepted Raw.format values
    "links":        true,
    "table":        true,
    "max_block_bytes": 65536,           // hint for renderers
    "nodes":        ["Text","Heading","Code","Inline","Block","List",
                     "KeyValue","Table","Divider","Image","Link",
                     "Callout","Collapsed","Raw","Unknown"]
  }
}
```

`nodes` is the closed set of variants the frontend understands. A
renderer SHOULD prefer those; the daemon MAY also downgrade unknown
variants (§7).

`scrollback: "append-only"` is the load-bearing flag for the TUI:
renderers must avoid emitting `Collapsed` with `default_open: false`
unless the frontend also reports `interactive: true`, because there's
nothing to click on in scrollback.

Capabilities are passed into every renderer call, so a renderer can,
for example, swap an `Image` for `KeyValue { alt, uri }` when the
frontend can't display images.

## 6. Fallback rendering

Two layers of fallback, in order:

1. **Author-provided.** Every `Entry` MAY include a top-level
   `fallback` (`{ text, markdown?, summary? }`). Plugins are
   *strongly* encouraged to fill this in. If the renderer for `kind`
   is unavailable, the daemon emits
   `Block { children: [Text { text: fallback.text }] }` (or markdown
   converted to a tree, if the frontend's capabilities cover it).
2. **Default kind renderer.** If no `fallback` is set, the daemon's
   built-in default renderer turns the payload into
   `Callout { kind: "warn", child: KeyValue { ... } }` listing
   `kind`, `schema`, and the payload's stringified fields. This is
   ugly on purpose so plugin authors notice and add a `fallback`.

The render tree itself also carries an `Unknown` variant for the
frontend's eyes: when the daemon ships a tree that includes a node
the frontend reported it doesn't support, the daemon downgrades that
subtree to `Unknown { kind, payload, fallback }` *server-side* before
sending. The frontend's job is then trivially to paint
`fallback.text`.

This design keeps the *frontend* dumb (it never has to invent a
fallback) and gives plugins one place to ship a textual answer.

## 7. Streaming entries

The model produces a reply token by token. The bus carries three
notifications per streaming entry:

- `core.session.entry.appended` — initial entry with
  `metadata.stream_state = "open"` and (usually) empty payload
  content. The entry's `id` is the handle for subsequent patches.
- `core.session.entry.patched` — `{ id, seq, patch }`. `patch` is a small
  tagged delta:
  - `{ "op": "append_text", "path": "$.text", "value": "..." }` for
    the common case
  - `{ "op": "replace", "path": "$.status", "value": "running" }`
  - `{ "op": "append_child", "path": "$.children", "value": <tree> }`
- `core.session.entry.finalized` — `{ id, payload }` containing the
  authoritative final payload, with `metadata.stream_state = "final"`.
  The frontend SHOULD treat this as the canonical version (e.g. for
  re-export, replay).

Re-rendering after each patch is the responsibility of the
*frontend*, but the daemon helps:

- For `scrollback: "redrawable"` frontends (web, ratatui in
  alternate-screen mode): the daemon ships patches; the frontend
  re-runs the renderer on the patched payload and repaints. Cheap,
  flicker-free.
- For `scrollback: "append-only"` frontends (TUI in inline mode,
  email): the daemon coalesces. It emits patches only as
  `append_text` deltas where possible, which the frontend can stream
  directly to stdout. When a non-append patch arrives (e.g. a tool
  call's `status` flips from `running` to `ok`), the daemon sends a
  *line-stable* `core.session.entry.patched` and the frontend prints a
  small status footer line below. On `finalized`, the frontend prints
  a final delimiter; nothing earlier is rewritten.

### 7.1 Worked example — 100-token assistant reply

Bus traffic (abridged):

```
1. core.session.entry.appended {
     id: "01J...A",
     kind: "text",
     payload: { text: "", markdown: true },
     metadata: { author: "assistant", stream_state: "open", seq: 0 }
   }
2. core.session.entry.patched { id: "01J...A", seq: 1,
     patch: { op: "append_text", path: "$.text", value: "Sure" } }
3. ... 99 more patched notifications ...
101. core.session.entry.finalized { id: "01J...A",
     payload: { text: "<full 100 tokens>", markdown: true },
     metadata: { stream_state: "final", seq: 101 } }
```

Frontends:

- **TUI (append-only, truecolor, width 120).** On `appended` it prints
  a `> ` prefix and parks the cursor. On each `patched` it streams
  the appended slice through a markdown-aware ANSI renderer that
  emits at most one line per token boundary. On `finalized` it
  prints the trailing newline and a footer with token usage from
  `metadata`. No redraw.
- **Web (redrawable, truecolor, image yes).** It mounts a React
  component bound to entry `id`; each patch updates state; the
  full `RenderTree` is recomputed on every patch (renderer is pure
  and cheap). On `finalized` nothing visible changes.
- **Email (no streaming surface).** The email frontend ignores
  `appended` and `patched` and only renders on `finalized`, then
  emits one HTML body per session at session end (or one MIME part
  per assistant turn).

## 8. Versioning

- `Entry` carries `schema` per payload. Payloads are versioned in
  their URIs (`.../text/v1`).
- `RenderTree` is versioned as a whole via the
  `frontend.hello#render_tree_version` field. v1 is the schema in
  §4. New variants get added in v2; the daemon negotiates the
  highest mutually supported version per attached frontend and
  downgrades trees with §6's `Unknown` mechanism for frontends still
  on v1.
- `kind` is an open string namespace. Built-in kinds are unprefixed;
  plugin kinds MUST be prefixed (`mermaid:diagram`, `myorg:trace`).
  This avoids collisions and makes ownership obvious in logs.
- Renderers themselves are versioned by the plugin manifest
  (stream F). The daemon refuses to load a renderer whose declared
  `render_tree_version` is higher than what the daemon supports.

Forward-compat rule (frontend perspective): a frontend MUST treat any
unknown `node` value in a `RenderTree` as if it were `Unknown`. The
daemon SHOULD downgrade before sending, but the rule is a
belt-and-braces safety net.

## 9. Plugin renderers over JSON-RPC

In-process Rust renderers register as
`fn(payload: Value, caps: &Capabilities) -> RenderTree`.

Subprocess (third-party) plugins implement a single JSON-RPC method
on their fittings server:

```jsonc
// request
{ "jsonrpc": "2.0", "id": 7, "method": "renderer.render",
  "params": {
    "kind": "mermaid:diagram",
    "schema": "mermaid://schema/diagram/v1",
    "payload": { "src": "graph TD; A-->B" },
    "capabilities": { /* see §5 */ }
  } }
// response
{ "jsonrpc": "2.0", "id": 7,
  "result": { "tree": { "node": "Block", "children": [ ... ] },
              "render_tree_version": "1" } }
```

The daemon caches the resulting tree keyed by
`(plugin, kind, payload_hash, caps_hash)` so a redrawable frontend
doesn't pay subprocess RTT per repaint. Cache is invalidated on
plugin reload.

Errors from renderers map to the `Unknown`/`fallback` path; a
crashing renderer never crashes the daemon.

## 10. Worked example — `rafaello-plugin-mermaid`

The plugin contributes:

- A new entry kind `mermaid:diagram` with payload schema
  `mermaid://schema/diagram/v1 = { "src": string, "title"?: string }`.
- A renderer registered for that `schema`.

Entry on the bus:

```jsonc
{
  "id": "01J...M",
  "kind": "mermaid:diagram",
  "schema": "mermaid://schema/diagram/v1",
  "payload": { "src": "graph TD; A-->B; B-->C", "title": "deploy" },
  "metadata": { "author": "plugin", "plugin": "rafaello-plugin-mermaid",
                "stream_state": "final" },
  "fallback": {
    "summary": "diagram: deploy",
    "text":    "deploy:\n  A -> B\n  B -> C",
    "markdown": "**deploy**\n\n```mermaid\ngraph TD; A-->B; B-->C\n```"
  }
}
```

Renderer behaviour, parameterised by `capabilities`:

- `image: ["png", ...]` → render Mermaid to PNG, return
  `Block { Heading "deploy", Image { mime: "image/png", bytes_b64, alt: "..." } }`.
- `image: ["kitty"]` (TUI with image protocol) → same, but with a
  Kitty-graphics-friendly `mime`.
- `image: ["none"]` and `unicode: "full"` → return a Unicode box-art
  approximation as `Code { lang: "mermaid-art", code: "..." }`.
- `image: ["none"]` and `unicode: "ascii"` → return
  `Block { Heading "deploy", Code { lang: "mermaid", code: src } }`.
- Any failure / capability mismatch the plugin doesn't want to
  handle → the plugin returns no tree, and the daemon synthesises
  one from `entry.fallback.markdown` (preferred) or `.text`.

TUI fallback in append-only inline mode without image support: the
final paint is just the heading "deploy" followed by the indented
text from `fallback.text`. Scrollback compatible, lossless enough,
no surprises.

## 11. Summary

- **One entry shape**, with `kind`/`schema`/`payload`/`metadata` and
  an author-supplied textual `fallback`.
- **One small render-tree ADT** of ~14 semantic variants, no styling,
  no layout, no colour.
- **Streaming via append/patch/finalize** with patches expressive
  enough for token-by-token text but limited enough for the TUI to
  stream straight to scrollback.
- **Capabilities negotiated at attach time** and threaded into every
  renderer call.
- **Server-side downgrade to `Unknown { fallback }`** for any node a
  frontend can't handle, so frontends never have to invent
  fallbacks.
- **Plugin renderers** in-proc (Rust) or subprocess (JSON-RPC), with
  a daemon-side cache and crash isolation.
- **Versioning** at three layers: payload schema URI, render-tree
  version (negotiated), plugin manifest.
