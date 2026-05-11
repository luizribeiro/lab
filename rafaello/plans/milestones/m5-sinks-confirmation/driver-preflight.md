# m5 driver pre-flight notes

Worktree: `/home/luiz/lab-wt/m5-driver` on `agents/m5/driver`, forked from
`rafaello-v0.1@2781470` (m4 RATIFIED 2026-05-11).

## What m5 is (per `milestones/README.md` roadmap row)

> **OpenAI-compatible provider + sinks + confirmation protocol +
> user_grants + exfil demo**

Pre-ratified deliverables:

1. Bundled `rfl-openai` subprocess plugin — speaks the OpenAI Chat
   Completions wire protocol generically (decisions row 38); default
   model `vllm/qwen3.6-27b`; LiteLLM proxy is *deployment config*,
   materialised by `rfl init` (which is **m6**, not m5) into
   `rafaello.lock`.
2. Manifest **sink classes**: `network`, `vcs_push`, `mail`,
   `workspace_write` (declared per-tool under
   `[provides.tool.<n>] sinks = [...]`; snapshotted into
   `bindings.tool_meta.<n>.sinks` per security RFC §7.2.5).
3. **Confirmation protocol** on the bus — three topics:
   `core.session.confirm_request`, `frontend.tui.confirm_answer`,
   `core.session.confirm_reply`. Core-mediated, fail-closed, blocks
   sink-declaring `tool_request` until matching reply or `user_grants`
   entry (decisions row 9; security RFC §7.2.3).
4. TUI **confirmation modal** (blocks input).
5. **`user_grants` session table** — in-memory only (never persisted to
   lock); created via `/grant`, `/grants list`, `/revoke` slash commands
   and `always_allow_session` answers; clears on `rfl chat` restart
   (overview §6.4; security RFC §7.2.4).
6. **One-hop trifecta guardrail at install time** — refuse plugin with
   all three of `reads_untrusted`, `has_outbound`, `has_workspace_write`
   unless `--i-know-what-im-doing` override (decisions row 11; security
   RFC §7.1.1).
7. **Taint matching + propagation rules** + the **broker-side gate**
   consuming the taint envelope on sink calls. m4 shipped the envelope
   (presence, structural shape, core-supplied origin); m5 adds the
   *propagation* half (matching arg values to recent results, security
   RFC §7.2.1–§7.2.2) and the *consumption* half (broker gate that
   reads taint when deciding whether a sink call needs confirmation).

Demo bar (positive + negative):

- **Positive**: real model call against configured OpenAI-compatible
  endpoint; model proposes a sink-declaring tool; confirmation prompt
  fires; user accepts → tool runs; user denies → tool refused.
- **Negative**: confirmation timeout denies; `always_allow_session`
  clears on `rfl chat` restart; **verbatim tool-result-to-sink flow
  blocked at the broker**; one-hop trifecta only (transitive flows
  explicitly out of v1 per decisions row 11).

## m4 carryovers routed to m5

From m4 `retrospective.md` and `scope.md` § "Out of scope":

- **§2.6 lock-side `check_lock_publish_topic` unknown-namespace gap**
  — broker rejects unknown namespaces at runtime; lock-side compile-time
  validation re-filed for m5+. Small but ties into the install-time
  validation surface m5 already touches.
- **§5.1 / §5.2 dead-code suppressions**: `RegisteredProvider.peer` and
  `SpawnRegistration::Provider` variant are stored-but-unread in m4;
  m5's confirmation gate is the documented natural reader.
- **§Out of scope (m4 scope.md L2952+)** explicitly hands to m5:
  sinks, confirmation protocol + UI, `user_grants`, taint matching /
  propagation, broker-side sink gate, slash commands, broker-side
  stale-correlation map on `plugin.<id>.tool_result.in_reply_to`
  (pi-3 M-2 — m5 needs the same per-plugin outstanding-tool_request
  map anyway, so they land together), `rfl-openai` provider plugin
  code, sink-class on `read-file`, `always_confirm = true`, audit-log
  table.
- m4 §5.5 "m5 inherits": Provider publisher class, `subscribe_internal`
  + `ReemitRouter`, `AgentLoop` + tool-dispatch wiring,
  `frontend.tui.user_message` ACL grant, `RFL_TUI_TEST_MESSAGE`
  env-hook, two fixture plugins, four-level orchestration tree in
  `rfl chat`, post-fix carveout decomposition. m5 builds **on top of**
  m4's stable envelope.
- **Decisions row 39** (m2 supervisor's `bindings.provider = true`
  refusal) was removed in m4 c19; m5 inherits provider spawn as
  routine.

NOT m5:

- `rfl init` (materialising the lock with endpoint URL and env-var
  name) → **m6**.
- Multiple active providers, `rfl provider use <id>`, provider
  hot-swap → post-v1.
- Streaming entry patch ops (`stream_state: "open"` / `"patch"`) →
  v2 per decisions row 28.
- Helper plugins → v2 per decisions row 26.
- TUI command palette (the slash commands are flat string commands in
  the input line, not a palette UI).

## Sizing & split recommendation: **split m5a / m5b**

The roadmap row explicitly allows this: "May split into m5a (sinks +
confirmation + user_grants) and m5b (taint matching + exfil tests) if
scoping finds it too big."

My read: it is too big. m4 was ~47 commits. m5 single-shot would
plausibly run 60–80 commits across:

- 6–10 commits for `rfl-openai` wire protocol plugin (request shape,
  response streaming, error mapping, `provider.openai.*` publish,
  fixture lock entry, tests against a stub endpoint + against the dev
  LiteLLM proxy).
- 8–12 commits for sink-class manifest schema, lock bindings, m1
  schema cutover, install-time trifecta refusal + override flag.
- 10–14 commits for the confirmation protocol (three topics, broker
  gate plumbing, `user_grants` table, slash commands `/grant`,
  `/grants list`, `/revoke`, `always_allow_session` answer state,
  TUI modal + input-blocking + render entry, audit-log table).
- 10–15 commits for taint propagation (arg-value matching against
  recent results, plugin-supplied taint validation via `in_reply_to`,
  broker-side superset enforcement on re-emission, broker-side gate
  consuming envelope on sink calls).
- 6–10 commits for the exfil demo and negatives (verbatim
  tool-result-to-sink blocked, one-hop trifecta verification,
  confirmation timeout, `always_allow_session` clear on restart,
  per-plugin stale-correlation map exercise).
- Plus m4 carryovers (lock-side `check_lock_publish_topic`, dead-code
  unsuppression) and integration tests.

That's two milestones of work. The split breakpoint is clean:

- **m5a** — *real provider + the gate's plumbing*. `rfl-openai` plugin;
  manifest sink classes + lock bindings; install-time trifecta refusal
  + override flag; confirmation protocol topics + broker gate (gate
  fires on **any** sink-declaring tool call lacking a matching
  `user_grants` entry — taint-independent path, decisions row 9);
  `user_grants` session table; slash commands; TUI modal; audit log;
  lock-side `check_lock_publish_topic`; per-plugin
  outstanding-tool_request map (consumed by gate + by m4 pi-3 M-2
  follow-up). Demo: real model call → sink-tool call → confirmation
  fires → accept/deny round-trip; negatives for timeout,
  `always_allow_session`-on-restart, install-time trifecta refusal.
- **m5b** — *taint matching + exfil*. Taint propagation rules (arg
  value matching against recent results, plugin-supplied taint
  validation, broker superset enforcement on re-emission); broker gate
  reads taint to refuse verbatim tool-result-to-sink flows even when a
  `user_grants` entry covers the invocation (the "trifecta caught at
  the bus" structural fix); exfil demo + the verbatim-flow-blocked
  negative.

m5a's demo bar is the roadmap's positive demo plus three of four
negatives; m5b adds the verbatim-flow-blocked negative. Each is sized
like m3 / m4 individually. Splitting also lets the dev-environment
LiteLLM round-trip land in m5a so m5b can lean on a real provider
fixture rather than another mock.

**Recommendation in `scope.md`**: propose the split with the breakdown
above, and let the pi rounds + owner decide. If owner pushes back I
fall back to single-milestone with the explicit understanding that
this will be a multi-week driver session.

## Risk inventory before spawning the scope agent

- **Stream A drift**: security RFC §7.4.1 still talks about helper
  plugins (deferred to v2 per decisions row 26); §10 still has the
  earlier "non-user taint AND sink" formulation superseded by decisions
  row 9. The m5 retrospective will patch these, but the scope agent
  must read the RFC **with decisions row 9 / row 26 / row 27 on top**.
- **Stream F drift**: manifest RFC predates overview §15.1 normative
  delta that locked in the `[provides]` + `[provides.tool.<n>]` shape
  including `sinks`, `grant_match`, `always_confirm`. The scope agent
  must treat overview §15.1 as the source of truth for the manifest
  fields landing in m5.
- **Confirmation protocol naming**: the roadmap row uses
  `frontend.tui.confirm_answer`; topic grammar is constrained by ACL.
  m4 already grants the `frontend.tui.user_message` publish; m5 will
  symmetrically extend it for `frontend.tui.confirm_answer`. Lock-side
  unknown-namespace enforcement (the §2.6 carryover) intersects with
  this.
- **Always-confirm vs. sink-confirm**: manifest can declare
  `always_confirm = true` on a non-sink tool. m5 must implement that
  gate path even though no v1 tool uses it yet (m6 may).
- **Real-network test**: the demo requires hitting the LiteLLM proxy.
  CI will not have `LITELLM_API_KEY`. The integration test that
  exercises `rfl-openai` end-to-end will need a stub endpoint
  (recorded fixture or local HTTP server) for CI, plus a manual
  validation entry that hits the real proxy from dev. Plan for both
  in `commits.md`.
- **Sizing signal m4 §4.3** explicitly warned: "m5 introduces sinks +
  confirmation + user_grants — likely back to a larger bracket." This
  is the strongest data point for the split.

## Initial driver plan

1. Commit these notes on `agents/m5/driver`.
2. Spawn the scope.md drafting agent (claude session
   `rafaello-m5-scope-claude` on a fresh worktree at
   `agents/m5/scope-claude`). Hand it: the roadmap row, this
   pre-flight, the m4 retrospective + m4 scope §"Out of scope" +
   security RFC + Stream F manifest RFC + overview §6 / §7.2 / §15.1,
   decisions rows 9–13, 26–29, 38, 39. Ask it to draft `scope.md`
   **for m5a** with m5b explicitly carved out as a follow-up
   milestone scope.md sketch. (If the owner rejects the split in the
   pre-flight commit response, fall back to a unified m5 scope.md.)
3. Pi round 1 review.
4. Iterate to convergence (budget 4–8 rounds per the patterns from
   prior milestones).
5. Owner ratification ping on `/tmp/rafaello-m5-owner-ping.txt`.
6. Then commits.md (3–5 rounds), then Phase 3, then retrospective.

If owner sign-off comes back rejecting the split, I'll re-spawn scope
agent with the unified-m5 framing.
