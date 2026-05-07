# Stream F — Plugin manifest schema + lazy-loading

## The question

What is rafaello's plugin manifest format, and how does it express
both capability declarations and a lazy-loading model?

The manifest is the plugin author's request for capabilities. It is
parsed by rafaello at install time (writing into `rafaello.lock`)
and at runtime (compiling to a lockin policy when the plugin is
spawned).

## What the manifest must express

- Plugin identity (`source:name@version` plus a content digest
  computed by rafaello, not declared by the plugin).
- Methods exposed (with JSON Schema or OpenRPC for params/results).
- Bus topics subscribed to and published on.
- Filesystem capabilities (read/write/exec paths and dirs), mapped
  1:1 onto `lockin`'s schema where possible.
- Network capabilities (allow_hosts in `proxy` mode, `deny`,
  `allow_all`).
- Environment variables to inherit/pass.
- Renderer registrations (which `kind` values it provides renderers
  for — feeds Stream E).
- Lazy-loading triggers (see below).

## Lazy-loading

Required-vs-optional was the first cut, but it's likely too coarse.
Look at neovim's lazy.nvim, packer.nvim, mini.deps for the patterns
that actually work:

- Load eagerly at startup.
- Load on a specific event firing on the bus.
- Load on a specific command being invoked.
- Load on a file-pattern match (rafaello equivalent: project type?
  presence of files? filetype heuristic?).
- Load lazily on first access (e.g., first time a tool is dispatched).

Decide which subset rafaello supports in v1 and how each is expressed
in the manifest. Map the agent loop boot sequence onto this — what
must finish loading before the loop accepts user input vs. what can
trickle in.

## Capability backend abstraction

The manifest should compile cleanly to a lockin policy today, but
the v2/v3 vision is per-tool capsa VMs (thin Nix-built initramfs).
Design the schema so swapping the runtime is a backend change, not a
manifest schema change.

## Deliverables

- `rfc-manifest-schema.md` — full manifest format with examples.
- `rfc-lazy-loading.md` — loading model and how the manifest expresses
  it. Could be a section in the manifest RFC if it's small enough.

## Inputs

- `lockin/docs/cli.md` for the existing TOML schema rafaello must
  compile down to.
- neovim plugin manager docs (lazy.nvim, packer.nvim, mini.deps).
- pi's package manifest format (`/tmp/pi-mono/packages/coding-agent/docs/packages.md`).
- Conversation history.
