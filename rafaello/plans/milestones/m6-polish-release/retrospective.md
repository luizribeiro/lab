# m6 — v1 polish + release readiness — retrospective

> **Status: round 1 draft — claude-authored 2026-05-12,
> awaiting pi round 1.** Drafted on `agents/m6/retro`
> forked from `agents/m6/driver` at `4e22268` (c28 — the
> last implementation commit). References:
>
> - RATIFIED scope: `a0764b3` (`scope.md` round 5 RATIFIED
>   — owner sign-off on 14 owner-judgment defaults
>   numbered 0–13).
> - RATIFIED commits plan: `79e3a1c` (`commits.md` round 8
>   RATIFIED — owner sign-off after the round-4 lazy-load
>   parser-only pivot was withdrawn under pi-4 §B-1 and
>   round-5/6 redesigned the supervisor API as a coherent
>   cluster-fix).
> - Implementation commits: c01 (`00f87b9`) .. c28
>   (`4e22268`), 28 contiguous commits, 1:1 with
>   `commits.md` rows c01..c28 (the c29 retro reservation
>   maps to this retrospective + the drift commits planned
>   in §7 below).
>
> Per the m5b precedent (4 rounds, m1's 4 rounds, m5a's 6
> rounds), retro convergence is budgeted at **3–4 rounds**
> — m1's "surface-size-predicts-rounds" lesson is the
> anchor; m6's surface is comparable to m5b's 28-row plan
> and the lazy-load runtime added a fresh API-redesign
> sub-surface in commits rounds 4–6 that mirrors m5b's
> §TR4b/§TM4 hot zones.

---

## 1. Headline outcome

**m6 closes v1.** `rafaello-v0.1` is now demo-ready and
ready to merge to `main` per `decisions.md` row 33 (the
v0.1 → main merge trigger is m6 retrospective
ratification).

**Headline facts.**

- **28 implementation commits** (c01 `00f87b9` .. c28
  `4e22268`), 1:1 with the `commits.md` round-8 RATIFIED
  table. No bundling; no docs-only commits inserted
  between plan-row commits.
- **`scope.md`: 5 rounds** to RATIFIED (`a0764b3`).
  Blocker trajectory: **5B → 3B → 2B → 0B → 0B →
  CONVERGED**. m5b was 7; m5a was 6; m4 was 6. m6's
  shorter scope bracket reflects the
  no-new-security-primitives framing (per m5b §"m5b → m6
  boundary") — the load-bearing invariants left for v1
  were UX shape, not security.
- **`commits.md`: 8 rounds** to RATIFIED (`79e3a1c`).
  Blocker trajectory: **4B → 3B → 4B → 2B → 5B → 0B → 0B
  → 0B (1N NON-BLOCKING) → CONVERGED**. The long count is
  driven entirely by the **lazy-load runtime surface**
  (rounds 4–6): pi-3 surfaced that the parser-only
  Phase-I2 wasn't load-bearing; round 4 attempted a
  parser-only pivot which pi-4 §B-1 rejected as a
  fixture-only crutch; rounds 5–6 redesigned the
  supervisor API (`register_lazy` / `ensure_spawned` /
  `lazy_candidates` / `tool_to_canonical`) as a coherent
  cluster, resolving five mechanical blockers in one
  patch instead of five. m5b's precedent of 6+ rounds for
  a hot-zone surface (taint + audit primitives) holds —
  m6's hot zone was the lazy-load runtime, not security.

**Hard requirement disposition** (owner-set 2026-05-12,
verbatim from driver prompt):

1. **First chat from cold MUST just work.** ✓ — landed
   via Phases A + B + C + H (5-line bootstrap in
   `rafaello/README.md`; `rfl init --yes` materialises
   the default lock + the bundled `rfl-openai` package
   tree per PP1; `rfl install rfl-mailcat` lands the
   tool-side; `rfl chat` inside the devshell discovers
   `syd-pty` via either the devshell export *or* the
   lockin sibling-lookup arm). Verified end-to-end by
   `rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`
   (deterministic, stub-driven) and by the §5 tmux
   transcript (real `rfl-openai` against the stub).
2. **The `syd-pty` discovery problem is solved at the
   right layer.** ✓ — Phase C landed the belt-and-braces
   fix: devshell exports `CARGO_BIN_EXE_syd-pty`
   (c08 `17f683f`), lockin sandbox's
   `SandboxBuilder::syd_pty_path` resolves via
   spec/env/sibling/PATH with **no `pty:off` fallback**
   at that layer (c09 `333e9d8`), and the env is set on
   the syd child command via `Command::env`. Tests
   (c10 `5467938`) exercise the resolution arms via a
   fake-`syd` `[[bin]]` gated by the net-new
   `lockin/crates/sandbox` `test-fixture` feature plus a
   rafaello-side devshell smoke.
3. **`manual-validation.md` §5 captures a real
   tmux-driven interactive `rfl chat` recording.** ✓ —
   c28 `4e22268` lands six transcript files under
   `milestones/m6-polish-release/transcripts/section-5/`
   (`01-after-launch.txt`, `02-modal.txt`,
   `03-response.txt`, `04-audit.txt`, `05-sqlite-audit.txt`,
   `06-sqlite-entries.txt`) wired into §5 with the
   send-mail flow against the stub. See §4 for the
   LiteLLM-driven second-transcript carveout.
4. **Bootstrap UX fits ≤5 shell lines.** ✓ — c21
   `85ce87b` lands the verbatim 5-line snippet in
   `rafaello/README.md` (one `cd`, one `nix develop …
   rfl init`, one `export`, one `nix develop … rfl
   install rfl-mailcat`, one `nix develop … rfl chat`).
5. **The syd-pty failure mode is documented for
   posterity.** ✓ — README has both a top-level
   troubleshooting paragraph and a separate
   "Pre-m6 workaround" subsection that documents the
   manual `CARGO_BIN_EXE_syd-pty=$(which syd-pty)`
   recipe under a "use only against pre-m6 builds"
   banner (c21 `85ce87b`). `manual-validation.md` §7
   reproduces the failure mode and the fix
   (c27 `9e1a563`).

---

## 2. What shipped (per phase)

Per the `commits.md` RATIFIED table; commit shas pin the
landing point. Phase letters match `scope.md` round-5
RATIFIED.

### Phase A — `rfl init` (c01–c04)

- **c01 `00f87b9`** — `rfl init` CLI scaffold +
  idempotency invariant. Extends `RflChatCommand` with
  `Init(InitArgs)`; new `crates/rafaello/src/init.rs`
  module. Re-runs over an existing lock exit 0 with a
  notice; `--force` rewrites byte-for-byte (owner-judgment
  item 7 default honoured); `--project-root` matches
  `rfl chat`.
- **c02 `c95f151`** — `rfl init` materialises the default
  lock against the live `Lock` schema with `rfl-openai`
  pre-installed; **PP1 bundled-plugin copy step**
  (round-3 B-1 / round-4 B-1 — copies
  `<release-prefix>/share/rafaello/plugins/rfl-openai/`
  into `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`
  with `bin/rfl-openai` as a real file and digests
  computed over the copied tree). `compile::resolve_entry`
  promoted to `pub` so init can assert containment.
- **c03 `eb1c650`** — install-time review prompt +
  decline-empty-lock path (owner-judgment item 8
  default).
- **c04 `6a0971e`** — Phase-A integration tests
  (idempotency, force, decline, package-dir
  materialisation, `compile::resolve_entry` containment).

### Phase B — `rfl install <plugin>` UX (c05–c07)

- **c05 `ded2276`** — positional plugin argument +
  bundled-source resolver. `InstallArgs.fixture` changed
  to `Option<PathBuf>` with clap `conflicts_with` /
  `required_unless_present` across `fixture` ↔ `plugin`
  (forced-monolithic per scope §"Internal split").
  `--project-root` added.
- **c06 `df31948`** — bundled plugin manifest trees
  promoted out of the test fixture into the workspace
  under `rafaello/crates/<plugin>/rafaello.toml` with
  sidecar `openrpc.json`. Five crates touched:
  `rafaello-mailcat`, `rafaello-readfile`,
  `rafaello-openai`, `rafaello-mockprovider`,
  `rafaello-fetch`.
- **c07 `56a7c61`** — integration suite for `rfl
  install`: positional-resolves-to-bundled,
  fixture-flag-still-works,
  positional-unknown-plugin-errors, requires-one-of,
  project-root-flag, resolves-entry-against-canonicalised,
  plus the init→install→in-tree-bundled-openai smoke.

### Phase C — syd-pty discovery (c08–c10)

- **c08 `17f683f`** — devshell export of
  `CARGO_BIN_EXE_syd-pty` in `rafaello/nix/devenv.nix`,
  mirroring the existing `LOCKIN_SYD_PATH` export.
- **c09 `333e9d8`** — `SandboxBuilder::syd_pty_path` +
  child-env injection in `lockin/crates/sandbox/src/lib.rs`.
  Resolution order: `spec.syd_pty_path` →
  `$CARGO_BIN_EXE_syd-pty` → sibling-of-syd → `PATH` →
  hard `Err(SandboxError::SydPtyNotFound)` (owner-judgment
  item 4 default: no `pty:off` fallback).
- **c10 `5467938`** — fake-syd `[[bin]]` (gated by the
  net-new `lockin/crates/sandbox/Cargo.toml`
  `test-fixture` feature, round-5 M-1) records argv +
  environ to a sentinel file; three lockin tests cover
  the explicit / sibling / hard-error arms, plus
  rafaello-side devshell smoke
  `rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty.rs`.

### Phase D — `rfl audit` read CLI (c11–c13)

- **c11 `96ddae1`** — `rfl audit` CLI scaffold against
  the **live** `audit_events` schema (`seq, at, kind,
  request_id, payload` — no `ts_unix_ms`, no `entries`
  join; pi round-2 B-4 fix carried through landing). New
  `crates/rafaello/src/audit_cli.rs`. `--project-root`
  mirrors `rfl init` / `rfl chat`.
- **c12 `9e10ef6`** — filter flags `--kind` (repeatable),
  `--since` (`1h`/`30m`/`24h`), `--request-id`
  (no-join, single-table filter), `--json`, `--full`.
- **c13 `dc7843a`** — seven-test integration suite +
  glossary update for `rfl audit`. Explicitly asserts
  `--request-id` does not touch `entries`.

### Phase E — multi-turn `rfl-openai-stub` (c14–c15)

- **c14 `7fc4fe2`** — `RFL_OPENAI_STUB_SCRIPTED_TURNS`
  HTTP-response selector; drops the `test-fixture` gate
  from the `[[bin]]` so the stub is buildable in the
  release tree (owner-judgment item 13 default —
  include).
- **c15 `9cdddf5`** — scripted-turns HTTP integration
  tests: happy two-turn, exhaustion-panic,
  `match_in_reply_to` correlation, mutual-exclusion with
  the singular env.

### Phase F — `nix build .#rafaello` repair (c16–c18)

- **c16 `ea0fd86`** — `cargoBuildFlags` expansion in
  `rafaello/nix/package.nix` from `[ "-p" "rafaello" ]`
  to the 8-package build set (`rafaello`, `rafaello-tui`,
  `rafaello-openai`, `rafaello-openai-stub`,
  `rafaello-readfile`, `rafaello-mailcat`,
  `rafaello-mockprovider`, `rafaello-fetch`); excludes
  `rafaello-bus-fixture` per owner-judgment item 9.
- **c17 `b51c8c2`** — `postInstall` reshape to PP1
  plugin-tree layout: each bundled plugin's binary moves
  from `$out/bin/<bin>` into
  `$out/share/rafaello/plugins/<plugin>/bin/<bin>` as a
  real file, plus the manifest + `openrpc.json` + any
  `schemas/`. Top-level `$out/bin/` retains only
  `rfl` + `rfl-tui` (round-4 B-1).
- **c18 `dfe0ee9`** — macOS + Linux CI matrix for
  `nix build .#rafaello` + workspace tests with
  `--features test-fixture` + F2 layout shell-step
  assertion.

### Phase G — Homebrew distribution (G.β default; c19–c20)

- **c19 `cbee0ea`** — `homebrew/rafaello.rb` formula +
  tap-pointer fixture (in-repo committed copy of the
  formula; the live tap repo is a one-time owner action
  documented in `manual-validation.md` §G).
- **c20 `c576844`** — release-tag automation in
  `.github/workflows/rafaello-release.yml`: per-arch
  `nix build .#rafaello` + tarball upload + formula
  SHA-pin. Architectures
  `aarch64-darwin / aarch64-linux / x86_64-linux` per
  the round-3 M-3 narrowing; `x86_64-darwin` deferred to
  v2 (§5).

### Phase H — README + CONTRIBUTING (c21–c22)

- **c21 `85ce87b`** — `rafaello/README.md` rewrite: the
  5-line bootstrap verbatim, Troubleshooting section, and
  the "Pre-m6 workaround" subsection that documents the
  manual `CARGO_BIN_EXE_syd-pty` recipe with a clear
  banner (hard requirement #5).
- **c22 `ee1c3e1`** — `CONTRIBUTING.md` rewrite:
  devshell entry, plans/streams/milestones structure,
  per-commit code-reviewer expectation,
  rebase-no-force branch model
  (`decisions.md` row 33).

### Phase I — coverage + lazy-load (c23–c26)

- **c23 `a9c07e7`** — `StartupEvent::ToolSchemaCatalogBuilt`
  instrumentation + regression-anchor test. This
  replaces I1's original
  `core_tools_list_registered_before_provider_spawn.rs`
  shape (owner-judgment item 11) — the structural
  guarantee is now asserted via the startup-event ordering
  rather than via the synthetic
  `core.tools_list`-before-spawn anchor; both are
  defence-in-depth for `decisions.md` row 49 and the
  observable event ordering is the cleaner anchor.
- **c24 `f6cc78f`** — **the lazy-load runtime**.
  `LoadPolicy::Lazy { command }` runtime end-to-end:
  supervisor `lazy_candidates` + `tool_to_canonical`
  state, `register_lazy(canonical, plan, paths,
  triggers)` registration API, `ensure_spawned(canonical)`
  spawn-and-await primitive, gate dispatch-after-validation
  hook routing tool dispatches through `ensure_spawned`
  before forwarding, `run_chat` startup routing, and
  `shutdown(&self)` (round-6 redesign of the
  `shutdown(self)` consume signature flagged by pi-5 B-2).
  Pi-3 round's parser-only Phase-I2 was found to be a
  fixture-only crutch; pi-4 §B-1 rejected the round-4
  parser-only pivot; rounds 5–6 redesigned the supervisor
  API as the coherent cluster-fix landed here.
- **c25 `5223d61`** — lazy-load integration test
  `lazy_load_tool_trigger_spawns_on_first_call.rs` via
  the net-new `RFL_SPAWN_TRACE_LOG` file-log
  observability seam. Pi-6 B-5 fold: the trace shape is
  asserted by reading the spawn-event log file, not by
  capturing stdout — the supervisor records spawn events
  via `record_spawn_event` to the path the env names, and
  the test grep-asserts the canonical id + trigger fired
  on first call.
- **c26 `f785165`** — `result_large_err` ratification:
  keep the module-level `#[allow(clippy::result_large_err)]`
  allows in place + comment-pin each to
  `decisions.md` row 67 (owner-judgment item 8 default —
  ratify means keep, not delete).

### Phase J — manual validation (c27–c28)

- **c27 `9e1a563`** — `manual-validation.md` §1–§7 + §G
  skeleton + audit-dump shape. §3 preserves the m5b c15
  wire-shape note; §4 records the macOS CI URL; §7
  reproduces the syd-pty failure-mode + the fix
  verification (hard requirement #5 manual-validation
  half).
- **c28 `4e22268`** — tmux-driven §5 recording: six
  captured transcript files under
  `transcripts/section-5/`, with greps against the live
  TUI overlay copy
  (`rafaello-tui/src/confirm.rs:160-211`) and the live
  gate-built summary
  (`crates/rafaello-core/src/gate/mod.rs:374-378`).

---

## 3. Adversarial review during drafting

### `scope.md` — 5 rounds

| Round | Blockers / Majors / Nits | Notable carries |
|---|---|---|
| 1 → 2 | 7 B / 7 M / 4 N | B-1 reframe Phase F (output already exists; gap is `-p rafaello`-only); B-2 add Phase B install UX; B-3 concrete tmux steps; B-4 drop `entries.call_id` join; B-6 no-`pty:off` fallback at lockin; B-7 G.β/G.α/G.γ alternatives surfaced |
| 2 → 3 | 4 B / 5 M / 3 N | B-1 PP1 invariant introduced (copy step + canonical name `rafaello.toml`); B-2 `--project-root` propagated to D1; M-1 G.β actionable default; M-3 architectures narrowed to three (no `x86_64-darwin`); M-4 fake-syd `[[bin]]` over real-syd-shim |
| 3 → 4 | 2 B / 3 M / 2 N | B-1 PP1 containment invariant — `bin/<plugin-bin>` must be a real file, not a symlink, because `compile::resolve_entry` canonicalises and rejects `EntryEscape`; B-2 J2 tmux `--project-root` plumbing across all `rfl <sub>` invocations |
| 4 → 5 | 0 B / 2 M / 1 N | M-1 fake-syd test-fixture feature net-new in `lockin/crates/sandbox/Cargo.toml`; N-2 README workaround banner framing |
| 5 → RATIFIED | 0 B / 0 M / 0 N | `a0764b3` |

The two-round B-tail (rounds 1–3) was driven by the
**PP1 invariant** — pi-2 B-1 surfaced the missing copy
step, pi-3 B-1 surfaced the symlink-containment
violation; the two folds together stitched
Phase A + Phase B + Phase F into a coherent
package-placement story (`rfl init` and `rfl install`
materialise into `${PROJECT_ROOT}/.rafaello/plugins/`;
Phase F lays the source tree at
`<release-prefix>/share/rafaello/plugins/`; `bin/` real
files in both). Pi B-7's Homebrew-model surfacing
mid-round-1 was the other load-bearing surface — round 3
made G.β actionable rather than blocking on owner reply.

### `commits.md` — 8 rounds

| Round | B/M/N | Notable carries |
|---|---|---|
| 1 → 2 | 4 B / 5 M / 3 N | B-1 row-by-row trace to scope phases; B-2 sizing per row; B-3 forced-monolithic rationale; M-1 reading-order section |
| 2 → 3 | 3 B / 4 M / 2 N | B-1 PP1 copy step per row; B-3 cross-checks; M-2 acceptance traceability appendix; M-5 `InstallArgs.fixture: Option` shape |
| 3 → 4 | 4 B / 3 M / 2 N | B-1 **parser-only Phase-I2 is a fixture-only crutch** — pi-3 surfaced that the manifest field is plumbed but the runtime isn't load-bearing; needs full runtime |
| 4 → 5 | 2 B / 2 M / 1 N | **B-1 round-4 parser-only pivot withdrawn** under pi-4's "fixture-only crutch is worse than no test" framing; round 5 reframed as full lazy-load runtime |
| 5 → 6 | 5 B / 2 M / 2 N | B-1..B-5 + M-1 — supervisor API surface (LazyCandidate visibility, `Arc<PluginSupervisor>` vs `shutdown(self)`, eager-tool dispatch regression, idempotent-redispatch state, trace-emission shape). Round 6 **redesigns the API as a coherent unit** — one cluster-fix, not five patches |
| 6 → 7 | 2 B / 1 M / 1 N | B-5 trace via file-log (`RFL_SPAWN_TRACE_LOG`) rather than stdout; two-stage c24a→c24b ladder for the supervisor / test pair |
| 7 → 8 | 0 B / 0 M / 1 N | one-word typo fold ("Three pairs" → "Four pairs") |
| 8 → RATIFIED | 0 B / 0 M / 0 N | `79e3a1c` |

**The lazy-load round-4 parser-only pivot is the
sharpest adversarial moment of m6.** Pi-3 surfaced that
the original Phase-I2 ("manifest parses
`load.triggers.kind = \"tool\"` ⇒ test asserts the
parser") didn't exercise the runtime. Round 4 attempted
the cheap fix (extend the parser test); pi-4 §B-1
rejected this as a **fixture-only crutch** —
*the structural guarantee the test claims to certify
doesn't exist in the runtime, so the test would mask the
absence rather than anchor a behaviour*. Round 5 took
the expensive path: full lazy-load runtime
(`register_lazy` + `ensure_spawned` +
`lazy_candidates` + `tool_to_canonical` +
gate-side dispatch-after-validation hook). Rounds 5–6
then converged on the API redesign as a single coherent
cluster after pi-5's B-1..B-5 surface review.

**Surface-size-predicts-rounds (m1 lesson) confirmed.**
m6's wide implementation surface (10 phases A–J, four of
which — A, B, C, I — carried load-bearing PP1 / syd-pty /
lazy-load invariants) drove the 8-round commits bracket,
exactly the same shape as m5b's 28-row plan at 7 scope
rounds + 6 commits rounds for an equally hot zone (taint
+ broker-intake). m4 (3 commits rounds) had fewer
load-bearing invariants left after m3's broker work; m6
is on the m5b end of the spectrum, not the m4 end.

**Pi rounds budget.** Scope predicted "4–5" pi rounds
(scope §"Internal split"). Actual: 5 scope + 8 commits =
**13**, vs m5b's 7 + 6 = 13. m6 met the m5b precedent;
the slight redistribution (more commits rounds, fewer
scope rounds) reflects that m6's hot surface was
implementation-side (lazy-load API) rather than
scope-side (taint invariants).

---

## 4. Coverage / negative-test report

Every roadmap negative landed; every `scope.md`
§"Acceptance summary" bullet maps to a commit row. Full
mapping lives in `commits.md` §"Acceptance traceability
appendix"; this section calls out the **deliberate
carveouts** that are not bugs.

### Acceptance bullet → commit map (summary)

| Acceptance bullet | Commit(s) |
|---|---|
| Every named deliverable in §"In scope" implemented + tests pass | c01–c28 |
| Linux `nix develop … cargo test --workspace --features test-fixture` green | c18 (CI matrix); each phase's tests run under the workspace flag |
| **macOS CI green (hard gate)** | c18 (matrix); the `macos-latest` leg covers both `cargo test --workspace` and `nix build .#rafaello` |
| `nix develop … cargo build --workspace --bins` green | c16 (8-package build set) |
| `nix build .#rafaello` produces the 8-binary release tree + PP1 plugin trees | c16 + c17 + c18 |
| Post-`rfl init --yes`: `.rafaello/plugins/<topic-id>/rafaello.toml` + `bin/rfl-openai` exist; digests match | c02 + c04 (`rfl_init_materialises_package_dir.rs`) |
| `brew install` works in a clean macOS shell | c19 + c20 + `manual-validation.md` §G (manual; owner-judgment item 10 default) |
| 5-line bootstrap in `README.md` verbatim + works against LiteLLM | c21 + manual-validation §1 (stub + LiteLLM) |
| `manual-validation.md` §1–§7 + §G + §5 tmux recording | c27 + c28 |
| Retrospective + decisions.md rows 59–68 + glossary additions + v0.1 → main merge plan | **this document** + drift commits planned in §7 |

### Defensive negatives landed

All Phase-A/B/C/D/E negatives from scope §"Demo bar"
landed and pass:

- `rfl_init_idempotent_no_overwrite.rs` (c04)
- `rfl_init_decline_writes_empty_lock.rs` (c04)
- `rfl_install_positional_unknown_plugin_errors.rs` (c07)
- `rfl_install_requires_one_of_fixture_or_plugin.rs` (c07)
- `fake_syd_records_cargo_bin_exe_env_when_set_explicitly.rs` /
  `…_from_sibling.rs` / `…_resolution_fails_hard_when_pty_missing.rs`
  (c10)
- `rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty.rs` (c10)
- `rfl_audit_empty_db.rs` (c13)
- `rfl_audit_filters_by_request_id_no_join.rs` (c13)
- `rfl_openai_stub_scripted_turns_panics_on_exhaustion` (c15)
- `lazy_load_tool_trigger_spawns_on_first_call.rs` (c25)

### Deliberate carveouts

1. **Headline integration test uses `rfl-openai-stub`,
   not real LiteLLM.** Per `/tmp/m6-phase-j-litellm-note.txt`
   (owner 2026-05-12 02:50 + 03:10 update), the LiteLLM
   proxy at `https://litellm.thepromisedlan.club/v1` was
   partially down at Phase-J start. The default
   `vllm/qwen3.6-27b` model was confirmed down (upstream
   `sodium:8001` unreachable). The owner's three-step
   guidance was:
   - **Step 1: stub transcript is canonical.** The
     wire protocol is the contract; the stub demonstrates
     it faithfully. The headline integration test
     (`rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`,
     deterministic, stub-driven) is the binding proof.
   - **Step 2: capture a second LiteLLM transcript if
     the proxy is back.** Owner's 03:10 update confirmed
     `mlx/qwen3.6-35b` ALIVE — the §5 LiteLLM transcript
     can be captured against this model by setting
     `RFL_OPENAI_MODEL=mlx/qwen3.6-35b` in the
     `rfl-openai` lock-side env. This second transcript
     is **carryover to retro convergence time** if not
     captured before round 1 lands. See §5 (Follow-ups).
   - **Step 3: stub-alone is acceptable.** If LiteLLM is
     still fully down at retro time and no alternative
     OpenAI-compatible endpoint is reachable, the stub
     transcript alone satisfies hard requirement #3.
     `manual-validation.md` §5 documents the outage so
     future maintainers know why the real-LLM transcript
     may be missing.

   **Status at round-1 draft time (2026-05-12).** The §5
   tmux transcript files captured at c28 use the stub.
   The LiteLLM `mlx/qwen3.6-35b` real-LLM second
   transcript is **not yet captured**; this is the only
   open Phase-J item. Owner did not require re-pinging
   on LiteLLM unless the proxy is still down at retro
   convergence AND it materially affects ratification —
   stub-alone is documented as acceptable.

2. **`rfl audit --request-id` is a single-table filter,
   not a provenance join.** The live `entries` schema
   has no `call_id` column (verified at
   `crates/rafaello-core/src/session/mod.rs:99-122`); pi
   round-2 B-4 surfaced and the implementation respects
   the live shape. A full provenance join is a v2 item
   requiring either a schema migration or a
   JSON-extract query path; see §5 follow-ups.

3. **`x86_64-darwin` Homebrew artefact is deferred to v2.**
   `flake.nix:24-28` has no `x86_64-darwin` system; the
   m6 Homebrew tarballs are `aarch64-darwin /
   aarch64-linux / x86_64-linux` only. Intel-Mac users
   have no `brew install` path until v2 adds the arch.
   Round-3 M-3 fold; §5 routes this.

4. **Phase I1 landed as `StartupEvent::ToolSchemaCatalogBuilt`
   ordering, not the literal
   `core_tools_list_registered_before_provider_spawn.rs`.**
   Owner-judgment item 11 default was "in scope" but
   the live structural guarantee
   (`CorePluginService::new` runs ahead of the
   supervisor's spawn loop by construction) is better
   asserted via the observable startup-event ordering
   (the `ToolSchemaCatalogBuilt` event fires before any
   provider-spawn event) than via the synthetic
   `core.tools_list`-before-spawn anchor. Defence-in-depth
   for `decisions.md` row 49 is preserved; the literal
   filename in the carryover punchlist is not. This is a
   deviation in shape (not scope) — flagged here for pi
   to ratify or push back.

---

## 5. Follow-ups routed beyond m6 (post-v1 / v2)

| Item | Source | Routing |
|---|---|---|
| `x86_64-darwin` Homebrew release artefact | scope §"Out of scope" #15; owner-judgment item 5 v2-expansion note | v2 (requires `x86_64-darwin` in `flake.nix` `systems` + macOS-13 CI builder) |
| Streaming entry patch ops | `decisions.md` row 28 | v2 |
| Helper plugins | `decisions.md` row 26 | v2 |
| External UDS frontend attach | `decisions.md` row 27 | v2 |
| Subprocess plugin renderers | `decisions.md` row 29 | v2 |
| Public `rfl serve` (any flavour) | `decisions.md` row 34; scope §"Out of scope" #7 | v2 |
| `rfl provider tool <plugin>` CLI | m5a §5 row 7; scope §"Out of scope" #8 | v2 |
| `rfl grant` / `rfl revoke` / `rfl provider use` / `rfl update` top-level CLI | scope §"Out of scope" #9 | v2 |
| `rfl audit --request-id` cross-table join (provenance) | scope §"Out of scope" #10 | v2 (schema migration or JSON-extract query path) |
| `rfl init --endpoint=<url>` reconfiguration UX | scope §"Out of scope" #11 | v2 |
| §A9 superset narrowing on `provider.<id>.assistant_message` / `frontend.<id>.confirm_answer` / `plugin.<a>.rpc_reply` | m5b §5 row 2; scope §"Out of scope" #1 | v2 |
| Real-network `rafaello-fetch` | m5b §5 row 3 | post-v1 |
| Substring-threshold tuning / Aho-Corasick | m5b §5 rows 4 + 6 | v2 |
| `TaintMatchMap` LRU cap | m5b §5 row 5 | v2 |
| Laundered-flow taint / CaMeL | m5b §5 row 7 | v2 |
| macOS-CI Homebrew install workflow (`brew install` in CI) | owner-judgment item 10; scope §"Out of scope" #14 | post-v1 |
| **LiteLLM real-LLM §5 transcript** | Phase J carryover per `/tmp/m6-phase-j-litellm-note.txt` | **Capture if proxy is up at retro convergence (using `mlx/qwen3.6-35b` per owner 03:10 update); stub-alone is acceptable per owner's three-step framing** |
| Non-test workspace `#[allow(...)]` audits beyond `result_large_err` | scope §"Out of scope" #13 | post-v1 |
| Renderer-shape extensions / new built-in renderer kinds / new tool plugins | scope §"Out of scope" #12 | v2 |

**No m6-surfaced items fall outside scope §"Out of scope"
or the v2 deferral list.** The lazy-load runtime
(c24–c25) was originally Phase I2 — pi-3..pi-6 expanded
it from a parser test to a runtime, but the runtime
itself is in-scope (the manifest field was already
plumbed; m6 closed the dispatch-side gap). No new
surface escaped the scope envelope.

---

## 6. Stream RFC drift surfaced by m6

**Expected disposition per the m1/m3 retro cleanup:
none.** m6 ships no new security primitives, no new
RPC surfaces, and no new bus topics. The new commands
(`rfl init`, `rfl audit`, the positional `rfl install`)
are CLI-layer additions over existing `rafaello-core`
shapes; the lazy-load runtime is an internal supervisor
extension, not a stream-RFC surface change.

The following items were checked and found NOT to
require stream patches:

- **Stream A (security):** unchanged; m6 inherits the
  m5b superset surface verbatim.
- **Stream B (fittings):** the gate-side
  `dispatch-after-validation` hook (c24) routes the
  existing `validation_complete` signal through
  `ensure_spawned` before forwarding — no fittings RPC
  shape changes.
- **Stream C (lockfile):** the lock TOML written by
  `rfl init` matches the live `Lock` shape; no schema
  drift.
- **Stream E (renderer):** unchanged.
- **Stream F (manifest):** `load.triggers.kind = "tool"`
  is the existing manifest field; the runtime is new but
  the field shape is not.

**One narrow candidate for pi to assess:** the
**`bin/<plugin-bin>` real-file PP1 layout** (round-4
B-1) is a new constraint in the bundled-plugin
package-tree shape. If the stream RFC table has an
explicit row for the package-tree layout under
`<release-prefix>/share/rafaello/plugins/`, that row
should be patched to spell out the
canonicalisation-rejects-symlinks constraint. If no such
row exists today, this is purely an `overview.md`
addition (§7 row 65) and not a stream drift.

---

## 7. Overview / decisions / glossary additions planned

Per `plans/README.md` Phase 4: these land as **drift
commits between retrospective ratification and the v0.1
→ main merge**. The retrospective drafts the text; the
edits do **not** land in this commit (per the m5a
`816b273` / m5b `bc6c…` precedent). The drift-commits
execution plan:

- One commit appending rows 59–68 to `decisions.md`.
- One commit appending the new entries to `glossary.md`.
- One commit patching `overview.md` §8.1 (bundled
  provider materialisation), §16 (v1 scope cut — add
  m6 entry), and a new §"Package placement" subsection
  documenting PP1.
- (If pi §6 surfaces stream drift) one commit patching
  the stream RFC table for the PP1 real-file constraint.

### `decisions.md` rows (59–68) — draft text

**Row 59 — Package placement invariant PP1.**
> `rfl init` and `rfl install <plugin>` materialise the
> bundled plugin source tree into
> `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/` where
> `<topic-id> = topic_id::derive(canonical_id)`. The
> manifest filename inside that directory is
> `rafaello.toml` (per row 25). The plugin's entry
> binary lives at `bin/<plugin-bin>` as a real file —
> symlinks into `<release-prefix>/bin/` (or anywhere
> outside the package directory) trip
> `compile::resolve_entry`'s `EntryEscape` containment
> check. Phase F's `nix build .#rafaello` writes plugin
> binaries directly under
> `$out/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`
> as real files; top-level `$out/bin/` carries only
> `rfl` and `rfl-tui`. Cites m6
> `rafaello/nix/package.nix:postInstall`,
> `crates/rafaello/src/{init,install}.rs`,
> `crates/rafaello-core/src/compile.rs::resolve_entry`,
> `crates/rafaello-core/src/manifest/validate_with_package.rs::resolve_inside_package`.

**Row 60 — `rfl init` semantics.**
> `rfl init` materialises `rafaello.lock` with the
> bundled `rfl-openai` provider pre-installed against
> the dev-environment LiteLLM endpoint
> (canonical id `builtin:openai@0.0.0`, per row 38).
> Idempotent: re-runs over an existing lock exit 0
> without overwrite. `--yes` accepts the default grant
> non-interactively. `--force` rewrites byte-for-byte
> from defaults, dropping user edits. Declining the
> install-time review writes an empty lock (no
> `[plugin.…]`, no `[session].provider_active`) and
> skips the PP1 copy step. `--project-root <path>`
> mirrors `rfl chat`. Cites m6
> `crates/rafaello/src/init.rs`.

**Row 61 — `rfl install <plugin>` positional resolver.**
> `rfl install` accepts a positional plugin id (e.g.
> `rfl-mailcat`) that resolves against the bundled
> plugin tree at
> `<release-prefix>/share/rafaello/plugins/<plugin>/`
> per row 25's `rafaello.toml` canonical name.
> `--fixture <path>` remains for fixture tests; clap
> enforces exactly-one-of via `conflicts_with` +
> `required_unless_present`. Both arms perform the PP1
> copy step (row 59). Cites m6
> `crates/rafaello/src/install.rs`.

**Row 62 — `syd-pty` discovery (belt-and-braces; no
`pty:off` fallback at the lockin layer).**
> Two-layer fix for the syd-pty discovery problem:
> (a) `rafaello/nix/devenv.nix` exports
> `CARGO_BIN_EXE_syd-pty` mirroring the existing
> `LOCKIN_SYD_PATH` export; (b)
> `lockin/crates/sandbox/src/lib.rs`
> `SandboxBuilder::syd_pty_path` resolves the path via
> spec → `$CARGO_BIN_EXE_syd-pty` → sibling-of-syd →
> `PATH`, then sets the env on the syd child command
> via `Command::env`. Resolution failure is a hard
> `Err(SandboxError::SydPtyNotFound)` — no `pty:off`
> fallback at this layer (a silent fallback would
> re-introduce the m5a wall the m6 owner hit on
> 2026-05-12). Cites m6
> `lockin/crates/sandbox/src/lib.rs`.

**Row 63 — `rfl audit` read CLI semantics.**
> `rfl audit` is a read-only CLI over the live
> `audit_events` schema (`seq, at, kind, request_id,
> payload` — no `ts_unix_ms`). Default render: one row
> per line with truncated payload summary; `--full`
> disables truncation; `--json` emits one JSON object
> per row. Filters: `--kind <kind>` (repeatable,
> against `AuditKind::as_str()`), `--since <duration>`
> (`1h`/`30m`/`24h`), `--request-id <id>`.
> `--request-id` is a single-table filter on
> `audit_events.request_id` — **no join against
> `entries`** (the live `entries` schema has no
> `call_id` column; full provenance is a v2 item).
> `--project-root <path>` mirrors `rfl chat` / `rfl
> init`. Cites m6 `crates/rafaello/src/audit_cli.rs`.

**Row 64 — `rfl-openai-stub` scripted turns.**
> `RFL_OPENAI_STUB_SCRIPTED_TURNS = <path-to-toml>`
> walks an N-turn TOML script and selects the HTTP
> response per matched turn. Exhaustion is a
> deterministic panic (mirrors the m5b multi-answer
> hook per row 56). Mutually exclusive with the
> singular-turn env from m5a/m5b; both-set is a stub
> startup error. The stub binary is **buildable in the
> release tree** (the `test-fixture` gate was dropped
> from `[[bin]]`) so the
> `manual-validation.md` §5 scripted demo and the
> headline integration test consume a runnable
> stub from `$out/share/rafaello/plugins/rfl-openai-stub/bin/`.
> Cites m6
> `crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`.

**Row 65 — `nix build .#rafaello` release artefact
shape.**
> `nix build .#rafaello` produces:
> (a) `$out/bin/rfl` and `$out/bin/rfl-tui` (the only
> user-facing entry points);
> (b) `$out/share/rafaello/plugins/<plugin>/` for each
> of the 8 packages in the build set (`rafaello`,
> `rafaello-tui`, `rafaello-openai`,
> `rafaello-openai-stub`, `rafaello-readfile`,
> `rafaello-mailcat`, `rafaello-mockprovider`,
> `rafaello-fetch`), containing
> `rafaello.toml` + `openrpc.json` (per row 31) +
> `bin/<plugin-bin>` as a real file + any `schemas/`
> templates. `rafaello-bus-fixture` is test-shaped and
> excluded. Cites m6 `rafaello/nix/package.nix`.

**Row 66 — Homebrew distribution model G.β
(separate-tap + Nix-built tarballs).**
> Distribution is via a separate
> `luizribeiro/homebrew-rafaello` tap whose
> `Formula/rafaello.rb` fetches per-arch tarballs
> uploaded by `.github/workflows/rafaello-release.yml`
> on `v*` tags. Architectures: `aarch64-darwin`,
> `aarch64-linux`, `x86_64-linux` matching
> `flake.nix:24-28` `systems`. `x86_64-darwin` is a v2
> expansion (no Nix builder for that arch in the root
> flake). The formula installs the row-65 layout
> verbatim. Cites m6 `homebrew/rafaello.rb`,
> `.github/workflows/rafaello-release.yml`.

**Row 67 — `result_large_err` allows are ratified.**
> Workspace-wide module-level
> `#[allow(clippy::result_large_err)]` allows are
> **ratified in place** — keeping them is preferred
> over boxing `ReemitError` / `AgentLoopError` (which
> would ripple through every `?` site at 5+ commits,
> against negligible runtime benefit since the large
> error variants are rare cold paths). Each allow site
> carries a comment pin to this row. Cites m6
> `crates/rafaello-core/src/reemit/`,
> `crates/rafaello-core/src/agent_loop/`,
> `crates/rafaello/src/lib.rs`.

**Row 68 — Lazy-load runtime via `LoadPolicy::Lazy {
command }`.**
> Manifests carrying `[load.triggers]` with
> `kind = "tool"` register their canonical id as a
> **lazy candidate**: `PluginSupervisor::register_lazy(
> canonical, plan, paths, triggers)` writes both
> `lazy_candidates: Mutex<BTreeMap<CanonicalId,
> LazyCandidate>>` and `tool_to_canonical:
> Mutex<BTreeMap<String, CanonicalId>>`. On the first
> tool dispatch whose tool name matches a trigger, the
> gate-side dispatch-after-validation hook calls
> `ensure_spawned(canonical)`, which idempotently
> spawns the plugin (removing the candidate from
> `lazy_candidates` so subsequent dispatches go through
> the normal `managed` path). Spawn events are
> recorded via `record_spawn_event` to the
> `RFL_SPAWN_TRACE_LOG` file path (observability seam;
> pi-6 B-5 file-log shape over stdout). Supervisor
> `shutdown(&self)` (round-6 redesign — `&self` over
> the original `shutdown(self)` consume signature
> flagged by pi-5 B-2) terminates both eager and
> lazy-spawned children. `register_lazy` takes
> **primitives** (canonical, plan, paths, triggers) so
> the private `LazyCandidate` struct stays internal to
> `rafaello-core` (pi-5 B-1 cross-crate visibility
> fix). Cites m6
> `crates/rafaello-core/src/supervisor.rs`,
> `crates/rafaello-core/src/gate/mod.rs`,
> `crates/rafaello/src/lib.rs::run_chat`.

### `overview.md` patches planned

- §8.1 (bundled provider plugin): add a banner-pointer
  to `rfl init` materialisation (row 60) and the PP1
  copy step (row 59).
- §16 (v1 scope cut): add an m6 entry listing the
  shipped surface (init / install positional / audit
  CLI / multi-turn stub / Homebrew G.β / lazy-load
  runtime).
- New subsection or §15.x append documenting the
  package placement layout (PP1) with the
  `<release-prefix>/share/rafaello/plugins/<plugin>/`
  source tree and the
  `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`
  materialised tree.

---

## 8. Glossary additions (text — per `glossary.md` one-line convention)

- **`rfl init`** — bootstrap CLI that materialises
  `rafaello.lock` with the bundled `rfl-openai` provider
  pre-installed and copies the bundled plugin tree into
  `.rafaello/plugins/<topic-id>/` per PP1. Idempotent;
  `--force` rewrites; `--yes` skips review; declining
  writes an empty lock. Cites
  `crates/rafaello/src/init.rs`, `decisions.md` rows
  59 + 60.
- **`rfl install <plugin>`** — positional plugin
  argument that resolves against the bundled tree at
  `<release-prefix>/share/rafaello/plugins/<plugin>/rafaello.toml`;
  `--fixture <path>` remains for fixture tests. Cites
  `crates/rafaello/src/install.rs`, `decisions.md` row 61.
- **`rfl audit`** — read CLI over the live
  `audit_events` SQLite table. Filters by `--kind`,
  `--since`, `--request-id` (no join against `entries`).
  Cites `crates/rafaello/src/audit_cli.rs`,
  `decisions.md` row 63.
- **`rfl-openai-stub` scripted turns** — N-turn TOML
  script consumed by the stub via
  `RFL_OPENAI_STUB_SCRIPTED_TURNS`. Deterministic-panic
  on exhaustion; mutually exclusive with the singular
  m5a/m5b env. Cites
  `crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`,
  `decisions.md` row 64.
- **package placement (PP1)** — invariant that bundled
  plugin trees materialise into
  `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`
  with `bin/<plugin-bin>` as a real file (no symlinks
  out of the package directory), enforced by
  `compile::resolve_entry`'s `EntryEscape` containment
  check. Cites `decisions.md` row 59.
- **`syd-pty` discovery** — the spawning-side problem
  where `syd` invokes `setup_pty` and fails because
  `syd-pty` is not on `PATH` and
  `CARGO_BIN_EXE_syd-pty` is not set on the syd child
  command. Fixed belt-and-braces (devshell export +
  lockin sandbox resolution). Cites
  `rafaello/nix/devenv.nix`,
  `lockin/crates/sandbox/src/lib.rs`,
  `decisions.md` row 62.
- **lazy-load runtime** — the supervisor's
  `LoadPolicy::Lazy { command }` path:
  `register_lazy` enrolls a candidate keyed by
  canonical id + tool triggers; first matching tool
  dispatch routes through `ensure_spawned(canonical)`
  before forwarding. Spawn events emit via
  `record_spawn_event` to the `RFL_SPAWN_TRACE_LOG`
  file. Cites `decisions.md` row 68.
- **`ensure_spawned`** — supervisor primitive that
  idempotently spawns a lazy candidate (or no-ops if
  already managed) and awaits readiness before
  returning. Used by the gate's
  dispatch-after-validation hook.
- **`register_lazy`** — supervisor registration API
  taking primitives (canonical, plan, paths, triggers)
  so callers in `rafaello` don't need to name the
  private `LazyCandidate` struct.
- **`record_spawn_event`** — supervisor seam that
  writes spawn-event records to the file path named by
  `RFL_SPAWN_TRACE_LOG` (test observability; production
  no-op when the env is unset).
- **`RFL_SPAWN_TRACE_LOG`** — env var naming a file
  path that receives spawn-event records from
  `record_spawn_event`. The lazy-load integration test
  asserts the trace by grep-reading this file.

Existing-entry banner-pointer extensions:

- **`rafaello.lock`** — append a one-line
  banner-pointer to the `rfl init`-materialised default
  shape (row 60).
- **Bundled provider** — append an `rfl init`
  cross-reference (row 60).

---

## 9. m6 RATIFIED triggers the `v0.1 → main` merge

Per `decisions.md` row 33 (the branch model decision):
**m6's retrospective RATIFIED is the trigger** for the
`rafaello-v0.1 → main` merge. The post-RATIFIED action
is the owner's `git merge --ff-only rafaello-v0.1` onto
`main`, executed immediately after the drift commits in
§7 land on `rafaello-v0.1` itself (drift commits land
between retrospective RATIFIED and the ff-only merge,
per the m5a `816b273` / m5b precedent — they're on the
v0.1 branch when the merge fires, so `main` receives
them in the same fast-forward).

After the merge:

- `main` HEAD includes every m0..m6 commit + the §7
  drift commits.
- `rafaello-v0.1` is preserved as the v1 release tag
  (G2's `.github/workflows/rafaello-release.yml` is
  triggered by `v*` tags; the owner cuts `v0.1.0`
  against the merged tip).
- v2 work begins on a new `rafaello-v0.2` branch
  forked from the merged `main`.

---

**Open items for pi round 1.**

1. The Phase I1 deviation (c23 startup-event ordering
   replaces the literal
   `core_tools_list_registered_before_provider_spawn.rs`
   anchor) — pi to ratify the shape change or push back
   for the literal filename. Both anchors are
   defence-in-depth for `decisions.md` row 49; the
   observable event ordering is, I'd argue, the cleaner
   anchor.
2. The §6 stream-RFC PP1 patch candidate (real-file
   constraint on `bin/<plugin-bin>`) — pi to confirm
   whether this needs a stream RFC patch or is
   `overview.md`-only.
3. The LiteLLM real-LLM §5 second transcript
   (carryover to retro convergence per owner's
   three-step guidance) — pi to confirm the
   stub-alone-acceptable framing for round 1, with the
   `mlx/qwen3.6-35b` real-LLM transcript captured if
   the proxy is up at convergence.
4. The `decisions.md` row 59–68 draft text — pi to
   review each row for wording precision and
   live-shape grounding.
