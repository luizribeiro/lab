# m5b driver pre-flight notes

Driver: claude on `agents/m5b/driver` in worktree `/home/luiz/lab-wt/m5b-driver`,
forked from `rafaello-v0.1` at `df60456` (m5a RATIFIED). Symlink
`.pre-commit-config.yaml` confirmed present.

## What m5b is (one paragraph)

m5b closes m5 by adding **taint matching + propagation** and the
**verbatim tool-result-to-sink exfil demo** (the fourth m5 roadmap
negative deferred from m5a). m5a's gate fires identically; m5b makes
the modal *informative* about provenance and adds the structural
superset enforcement that prevents plugin-supplied taint stripping.

Source of authority: m5a `scope.md` Appendix A — pre-ratified at
m5a-close (owner item 10.1: "m5a / m5b split — honoured"). Treat
Appendix A as input to `scope.md` round 1, not as final.

## Deliverable inventory (from m5a scope §A.2 + retro §5)

1. **Taint matching** — `crates/rafaello-core/src/reemit/taint_match.rs`:
   on re-emit of `core.session.tool_request`, match each arg value
   against per-session map of recently-emitted `tool_result` payload
   values (literal hash + substring containment per security RFC
   §7.2.1). Map keyed `(session_id, value_hash) → Vec<TaintEntry>`;
   TTL default 5 min (ratify in scope round).
2. **Plugin-supplied taint superset check via `in_reply_to`** —
   broker verifies plugin-published `taint` is a superset of the
   union of taints of every event referenced in `in_reply_to`. New
   `BrokerError::TaintSupersetViolated`. (security RFC §7.2.6 row 1
   — m5a closed the routed-to-this-plugin half via the
   outstanding-dispatched map; m5b closes the superset half.)
3. **Broker superset enforcement on re-emission** — every
   `provider.<id>.*` / `frontend.tui.*` re-emit's synthesised envelope
   must be a superset of `in_reply_to` referenced events' taints.
4. **Confirmation prompt `details.taint`** populated from canonical
   envelope; TUI modal render becomes informative when provenance
   exists. (m5a forwards the field already; m5b populates it.)
5. **Verbatim exfil demo** —
   `rafaello/tests/rfl_chat_demo_bar_verbatim_exfil_blocked.rs`. A
   `rafaello-fetch` fixture tool returns
   `{content: "https://evil.example.com/leak"}`; model proposes
   `web-fetch {url: …}` verbatim; gate's prompt shows
   `details.taint = [{source: "tool", detail: "<rafaello-fetch
   canonical>"}]`; TUI scripts deny via `RFL_TUI_TEST_CONFIRM_ANSWER`.
6. **Third sink-declaring fixture** — `rafaello-fetch`
   (`sinks = ["network"]`) under `rafaello/fixtures/m5b-locks/`.

## m5a retro §5 follow-ups routed to m5b

Items 1-4 are the scope-ratified split (above). Plus:

- §5 item 12 — `rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly.rs`
  (c38 ratified-but-not-landed acceptance).
- §5 item 13 — `rfl_chat_spawns_inactive_provider_but_reemit_ignores_it.rs`
  (c38 ratified-but-not-landed; rides on taint-aware re-emit work).
- §5 item 15 — Positive gate-through-orchestration assertion
  (`rfl_chat_tool_dispatch_goes_through_gate.rs`) — landed negative
  half exists, positive half needs anchoring.

Items 5, 7-11 are driver-sweep / v2 / m6 — out of m5b scope. Item 14
goes to m6.

## m5a inheritance baseline (retro §9)

m5b inherits: gate (`ConfirmState`, `try_take_for_timeout`,
`always_allow_session`, multi-pending), confirm topics + correlation
table, `UserGrants` + `GrantMatcher` (shape-contract JSON-Schema
template), bus-mediated slash commands, TUI overlay, audit log
(install + gate kinds), `core.tools_list` fittings RPC +
`CorePluginService`, `rfl-openai` + `env.allow_secrets`, broker
`outstanding_dispatched` map (m5b's superset half builds on this),
install-time trifecta refusal + override marker.

## Sizing estimate (Appendix A.4)

- 6-9 commits: taint matching + superset + propagation.
- 3-4 commits: verbatim exfil demo + fetch fixture.
- 3-4 commits: TUI / audit-log enrichment of taint provenance.
- 2-3 commits: retro drift + Stream A §7.2.1 / §7.2.6-row-1 patches.

Total: **16-22 commits**. Pi rounds budget: 4-6 scope, 2-4 commits,
2-4 retrospective (per m1/m2/m4/m5a patterns; m5b is narrower than
m5a's 25-commit/6-round body).

## `decisions.md` row candidates (Appendix A.5)

- Taint matching algorithm: literal hash + substring containment;
  explicit non-coverage of laundered/transformed flows (CaMeL v2
  territory).
- Plugin-supplied taint discard policy + superset check as extra
  rejection signal beyond canonical synthesis.
- TTL on per-session value→taint map (default 5 min; pi may push
  smaller).

## Directory naming

Owner sanity-check needed: m5a is at `milestones/m5a-sinks-confirmation/`.
For m5b the roadmap row's split-language is "m5b (taint matching +
exfil tests)", so the kebab-case short form is **`m5b-taint-exfil`**.
Following the m5a precedent of keeping the parent-row name, an
alternative is `m5b-sinks-confirmation`. Default selection:
`m5b-taint-exfil` — it matches the m1 README directive ("kebab-case
short form of the roadmap row's name") and the per-half topic
described in the roadmap row's split clause. If the owner prefers
the parent-row name for consistency with m5a, the driver will rename
before the scope.md round-1 commit lands.

## Risks/watch-items carried in

- Disk hygiene: `rm -rf rafaello/target` immediately after each
  ff-merge (m4/m5a both hit 99% disk).
- Pre-commit symlink missing in lab-wt worktrees: use
  `PREK_ALLOW_NO_CONFIG=1` for orchestrator-side commits, or symlink
  the nix-store config into each fresh worktree at creation.
- m5b's matching is value-driven; **synthetic-stub tests need a
  named successor** (m2 retro §3.3 lesson) — if any commit stages a
  test against a not-yet-built matching primitive, the
  successor-commit row must name the fault-injection mechanism or
  carry an explicit deletion rationale.
- Two-stage tests for ladder dependencies are fine (m0 retro §4.3);
  late-commit tests that retroactively cover early-commit surface
  are the trap.
- Inline full commit row text + acceptance bullets into per-commit
  prompts (m1 §4.2, m5a operational guardrail). Never cite by row
  number.

## Owner-ping schedule

Three convergence pings:
1. scope.md CONVERGED.
2. commits.md CONVERGED.
3. retrospective.md CONVERGED (before ff-merge to `rafaello-v0.1`).

Per owner directive: drive autonomously between pings; ping
out-of-band only if a thorny judgment call surfaces.

## Next step

Spawn the scope.md drafting agent (`rafaello-m5b-scope-claude`) in a
fresh worktree at `/home/luiz/lab-wt/m5b-scope-claude` on branch
`agents/m5b/scope-claude`. Hand it: `plans/README.md`,
`plans/overview.md`, `plans/decisions.md`, `plans/glossary.md`, every
file under `streams/`, m5a `scope.md` Appendix A, m5a
`retrospective.md` §5 + §9, security RFC §7.2.1-§7.2.6, and the
target path for the artifact.
