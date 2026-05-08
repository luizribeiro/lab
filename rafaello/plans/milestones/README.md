# Milestones

> **Status:** revised by claude (orchestrator) after pi review 1
> (`pi-review-1.md`). Pi review 2 pending. Owner ratification follows.

Bottom-up roadmap from m0 to rafaello v1, accumulating on the
long-running `rafaello-v0.1` integration branch and merging to
`main` once v1 is demo-ready (`decisions.md` row 33). Each milestone
is sized so a driver agent can walk its `commits.md` in a small
number of sessions; if a milestone exceeds that during scoping, it
splits.

## Sequencing rules

- Milestones advance only after the prior milestone's
  `retrospective.md` updates `overview.md` and `decisions.md` with
  any learnings.
- Demo bar per milestone: an integration test in the relevant
  `tests/` directory exercising the milestone's primitives end-to-end
  (positive paths AND negative/security invariants), plus a
  `manual-validation.md` in the milestone's directory listing the
  things exercised by hand (litellm chat, tmux drives, plugin spawn,
  capability denials, etc.).
- Branch model: agent branches under `agents/m<N>/...`, rebased onto
  `rafaello-v0.1` as commits land. No merge commits, no force pushes.

## v1 deferrals

The four design-phase deferrals live in `decisions.md` rows 26–29:

- **Helper plugins** (`bindings.helper_for`, `RFL_HELPER_FD`)
  → `decisions.md` row 26, reverses #14.
- **External UDS-attached frontend principals** (TUI is the only
  frontend in v1; namespace `frontend.<attach-id>.*` reserved)
  → `decisions.md` row 27, partially reverses #15.
- **Streaming entry patch ops** (`stream_state: "open"` / `"patch"`;
  v1 emits `final` only) → `decisions.md` row 28, partially reverses
  #20.
- **Subprocess plugin renderers** (v1 ships built-in Rust renderers
  only; `renderer.render` deferred) → `decisions.md` row 29,
  partially reverses #19.

Plus the manifest simplifications (rows 30–32) and the branch-model
choice (row 33) settled at the same time.

## v1 milestones

| # | Name | Goal | Demo |
|---|------|------|------|
| m0 | fittings v1 | Land the fittings RFCs (PeerHandle, ServiceContext, error preservation, JsonRpcId migration, cancellation semantics, bounded notify). Self-contained — modifies `fittings/`, no rafaello-core yet. May split internally by RFC area if scoping finds more than a small PR series. | `examples/mcp-server` exercises outbound notifications + bidirectional `PeerHandle::call` + cancellation; JS-SDK interop test passes; new tests in `fittings/tests/` cover each RFC bullet (positive + malformed/edge-case negatives). |
| m1 | manifest / lock / grant / compiler foundation | Manifest parser (post-simplifications: no `runtime`, no `[rpc]`, `openrpc.json` sibling required at install time). `rafaello.lock` schema: digest-pinned per-plugin entries with bindings (granted capabilities, manifest-snapshot digest, content digest). Grant compiler that produces lockin builder calls from a granted lock entry. Topic-id derivation (sha256, base32, collision detection). No runtime yet — pure data transformation, easy to test. | `cargo test -p rafaello-core` integration covers: a fixture manifest+lock pair compiles to the expected lockin policy; an invalid manifest (unknown field, conflicting tool names, malformed sinks) is rejected with a typed error; a digest mismatch refuses to compile; carve-out decomposition produces the expected grant-shape under pathological project layouts. |
| m2 | rafaello-core broker + locked plugin spawn | Minimal `rafaello-core` crate. Bus broker registers subscribers, publishes events on the four namespaces (`core.*`, `provider.*`, `plugin.<topic-id>.*`, `frontend.<attach-id>.*` reserved), enforces topic-namespace ACL. PluginSupervisor spawns subprocess plugins **only from a granted lock entry** (no hardcoded bypass). Inherited socketpair fd via `RFL_BUS_FD`. Lockin enforcement via the Rust API (no `lockin.toml`). One built-in test fixture plugin exercises the path. | Spawn the fixture plugin from a fixture-locked entry; observe one `peer.call` round-trip each direction; observe one `bus.publish` reaching a test subscriber. **Negatives**: plugin publishing on `core.*` is rejected; plugin publishing on another plugin's namespace rejected; plugin opening a file outside its grant denied by lockin; spawn without a matching lock entry refused; topic with invalid grammar rejected. |
| m3 | sessions, daemon, local-spawned TUI, built-in rendering | `rfl serve` daemon. Session entry persistence (SQLite under `${PROJECT_ROOT}/.rafaello/state/`). `rafaello-tui` as a separate crate; local-spawned by `rfl chat` (no external attach in v1). Built-in in-process Rust renderers for `text`, `code_block`, `tool_call`, `tool_result`, `error`, `heading`, `thinking`, `image`. Server-side downgrade of unknown kinds to `Unknown { fallback }`. Turn-by-turn entries (`stream_state: "final"` only). No agent loop, no tool dispatch, no provider — TUI renders manually-published or test-injected entries. | `rfl chat` spawns daemon + TUI; static fixture entries publish on `core.session.entry.*`; TUI renders all built-in kinds + an unknown kind via fallback. **Negatives**: daemon refuses second `rfl serve` for the same project; TUI publishing on `core.*` is rejected; renderer crash for one kind doesn't crash the TUI; entry persisted to SQLite is replayed on restart. |
| m4 | provider fixture + secure agent loop + one read-only tool | A bundled deterministic mock provider as a **subprocess plugin** (locked through the m1/m2 path), publishing `provider.<provider-id>.tool_request` and `assistant_message`. Core re-emits canonical `core.session.tool_request` / `tool_result`. `in_reply_to` enforcement on tool_result, RPC reply, provider events. One read-only tool plugin (`read-file`) with no sink declarations — sink confirmation is not required for non-sink tools, so this milestone exercises the dispatch path without yet needing the confirmation UI. Agent loop reads provider events, dispatches tool calls, returns results to the provider. | `rfl chat` against the mock provider; user prompt "what's in README.md" emits a `tool_call` for `read-file`; tool runs, result rendered. **Negatives**: `tool_result` missing `in_reply_to` rejected; provider tool_request with stale/unknown id fails closed; tool plugin called directly by another plugin (not via core re-emission) doesn't reach the dispatch path; tool requested outside its grant denied at lockin. |
| m5 | LiteLLM provider + sinks + confirmation protocol + taint + user_grants + exfil demo | Bundled `rfl-litellm` subprocess plugin (default `vllm/qwen3.6-27b`, key from `LITELLM_API_KEY`). Manifest sink classes (`network`, `vcs_push`, `mail`, `workspace_write`). Confirmation protocol on the bus (`core.session.confirm_request` / `frontend.<id>.confirm_answer` / `core.session.confirm_reply`). TUI confirmation UI (modal, blocks input). Taint propagation (`{source, detail}`) on `tool_request` / `tool_result`. `user_grants` table — slash commands (`/grant`, `/grants list`, `/revoke`) + `always_allow_session`. One-hop trifecta guardrail at install time. May split into m5a (sinks + confirmation + user_grants) and m5b (taint + exfil) if scoping finds it too big. | Real model call against litellm; model proposes a sink-declaring tool; confirmation prompt fires; user accepts → tool runs; user denies → tool refused. **Negatives**: confirmation timeout denies; `always_allow_session` clears on daemon restart; verbatim tool-result-to-sink flow blocked at the broker; one-hop-only guardrail (transitive flows are NOT caught — explicitly out of v1 per `decisions.md` row 11). |
| m6 | v1 polish + release readiness | Test coverage gaps closed (per coverage report). Documentation pass on `rafaello/README.md` + `CONTRIBUTING.md`. Homebrew formula matching scope/tempo. `nix build .#rafaello` green on Linux + macOS. No opportunistic new tools — every shipped tool is owner-ratified in this milestone's `scope.md`. | `nix build .#rafaello` produces a binary that runs on both supported platforms. A manual end-to-end session against litellm captures the full happy path (init → install rfl-litellm → install one tool → chat → tool call with confirmation → response render → session persist) in `manual-validation.md`. |

## Dependency graph

m0 → m1 → m2 → m3 → m4 → m5 → m6

No parallelism between milestones in v1 (each strictly depends on the
previous; m1 has no runtime dependency on m0 but the order is set so
the agents working on m1 can assume the new fittings API exists in
their dev shell). Within a milestone, agent commits are sequential
per `commits.md`.

## Branch model

All milestone work accumulates on `rafaello-v0.1` (`decisions.md` row
33). Per-milestone branches under `agents/m<N>/...` rebase into it as
commits land. `main` stays at the rafaello-design merge until v1 is
demo-ready, at which point `rafaello-v0.1` merges to `main` in one
pass.

The fittings changes in m0 are useful to fittings consumers regardless
of rafaello status; they could optionally be split out as a separate
`fittings-v0.X` merge to `main` ahead of v1 if a fittings consumer
needs them earlier. Default: stay on `rafaello-v0.1`.

## Per-milestone deliverables

Each milestone subdirectory under `milestones/m<N>-<name>/` has:

- `scope.md` — what's in, what's deferred, the demo bar (positive +
  negative tests). Drafted by claude, pi-reviewed, owner-ratified
  before commits-list work begins.
- `commits.md` — ordered commit list, each with subject + rationale +
  acceptance criteria + dependency on prior commits. Drafted by
  claude, pi-reviewed, owner-ratified before per-commit agent work
  begins.
- `retrospective.md` — milestone-end review, including: pi review of
  the diff against `scope.md` and `commits.md`; any updates to
  `overview.md`, `decisions.md`, or stream RFCs that the milestone's
  implementation surfaced; coverage report.

## What changed from the first draft

Round-1 pi review (`pi-review-1.md`) prompted the following revisions
to the first milestones draft:

- Reordered m1 ↔ m2 so manifest/lock/compiler land before any plugin
  spawning (no hardcoded bypass path that retrofits later).
- Split the previous m3 into m3 (frontend infrastructure) and m4
  (agent loop + read-only tool dispatch).
- Mock provider is now a locked subprocess plugin fixture, not
  built-in core code.
- m4's first tool is read-only (no sinks), so the dispatch path
  exists before the confirmation UI does.
- m5 keeps sinks + confirmation + taint + `user_grants` together
  because they're tightly coupled architecturally.
- The "v1 deferrals" section now points at `decisions.md` rows
  rather than re-asserting deferrals here (the scope-drift problem
  pi flagged in finding 1).
- Demo bars now explicitly include negative/security tests, not just
  happy paths.
- Branch model conflict with `plans/README.md` resolved by patching
  `plans/README.md` to match (`decisions.md` row 33).
