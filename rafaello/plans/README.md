# rafaello plans/

This directory holds in-flight design work. One subdirectory per
stream of investigation. Each stream has:

- `README.md` — the question the stream is answering and the expected
  deliverable.
- `notes.md` — running notes, links, and intermediate findings. Free
  form; an audit trail.
- Named RFC documents that land as the stream converges.

Streams:

- [`stream-a-security/`](./stream-a-security/) — trust model, permission
  grant flow, plugin sandboxing, lockin-vs-capsa, CaMeL-as-plugin viability.
- [`stream-b-fittings/`](./stream-b-fittings/) — outbound notifications,
  ServiceContext API, error-handling story, anything else surfaced by an
  audit. Output feeds a GitHub issue against fittings.
- [`stream-c-scripting/`](./stream-c-scripting/) — do we need an
  embedded scripting language? Sketch the world with and without one.
- [`stream-e-renderer/`](./stream-e-renderer/) — chat-history-entry
  model, structured rendering tree, scrollback semantics.
- [`stream-f-manifest/`](./stream-f-manifest/) — plugin manifest
  schema, capability declarations, lazy-loading model, compilation to
  lockin policy.

A planned Stream D (agent loop language prototype) is gated on
Stream C's outcome and lives elsewhere if/when it runs.

## Convention

- Streams write directly into their own subdirectory and nowhere else
  in the repo. No drive-by changes to source.
- Commits inside a stream's worktree are encouraged to be incremental
  so the thinking is visible in `git log`.
- Final RFCs are named `rfc-<topic>.md` in the stream directory.
