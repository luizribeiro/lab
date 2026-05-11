# m5a — sinks + confirmation protocol + user_grants + rfl-openai — retrospective

> **Status: round 1 draft.**
> Worktree `/home/luiz/lab-wt/m5-retro-claude` on branch
> `agents/m5/retro-claude`, forked off `agents/m5/driver` at the c41
> tip (`5e36890`) where all 41 m5a plan-row commits have landed in
> 1:1 correspondence with `commits.md` round-6 ratification (`3759d5e`).
>
> `scope.md` converged in **6 pi rounds** (matching m4's bracket
> exactly); `commits.md` converged in **6 pi rounds** (m4 was 3, m3
> was 9, m2 was 4). The wider commits-round count reflects m5a's
> larger plan-row count (41 vs. m4's 28) and the unsplittable c38
> cutover that took two rounds of pi pressure to size.
>
> Companion: `manual-validation.md` — landed as the c41 skeleton
> (`5e36890`). §8 below enumerates the bullets that skeleton needs
> filled before merge.
>
> This document is the milestone-level review against `scope.md`
> round 6 and `commits.md` round 6, following the `plans/README.md`
> Phase-3 contract and the m4 retrospective shape (extended to ten
> sections per the m5a driver brief — m5a introduces enough new
> surface, plus two in-flight orchestrator carveouts, that the m4
> five-section shape was too coarse).

---

## 1. Summary

m5a ships **sinks + confirmation protocol + user_grants + the
bundled `rfl-openai` provider + install-time trifecta refusal + the
demo-bar headline + three of the four roadmap-row negatives**. The
fourth negative (verbatim tool-result-to-sink exfil, blocked at the
broker via taint matching) is m5b's territory per the scope split
the owner ratified at `95d6f12`.

**Commit count.** 41 plan-row commits on `agents/m5/driver`
(`1c844e5..5e36890`), in 1:1 correspondence with `commits.md` rows
c01–c41; no mid-Phase-3 bundling, no docs-only insertions between
rows. Phase-2 docs-iteration commits (scope rounds 1–6 + 6
pi-review files; commits rounds 1–6 + 5 pi-review files; the two
ratification commits — 28 commits total, `aaa7ccc..3759d5e`) land
before c01 and are not counted in the plan-row total.

**LoC.** `git diff rafaello-v0.1..HEAD --shortstat` reports
**338 files changed, 28,246 insertions, 254 deletions** across the
41 plan-row commits + 28 docs commits. The plan-row half adds
**206 new `tests/*.rs` files** across `rafaello-core/tests/`,
`rafaello-openai/tests/`, `rafaello-mailcat/tests/`, and
`rafaello/tests/`. (Pre-m5a inventory was ~382 top-level test files
per m4 retro §1; m5a brings the workspace to ~588.)

**Demo bar status.** All four m5a-scoped demo-bar arms green:
- Positive (`rfl_chat_demo_bar_send_mail.rs` allow + deny) — c39
  (`a63dbbc`).
- Negative 1 (timeout, `rfl_chat_demo_bar_send_mail_timeout.rs`) —
  c40 (`a12ef69`).
- Negative 2 (`always_allow_session` clears on restart,
  `rfl_chat_always_allow_session_clears_on_restart.rs`) — c40.
- Negative 3 (install-time trifecta one-hop refusal,
  `rfl_install_refuses_trifecta_plugin.rs` +
  `rfl_install_does_not_chase_transitive_outbound.rs` +
  `rfl_install_refuses_one_hop_outbound_via_other_plugin.rs`) —
  c29 (`b6da311`).

Bonus negatives all green: `rfl_chat_always_confirm_true_holds_non_sink_tool.rs`,
`rfl_install_status_shows_red_for_override.rs`,
`rfl_chat_grant_revoked_blocks_next_call_but_not_in_flight.rs`,
and the m4 §5.1 closer
`broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`.
Negative 4 (verbatim exfil) deferred to m5b.

**Pi convergence trajectory.**
- `scope.md`: 6 rounds (round 1 → round 6 ratified `95d6f12`). m4
  was 6, m3 was 22, m2 was 8, m1 was 4, m0 was 3.
- `commits.md`: 6 rounds (round 1 → round 6 ratified `3759d5e`).
  m4 was 3, m3 was 9, m2 was 4, m1 was 3, m0 was 3.

The pi-3 driver brief predicted "back to a larger bracket" for m5
relative to m4 (m4 retro §4.3) because m5 introduces sinks +
confirmation + user_grants. The actual scope bracket matched m4's
6 rounds exactly; the **commits bracket doubled** (3 → 6) because
the c38 unsplittable cutover required adversarial pressure across
pi-1 B-3 (bundle the agent-loop pivot), pi-3 M-2 (spell out the
spawn order), pi-4 M-1 (active-canonical vs. provider-id resolution),
and pi-5 M-1 (catalog-build wiring). Two of those four findings
landed in the same row.

---

## 2. Implementation surprises / non-obvious decisions

### 2.1 c38 SlashHandler `Mutex` → `RwLock<UserGrants>` lock-type unification (in-flight carveout)

**What happened.** c18 (`8b92487`) implemented `SlashHandler` with
`Arc<parking_lot::Mutex<UserGrants>>` because the slash-command
handler only ever needs short critical sections (insert / remove /
list). c20+ specified `Arc<RwLock<UserGrants>>` for the
confirmation gate because the gate's hot path does read-only
grant-match lookups under contention with re-emit traffic; a
`Mutex` there would serialise every passthrough check.

The two lock types collide in `run_chat`: the orchestrator owns one
`Arc<UserGrants>` and shares it between the slash handler and the
gate. At c38 the orchestrator had to pick one, and the gate's
hot-path requirement won.

**How it was authorised.** The orchestrator authorised c38 as an
**unsplittable cutover** that migrates `SlashHandler`'s field type
from `Mutex` → `RwLock` plus the call-site updates (slash test kit,
seven `core_slash_command_*` tests) in the same commit that lands
the chat orchestration extension and the agent-loop pivot (CG6).
This matches the m0 c08 / m4 c07 precedent for cutovers that
cannot be split without leaving the tree in a broken intermediate
state: a "slash handler still on Mutex" intermediate commit would
not compile because `run_chat` would have to choose one shared
`Arc<…<UserGrants>>` type.

Additional sub-changes folded into c38 under the same authorisation:
- `PluginSupervisor::TestHooks::first_spawn_instant_nanos()` —
  added so `rfl_chat_constructs_gate_before_provider_spawn.rs`
  can assert (at the supervisor seam) that gate construction
  precedes the first plugin spawn by some positive nanos.
- `tests/common/synthetic_dispatch.rs` test helper — the agent-loop
  pivot deleted `broker.publish_for_tool_dispatch` from
  `agent/mod.rs::handle_tool_request`, which forced the m4
  `cross_provider_request_to_tool_only_routes_via_core` test to
  acquire a parallel construction path (synthetic dispatch through
  the gate) for its assertion shape.

**m5a equivalent of m2's fixture/Stream A drift.** This is the same
class of late-discovered consistency requirement that m2 carried
through Stream A's fixture-validator banner: a design choice from
one phase (c18) that the next phase (c20–c24) needs to consume
differently, ratified mid-flight by the orchestrator and recorded
in the cutover commit body.

### 2.2 c39 `ENV_PASS_ALLOWLIST` extension (one-line in-flight carveout)

**What happened.** c37 (`5590a2f`) added three new TUI test-driver
env hooks — `RFL_TUI_TEST_CONFIRM_ANSWER`,
`RFL_TUI_TEST_CONFIRM_DELAY_MS`,
`RFL_TUI_TEST_GRANT_BEFORE_MESSAGE` — extending the m4
`RFL_TUI_TEST_MESSAGE` pattern. c38 (the chat orchestration
cutover, §2.1) wired the gate + slash plumbing the new hooks
depend on, but did **not** extend
`rafaello/crates/rafaello/src/lib.rs::ENV_PASS_ALLOWLIST` to
forward the new vars through `Frontend::spawn`'s `env_clear`. The
parent process scrubs every env var not on the allowlist before
spawning the TUI subprocess, so the new hooks reached the parent
but never reached the child.

The gap surfaced at c39 (the demo-bar headline) when the test could
not drive the confirm modal end-to-end. c39 folded the one-line
allowlist extension as a `c37/c38 follow-up` in the same commit,
declared in the commit body (`a63dbbc`).

**Why this is smaller than §2.1.** It's a single-line allowlist
add, not a cross-module lock-type cutover. But it shares the same
class: a phase-N test surface that depends on a phase-N-1
plumbing edit nobody noticed was missing until phase N tried to
consume it. Logging it for the next milestone driver as a thing
to scan for: **whenever a new `RFL_*` test env var lands in a
child crate, grep the parent's `ENV_PASS_ALLOWLIST` in the same
commit or the next.**

### 2.3 Test-seam shape: `PluginSupervisor::TestHooks::first_spawn_instant_nanos()`

The c38 test
`rfl_chat_constructs_gate_before_provider_spawn.rs` needs to assert
a strict ordering ("gate constructed before any plugin spawn")
without coupling to wall-clock or `tokio::time::Instant`
constructor flakiness. The chosen seam exposes a monotonic
`Instant::now().elapsed().as_nanos()` captured **inside** the
supervisor on the first spawn call, gated behind a `cfg(test)`
`TestHooks` impl. Rationale: the seam is supervisor-internal
(constructor-of-construction), not bus-observable, so it does not
need a topic / ACL change. m6 should keep this pattern in mind for
any other "this happens before that" assertions where wall-clock
ordering is the assertion target.

### 2.4 `audit_events` SQLite location decision deferred

c08 (`5bfec74`) lands the `audit_events` table + `AuditWriter`,
but the writer's per-session on-disk path is **not** pinned in
either `scope.md` §AL or `commits.md`. c27 (`ad56317`) hard-codes
`<project_root>/.rafaello/state/audit.sqlite` for the install
audit path and creates the dir on `open_for_install` (pi-2 M-2),
but the `rfl chat` audit writer in c38 inherits the path from the
in-flight session tempdir — not a stable on-disk location. Routed
to §5 as an m6-or-earlier follow-up; the choice is between
"per-session subdir under the project state dir" and "single
append-only DB shared across sessions with a `session_id` column".

### 2.5 Five-tree spawn order (c38)

`commits.md` pi-3 M-2 / pi-4 M-1 ratified the c38 spawn order:
**active provider → other providers (inactive) → tool plugins**,
where "active" means the canonical id named in
`lock.session.provider_active` resolved via `CanonicalId::parse`
against `compiled_plugins` (not the `provider_id` public-namespace
segment from `PluginAcl.provider_id`). The implementation matches.
The c38 commit body (`ce0342b`) records the resolution and cites
pi-3 M-2 + pi-4 M-1; the supervisor patch lands two new fields on
its spawn-record (`provider_active: bool`, `is_tool: bool`) to
gate the ordering loop.

### 2.6 `effective_grant` union mechanics across `grant.bundles`

`compile.rs::effective_grant` unions `env.pass` and
`env.allow_secrets` across **every value** in `grant.bundles`
(`for bundle in grant.bundles.values()` per `compile.rs:260-312`).
The round-5 scope.md drafted a `default+named` framing that the
live code does not implement; pi-5 M-3 caught the mismatch, and
the round-6 scope.md (and the c06 commit body `f77cc7d`) reflect
the live union-across-all shape. The retrospective records this
to keep future readers from re-introducing the `default+named`
mental model.

### 2.7 `grant_match` as a JSON-Schema template, not a per-tool-call validator

§UG2 / §A1 (settled scope round 2) makes `grant_match` validate
the **user's matcher template** at `/grant` time, then performs
runtime matching as a structural-subset comparison. It is **not**
full per-tool-call JSON-Schema validation against the tool's
OpenRPC schema. Two design implications surfaced during
implementation:

1. c16 (`0fb0ddd`) compiles each template against the
   tool's `grant_match` schema at `/grant` time, **caches the
   compiled validator** on the grant entry, then discards the
   validator after compile (the runtime check is structural).
2. The slash handler's "default plugin resolution" (pi-4 B-2)
   uses `BrokerAcl::tool_route(tool)` from the dispatch-target
   table, not `session.tool_owner` (which is empty when there's
   no conflict). c18's `SlashHandler` gains an `Arc<BrokerAcl>`
   field for this lookup.

### 2.8 Scrubber-signature cutover (c06, large body-justified)

c06 (`f77cc7d`) is the m5a `allow_secrets` schema + scrubber
signature cutover — declared `large` (~300 LoC) in
`commits.md` §"Sizing summary" with body justification (m0 c08 /
m4 c07 precedent). The cutover updates every live scrubber call
site in the same commit so the in-tree state never carries a
two-shape scrubber. No agent-side pressure to split during Phase
3. The c06 commit body cites scope §M1.1 and the m4 c07
precedent.

---

## 3. What deviated from commits.md

Of the 41 plan rows, **39 landed exactly as written**. Two rows
deviated, both as in-flight carveouts described in §2:

| Row | Deviation | Rationale | Routed forward to |
|-----|-----------|-----------|-------------------|
| c18 + c38 | `SlashHandler` shipped at c18 with `Arc<Mutex<UserGrants>>`; c38 cutover migrates it to `Arc<RwLock<UserGrants>>` plus call-site / test-kit updates (§2.1) | Lock-type chosen at c18 collides with gate hot-path requirement at c20+; cannot split the cutover without an intermediate non-compiling state | none (cutover lands self-contained) |
| c37 + c39 | `ENV_PASS_ALLOWLIST` extension for the three new `RFL_TUI_TEST_*` vars folded into c39 (the demo-bar headline) rather than c37 or c38 (§2.2) | The need surfaced at c39 when the demo-bar test could not drive the modal; c37 added the vars in the child crate but c38's parent-side wiring missed the allowlist extension | §6 (driver-process gotcha) + §4 (process notes) |

No mid-Phase-3 file renames or test relocations. No mid-Phase-3
row reorderings. Three rows (c06, c10, c38) are unsplittable
cutovers and were declared as such in `commits.md` round 6 — none
needed further pi pressure on the size declaration during Phase 3.

---

## 4. Sizing signal

### Pi rounds

- `scope.md`: **6** (m4: 6, m3: 22, m2: 8, m1: 4, m0: 3).
- `commits.md`: **6** (m4: 3, m3: 9, m2: 4, m1: 3, m0: 3).

The scope bracket matched m4 exactly despite m5a's larger surface
(sinks + confirmation + user_grants + a bundled provider plugin
+ install-time refusal). The commits bracket **doubled relative
to m4**, which the retrospective attributes primarily to the c38
unsplittable cutover. c38's wiring (gate construction order,
spawn order, agent-loop pivot, slash-handler lock migration,
test-seam additions) generated four separate pi findings across
rounds 1, 3, 4, and 5. No other row generated more than two pi
findings.

### Phase-3 walltime

Phase 3 ran roughly one driver day for the 41 plan-row commits.
Per-commit walltimes (orchestrator log spot-checks):

- c06 (the `allow_secrets` cutover, large): ~22 min.
- c10 (broker outstanding-dispatched map cutover): ~18 min.
- c38 (unsplittable cutover): **~48 min** — within the driver
  brief's ~48 min budget for the row.
- Median across the remaining 38 rows: ~9-12 min, matching m4's
  per-commit profile.

No disk-full restarts during Phase 3 (the m4 §4.1 mitigation —
`rm -rf <wt>/rafaello/target` after each ff-merge — held across
all 41 ff-merges). No `Cargo.lock` ff-merge aborts (m2 / m4 §4.5
stash mitigation held).

### Mis-budgeted rows

None observed. The 30 small / 4 small-medium / 13 medium / 2
medium-large / 1 large / 1 unsplittable-cutover-not-already-counted
declaration in `commits.md` §"Sizing summary" landed without
agent-side splitting requests on any of the 41 rows. The
c38 ~48-min walltime landed in the budgeted 30-60 min window the
driver brief reserved for an unsplittable cutover.

---

## 5. Follow-ups routed to m5b or m6

| # | Item | Surface | Routed to |
|---|------|---------|-----------|
| 1 | Verbatim tool-result-to-sink exfil negative + taint propagation primitives (§7.2.1 security RFC) | `rafaello/tests/rfl_chat_demo_bar_verbatim_exfil_blocked.rs` + `rafaello-core/src/reemit/taint_match.rs` | → m5b (scope.md Appendix A.2 items 1-5) |
| 2 | Plugin-supplied taint superset check against `in_reply_to` referenced events (`BrokerError::TaintSupersetViolated`) | broker re-emit path | → m5b (Appendix A.2 item 2) |
| 3 | Confirmation prompt `details.taint` population from canonical envelope | gate + TUI overlay render | → m5b (Appendix A.2 item 4) |
| 4 | Third sink-declaring fixture (`rafaello-fetch` with `sinks = ["network"]`) under `rafaello/fixtures/m5b-locks/` | new fixture | → m5b (Appendix A.2 item 6) |
| 5 | `audit_events` SQLite on-disk location decision (per-session subdir vs. single append-only DB with `session_id` column) — §2.4 | `AuditWriter` open-for-session path | → m6 (`rfl init` materialises the project layout) or earlier if m5b's audit-log enrichment forces the choice |
| 6 | Production `#[allow(clippy::result_large_err)]` sweep — m5a likely adds two new sites (`gate/mod.rs`, `slash.rs`) on top of m4's reemit + agent_loop pair; pin the boxing convention | workspace-wide error-shape choice | → m6 (deferred per m4 retro §5.5) |
| 7 | `rfl provider tool <plugin>` CLI for the manual-validation `grant_for_one_plugin_does_not_authorise_another` case | post-v1 per overview §8 | → v2 |
| 8 | Lazy-load via `load.triggers.kind = "tool"` still not exercised (m4 retro §5.8 carryover) | manifest schema | → m6+ |
| 9 | macOS CI green hard gate (m3 / m4 carryover ratification gate) | CI run URL in §4 of `manual-validation.md` | → driver post-merge sweep |
| 10 | Interactive `rfl chat` recording for `manual-validation.md` §1 (LiteLLM proxy + `send-mail` walkthrough) | recorded asciinema/transcript | → driver post-merge sweep (m4 §5.3 pattern) |
| 11 | `manual-validation.md` skeleton fill (§8 below enumerates the bullets) | the c41 skeleton | → driver post-merge sweep |

Item 5 (audit-log location) is the only **load-bearing decision**
the orchestrator deferred; items 6 + 8 are m4 carryovers; items
1-4 are the scope-ratified m5a/m5b split; items 7 + 9-11 are
known driver-sweep follow-ups.

---

## 6. Stream RFC drift

`git diff rafaello-v0.1..HEAD --name-only | grep streams` returns
empty: **no `streams/` RFC was modified during m5a Phase 3.**
`git diff rafaello-v0.1..HEAD --name-only | grep -E '^rafaello/plans/'`
returns only m5a-internal files (scope, commits, pi-review-N,
manual-validation, m5 driver-preflight) — no `overview.md`, no
`decisions.md`, no `glossary.md`, no `streams/*` files.

This is the **same shape m4 closed with** (m4 retro §2 lists
drift but it landed as separate follow-up commits **after**
Phase 3, not during). The retrospective predicts m5a will follow
the same pattern: §6 patches land as separate follow-up commits
on the retro branch before merge to `rafaello-v0.1`. Candidate
patches:

### 6.1 Stream A (security) — sink-class + confirmation + grants surface

Stream A's §10 banner currently records m4's broker provider
extensions. m5a adds:

- `SinkClass` enum + `CompiledPlugin::sinks()` accessor (c09).
- `ConfirmationGate` (c20-c24, c38) + the `core.session.confirm_*`
  + `core.session.confirm_resolved` topic family (c11).
- `UserGrants` + `GrantMatcher` + `jsonschema`-template-as-shape-contract
  (c15-c16).
- The broker `outstanding_dispatched` map atomic intake check
  (c10) — **partially closes** Stream A §7.2.6 row 1's
  "must reference the matching tool_request previously routed to
  this plugin" check (the routed-to-this-plugin half; the
  superset half is m5b).
- `audit_events` SQLite table + `AuditWriter` (c08).
- `env.allow_secrets` manifest extension + scrubber-signature
  cutover (c06).
- One-hop trifecta refusal + transitive-not-chased semantics at
  `rfl install` (c27 + c29).

Concrete patch: extend Stream A §10 banner with one paragraph per
bullet above, citing the implementing commit hash. Keep the m4
banner content; add an m5a section beneath it.

### 6.2 Stream F (manifest) — `env.allow_secrets`

Stream F's manifest schema currently does not record
`env.allow_secrets` because the field was added at m5a scope
round 3 (an `i_know_what_im_doing` fallback was the round-2
shape; the owner ratified the `env.allow_secrets` choice at
round-3 ratification). Concrete patch: Stream F §"Capabilities"
gains a one-paragraph entry citing scope §OP6 + c06 + decisions
row candidate (§7 below).

### 6.3 Other streams

Streams B/C/D/E (process / supervisor / TUI / install) are
unaffected by m5a — no patches needed.

---

## 7. `decisions.md` additions

m5a lands **three** load-bearing design choices that warrant new
`decisions.md` rows. Sketches below; an editor commit during the
retro-branch sweep adds them to `decisions.md` proper (m4
precedent: rows 43-45 landed as separate commits after the retro).

### 7.1 Row candidate: `env.allow_secrets` manifest extension

**Choice.** Manifest `[capabilities.<bundle>.env].allow_secrets:
Vec<String>` lists env var names that the scrubber honours
without the operator passing `flags.i_know_what_im_doing` at
install. Pairs with `env.pass`: only names that also appear in a
matching `env.pass` entry are forwarded; unused
`allow_secrets` entries emit a yellow stderr warning at
install + an `install_accepted` audit-payload entry (c27).

**Rationale.** The round-2 fallback (force every bundled provider
to use `i_know_what_im_doing`) made the demo-bar walkthrough hostile
to first-time users: a fresh operator running the bundled
`rfl-openai` would hit the red `[OVERRIDE]` marker for a routine
API-key forward. `env.allow_secrets` makes the consent explicit
at the manifest layer (plugin author declares which secret names
they need) and shifts the override marker from red `[OVERRIDE]` to
yellow `[explicit secret <NAME>]` at `rfl status`.

### 7.2 Row candidate: `grant_match` is a shape contract on the user's matcher template

**Choice.** `bindings.tool_meta.<tool>.grant_match` points at a
JSON-Schema file in the plugin package. At `/grant` time, the
slash handler validates the user's matcher **template** against
that schema. Runtime matching of a `tool_request` against the
grant is **structural subset** comparison, not full
per-tool-call JSON-Schema validation against the tool's OpenRPC
parameter schema.

**Rationale.** The shape-contract framing keeps the matching
algorithm O(structural-walk) at the gate's hot path while still
catching obvious template-shape errors (`/grant send-mail
to="alice"` when the tool's `to` arg is `array<string>`) at
template-compile time. Full per-call schema validation would
duplicate the existing OpenRPC parameter check and cost a
validator allocation on every tool dispatch.

### 7.3 Row candidate: the m5a / m5b split

**Choice.** The roadmap row for m5 (sinks + confirmation + secure
agent loop + verbatim demo) splits across two milestones:
- **m5a** lands sinks + confirmation + grants + the
  agent-loop pivot (gate drives dispatch) + bundled provider +
  install-time refusal + three of the four demo-bar negatives.
- **m5b** lands taint propagation (literal hash + substring
  containment) + plugin-supplied-taint superset check + the
  verbatim exfil negative + the `rafaello-fetch` fixture.

**Rationale.** Pre-authorised by the roadmap row's "May split…"
language (`milestones/README.md`). The taint primitives are a
self-contained surface that pairs naturally with the verbatim
demo; bundling them with m5a would have grown the milestone
past the m3 22-pi-round threshold. The pi-bracket evidence
(m5a scope landed at 6 rounds against the m4 baseline) supports
the split as the right line.

---

## 8. Coverage report

### What's tested

- All §I positive matrix rows: §W (workspace + scaffolds), §Si
  (sinks), §Tr (trifecta refusal), §CT (confirm topics), §CG
  (gate), §OM (outstanding-dispatched), §UG (user_grants), §SL
  (slash), §TUI (overlay), §OP (rfl-openai), §TP (mailcat),
  §AL (audit), §CHAT (rfl chat orchestration), §M1.1 (allow_secrets),
  §M1.2 (validate::check_lock_publish_topic). 206 new
  `tests/*.rs` files across the workspace.
- All §I negative matrix rows m5a-scoped: timeout, restart,
  one-hop trifecta, transitive-not-chased, bonus negatives
  (`always_confirm_true_holds_non_sink_tool`,
  `install_status_shows_red_for_override`,
  `grant_revoked_blocks_next_call_but_not_in_flight`).
- m4 §5.1 closer
  (`broker_plugin_tool_result_unknown_in_reply_to_rejected.rs`)
  via c10's outstanding-dispatched map (the routed-to-this-plugin
  half of the security RFC §7.2.6 row 1 check).
- All scope §"Demo bar" positive + 3 negatives + bonus
  negatives (§1 above).

### What's not tested

- **Verbatim exfil** (§5 item 1) — m5b's territory.
- **`rfl provider tool <plugin>`** rebinding (§5 item 7) — post-v1.
- **Lazy-load via `load.triggers.kind = "tool"`** (§5 item 8) —
  m4 carryover.
- **macOS CI green** (§5 item 9) — pending post-merge driver sweep.
- **`grant_for_one_plugin_does_not_authorise_another`** at the
  integration level — reachable only via `rfl provider tool`
  (v2); the unit-level `user_grants_plugin_pinned_does_not_match_other_plugin.rs`
  covers the data-structure half (`rafaello-core/tests/`).

### `manual-validation.md` bullets the c41 skeleton needs filled

The c41 commit (`5e36890`) lands the manual-validation.md skeleton
following the m4 §5.3 pattern. Each numbered section is currently
a step-list scaffold awaiting Phase-3 manual runs. To close before
merge:

1. **§1 Real-network demo** — command line for `rfl chat` against
   the dev LiteLLM proxy; captured stdout transcript of the
   `allow` arm and the `deny` arm; recorded asciinema or
   plain-text transcript demonstrating the confirmation overlay
   fires.
2. **§2 Slash-command demo** — captured `core.session.command_result`
   payloads for `/grant`, `/grants list`, `/revoke`; observation
   that the post-`/grant` user message does **not** prompt and
   the post-`/revoke` user message **does**.
3. **§3 Trifecta refusal demo** — `rfl install` stderr capture
   of the typed `TrifectaRefused` error naming the three booleans;
   `rfl status` ANSI capture showing the red `[OVERRIDE]` marker.
4. **§4 macOS CI URL** — the run URL after branch push.
5. **§5 TUI keyboard walkthrough** — observed `core.session.confirm_answered`
   event payload + downstream behaviour for each of `y` / `a` /
   `Enter` / `n` / `d` / `Esc` / `s`.
6. **§6 Audit-log inspection** — locate the on-disk SQLite
   (§5 item 5 / §2.4 above), dump the `audit_events` rows,
   assert ordering against operator actions.

Acceptable substitute coverage (m4 retro §5.3 precedent): the
mechanical green on `rfl_chat_demo_bar_send_mail.rs` allow + deny
arms suffices for §1's observation, **if** the owner accepts
mechanical-green-as-substitute. Default expectation is a recorded
run.

---

## 9. Inheritance — what m5b inherits

Per scope.md Appendix A.3, m5b inherits the m5a surface in full:

- **Confirmation gate** (`crates/rafaello-core/src/gate/`) —
  including the shared `ConfirmState` atomic state machine
  (§CG1a / c13), `try_take_for_timeout` (c23),
  `always_allow_session` short-circuit + `mark_session_grant_requested`
  (c24), and the multi-pending machinery (c24).
- **Confirm topic family** —
  `core.session.confirm_request`, `core.session.confirm_reply`,
  `core.session.confirm_resolved`, `frontend.tui.confirm_answer`
  (c11 + c12) — with the `confirm_resolved` wire-contract
  table.
- **`UserGrants`** + `GrantMatcher` + the `jsonschema`-template-
  as-shape-contract (c15 + c16) + the slash handler that mutates
  it (c18).
- **Bus-mediated slash commands** — `frontend.tui.slash_command`
  + `core.session.command_result` (c17 + c19), the
  `SlashHandler` shape (c18, RwLock'd at c38).
- **TUI overlay** — `InputMode::ConfirmOverlay` + multi-pending
  queue + ttl countdown (c25 + c26).
- **Audit log** — `audit_events` SQLite table + `AuditWriter`
  (c08), the install-time audit payloads (c27), the gate-time
  audit kinds (`confirm_request`, `confirm_allowed`,
  `confirm_denied`, `confirm_timeout`, `confirm_late`,
  `confirm_duplicate`, `confirm_unknown`,
  `confirm_allowed_with_session_grant`, `grant_added`).
- **`core.tools_list` fittings RPC** + `CorePluginService`
  shape (c31) on the supervisor.
- **`rfl-openai`** plugin (c03 + c32-c36) with the
  `env.allow_secrets` opt-in (c34's manifest + the c06 scrubber
  honour path) and the `core.tools_list` cache (c36).
- **Broker `outstanding_dispatched` map** (c10) — m5b's
  superset half builds on this map's existence.
- **Install-time trifecta refusal** via `rfl install --fixture`
  (c27) + `rfl status` override marker (c28).

m5b's superset-check on plugin-supplied taint (Appendix A.2 item
2) reads m5a's outstanding-dispatched map to find the
`in_reply_to` events whose taints need unioning. The same map
also gates m5b's "must reference the matching tool_request
previously routed to this plugin" check (the half m5a closed
mechanically via the broker atomic intake check).

---

## 10. Owner-judgment items still standing

The three items the scope ratification commit (`95d6f12`,
scope.md §"Owner-judgment items") surfaced for explicit owner
sign-off:

### 10.1 m5a / m5b split

**Status: honoured.** All four roadmap negatives split as
drafted: m5a covers positive + negatives 1-3 + bonus; m5b covers
negative 4 (verbatim exfil) per Appendix A.2 item 5. No
mid-Phase-3 pressure to expand m5a's negatives into negative-4
territory.

**Owner re-look before merge:** confirm the §5 item 1-4 routing
to m5b matches the owner's mental model of the m5b scope.md
draft (which writes after this retrospective ratifies).

### 10.2 `grant_match` shape-contract interpretation

**Status: honoured.** §2.7 above records the
template-validation-at-`/grant`-time + structural-subset-runtime
shape. c16 (`0fb0ddd`) compiles the template against the
plugin's `grant_match` schema; runtime matching is structural.
No agent-side pressure to expand to per-call JSON-Schema
validation.

**Owner re-look before merge:** confirm the runtime structural
shape is what owner expected. Specifically: the
`rfl_chat_grant_revoked_blocks_next_call_but_not_in_flight.rs`
+ `user_grants_plugin_pinned_does_not_match_other_plugin.rs`
test pair encodes the m5a structural-subset semantics; either
test failing under m5b's taint extension would signal drift.

### 10.3 `env.allow_secrets` manifest extension

**Status: honoured.** c06 (`f77cc7d`) lands the schema + scrubber
signature cutover; c27 (`ad56317`) lands the unused-entry warning
+ the audit-payload list; c34 (`dfeb465`) lands the `rfl-openai`
manifest using the field for `LITELLM_API_KEY`. `rfl status`
shows yellow `[explicit secret <NAME>]` rather than red
`[OVERRIDE]` for the bundled-provider path (c28's marker
extension).

**Owner re-look before merge:** confirm the `rfl status` marker
copy (`yellow`, the prose "explicit secret <NAME>") matches the
operator-facing tone the owner wanted at scope ratification. The
red `[OVERRIDE]` fallback path remains for
`flags.i_know_what_im_doing` plugins; the yellow path is **only**
for `env.allow_secrets` declarations. The two are mutually
exclusive at install time.

---

*End of m5a retrospective round 1 draft. Submitted for pi
adversarial review.*
