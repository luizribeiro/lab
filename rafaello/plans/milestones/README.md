# Milestones

> **Status:** drafted by claude (orchestrator) for pi adversarial
> review. Pending owner ratification.

Bottom-up roadmap from m0 to rafaello v1, accumulating on a
`rafaello-v0.1` branch and merging to `main` once v1 is demo-ready.
Each milestone is sized so a driver agent can walk its `commits.md`
in a small number of sessions; if a milestone exceeds that during
scoping, it splits.

## Sequencing rules

- Milestones advance only after the prior milestone's
  `retrospective.md` updates `overview.md` and `decisions.md` with
  any learnings.
- Demo bar per milestone: an integration test in the relevant `tests/`
  directory exercising the milestone's primitives end-to-end, plus a
  `manual-validation.md` in the milestone's directory listing the
  things exercised by hand (litellm chat, tmux drives, plugin spawn,
  capability denials, etc.).
- Branch model: agent branches under `agents/m<N>/...`, rebased onto
  `rafaello-v0.1` as commits land. No merge commits, no force pushes.

## Confirmed deferrals (per design discussion)

The following primitives stay in `decisions.md` as `proposed` /
`ratified` but are not implemented in v1:

- Helper plugins (`bindings.helper_for`, `RFL_HELPER_FD`).
- Frontend principals beyond TUI; v1 ships TUI only, local-spawned,
  with the `frontend.<attach-id>.*` namespace reserved for v2.
- Streaming patch ops (`stream_state: "open"` / `"patch"`); v1 emits
  `stream_state: "final"` only — turn-by-turn replies.
- Subprocess plugin renderers; v1 renderers are built-in Rust,
  registered at compile time. (This is a separate explicit cut, worth
  its own `decisions.md` row.)
- Plugin-level interception (a plugin observing and blocking another
  plugin's events); v2 territory together with CaMeL.
- Capsa runtime backend.

## v1 milestones

| # | Name | Goal | Demo |
|---|------|------|------|
| m0 | fittings v1 | Land the fittings RFCs (PeerHandle, ServiceContext, error preservation, JsonRpcId migration, cancellation semantics, bounded notify). Self-contained — modifies `fittings/`, no rafaello-core yet. | `examples/mcp-server` exercises outbound notifications + bidirectional calls; the JS-SDK interop test passes; new tests in `fittings/tests/` cover each RFC bullet. |
| m1 | rafaello-core skeleton + bus broker + sandboxed plugin spawn | Minimal `rafaello-core` crate. Spawn a hardcoded test plugin under lockin (Rust API), exchange one `peer.call` each direction, observe one `bus.publish` event reaching a subscriber via the broker. | `cargo test -p rafaello-core` integration test: spawn → `peer.call` → reply → `bus.publish` → broker fan-out → tear down clean. |
| m2 | manifest + lock + grant flow + lockin policy compilation | Manifest format (no `runtime`, no `[rpc]`, `openrpc.json` sibling). `rfl init`, `rfl install <path>` interactive grants, `rafaello.lock` with digest pinning. Lockin policy compiled from a granted lock entry via lockin's Rust API. A built-in `read-file` plugin installed via the real flow. | Hand-authored plugin requesting `${project}` rw + a network host; user accepts; plugin spawns under correct lockin policy; an out-of-grant operation is denied in tests. |
| m3 | rafaello-tui + daemon mode + agent loop with one tool | `rafaello-tui` as a separate crate. `rfl serve`. TUI attaches via inherited fd or the attach socket. Agent loop dispatches tool calls. Built-in renderers for `text`, `tool_call`, `tool_result`, `error`. Turn-by-turn (no streaming patch ops). Mock provider. | Demo session: TUI opens, user types "what's in README.md", mock model invokes the tool plugin, result renders. |
| m4 | provider plugin (rfl-litellm) + sink declarations + confirmation protocol | `rfl-litellm` as a bundled subprocess plugin (default model: `vllm/qwen3.6-27b`). Manifest sink classes. `core.session.confirm_*` flow. TUI confirmation UI. | Real model proposes a sink-declaring tool; confirmation prompt; user accepts/denies; outcome correct in both branches. |
| m5 | taint propagation + user_grants + security story end-to-end | Mandatory taint envelope on `tool_request`/`tool_result`. `in_reply_to` enforcement. `user_grants` via slash command + "always allow this session". One-hop trifecta guardrail. | Scripted exfiltration attempt blocked at the broker; trace shows the right confirmation gate fired. |
| m6 | v1 polish + release readiness | Test coverage gaps closed. Documentation pass on `rafaello/README.md` + `CONTRIBUTING.md`. Homebrew formula matching scope/tempo. Additional built-in tools if useful (`write-file`, `run-shell`). | `nix build .#rafaello` produces a binary that runs on Linux and macOS; a manual end-to-end session against litellm captures the full happy path. |

## Dependency graph

m0 → m1 → m2 → m3 → m4 → m5 → m6

No parallelism between milestones in v1 (each milestone strictly
depends on the previous). Within a milestone, agent commits are
sequential per `commits.md`.

## Branch model

All milestone work accumulates on `rafaello-v0.1`. Per-milestone
branches under `agents/m<N>/...` rebase into it as commits land.
`main` stays at the `rafaello-design` merge until v1 is demo-ready,
at which point `rafaello-v0.1` merges to `main`.

The fittings changes in m0 are useful to fittings consumers regardless
of rafaello status; they could optionally be split out as a separate
`fittings-v0.X` merge to `main` ahead of v1 if a need arises. Default:
stay on `rafaello-v0.1`.

## Per-milestone deliverables

Each milestone subdirectory under `milestones/m<N>-<name>/` has:

- `scope.md` — what's in, what's deferred, the demo bar. Drafted by
  claude, pi-reviewed, owner-ratified before commits-list work begins.
- `commits.md` — ordered commit list, each with subject + rationale +
  acceptance criteria + dependency on prior commits. Drafted by
  claude, pi-reviewed, owner-ratified before per-commit agent work
  begins.
- `retrospective.md` — milestone-end review, including any updates to
  `overview.md` / `decisions.md` and a coverage report against
  `commits.md`.

## Open questions for pi review

- Is the m1 → m2 → m3 ordering correct, or should `rafaello.lock` come
  before any plugin spawning so the very first plugin we run goes
  through the real install flow rather than getting retrofitted? My
  argument for the current order: m1 is small enough that retrofitting
  is cheap; m2 wants a real plugin to install which means m1's test
  fixture is ideal as the install target. Argument against: real grant
  flow first means we never have a not-yet-locked code path.
- Is splitting m3 into "m3a daemon+TUI without agent loop" and "m3b
  agent loop with mock provider" warranted, or is m3 the right size?
- Should a built-in mock provider exist before m4, or do we use
  rfl-litellm against a deterministic local model the entire time?
- Is "no third-party plugin renderers in v1" worth a `decisions.md`
  row even though it's a deferral? My instinct is yes — it's a v1
  commitment that affects plugin authors today.
- Is m6 (polish) better folded into m5 to ship v1 sooner, or is the
  cleanup work substantial enough to merit its own milestone?
