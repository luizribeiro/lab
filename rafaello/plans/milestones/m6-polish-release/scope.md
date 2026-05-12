# m6 — v1 polish + release readiness — scope

> **Status:** round 1 draft — claude-authored 2026-05-12, awaiting
> pi review. Forked off `rafaello-v0.1` at `b989fb2` (after m5b
> RATIFIED + the m6 driver pre-flight commit). m6 is the **LAST**
> milestone on the `rafaello-v0.1` integration branch; on its
> RATIFIED close, `rafaello-v0.1` merges to `main` per
> `decisions.md` row 33 and v1 is demo-ready.
>
> Round-1 inputs:
>
> - the m6 roadmap row (`milestones/README.md`, last row);
> - the m6 driver pre-flight
>   (`milestones/m6-polish-release/driver-preflight.md`,
>   2026-05-12), which captures the owner-set hard requirements,
>   the syd-pty wall investigation summary, the m4 / m5a / m5b §5
>   carryovers, and the candidate phase shape;
> - `decisions.md` rows 33 (branch model — m6 closes v0.1 merge),
>   34 (no `rfl serve` in v1), 38 (`rfl-openai` as the bundled
>   provider plugin), 48 (m5a/m5b split — m6 inherits both), and
>   46–58 (m5a/m5b additions live in core);
> - `overview.md` §8.1 (the bundled-default-provider model), §16
>   (v1 scope cut), §15.6 (bidirectional fittings peer);
> - the §5 follow-ups in
>   `milestones/m4-provider-agent-loop/retrospective.md`,
>   `milestones/m5a-sinks-confirmation/retrospective.md`,
>   `milestones/m5b-taint-exfil/retrospective.md`;
> - prior scope.md template conventions
>   (`m5a-sinks-confirmation/scope.md`,
>   `m5b-taint-exfil/scope.md`).
>
> No `decisions.md` row, stream RFC section, glossary entry, or
> overview paragraph is rewritten in this draft. m6 may **append**
> new `decisions.md` rows (placeholders below: rows 59 onward) and
> `glossary.md` entries at retrospective time; round 1 only
> *proposes* them.

---

## Goal

Close the m6 roadmap row by landing the **first-chat-from-cold
experience** plus the **release-engineering surface** required to
ship `rafaello-v0.1` to `main` as a usable v1. The roadmap row
asks for:

- Test-coverage gaps closed (per coverage report).
- Documentation pass on `rafaello/README.md` + `CONTRIBUTING.md`.
- Homebrew formula matching scope/tempo.
- `nix build .#rafaello` green on Linux + macOS.
- A manual end-to-end transcript in `manual-validation.md`
  covering `init → install rfl-openai → install one tool → chat →
  tool call with confirmation → response render → session
  persist`.
- No opportunistic new tools — every shipped tool is
  owner-ratified in this `scope.md`.

The owner-set hard requirements ratified 2026-05-12 (inlined
verbatim into the driver prompt) layer on top of the roadmap row:

1. **First chat from cold MUST just work.** A new user with the
   lab repo checked out runs a documented ≤5-line shell sequence
   inside `nix develop --impure --command …` and lands in a
   functioning interactive chat against the dev LiteLLM proxy. No
   `env CARGO_BIN_EXE_syd-pty=…` workaround. No
   `export PATH=/nix/store/…` workaround. No hand-crafted lock.
2. **The `syd-pty` discovery problem is solved at the right
   layer.** Owner spent ~30 min on 2026-05-12 against the
   m5a-RATIFIED build hitting `setup_pty` failures because
   `syd-pty` was not adjacent on `PATH`/`CARGO_BIN_EXE_syd-pty`
   when `syd` spawned the plugin subprocess.
3. **`manual-validation.md` §5 captures a real tmux-driven
   interactive `rfl chat` recording** (per the drive-agents
   skill: `tmux send-keys` a prompt that triggers a sink-declaring
   tool, capture-pane the modal, send-keys to allow, capture-pane
   the response, send-keys quit, dump SQLite). The m5a precedent
   of "mechanical coverage in lieu of recording" is **not**
   acceptable for m6 — this is the headline proof-of-life for v1.
4. **Bootstrap UX fits ≤5 shell lines.** Target shape:

       cd ~/your/project
       nix develop --impure --command rfl init
       export LITELLM_API_KEY=…
       nix develop --impure --command rfl install <tool>  # optional
       nix develop --impure --command rfl chat
5. **The syd-pty failure mode is documented** in the m6
   retrospective + the user-facing docs for posterity.

What's load-bearing for v1-demo-readiness — i.e. what is new in
m6 vs the m5b-shipped state:

- `rfl init` materialises `rafaello.lock` with the bundled
  `rfl-openai` pre-installed against the dev environment's
  LiteLLM endpoint (`decisions.md` row 38; `overview.md` §8.1).
  Today `rfl init` does **not exist**.
- A canonical syd-pty discovery fix at the devshell + lockin
  wrapper layers ("belt-and-braces" per pre-flight) so the
  bundled provider plugin spawns cleanly inside the canonical
  `nix develop` shell.
- `rfl audit` read CLI (m5b §5 row 8) so the operator can inspect
  the `audit_events` SQLite table without raw `sqlite3` queries —
  the v1 demo's "show the operator what just happened" affordance.
- A multi-turn `rfl-openai-stub` shape (m5b §5 row 1) so the §J
  scripted-turn recording covers `init → install →
  chat → tool_call → confirm → response → persist` against a
  deterministic backend, complementing the LiteLLM live recording.
- `nix build .#rafaello` package output in the repo-root
  `flake.nix` (today only `nix develop .#rafaello` exists; there
  is no `.#rafaello` *package* output), green on Linux + macOS CI.
- Homebrew formula for the macOS install path.
- README + CONTRIBUTING pass capturing the 5-line bootstrap
  snippet + the syd-pty failure-mode user-facing doc.
- The deferred regression-anchor tests from m4 / m5a / m5b §5
  + the workspace-wide `#[allow(clippy::result_large_err)]`
  sweep.
- A tmux-driven `manual-validation.md` §5 recording — the
  canonical proof of life.

m6 ships **no new security primitives** (per m5b §"m5b → m6
boundary" + §5 carryover row 2 v2-routing). The §A9 superset
narrowing on `assistant_message` / `confirm_*` / `rpc_reply`
remains a v2 candidate, not an m6 surface.

---

## Inputs

### From the plans tree

- `milestones/README.md` — m6 roadmap row (source of truth for
  m6's name + topic + demo).
- `milestones/m6-polish-release/driver-preflight.md` — 2026-05-12
  pre-flight notes (hard requirements + syd-pty investigation +
  candidate phase shape + sizing + single-vs-split call).
- `overview.md` §8.1 (bundled provider plugin), §15.1 (manifest
  shape), §15.6 (bidirectional fittings peer), §16 (v1 scope
  cut + deferrals), §4.6 (reserved env vars — affects the
  syd-pty discovery env-var choice if the lockin wrapper goes
  that route).
- `decisions.md` rows: **33** (branch model — m6 closes v0.1
  merge), **34** (no public `rfl serve` in v1), **38**
  (`rfl-openai` bundled provider), **46** (`env.allow_secrets`),
  **47** (`grant_match` template contract), **48** (m5a/m5b
  split — m6 inherits everything from both), **49**
  (`core.tools_list` RPC), **50–58** (m5b taint primitives +
  scripted-hook env var). All ratified.
- `glossary.md` — load-bearing terms; m6 may **add** entries at
  retrospective time (`rfl init`, `rfl audit`, syd-pty
  discovery fix, multi-turn stub shape are the candidates).
- `streams/a-security/rfc-security-model.md`,
  `streams/b-fittings/`, `streams/c-scripting/`,
  `streams/e-renderer/`, `streams/f-manifest/`. Per m5b
  retrospective §6.5, no live drift remains — m6's surface
  does not touch the security / fittings / scripting RFC text.
  Anything the implementation surfaces is patched at m6
  retrospective per the plans/README.md authoring convention.

### From prior milestones (live state on `rafaello-v0.1`)

- `rfl chat`, `rfl install`, `rfl status`, `rfl grant`,
  `rfl revoke`, `rfl provider use`, `rfl update` — implemented
  through m5b.
- `rfl init` — **does not exist.** First m6 deliverable
  (§Phase-A below).
- `rfl audit` — **does not exist.** m5b §5 row 8 routed it to
  m6 (§Phase-D below).
- `flake.nix` at repo root has a `nix develop .#rafaello`
  devshell but **no `nix build .#rafaello` package output**.
  m6 ships the package (§Phase-F below).
- `rafaello/README.md` (~1.3 KB) and `CONTRIBUTING.md` (~1 KB)
  are placeholders. m6 fills them (§Phase-H below).
- No Homebrew formula. m6 adds one (§Phase-G below).
- `manual-validation.md` (m5b c15 — 9-line file) exists with the
  §3 wire-shape note. m6 extends with the bootstrap walkthrough +
  the tmux-driven recording (§Phase-J below).

### From the §5 retrospectives (carryover punchlist)

m4 §5.5 + m5a §5 row 6 + m5b §5 row 13 — **workspace-wide
`#[allow(clippy::result_large_err)]` sweep** on `ReemitError` +
`AgentLoopError` + any m5b-introduced sites. Same item recorded
three times across the trail.

m5a §5 + m5b §5 routed to m6:

- **m5a-8 / m5b-(implicit)** — `load.triggers.kind = "tool"`
  lazy-load not exercised. Carry-forward from m4 §5.8 → m5a §5
  row 8 → m6.
- **m5a-9 / m5b-9** — macOS CI green hard gate (ratification
  gate from m3 onward).
- **m5a-10 / m5b-10** — interactive `rfl chat` recording for
  `manual-validation.md` §1 (LiteLLM + `send-mail` walkthrough).
- **m5a-11 / m5b-11** — `manual-validation.md` skeleton fill /
  additions (audit-log dumps + macOS CI URL).
- **m5a-12** — `rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly.rs`
  (c38 acceptance-test deviation — eager-spawn five-tree
  shutdown). Already filed under m5b §"C38" in the m5b scope; if
  the m5b implementation already landed the test (m5b commit-23
  in its internal split table), no m6 work; otherwise carried
  forward.
- **m5a-13 → m5b** (landed) — `rfl_chat_spawns_inactive_provider_but_reemit_ignores_it.rs`
  (m5b §C38b).
- **m5a-14 / m5b-12** — `core_tools_list_registered_before_provider_spawn.rs`
  defence-in-depth regression anchor. Owner-judgment whether m5b
  landed this; if not (m5b §5 row 12 routed it forward
  "unchanged"), m6 owes it.
- **m5a-15** — positive gate-through-orchestration assertion
  (`rfl_chat_tool_dispatch_goes_through_gate.rs`). m5b §C38c
  internal split scheduled it; carry-forward to m6 only if not
  landed by m5b.

m5b §5 routed to m6:

- **m5b-1** — multi-turn `rfl-openai-stub` shape (load-bearing
  for §J's scripted-turn recording).
- **m5b-8** — `rfl audit` read CLI (already counted above).

m5b §5 routed to v2 (NOT m6) — recorded so pi review doesn't
re-surface them:

- §A9 superset narrowing (m5b §5 row 2).
- Real-network `rafaello-fetch` (m5b §5 row 3).
- Substring threshold tuning + Aho-Corasick (m5b §5 rows 4 + 6).
- `TaintMatchMap` LRU cap (m5b §5 row 5).
- Laundered-flow taint / CaMeL (m5b §5 row 7).

The owner has the final call on which §5 carryovers actually
sit inside m6 vs route forward; the list above is the m6 default.

---

## In scope

Grouped by phase. The phase letters match the driver-preflight
candidate phase shape (A–J). Each phase lists what it adds:
commands, files, invariants, tests. Commit-count estimates feed
the `commits.md` budget (driver pre-flight estimated 18–28
commits total; this draft itemises into a default of 24).

### Phase A — `rfl init` (≈4 commits)

Lands the cold-start command per hard requirement #1.

**A1 — CLI scaffold.** `rfl init` subcommand in
`crates/rafaello/src/main.rs` + `crates/rafaello/src/init.rs`
module. Idempotent: re-running over an existing `rafaello.lock`
prints "lock already present at <path>" and exits 0 (no
overwrite, no prompt mid-script). `--force` flag rewrites.

**A2 — `rfl-openai` default entry materialisation.** Per
`overview.md` §8.1 + `decisions.md` row 38, the materialised
lock contains:

- `[plugins.rfl-openai]` with bindings `provider = true`,
  `provider_id = "openai"`, content digest pinned against the
  shipped binary's manifest snapshot;
- `[session]` `provider_active = "<rfl-openai-id>"`;
- the conservative dev-environment grant: `network.mode =
  "proxy"`, `allow_hosts = ["litellm.thepromisedlan.club"]`,
  `env.pass = ["LITELLM_API_KEY"]`,
  `env.allow_secrets = ["LITELLM_API_KEY"]` (per
  `decisions.md` row 46, so the scrubber honours the secret
  without `flags.i_know_what_im_doing`), no `read_dirs` /
  `write_dirs` apart from per-plugin private state.
- `rfl-openai` is **pre-installed** at `rfl init` time — the
  owner-set bootstrap shape (§Goal item 4) calls
  `rfl install <tool>` only for *tool* plugins, never for the
  bundled provider. This matches `overview.md` §8.1's
  "materialises `rafaello.lock` with a default entry for
  `rfl-openai`" wording.

The dev environment's endpoint + env-var name (LiteLLM proxy +
`LITELLM_API_KEY`) are baked into the materialised defaults per
`plans/README.md` §"Tooling notes" + `overview.md` §8.1. A
future deployment (vanilla OpenAI) overrides by editing the lock
post-init or by running `rfl install rfl-openai
--endpoint=…` (post-v1 — m6 does **not** ship a
configuration-override CLI; manual edit is acceptable for v1).

**A3 — install-time review prompt.** `rfl init` prompts the user
to confirm the default grant before writing the lock (the same
`rfl install` review flow per `overview.md` §8.1). A
`--yes` flag bypasses the prompt for scripted use. The prompt
must clearly label "this is the bundled default provider".
Declining writes a lock **without** the `rfl-openai` entry so
the user can later `rfl install` an alternate provider (the
"tool-less LLM-only configuration" overview §8.1 mentions is
v2; for v1 declining produces an empty-lock state and the user
must install a provider before `rfl chat` works).

**A4 — tests.** Integration tests in
`rafaello/tests/rfl_init_*.rs`:

- `rfl_init_writes_default_lock.rs` — happy path with `--yes`,
  asserts the lock fields above.
- `rfl_init_idempotent_no_overwrite.rs` — second run leaves the
  lock byte-identical and exits 0.
- `rfl_init_force_rewrites.rs` — `--force` rewrites under a
  different `xdg_state`-style scratch home.
- `rfl_init_decline_writes_empty_lock.rs` — declining the
  prompt writes a lock without the rfl-openai entry.

Glossary candidate: `rfl init` (new entry; cite the m6 impl).

### Phase B — syd-pty discovery fix (≈3 commits)

Per hard requirement #2 + pre-flight §"Hard requirement #1: the
syd-pty wall". Belt-and-braces approach (devshell export +
lockin wrapper resolution), default-recommended in the
pre-flight — round 1 proposes both; the alternative
single-layer positions are owner-judgment items 1 + 2 below.

**B1 — devshell export of `CARGO_BIN_EXE_syd-pty`.**
`rafaello/nix/devenv.nix` exports `CARGO_BIN_EXE_syd-pty`
mirroring how it already exports `LOCKIN_SYD_PATH`, resolving
`syd-pty` to the same nix-store directory as `syd`. The
`flake.nix` `inputs.lockin` resolution already exposes the
binary; the new line just plumbs the env var through. Covers
the interactive `rfl chat` case in the canonical devshell.

**B2 — lockin wrapper resolve-adjacent.** `lockin`'s child-
command setup (the equivalent of `Command::spawn`) resolves
`syd-pty` from `$CARGO_BIN_EXE_syd-pty`, then from `PATH`, then
from the directory containing the resolved `syd` binary, then
falls back to `sandbox/pty:off` with a one-line stderr warning.
This is the layer that catches "any future caller that didn't
enter the devshell first" — pre-flight cites a Homebrew-
installed `rfl` as the canonical future caller. The lockin
crate is rafaello's dependency; the patch lands in the lockin
workspace and is picked up by rafaello via lock bump.

Owner-judgment **item 1 below** asks whether the lockin patch
goes upstream-lockin (preferred — fix at the layer that owns
the spawn) vs a lockin patch carried inside rafaello (faster
to land, but creates a rafaello-local fork of lockin). The
pre-flight defaults to "upstream lockin"; round 1 proposes the
same.

**B3 — regression test.** A bus-fixture-driven integration test
that spawns `syd` via lockin **without** `CARGO_BIN_EXE_syd-pty`
set (overriding the env that B1 plumbs in) and asserts the
plugin still spawns thanks to B2's resolve-adjacent fallback.
Lives at
`rafaello/tests/rfl_chat_spawns_plugin_without_cargo_bin_exe_env.rs`.

If owner picks the "B1 only" or "B2 only" position at
ratification, B3 narrows accordingly.

Glossary candidate: `syd-pty discovery` (new entry; cite the
failure mode + the belt-and-braces fix).

### Phase C — `rfl audit` read CLI (≈3 commits)

m5b §5 row 8 routed this to m6 as the v1 demo's "show the
operator what just happened" affordance.

**C1 — `rfl audit` subcommand.** New `crates/rafaello/src/audit_cli.rs`
module. Reads `<project_root>/.rafaello/state/session.sqlite`'s
`audit_events` table (per `decisions.md` row 46 surrounding
context — the m5a `AuditWriter::open_for_install` path). Default
output: `SELECT seq, ts_unix_ms, kind, request_id, payload FROM
audit_events ORDER BY seq ASC` rendered as one row per line
(`<seq> <kind> <request_id> <payload-summary>`).

**C2 — filter flags.**
- `--kind <kind>` (repeatable) — filters by audit kind family.
  The kind list is the live `AuditKind::as_str()` enum (m5b
  c-row 1'' table).
- `--since <duration>` — relative time filter
  (`--since 1h`, `--since 30m`).
- `--request-id <id>` — joins the audit row with the
  `entries` table on `call_id` to reconstruct a tool-call's
  full provenance.
- `--json` — emits one JSON object per row for scripting.

**C3 — tests.** Integration tests in
`rafaello/tests/rfl_audit_*.rs`:

- `rfl_audit_lists_all_kinds.rs` — populates the audit table
  via the same `AuditWriter` fixture m5a/m5b use, runs
  `rfl audit`, asserts the table renders.
- `rfl_audit_filters_by_kind.rs` — exercises `--kind`.
- `rfl_audit_filters_by_request_id.rs` — exercises
  `--request-id` + joins.
- `rfl_audit_empty_db.rs` — fresh `rfl init` lock, no
  audit rows yet, `rfl audit` exits 0 with a "no audit
  events" banner.

Glossary candidate: `rfl audit` (new entry).

### Phase D — multi-turn `rfl-openai-stub` shape (≈2 commits)

m5b §5 row 1 routed this to m6 as load-bearing for §J's
scripted-turn recording. The m5a stub emits a single chat-
completion response per stubbed turn; m6 extends to N-turn
scripts.

**D1 — scripted-turns env-var conventions.** Extend
`crates/rafaello-openai/src/bin/rfl_openai_stub.rs` with
`RFL_OPENAI_STUB_SCRIPTED_TURNS = <path-to-toml>` (and
backward-compat with the existing singular env var). The TOML
schema:

```toml
[[turn]]
match_user_message = "what's in README.md"
emit = "tool_call"
tool_name = "read-file"
tool_args = { path = "README.md" }

[[turn]]
match_in_reply_to = "<previous tool_request id>"
emit = "assistant_message"
content = "The README says: ..."
```

The stub walks the turn list in order; first matching turn
fires, then the row is marked consumed. Exhaustion is a hard
panic (mirrors the m5b multi-answer-hook deterministic-panic
pattern, `decisions.md` row 56).

**D2 — tests.** Unit tests in
`crates/rafaello-openai/tests/rfl_openai_stub_scripted_turns.rs`
cover (a) two-turn happy path, (b) exhaustion panics, (c)
match-by-`in_reply_to` plumbs the right correlation id.

Glossary candidate: `rfl-openai-stub scripted turns` (new
entry).

### Phase E — `nix build .#rafaello` package output (≈3 commits)

Per roadmap row + hard requirement #1 (the 5-line bootstrap
references `nix develop --impure --command rfl …` — the package
output is also required because the demo's "homebrew install"
path in Phase G derives from the same Nix package; without
`.#rafaello` there's nothing for Homebrew to consume).

**E1 — `flake.nix` package output.** Add a `packages.rafaello`
attribute (alongside the existing devshell) per the standard
Nix rust package shape. Pins the rust toolchain via
`rust-toolchain.toml` (per `plans/README.md` §"Tooling notes").

**E2 — build outputs.** The package builds **every binary** in
the rafaello workspace: `rfl`, `rfl-tui`, `rfl-openai`,
`rfl-readfile`, `rfl-mailcat`, `rfl-mockprovider`,
`rfl-openai-stub`, `rfl-bus-fixture`, `rafaello-fetch`. The
output `bin/` directory matches the canonical install layout
the Homebrew formula (Phase G) consumes.

**E3 — CI matrix coverage.** GitHub Actions workflow (`.github/
workflows/rafaello.yml` or wherever the existing rafaello CI
lives — pre-flight notes the m5b `cargo test` matrix is in
place) extends with a `nix build .#rafaello` job on both
`ubuntu-latest` and `macos-latest`. The macOS leg is also the
ratification gate per m4 / m5a / m5b precedent.

Per the m2 §5.7 lesson cited in pre-flight ("push to CI early
when introducing system dependencies"), the Phase B syd-pty
fix exercises against this CI matrix during m6 implementation,
not at retrospective time.

### Phase F — Homebrew formula (≈2 commits)

Per roadmap row.

**F1 — formula scaffold.** A `homebrew/rafaello.rb` file in the
repo (the actual tap location is owner-judgment **item 3** —
in-repo Homebrew tap directory vs separate `homebrew-rafaello`
tap repo). The formula consumes the Phase-E `nix build`
output as the source, installs the binaries under
`/usr/local/bin/` (or arm64 equivalent), and depends on a
minimum nix version for the syd-pty path (or vendors the
binaries — owner-judgment **item 4**).

**F2 — install smoke test.** A manual-validation entry
(extends §Phase-J's `manual-validation.md` rather than landing
a separate file) documenting a clean macOS shell flow:
`brew install rafaello/tap/rafaello && rfl init && rfl chat`.
No CI coverage for `brew install` itself — owner-judgment
**item 5**.

### Phase G — README + CONTRIBUTING pass (≈2 commits)

Per roadmap row + hard requirements #4 + #5.

**G1 — `rafaello/README.md` rewrite.** Replace the placeholder
with:

- One-paragraph project summary.
- The 5-line bootstrap snippet (per hard requirement #4 target
  shape):

      cd ~/your/project
      nix develop --impure --command rfl init
      export LITELLM_API_KEY=…
      nix develop --impure --command rfl install <tool>     # optional
      nix develop --impure --command rfl chat

- Architecture-at-a-glance pointer to `plans/overview.md`.
- The syd-pty failure-mode user-facing doc (per hard
  requirement #5): a "Troubleshooting" section naming the
  symptom (plugin spawn fails inside `rfl chat`; `setup_pty`
  error in the audit log) and the fix (either rely on
  `nix develop --impure --command` which exports the env, or
  set `CARGO_BIN_EXE_syd-pty=$(which syd-pty)` manually). Names
  the m6 belt-and-braces fix per Phase B.
- Installation instructions covering both the Nix flake path
  and the Homebrew path (Phase F).

**G2 — `CONTRIBUTING.md` rewrite.** Replace the placeholder
with: dev-shell entry instructions, the milestone / plans /
streams structure (one-paragraph), the per-commit
`code-reviewer` agent expectation per the repo's
`~/.claude/CLAUDE.md`, and the rebase-no-force branch model
(`decisions.md` row 33).

### Phase H — Coverage / regression-anchor sweep (≈3 commits)

Carryover backlog from m4/m5a/m5b §5. Each test below cites the
retrospective row that filed it.

**H1 — `core_tools_list_registered_before_provider_spawn.rs`**
(m5a §5 row 14, m5b §5 row 12). Defence-in-depth regression
anchor for `decisions.md` row 49 (the `CorePluginService`
registers `core.tools_list` before the spawn loop by
construction). Lives at
`crates/rafaello-core/tests/core_tools_list_registered_before_provider_spawn.rs`.
Only lands in m6 if m5b's c38c sequence did not pick it up;
otherwise this row vacates (per the m5b §C38c internal-split
table — verify in round 2).

**H2 — `load.triggers.kind = "tool"` lazy-load coverage**
(m4 §5.8 → m5a §5 row 8). The manifest field is plumbed but
never exercised end-to-end. m6 lands a fixture lock that uses
the trigger + an integration test in
`rafaello/tests/lazy_load_tool_trigger_spawns_on_first_call.rs`.
Owner-judgment **item 6** asks whether this is in scope (it's
been a carryover since m4) or routed to v2.

**H3 — workspace-wide `#[allow(clippy::result_large_err)]`
sweep** (m4 §5.5 / m5a §5 row 6 / m5b §5 row 13). The honest
choice articulated in the m4 retro: either box the error
hierarchy (`Box<ReemitError>`, `Box<AgentLoopError>`), or
ratify the module-level allow with a `decisions.md` row.
Round 1 proposes the **ratify-and-allow** path because the
boxing buys nothing the live shape needs and the call sites
match exhaustively. Owner-judgment **item 7** asks for the
final call; the cost asymmetry is "1 commit (delete the
allows + add a decision row)" vs "5+ commits + ripple to every
`?`-via call site" (boxing).

### Phase I — Manual-validation transcript (≈2 commits)

Per hard requirement #3 + roadmap row demo. This is the
**canonical proof of life for v1.**

**I1 — `manual-validation.md` skeleton fill.** Extend the
existing 9-line c15 file (m5b) with the canonical layout the
m4 / m5a precedents established:

- §1 — `rfl chat` cold-start walkthrough (5-line bootstrap →
  `rfl chat`). Records the operator's tmux send-keys list, the
  capture-pane outputs, and a SQLite dump.
- §2 — `rfl install <tool>` walkthrough (one tool — `read-file`
  is the m4-introduced default; owner may swap).
- §3 — wire-shape note (preserved from m5b c15).
- §4 — macOS CI run URL (driver post-merge sweep — m5a §5 row 9
  + m5b §5 row 9).
- §5 — **the tmux-driven interactive recording** (hard
  requirement #3): the headline ≤5-line bootstrap → `rfl chat`
  → user prompt that triggers `send-mail` (sink-declaring) →
  confirm modal → allow → assistant response → quit → SQLite
  dump showing the `entries` + `audit_events` rows.
- §6 — audit-log inspection walkthrough using the new
  `rfl audit` CLI (Phase C).
- §7 — syd-pty failure-mode reproduction + the fix verification
  (covers hard requirement #5's "documented for posterity"
  half — the README docs the user-facing fix; manual-validation
  §7 docs the reproduction recipe).

**I2 — the actual tmux-driven recording.** Per the
drive-agents skill: spawn `rafaello-m6-manual-validation`
tmux session, send-keys the bootstrap, capture-pane the modal,
send-keys allow, capture-pane the response, send-keys
`q`/Ctrl-C, dump the `entries` + `audit_events` tables. The
captured transcript IS the "interactive recording" m3 / m4 /
m5a / m5b all deferred. Lives as `manual-validation.md` §5 +
linked transcript files under
`milestones/m6-polish-release/transcripts/`.

The m5a "mechanical coverage in lieu of recording" owner-
acceptance is **not acceptable for m6** (hard requirement #3
explicit). The transcript is a hard ratification gate.

### Phase J — m6 retrospective gate (≈1 commit, not counted in core 22)

Not strictly a phase of m6's scope, but called out so the
driver remembers: per `plans/README.md` Phase 2/3 the
retrospective lands new `decisions.md` rows for everything m6
ratified (cold-start command, syd-pty fix layer choice,
`result_large_err` disposition, etc.). The retrospective is
also where m6 captures the syd-pty wall narrative for
posterity (hard requirement #5 "documented in the m6
retrospective").

Pre-emptive new `decisions.md` row candidates (placeholder
numbers 59 onward; final numbers assigned at retrospective
ratification time per the append-only convention):

- row **59** — `rfl init` materialises the bundled
  `rfl-openai` lock entry against the dev-environment LiteLLM
  endpoint; declining the prompt writes an empty lock.
- row **60** — syd-pty discovery fix: belt-and-braces
  (devshell export + lockin wrapper resolve-adjacent). Cites
  the failure mode that prompted the fix.
- row **61** — `rfl audit` read CLI semantics (default
  ordering, filter flag set, JSON output).
- row **62** — `rfl-openai-stub` scripted-turns env-var
  (`RFL_OPENAI_STUB_SCRIPTED_TURNS`) + the TOML schema +
  exhaustion-panics-deterministically.
- row **63** — `nix build .#rafaello` package output as the
  canonical v1 install path; macOS CI green is the
  ratification gate (formalises the m3/m4/m5a/m5b precedent).
- row **64** — Homebrew formula tap location + dependency
  shape (owner-judgment items 3 + 4 final call).
- row **65** — `result_large_err` disposition (ratify
  module-level allow vs box). Refines the m4 retro §5.5
  punchlist.
- row **66** — m6 ratification = `rafaello-v0.1` merges to
  `main`; v1 demo-ready. Closes the v1 path opened by row 33.

---

## Out of scope

Explicit deferrals to v2 or beyond (kept short — every entry
points at the row / RFC / overview section that pins the
deferral).

1. **§A9 superset narrowing** on `provider.<id>.assistant_message`,
   `frontend.<id>.confirm_answer`, `plugin.<a>.rpc_reply`. m5b §5
   row 2 routed to v2; m5b owner-judgment item 9 ratified the
   narrowing. m6 ships **no new security primitives** per the m5b
   §"m5b → m6 boundary" framing.
2. **Real-network `rafaello-fetch`** (m5b §5 row 3). The file-
   backed handler shipped in m5b is sufficient; real HTTP is post-
   v1.
3. **Substring-threshold tuning / Aho-Corasick** (m5b §5 rows
   4 + 6). Single 16-byte threshold + linear scan is the v1
   commitment per `decisions.md` row 50.
4. **`TaintMatchMap` LRU cap** (m5b §5 row 5). Lazy TTL is the v1
   commitment per `decisions.md` row 52.
5. **Laundered-flow taint / CaMeL** (m5b §5 row 7). v1 ships the
   primitives; v2 ships the Q-LLM (`overview.md` §16).
6. **Helper plugins / external attach / patch ops / subprocess
   renderers** (`decisions.md` rows 26–29). All design-phase
   deferrals; unchanged in m6.
7. **`rfl serve` (any flavour)** (`decisions.md` row 34).
   v1 entrypoint is `rfl chat`; serve is v2.
8. **`rfl provider tool <plugin>` CLI** (m5a §5 row 7). v2 per
   overview §8.
9. **`rfl init` reconfiguration UX** — m6 ships init but no
   `rfl init --endpoint=<url>` style override. Operators on a
   non-LiteLLM deployment edit the lock manually for v1.
   `overview.md` §8.1 marks the install-time configuration
   surface as lock-edit-level for v1.
10. **Renderer-shape extensions, new built-in renderer kinds,
    new tool plugins.** Per roadmap row's explicit "No
    opportunistic new tools — every shipped tool is
    owner-ratified in this milestone's scope.md": the only
    tool plugin paths m6 touches are `read-file` (m4) +
    `send-mail` (m5a) + `rafaello-fetch` (m5b). No new tool
    plugins land in m6.
11. **Non-test workspace `#[allow(...)]` audits.** m6's clippy
    sweep targets `result_large_err` only; other suppressions
    are recorded in m4/m5a/m5b retros §5.5 and left as-is.

---

## Demo bar

Per `milestones/README.md` §"Demo bar per milestone": positive
+ negative tests, plus a `manual-validation.md` entry.

### Headline integrated demo (positive)

The 5-line bootstrap → `rfl chat` → tool call → confirm →
response → persist. Captured both as:

- **An integration test** at
  `rafaello/tests/rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`.
  Uses `rfl-openai-stub` (Phase D's multi-turn shape) + the
  `RFL_TUI_TEST_CONFIRM_ANSWERS` hook (`decisions.md` row 56)
  to drive the full flow deterministically end-to-end:
  - `rfl init --yes` writes the lock with `rfl-openai`
    pre-installed.
  - Lock-bump installs `rafaello-fetch` (one declared tool, the
    m5b fixture — owner may swap to `send-mail` to exercise
    the sink-confirmation modal; round 1 proposes
    `send-mail` because the modal-fire path is THE v1
    demo).
  - `rfl chat` spawns; the stub scripts turn 1
    (assistant proposes `send-mail`), the modal fires, the
    hook allows, the stub scripts turn 2 (assistant
    acknowledges).
  - Assertions: the `entries` SQLite table has the canonical
    `tool_call` + `tool_result` + assistant-message rows;
    the `audit_events` table has the `confirm_request` +
    `confirm_allowed` rows; the chat process exits cleanly on
    `q`.
- **The §5 tmux-driven recording** in `manual-validation.md`
  (Phase I). Same flow, run by hand against the LiteLLM proxy
  with the real `rfl-openai` plugin. Captures asciinema +
  the SQLite dumps.

### Negative / security tests

The roadmap row does not enumerate negatives because m5a/m5b
already landed every required negative. m6 inherits and runs
the existing negative test suite — it does **not** add new
security negatives.

What m6 **does** add as defensive negatives:

- `rfl_init_idempotent_no_overwrite.rs` (Phase A4) — re-running
  `rfl init` over an existing lock is a no-op.
- `rfl_init_decline_writes_empty_lock.rs` (Phase A4) — declining
  the prompt does not pre-install the bundled provider.
- `rfl_chat_spawns_plugin_without_cargo_bin_exe_env.rs`
  (Phase B3) — the lockin wrapper resolve-adjacent fallback
  works when the devshell env is not present (defends against
  Homebrew-install regressions).
- `rfl_audit_empty_db.rs` (Phase C3) — `rfl audit` against a
  fresh lock exits 0 without crashing.
- `rfl_openai_stub_scripted_turns_panics_on_exhaustion.rs`
  (Phase D2) — exhaustion fails the test loudly.

### macOS CI green

Hard ratification gate per m3/m4/m5a/m5b precedent. The macOS
leg in Phase E3 runs `cargo test --workspace
--features test-fixture` + `nix build .#rafaello` and must be
green. The only exception is tests explicitly gated
`#[cfg(target_os = "linux")]`.

---

## Single milestone vs split (owner-judgment item 0)

Pre-flight recommendation: **single milestone**, with an
m6a/m6b fold-line documented as a fallback. Round-1 scope
draft proposes the same.

Rationale for single milestone:

- The headline demo (5-line bootstrap → chat → tool call →
  confirm → response → persist) is the integrated flow. Phases
  A (init), B (syd-pty), C (audit), D (multi-turn stub), E
  (nix build), F (Homebrew), G (README), H (regression
  anchors), I (manual-validation transcript) all contribute
  pieces of the same demo. Splitting forces the headline
  recording into m6b's retrospective only, and the headline
  demo IS what v1-demo-readiness is.
- Surface inventory:
  - new CLI commands: `rfl init`, `rfl audit` (2)
  - syd-pty fix (2 layers)
  - multi-turn stub (1)
  - `nix build` package + CI matrix (2-3)
  - Homebrew formula (1-2)
  - README + CONTRIBUTING (2)
  - regression anchors + clippy sweep (3)
  - manual-validation transcript (2)

  Rough total: ~17–22 commits before owner-judgment items
  resolve; pre-flight estimated 18–28. m5b was 28; m4 was
  ~37; m3 was ~34. m6 sits at the lower end of m5-family
  sizing.

Fallback fold-line if pi pushes back at round 2+:

- **m6a — ergonomics** (Phases A, B, C, D, H, I).
  First-chat-from-cold works; audit CLI lands; transcript
  captured; regression anchors closed. Demo: cold-start
  first-chat against the dev LiteLLM endpoint produces a tool
  call + confirm + response.
- **m6b — release**: Phases E, F, G. `nix build` green on
  Linux + macOS; Homebrew works; README + CONTRIBUTING shipped.
  Demo: clean macOS shell `brew install rafaello && rfl init &&
  rfl chat`. m6b is the milestone that closes the
  `rafaello-v0.1 → main` merge (`decisions.md` row 33), not
  m6a.

The cohesion cost of the split: the headline tmux-driven
recording lands in m6a but the `brew install` shell flow only
exists at m6b time, so the "show v1" narrative splits across two
retrospectives. Single-milestone keeps both in one place.

---

## Owner-judgment items

Numbered list — each item has a default-selected position; the
owner may override at convergence-round cost.

0. **Single milestone vs m6a/m6b split.** Default:
   **single milestone**. Pre-flight + this draft both default
   here. Override: m6a / m6b per §"Single milestone vs split"
   above.

1. **syd-pty discovery fix layer.** Default:
   **belt-and-braces** (B1 devshell export + B2 lockin
   wrapper). Alternative A: B1 only (cheaper; doesn't cover
   Homebrew-installed `rfl`). Alternative B: B2 only (richer;
   defers devshell visibility — environment-variable leaks to
   non-rafaello tools). Pre-flight defaults to belt-and-
   braces; this scope round 1 inherits.

2. **B2 lockin wrapper landing layer.** Default: **upstream
   lockin** (clean fix at the layer that owns the spawn).
   Alternative: rafaello-local lockin patch (faster to land,
   creates a local fork of lockin to maintain). Round 1
   defaults to upstream.

3. **Homebrew tap location.** Default: **separate
   `homebrew-rafaello` tap repo** under `luizribeiro/` (clean
   namespace; matches Homebrew convention). Alternative:
   in-repo `homebrew/rafaello.rb` (simpler, no second repo).
   Round 1 defaults to separate tap.

4. **Homebrew dependency shape.** Default: **`brew install
   nix` + `nix build .#rafaello` under the hood** (clean
   build provenance, matches the canonical install path).
   Alternative: vendor pre-built binaries per arch
   (faster install, no nix-on-macos requirement, but the tap
   has to track per-arch artefacts).

5. **Homebrew install CI coverage.** Default: **manual
   validation only** (Phase F2 / §Manual-validation §F bullet).
   Alternative: a macOS-CI workflow that `brew install`s the
   formula in a clean container. The wall-clock cost is
   significant; the failure mode is rare in practice.
   Round 1 defaults to manual.

6. **`load.triggers.kind = "tool"` lazy-load coverage**
   (Phase H2). Default: **in scope** (m6 closes the carryover).
   Alternative: route to v2 (the feature is plumbed but unused;
   nothing in v1 needs it). Round 1 defaults to in-scope —
   it's a carryover since m4 and the test is small.

7. **`result_large_err` disposition** (Phase H3). Default:
   **ratify the module-level allow + add a `decisions.md`
   row**. Alternative: box `ReemitError` + `AgentLoopError`
   (5+ commits, ripples through every `?` site, buys no live
   ergonomics).

8. **`rfl init` empty-lock behaviour on prompt decline.**
   Default: **write an empty lock** so the user can later
   `rfl install` an alternate provider. Alternative: write
   a "tool-less LLM-only" config — but `overview.md` §8.1
   marks tool-less as v2. Round 1 defaults to empty-lock.

9. **`rfl audit` default output format.** Default: **one row
   per line, human-readable**. Alternative: JSON-by-default
   (scriptable but worse default UX). Round 1 defaults to
   human-readable; `--json` is the flag.

10. **`manual-validation.md` §5 transcript backend.**
    Default: **the real LiteLLM proxy** (matches the
    operator's real workflow). Alternative: `rfl-openai-stub`
    (deterministic, faster, but doesn't exercise the
    network path). Round 1 defaults to LiteLLM for the §5
    recording; the integration test in
    `rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`
    uses the stub for determinism.

11. **`rfl init` `--force` semantics.** Default: **`--force`
    rewrites the lock byte-for-byte from defaults**, dropping
    any user edits. Alternative: `--force` merges new
    defaults into the existing lock preserving user edits
    (much harder to get right; merge semantics on TOML are
    not standard). Round 1 defaults to rewrite.

12. **Phase H1 (`core_tools_list_registered_before_provider_spawn.rs`)
    inclusion.** Default: **only if m5b's c38c sequence did
    not land it** (verify in round 2 — read the m5b commits
    list). Alternative: land regardless (defends against
    drift). Round 1 defaults to conditional.

---

## Coverage / regression-anchor list

Verbatim from the §5 retrospectives, with the m6 disposition
inline. Pi review should treat this as the authoritative
checklist for "what m4/m5a/m5b owe to m6".

| Source row | Item | File path | m6 phase | Disposition |
|---|---|---|---|---|
| m4 §5.5 / m5a §5 row 6 / m5b §5 row 13 | `#[allow(clippy::result_large_err)]` sweep | `crates/rafaello-core/src/{reemit,agent}/mod.rs:1` + any m5b sites | H3 | in scope (default: ratify) |
| m4 §5.8 / m5a §5 row 8 | `load.triggers.kind = "tool"` lazy-load exercise | `rafaello/tests/lazy_load_tool_trigger_spawns_on_first_call.rs` (new) | H2 | in scope (default; owner-judgment item 6) |
| m5a §5 row 9 / m5b §5 row 9 | macOS CI green hard gate | `manual-validation.md` §4 (URL) | E3 + I1 §4 | in scope |
| m5a §5 row 10 / m5b §5 row 10 | Interactive `rfl chat` recording | `manual-validation.md` §5 + `transcripts/` | I2 | in scope (hard requirement #3) |
| m5a §5 row 11 / m5b §5 row 11 | `manual-validation.md` skeleton fill | `manual-validation.md` §1–§7 | I1 | in scope |
| m5a §5 row 14 / m5b §5 row 12 | `core_tools_list_registered_before_provider_spawn.rs` | `crates/rafaello-core/tests/core_tools_list_registered_before_provider_spawn.rs` | H1 | conditional (owner-judgment item 12) |
| m5b §5 row 1 | Multi-turn `rfl-openai-stub` shape | `crates/rafaello-openai/src/bin/rfl_openai_stub.rs` | D1 + D2 | in scope |
| m5b §5 row 8 | `rfl audit` read CLI | `crates/rafaello/src/audit_cli.rs` (new) | C1–C3 | in scope |

Items m5a §5 / m5b §5 routed to v2 and **NOT** in scope for
m6: m5a-7 (`rfl provider tool`), m5b-2 (§A9 narrowing), m5b-3
(real-network fetch), m5b-4 (substring threshold tuning), m5b-5
(LRU cap), m5b-6 (aho-corasick), m5b-7 (laundered-flow). All
recorded in §"Out of scope" above.

Items m5a/m5b already covered ("verification-only follow-ups"):
m5a-5 (audit-DB path verification — Phase I1 §6 verifies the
path).

c38 acceptance-test deviation items (m5a §5 rows 12 + 13 + 15)
should already have landed during m5b's c38a / c38b / c38c
internal-split rows. Round 2 verifies in the live m5b commits
list; if any did not land, they fold into Phase H. Default:
**not in m6 scope** (assume m5b landed them).

---

## Glossary additions

Proposed at retrospective time (no live `glossary.md` edits at
scope-drafting time per the plans/README.md authoring
convention):

- **`rfl init`** — materialises `rafaello.lock` with the
  bundled `rfl-openai` provider pre-installed against the
  dev-environment LiteLLM endpoint. Idempotent; `--force`
  rewrites; declining the install-time review writes an empty
  lock. Cites `crates/rafaello/src/init.rs` + `decisions.md`
  row 59.
- **`rfl audit`** — read CLI over `audit_events` SQLite table.
  Default human-readable; `--json` for scripting. Filters:
  `--kind`, `--since`, `--request-id`. Cites
  `crates/rafaello/src/audit_cli.rs` + `decisions.md` row 61.
- **`syd-pty discovery`** — the spawning-side problem where
  `syd` invokes `setup_pty` and fails because `syd-pty` is
  not on `PATH` and `CARGO_BIN_EXE_syd-pty` is not set.
  v1 fix is belt-and-braces: devshell export +
  lockin-wrapper resolve-adjacent. Cites
  `rafaello/nix/devenv.nix` + the lockin patch + the m6
  retrospective narrative.
- **`rfl-openai-stub scripted turns`** — N-turn TOML script
  consumed by the stub via `RFL_OPENAI_STUB_SCRIPTED_TURNS`.
  Deterministic-panic on exhaustion; mutually compatible with
  the single-answer hook from m5b
  (`RFL_TUI_TEST_CONFIRM_ANSWERS`, `decisions.md` row 56).
  Cites `crates/rafaello-openai/src/bin/rfl_openai_stub.rs` +
  `decisions.md` row 62.

The m6 retrospective is also expected to extend the existing
`rafaello.lock` glossary entry with a banner-pointer at the
`rfl init`-materialised default shape, and the existing
`Bundled provider` entry with a one-line `rfl init`
cross-reference.

---

## Internal split (driver guidance for `commits.md`)

Round-1 sizing target: **22 commits** (default-selected owner
positions on every judgment item) with **24–28 max** (if
H1 lands per item 12 + H2 routes to v2 per item 6 + every
phase shows scope creep during commits.md drafting). The
pi-round budget is 4–6 (m5b was 7 with a wider security
surface; m6 is mostly ergonomics + release engineering,
expect to land closer to 4).

Suggested grouping (`commits.md` picks final granularity):

| # | Section | Subject sketch | ~commits |
|---|---------|----------------|----------|
| 1 | A1 | `rfl init` CLI scaffold + idempotency invariant | 1 |
| 2 | A2 | `rfl-openai` default-entry materialisation under `--yes` | 1 |
| 3 | A3 | install-time review prompt (TTY + `--yes` paths) | 1 |
| 4 | A4 | `rfl init` integration tests (4 tests) | 1 |
| 5 | B1 | devshell `CARGO_BIN_EXE_syd-pty` export | 1 |
| 6 | B2 | lockin wrapper resolve-adjacent fallback (upstream lockin) | 1 |
| 7 | B3 | `rfl_chat_spawns_plugin_without_cargo_bin_exe_env.rs` | 1 |
| 8 | C1 | `rfl audit` CLI scaffold + default output | 1 |
| 9 | C2 | filter flags (`--kind`, `--since`, `--request-id`, `--json`) | 1 |
| 10 | C3 | `rfl_audit_*` integration tests (4 tests) | 1 |
| 11 | D1 | `RFL_OPENAI_STUB_SCRIPTED_TURNS` parser + dispatcher | 1 |
| 12 | D2 | stub scripted-turns unit tests | 1 |
| 13 | E1 | `flake.nix` `packages.rafaello` output | 1 |
| 14 | E2 | every-bin coverage in the package output | 1 |
| 15 | E3 | CI matrix `nix build` on Linux + macOS | 1 |
| 16 | F1 | Homebrew formula scaffold (separate tap) | 1 |
| 17 | F2 | manual-validation Homebrew smoke entry | folded into I |
| 18 | G1 | `rafaello/README.md` rewrite (5-line bootstrap + troubleshooting) | 1 |
| 19 | G2 | `CONTRIBUTING.md` rewrite | 1 |
| 20 | H1 | `core_tools_list_registered_before_provider_spawn.rs` *(conditional)* | 0–1 |
| 21 | H2 | `load.triggers.kind = "tool"` lazy-load fixture + test | 1 |
| 22 | H3 | `result_large_err` allow + `decisions.md` row 65 placeholder | 1 |
| 23 | I1 | `manual-validation.md` §1–§7 skeleton + audit dump shape | 1 |
| 24 | I2 | tmux-driven §5 recording (transcripts under `transcripts/`) | 1 |

Realistic total: **22 commits** at default ownership (H1
vacates, F2 folds into I). **24 max** if H1 lands and F2
splits out. **28 max** if pi pushes back and any phase grows
(syd-pty fix landing layer ratifies to both layers + a CI
trio in §3, init prompt UX gets split into TTY vs `--yes`
across two commits, etc.).

**Forced-monolithic commits called out:**

- Row 13 (`flake.nix` package output) lands as one commit
  because the Nix package + the lock-file pin + the CI
  workflow update are coupled at the flake-evaluation layer.
- Row 18 (README) and row 19 (CONTRIBUTING) are kept as
  separate commits (one logical idea each per the repo's
  commit guidelines) even though both are docs.
- Row 24 (the §5 recording) lands the transcript artefacts +
  the `manual-validation.md` §5 references in one commit so
  the cross-reference doesn't dangle.

**Test ladder dependencies:**

- Row 4 (Phase A4 tests) extends in row 23 (`manual-validation.md`
  §1 walkthrough) — A4 covers happy path in isolation; §1
  extends with the operator's tmux send-keys narrative.
- Row 15 (CI matrix nix build) gates row 23 §4 (macOS CI URL
  capture). The driver post-merge sweep records the URL once
  the workflow is green.
- Row 11 (stub multi-turn) is consumed by the integration test
  in row 24 (`rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`).

---

## Acceptance summary

m6 is done when:

- Every named deliverable in §"In scope" is implemented and its
  tests pass. Tests may split or merge during `commits.md`
  drafting as long as the named behaviours are covered (m5a/m5b
  precedent).
- `nix develop --impure --command cargo test --manifest-path
  rafaello/Cargo.toml --workspace --features test-fixture`
  green on Linux.
- **macOS CI green is a hard ratification gate** (m3 / m4 / m5a
  / m5b precedent). Both `cargo test --workspace` and
  `nix build .#rafaello` on `macos-latest` must be green
  before retrospective ratification, with the only exception
  being tests gated `#[cfg(target_os = "linux")]`.
- `nix develop --impure --command cargo build --manifest-path
  rafaello/Cargo.toml --workspace --bins` green.
- `nix build .#rafaello` produces a runnable binary on both
  Linux + macOS.
- `brew install <tap>/rafaello` works in a clean macOS shell
  (manual validation per Phase F2).
- The 5-line bootstrap (`cd …; nix develop --impure --command
  rfl init; export LITELLM_API_KEY=…; nix develop … rfl install
  <tool>; nix develop … rfl chat`) lands in `rafaello/README.md`
  verbatim and works against the dev LiteLLM endpoint.
- `manual-validation.md` records the seven sections in §"Phase
  I" with operator-witnessed evidence, including the §5
  tmux-driven recording (hard requirement #3).
- `retrospective.md` written with the syd-pty narrative captured
  for posterity (hard requirement #5) + the
  `decisions.md` row appends sketched in Phase J + the m6
  glossary additions sketched in §"Glossary additions" + the
  v0.1 → main merge plan executed.
- All §"Owner-judgment items" resolved at convergence.

The m6 roadmap row closes when m6 ratifies; `rafaello-v0.1`
merges to `main` immediately after per `decisions.md` row 33
and v1 is demo-ready.

---

## References

- Roadmap row — `milestones/README.md` (last row).
- Driver pre-flight —
  `milestones/m6-polish-release/driver-preflight.md`.
- Branch model — `decisions.md` row 33.
- No `rfl serve` in v1 — `decisions.md` row 34.
- Bundled `rfl-openai` provider — `decisions.md` row 38,
  `overview.md` §8.1.
- `env.allow_secrets` — `decisions.md` row 46.
- `grant_match` template contract — `decisions.md` row 47.
- m5a/m5b split — `decisions.md` row 48.
- `core.tools_list` RPC — `decisions.md` row 49.
- Taint primitives (matching + ancestry + intake check + audit
  predicate) — `decisions.md` rows 50–58.
- v1 scope cut — `overview.md` §16.
- Bidirectional fittings peer — `overview.md` §15.6.
- m4 §5 carryovers —
  `milestones/m4-provider-agent-loop/retrospective.md` §5.5,
  §5.8.
- m5a §5 carryovers —
  `milestones/m5a-sinks-confirmation/retrospective.md` §5 rows
  6 / 8 / 9 / 10 / 11 / 12 / 13 / 14 / 15.
- m5b §5 carryovers —
  `milestones/m5b-taint-exfil/retrospective.md` §5 rows 1 / 8 /
  9 / 10 / 11 / 12 / 13.
- Recurring operational gotchas (relevant to m6's CI work) —
  `plans/README.md` §"Recurring operational gotchas".
- m2 §5.7 push-to-CI-early lesson (relevant to Phase B) —
  `milestones/m2-broker-spawn/retrospective.md` §5.7.

---

## Changelog

- Round 1 → claude-authored 2026-05-12 against the m6 driver
  pre-flight + m5b RATIFIED baseline. Awaiting pi review.

---

*End of m6 scope round 1 draft. Claude-authored; awaiting pi
adversarial review per `plans/README.md` Phase 2.*
