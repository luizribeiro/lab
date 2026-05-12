# m6 — v1 polish + release readiness — scope

> **Status:** round 4 — claude-authored 2026-05-12, awaiting pi
> round 4. Folds `scope-pi-review-3.md` (B/2 M/1 N/1, verdict:
> blocking-but-very-narrow) on top of rounds 2–3. Round-3
> residuals: (a) round-3's `bin/<plugin-bin>` symlinks in the
> release plugin tree would canonicalise outside `package_dir`
> and trip `compile::resolve_entry`'s `EntryEscape` check
> (`rafaello-core/src/compile.rs:440-465`) and
> `validate_with_package::resolve_inside_package`'s
> `!canon.starts_with(pkg_canon)` rejection
> (`rafaello-core/src/manifest/validate_with_package.rs:100-117`);
> (b) the J2 tmux script `cd`-ed into an empty `mktemp -d`
> with no flake, and `rfl install` lacked `--project-root` so
> init/install/chat/audit could not all be redirected the
> same way; (c) the fake-syd fixture path
> `tests/fixtures/fake_syd.rs` is not auto-built by Cargo;
> (d) F1 mixed package-name list with the acceptance
> installed-binary list. Round 4 folds:
>
> - **Release plugin tree carries real binaries, not symlinks**
>   (B-1). Each `<release-prefix>/share/rafaello/plugins/<plugin>/`
>   contains an **actual** `bin/<plugin-bin>` file alongside
>   the manifest, so `canonicalize → starts_with(package_dir)`
>   holds. Top-level `<release-prefix>/bin/` carries only the
>   user-facing `rfl` (+ `rfl-tui` for the chat subprocess
>   tree per `decisions.md` row 34). PP1 acceptance asserts
>   `compile::resolve_entry` succeeds against the post-`rfl init`
>   `.rafaello/plugins/<topic-id>/bin/rfl-openai`.
> - **J2 runs from the lab worktree, not the throwaway
>   project** (B-2). All `rfl <subcommand>` invocations get
>   `--project-root "$PROJECT"`; `rfl install` gains
>   `--project-root` in Phase B1 (one-line addition for
>   symmetry with `init`/`chat`/`audit`); J2 explicitly
>   `mkdir -p`s the transcript dir; a final copy step lands
>   the captures under
>   `rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`
>   for commit.
> - **Fake-syd as a Cargo `[[bin]]`** (M-1). Adds
>   `[[bin]] name = "fake-syd" path = "tests/bin/fake_syd.rs"
>   required-features = ["test-fixture"]` to
>   `lockin/crates/sandbox/Cargo.toml` so Cargo discovers and
>   builds it under the existing `test-fixture` feature gate;
>   C3 tests use `env!("CARGO_BIN_EXE_fake-syd")` to locate
>   the binary at runtime.
> - **F1 list renamed "package build set"** (N-1); acceptance
>   keeps the "release binary set" naming for the
>   installed-binary list.
>
> Cumulative trajectory: round 1 → 7B/6M/4N (BLOCKING) →
> round 2 → 2B/5M/3N (BLOCKING-narrowing) → round 3 →
> 2B/1M/1N (BLOCKING-very-narrow) → round 4 (this commit),
> target verdict CONVERGED.
>
> Prior-round status text (preserved for traceability):
>
> Round 3 folded `scope-pi-review-2.md` (B/2 M/5 N/3,
> verdict: blocking-but-narrowing). Round 2's residual issues
> were
> all about lining the new install/release-tree story up with
> the **live** `rfl chat` package resolver
> (`${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/rafaello.toml`,
> `decisions.md` row 25), making the J2 tmux script executable
> against the **live** confirm overlay copy
> (`rafaello-tui/src/confirm.rs:160-211` — title `confirm`,
> body `args:` / `sinks:` / `taint:` lines), and tidying small
> contradictions (Homebrew default not actionable; macOS x86_64
> promised but unsupported; bus-fixture inclusion contradicted
> between owner item 9 and acceptance). Round 3 folds:
>
> - **Phase A + Phase B now copy the bundled package tree into
>   `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`** with the
>   manifest named `rafaello.toml` (per `decisions.md` row 25)
>   so the live `rfl chat` resolver finds it (B-1). New
>   acceptance assertion: post-`rfl init`,
>   `.rafaello/plugins/<topic-id-of-openai>/rafaello.toml`
>   exists and digests match the lock.
> - **Phase D `rfl audit` gains `--project-root`** mirroring
>   `rfl chat` + `rfl init`; J2 calls it with `--project-root
>   "$PROJECT"`. **J2 expected substrings rewritten** against
>   live `confirm.rs` overlay copy: ` confirm ` (title border),
>   `sinks: mail` (overlay sinks line), `alice@example.com`
>   (echoed `args:` JSON) (B-2).
> - **Phase G G.β default made actionable** absent owner reply
>   — `commits.md` can proceed on G.β; owner can still override
>   at ratification (M-1).
> - **Acceptance + decisions row 65 use "every release binary"**
>   listed explicitly; `rafaello-bus-fixture` excluded per
>   owner-judgment item 9 default (M-2).
> - **macOS Homebrew promise narrowed to `aarch64-darwin`
>   only**, matching the flake's `systems = [aarch64-darwin,
>   aarch64-linux, x86_64-linux]` (M-3). `x86_64-darwin` listed
>   in §"Out of scope" as a v2 expansion if demand surfaces.
> - **Phase C C3 swapped for a fake-`syd` wrapper test** that
>   records `argv` + environment and asserts
>   `CARGO_BIN_EXE_syd-pty` was set on the child, plus a
>   sibling-discovery tempdir test (M-4).
> - **Phase B1 makes `--fixture: Option<PathBuf>` explicit**
>   with clap `required_unless_present = "plugin"` and a
>   `conflicts_with` clause (M-5).
> - **Owner-judgment item count corrected** to 13 (N-1);
>   `--no` mention reworded (N-2); §5 path component
>   replaced with `transcripts/section-5/` (N-3).
>
> Round 1 was BLOCKING on live-code contradictions:
> the `nix build .#rafaello` package output already exists but
> is single-bin; the advertised `rfl install <tool>` is not the
> live install shape; the audit-CLI SQL targeted non-existent
> columns; the `rfl init` lock sketch used the wrong table
> shape; the syd-pty test could pass via the `pty:off` fallback;
> the Homebrew model conflated three distribution stories; and
> the headline demo named three different tools across the
> Phase A / Phase D / Phase J / demo-bar sections. Round 2:
>
> - **Reframes Phase E** as *repair* of the existing
>   `.#rafaello` output to install every required binary (B-1).
> - **Adds Phase B** (install-UX) so `rfl install <tool>` is a
>   real positional-arg command shape that the README + the
>   manual-validation transcript can drive (B-2; restores the
>   pre-flight phase shape M-4).
> - **Locks the canonical demo tool** to `send-mail` via
>   `rafaello-mailcat` end-to-end (Phase A pre-install of
>   `rfl-openai`, Phase B install of `rfl-mailcat`, Phase D
>   audit cite, Phase J §5 tmux transcript, demo-bar
>   integration test, 5-line bootstrap) (B-3).
> - **Rewrites the `rfl audit` SQL** against the live
>   `audit_events` schema (`seq, at, kind, request_id,
>   payload`); drops the unsupported `entries.call_id` join
>   (B-4).
> - **Rewrites the `rfl init` lock sketch** to the live
>   `[plugin."<canonical-id>"]` + `grant.bundles.default.*`
>   shape (B-5).
> - **Hardens the syd-pty test** to require actual `syd-pty`
>   discovery (the `pty:off` fallback is a documented residual-
>   risk diagnostic, not a success path) (B-6).
> - **Elevates the Homebrew distribution model** to a
>   three-alternative owner-judgment item; defaults to a
>   separate-tap formula that fetches Nix-built release
>   tarballs (B-7).
> - **Corrects the CLI inventory** to the live top-level
>   subcommand list `chat / install / status` (M-1); fixes the
>   `rafaello-openai-stub` crate path (M-2); raises the commit
>   budget to **28 default / 30 max** (M-3); restores the
>   pre-flight phase letters A–J (M-4); reverses the
>   `result_large_err` ratify path description (M-5); uses
>   `nix develop .#rafaello --impure` in acceptance gates (M-6).
> - **Folds the four nits**: anchors the audit-kind reference
>   to `AuditKind::as_str()` + `decisions.md` rows 55/58 (N-1);
>   downgrades the README's manual env-export to a "pre-m6
>   workaround" subsection (N-2); reserves a single retro-phase
>   commit in the budget (N-3); drops the asciinema mention
>   (tmux transcript files are the only required evidence)
>   (N-4).
>
> Live-code spot-checks (cumulative round 2 + round 3):
> - `flake.nix:24-28` — `systems = [aarch64-darwin,
>   aarch64-linux, x86_64-linux]`; no `x86_64-darwin` (round-3
>   M-3).
> - `flake.nix:71-73` + `rafaello/nix/default.nix:10-12` +
>   `rafaello/nix/package.nix:16` — confirms round-2 B-1.
> - `rafaello/crates/rafaello/src/install.rs:32-43` — confirms
>   round-2 B-2 (`fixture: PathBuf` required).
> - `rafaello/crates/rafaello/src/install.rs:155-176` and
>   `rafaello/crates/rafaello/src/lib.rs:235-276` — confirms
>   round-3 B-1: both `install` (other plugins' validation) and
>   `chat` resolve plugin directories via
>   `${PROJECT_ROOT}/.rafaello/plugins/<topic-id-derive(canonical_id)>/`
>   and read `rafaello.toml` (per `decisions.md` row 25).
> - `rafaello/crates/rafaello-tui/src/confirm.rs:160-211` —
>   confirms round-3 B-2: live overlay renders title `confirm`,
>   then lines `<summary>` (gate-built
>   `"<tool> via <plugin> — sinks: [<classes>]"`), `args:
>   <json>`, `sinks: <comma-list>`, `taint:` / `provenance:`,
>   `<ttl>s remaining`. No "Allow this call?" prompt string
>   exists.
> - `rafaello/crates/rafaello-tui/src/lib.rs:70-72` — confirms
>   `KeyCode::Char('a')` (with `'y'` and Enter) maps to
>   `Answer::Allow`; `'q'` is not listed there but is the chat-
>   loop quit key from m3.
> - `rafaello/crates/rafaello-core/src/gate/mod.rs:374-378` —
>   confirms summary format
>   `"{tool} via {plugin} — sinks: [{classes}]"`.
> - `rafaello/crates/rafaello-core/src/audit/mod.rs:114-123` —
>   confirms round-2 B-4 column list.
> - `rafaello/crates/rafaello-core/src/session/mod.rs:99-128` —
>   confirms round-2 B-4 `entries` shape (no `call_id`).
> - `rafaello/crates/rafaello-core/src/lock/lock_file.rs:18-34`
>   + `rafaello/fixtures/m5b-locks/rafaello.lock` — confirms
>   round-2 B-5 lock schema.
> - `lockin/crates/sandbox/src/lib.rs:209-232` — confirms
>   round-2 B-6.
> - `rafaello/crates/rafaello/src/lib.rs:57-69` — confirms M-1
>   (only `Chat`, `Install`, `Status`).
> - `rafaello/crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`
>   — confirms M-2 crate path.

---

## Goal

Close the m6 roadmap row by landing the **first-chat-from-cold
experience** plus the **release-engineering surface** required
to ship `rafaello-v0.1` to `main` as a usable v1. Roadmap row:

- Test-coverage gaps closed (per coverage report).
- Documentation pass on `rafaello/README.md` + `CONTRIBUTING.md`.
- Homebrew formula matching scope/tempo.
- `nix build .#rafaello` green on Linux + macOS.
- A manual end-to-end transcript in `manual-validation.md`
  covering `init → install rfl-openai → install one tool →
  chat → tool call with confirmation → response render →
  session persist`.
- No opportunistic new tools — every shipped tool is
  owner-ratified in this `scope.md`.

Roadmap-text reconciliation: the row says `install rfl-openai`
as a *step*, but `overview.md` §8.1 + `decisions.md` row 38
have `rfl-openai` as a **pre-installed bundled provider** that
`rfl init` materialises. m6 reads the roadmap row as
"`rfl init` materialises the bundled provider lock entry; the
operator's explicit `rfl install` is the *tool* plugin step".
That is the m6 commitment; the m6 retrospective records the
roadmap-text reconciliation as a row append (placeholder row
59 below).

Owner-set hard requirements (2026-05-12, verbatim from the
driver prompt):

1. **First chat from cold MUST just work.** A new user with the
   lab repo checked out runs a documented ≤5-line shell
   sequence inside `nix develop .#rafaello --impure --command …`
   and lands in a functioning interactive chat against the dev
   LiteLLM proxy. No `env CARGO_BIN_EXE_syd-pty=…` workaround.
   No `export PATH=/nix/store/…` workaround. No hand-crafted
   lock.
2. **The `syd-pty` discovery problem is solved at the right
   layer.** Owner hit `setup_pty` failures on 2026-05-12
   against the m5a-RATIFIED build because `syd-pty` was not
   adjacent on `PATH`/`CARGO_BIN_EXE_syd-pty` when `syd`
   spawned the plugin subprocess.
3. **`manual-validation.md` §5 captures a real tmux-driven
   interactive `rfl chat` recording.** No "mechanical coverage
   in lieu of recording."
4. **Bootstrap UX fits ≤5 shell lines.** Target shape:

       cd ~/your/project
       nix develop .#rafaello --impure --command rfl init
       export LITELLM_API_KEY=…
       nix develop .#rafaello --impure --command rfl install rfl-mailcat
       nix develop .#rafaello --impure --command rfl chat

5. **The syd-pty failure mode is documented** in the m6
   retrospective + the user-facing README troubleshooting
   section (per N-2 framing — current devshell users are
   never directed at the manual `CARGO_BIN_EXE_syd-pty=$(which
   syd-pty)` recipe; that recipe lives only as a "temporary
   workaround for pre-m6 builds" subsection).

**Canonical demo tool (round-2 lock-in).** Every section that
mentions an installed tool refers to **`rafaello-mailcat`**
(canonical id `local:mailcat@0.0.0`, declaring the tool
`send-mail` with `sinks = ["mail"]` per the live m5a fixture
lock at `rafaello/fixtures/m5b-locks/rafaello.lock`). The 5-line
bootstrap, the Phase B install-UX flow, the Phase D `rfl audit`
cite, the Phase J §5 tmux transcript, the integration test, and
the README all use `rfl-mailcat` / `send-mail`. The non-sink
`rfl-readfile` (m4) and the network-sink `rafaello-fetch`
(m5b) remain available in the workspace but are **not** the m6
demo tool — m5a's send-mail / mailcat path is the v1 proof
because it fires the confirmation modal, which is the
load-bearing UX surface for v1.

What is **new** in m6 vs the m5b-shipped state:

- `rfl init` materialises `rafaello.lock` with `rfl-openai`
  pre-installed against the dev environment's LiteLLM endpoint
  (`decisions.md` row 38; `overview.md` §8.1). `rfl init` does
  not exist today.
- `rfl install <plugin>` grows a positional plugin-id argument
  so the live `--fixture <path>`-only shape can be invoked
  ergonomically from the README. Today `rfl install` requires
  `--fixture <path>` exclusively.
- A belt-and-braces syd-pty discovery fix: devshell export of
  `CARGO_BIN_EXE_syd-pty` + lockin sandbox plumbing of
  `syd-pty` into the syd child's environment.
- `rfl audit` read CLI over `audit_events` (live schema).
- Multi-turn `rfl-openai-stub` script shape (m5b §5 row 1).
- **Repaired** `nix build .#rafaello` package output — today
  it builds `-p rafaello` only; m6 expands to every workspace
  binary so the Homebrew formula consumes a single
  ready-to-install layout.
- Homebrew distribution (model TBD; owner-judgment item 5
  below).
- README + CONTRIBUTING pass — 5-line bootstrap, troubleshooting,
  contributor flow.
- Regression-anchor + `#[allow(clippy::result_large_err)]`
  ratification sweep (m4/m5a/m5b §5 carryovers).
- The tmux-driven `manual-validation.md` §5 recording — the
  canonical v1 proof of life.

m6 ships **no new security primitives** (per m5b §"m5b → m6
boundary" + §5 carryover row 2 v2-routing).

---

## Inputs

### From the plans tree

- `milestones/README.md` — m6 roadmap row (last row;
  pre-ratified definition of m6's name + topic + demo).
- `milestones/m6-polish-release/driver-preflight.md` —
  2026-05-12 pre-flight notes.
- `milestones/m6-polish-release/scope-pi-review-1.md` — round 1
  pi review (B/7 M/6 N/4, BLOCKING) folded into this draft.
- `overview.md` §8.1 (bundled provider plugin), §15.1
  (manifest), §15.6 (bidirectional fittings peer), §16 (v1
  scope cut), §4.6 (reserved env vars).
- `decisions.md` rows **33** (branch model — m6 closes v0.1
  merge), **34** (no public `rfl serve` in v1), **38**
  (`rfl-openai` bundled provider), **44** (`[session].provider_active`
  selector), **46** (`env.allow_secrets`), **47** (`grant_match`
  template contract), **48** (m5a/m5b split), **49**
  (`core.tools_list` RPC), **50–58** (m5b taint primitives +
  scripted-hook env var). All ratified.

### Live code shapes (round-2 verifications)

- **`flake.nix:71-73`** merges `rafaello.packages` into the root
  flake; **`rafaello/nix/default.nix:10`** already exports
  `packages.rafaello`; **`rafaello/nix/package.nix:16`** builds
  with `cargoBuildFlags = [ "-p" "rafaello" ]` — the gap is the
  single-bin scope, not a missing output.
- **`rafaello/crates/rafaello/src/lib.rs:57-69`** —
  top-level `RflChatCommand` enum exposes only `Chat`,
  `Install(InstallArgs)`, `Status`. There is no `rfl grant`,
  `rfl revoke`, `rfl provider use`, `rfl update`, `rfl init`,
  or `rfl audit` at the top level (slash commands exist
  in-session; that's a separate surface).
- **`rafaello/crates/rafaello/src/install.rs:32-43`** —
  `InstallArgs { fixture: PathBuf, lock: Option<PathBuf>,
  i_know_what_im_doing, allow_credential_paths, verbose }`.
  No positional plugin id; `--fixture <path>` is mandatory.
- **`rafaello/crates/rafaello-core/src/audit/mod.rs:114-123`** —
  live `audit_events` schema: `seq INTEGER PRIMARY KEY AUTOINCREMENT,
  at TEXT NOT NULL, kind TEXT NOT NULL, request_id TEXT, payload
  TEXT NOT NULL`. Same definition appears in
  `crates/rafaello-core/src/session/mod.rs:115-122`. There is
  no `ts_unix_ms` column.
- **`rafaello/crates/rafaello-core/src/session/mod.rs:99-122`** —
  `entries` schema: `id, seq, parent, kind, schema, payload,
  metadata, fallback, created_at`. There is **no `call_id`
  column**.
- **`rafaello/crates/rafaello-core/src/lock/lock_file.rs:18-34`** —
  `Lock` has `#[serde(rename = "plugin")]` over a
  `BTreeMap<CanonicalId, PluginEntry>`, plus `session:
  SessionTable`. `PluginEntry` requires `entry, digest,
  manifest_digest, granted_at`, with optional `grant`,
  `bindings`. Grants live under `[plugin."<canonical-id>".grant.bundles.default.*]`
  per the live fixture lock at
  `rafaello/fixtures/m5b-locks/rafaello.lock`.
- **`lockin/crates/sandbox/src/lib.rs:209-232`** —
  `resolve_syd_path` resolves `syd` via explicit `spec.syd_path`
  → `LOCKIN_SYD_PATH` → `PATH`. There is **no equivalent for
  `syd-pty`**; nothing yet plumbs it into the syd child
  command. `rg "CARGO_BIN_EXE_syd-pty|syd-pty|setup_pty"`
  across the lab repo confirms zero existing references.
- **`rafaello/nix/devenv.nix:7-9`** — exports
  `LOCKIN_SYD_PATH = "${pkgs.sydbox}/bin/syd"` on Linux. The
  devenv has no `CARGO_BIN_EXE_syd-pty` export today.
- **`rafaello/crates/rafaello-openai-stub/`** — the stub is its
  own crate; the binary is `bin/rfl_openai_stub.rs` inside
  this crate, not under `crates/rafaello-openai/`.
- **`crates/rafaello-core/src/audit/`** — the `AuditKind` enum
  with `as_str()` table is the authoritative list of audit
  kind families; m5b extended it per `decisions.md` rows 55 +
  58.

### From the §5 retrospectives (carryover punchlist — unchanged from round 1)

- m4 §5.5 / m5a §5 row 6 / m5b §5 row 13 — workspace
  `#[allow(clippy::result_large_err)]` sweep.
- m4 §5.8 / m5a §5 row 8 — `load.triggers.kind = "tool"`
  lazy-load not exercised.
- m5a §5 row 9 / m5b §5 row 9 — macOS CI green hard gate.
- m5a §5 row 10 / m5b §5 row 10 — interactive `rfl chat`
  recording.
- m5a §5 row 11 / m5b §5 row 11 — `manual-validation.md`
  skeleton fill / additions.
- m5a §5 row 14 / m5b §5 row 12 — `core_tools_list_registered_before_provider_spawn.rs`
  defence-in-depth regression anchor.
- m5b §5 row 1 — multi-turn `rfl-openai-stub` shape.
- m5b §5 row 8 — `rfl audit` read CLI.

Items m5a/m5b routed to v2 (NOT m6): §A9 narrowing, real-network
`rafaello-fetch`, substring-threshold tuning, LRU cap,
Aho-Corasick, laundered-flow taint. Recorded under §"Out of
scope" below.

---

## In scope

Grouped by phase. Phase letters **restored to match the
driver-preflight candidate shape**: A `rfl init`, B install UX,
C syd-pty discovery, D `rfl audit`, E multi-turn stub,
F `nix build` repair, G Homebrew, H README + CONTRIBUTING,
I coverage / regression anchors, J manual validation.

Commit-count estimate per phase feeds the `commits.md` budget.

### Package-placement invariant (referenced by Phase A + Phase B + Phase F)

**Invariant PP1 (round-3 B-1 fold).** Both `rfl init` and `rfl
install <plugin>` materialise the bundled package tree into
`${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/` where
`<topic-id> = topic_id::derive(canonical_id.to_string())` per
the live `rfl chat` resolver
(`crates/rafaello/src/lib.rs:235-244`) and `rfl install`
already-installed-plugin resolver
(`crates/rafaello/src/install.rs:160-169`). The manifest file
inside that directory is named **`rafaello.toml`** (canonical
name per `decisions.md` row 25) — not `manifest.toml`. The
directory layout is:

```
${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/
├── rafaello.toml           # bundled plugin manifest
├── openrpc.json            # sibling per decisions.md row 31
├── bin/
│   └── rfl-openai          # entry binary — REAL FILE (not symlink)
└── schemas/                # grant-match templates (if any)
    └── …
```

**PP1 containment invariant (round-4 B-1 fold).** The
`entry = "bin/rfl-openai"` field in the lock's `[plugin."…"]`
table resolves through `compile::resolve_entry`
(`crates/rafaello-core/src/compile.rs:440-465`) and
`validate_with_package::resolve_inside_package`
(`crates/rafaello-core/src/manifest/validate_with_package.rs:100-117`),
both of which **canonicalise the joined path and reject
anything whose canonical target escapes `package_dir`**
(`EntryEscape`). Therefore the binary inside the plugin's
`bin/` directory **must be an actual file**, not a symlink
into `<release-prefix>/bin/` or anywhere else outside the
plugin's package directory. The Phase F package output writes
the binary directly to
`$out/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`; the
PP1 copy step uses a recursive file copy (or
dereferencing-symlink copy if upstream Nix store layout
produces symlinks), never a symlink-preserving copy.

Top-level `<release-prefix>/bin/` is irrelevant for plugin
binaries; it carries only the user-facing entry points
**`rfl`** (the CLI binary) and **`rfl-tui`** (spawned as a
subprocess by `rfl chat` per `decisions.md` row 34). The
bundled plugin binaries — `rfl-openai`, `rfl-mailcat`,
`rfl-readfile`, `rfl-mockprovider`, `rafaello-fetch`,
`rfl-openai-stub` — live **only** inside their respective
`share/rafaello/plugins/<plugin>/bin/` directories.

The bundled package **source** lives in the release tree at
`<release-prefix>/share/rafaello/plugins/<plugin-name>/`
(Phase F lays this out). Cold-start (Phase A) and explicit
install (Phase B) **copy** that tree (dereferencing any
symlinks) into the project's `.rafaello/plugins/<topic-id>/`
so the live runtime resolver finds it. `digest` and
`manifest_digest` are computed over the **copied** tree so
subsequent `rfl chat` revalidation passes.

This invariant ties the entire install/runtime story together:
without it, Phase A writes a valid-looking lock but `rfl chat`
fails to find the manifest, or `compile::resolve_entry`
rejects the entry as `EntryEscape`. Pi round-2 B-1 surfaced
the missing copy step; round-3 B-1 surfaced the symlink
containment violation.

### Phase A — `rfl init` (≈4 commits)

Lands the cold-start command per hard requirement #1.

**A1 — CLI scaffold.** Extends `RflChatCommand` (live
`rafaello/crates/rafaello/src/lib.rs:57-69`) with `Init(InitArgs)`.
New `crates/rafaello/src/init.rs` module. Idempotent: re-running
over an existing `rafaello.lock` prints a one-line "lock
already present at <path>" notice and exits 0 (no overwrite,
no mid-script prompt). Flags:

- `--yes` — accepts the default grant non-interactively.
- `--force` — rewrites the lock byte-for-byte from defaults,
  dropping any user edits (owner-judgment item 7 — default:
  rewrite).
- `--project-root <path>` — matches the existing `rfl chat
  --project-root` ergonomics.

**A2 — `rfl-openai` default-entry materialisation against the
live lock schema.** Per `overview.md` §8.1 +
`decisions.md` row 38, the materialised lock matches the live
`Lock` shape (`lock_file.rs:18-34` + the m5b fixture template):

```toml
[plugin."builtin:openai@0.0.0"]
entry = "bin/rfl-openai"
digest = "sha256:<computed-at-init>"
manifest_digest = "sha256:<computed-at-init>"
granted_at = "<RFC3339 timestamp>"

[plugin."builtin:openai@0.0.0".grant]
subscribes = ["core.session.user_message", "core.session.tool_result"]
publishes = ["provider.openai.tool_request", "provider.openai.assistant_message"]

[plugin."builtin:openai@0.0.0".grant.bundles.default.network]
mode = "proxy"
allow_hosts = ["litellm.thepromisedlan.club"]

[plugin."builtin:openai@0.0.0".grant.bundles.default.env]
pass = ["LITELLM_API_KEY"]
allow_secrets = ["LITELLM_API_KEY"]

[plugin."builtin:openai@0.0.0".grant.bundles.default.env.set]
RFL_OPENAI_API_KEY_ENV = "LITELLM_API_KEY"
RFL_OPENAI_ENDPOINT_URL = "https://litellm.thepromisedlan.club/v1"
RFL_OPENAI_MODEL = "vllm/qwen3.6-27b"

[plugin."builtin:openai@0.0.0".bindings]
provider = true
provider_id = "openai"
load = "eager"

[session]
provider_active = "builtin:openai@0.0.0"
```

**A2 also materialises the bundled package tree** per
invariant PP1: copies `<release-prefix>/share/rafaello/plugins/rfl-openai/`
to `${PROJECT_ROOT}/.rafaello/plugins/<topic-id-of-openai>/`
with the manifest as `rafaello.toml`. `<topic-id-of-openai> =
topic_id::derive("builtin:openai@0.0.0")`. The `digest` /
`manifest_digest` fields written into the lock are the digests
of the **copied** tree (so subsequent `rfl chat` validation
matches).

The canonical id `builtin:openai@0.0.0` follows the live
`CanonicalId` `<source>:<name>@<version>` shape (lockin's m5b
fixture uses `builtin:` for the bundled provider). The
`digest` and `manifest_digest` are computed at `rfl init` time
from the copied tree — not pinned in
source (this mirrors how `rfl install --fixture` computes
digests today; see `install.rs`).

A2 lives or dies on `Lock::from_toml` accepting the generated
TOML. The A4 happy-path test asserts that round-tripping the
init lock through `Lock::from_toml → Lock::to_toml` is
byte-stable.

**A3 — install-time review prompt.** Prints the default grant
to stdout (one section per `grant.bundles.default.*` subtable)
and prompts "Proceed? [y/N]" unless `--yes`. Answering no at
the TTY prompt writes a lock **without** the `rfl-openai`
entry (empty `[plugin.…]` map, no
`[session].provider_active`) and skips the PP1 copy step, so
the user must later `rfl install` an alternate provider.
(Owner-judgment item 8 — default: empty-lock on decline.)

**A4 — tests.** Integration tests in
`rafaello/tests/rfl_init_*.rs`:

- `rfl_init_writes_default_lock.rs` — `--yes` happy path;
  asserts the generated TOML deserialises via `Lock::from_toml`
  and the round-trip is byte-stable.
- `rfl_init_materialises_package_dir.rs` (**round-3 B-1 +
  round-4 B-1**) — after `rfl init --yes`, asserts:
  - `${PROJECT_ROOT}/.rafaello/plugins/<topic-id-of-openai>/rafaello.toml`
    exists and parses via `Manifest::parse`;
  - `bin/rfl-openai` exists inside the same directory as a
    **regular file** (`std::fs::metadata(...).file_type().is_file()`
    is `true`, `is_symlink()` is `false`);
  - the lock's `digest` / `manifest_digest` fields match the
    digests of the copied tree (per PP1);
  - `compile::resolve_entry(&plugin_dir, "bin/rfl-openai")`
    returns `Ok(_)` with the canonical path inside
    `plugin_dir` (no `EntryEscape`).
- `rfl_init_idempotent_no_overwrite.rs` — second run leaves the
  lock byte-identical and exits 0; the materialised package
  dir is also unchanged.
- `rfl_init_force_rewrites.rs` — `--force` rewrites lock + the
  package dir from defaults.
- `rfl_init_decline_writes_empty_lock.rs` — declining the prompt
  writes an empty lock (no `[plugin.…]` entries, no
  `[session].provider_active`) and creates no
  `.rafaello/plugins/…` entries.

Glossary candidate: `rfl init`.

### Phase B — `rfl install <plugin>` UX (≈3 commits)

Closes pi B-2. Today `rfl install` requires `--fixture <path>`;
the README cannot ship a fixture-only command as the user
bootstrap, and the m6 demo's `rfl install rfl-mailcat` does not
work against live `InstallArgs`.

**B1 — positional plugin-id argument + optional `--fixture`.**
Extends `InstallArgs` (live
`crates/rafaello/src/install.rs:32-43`) with:

- `plugin: Option<String>` — positional plugin name (e.g.
  `rfl-mailcat`).
- `fixture: Option<PathBuf>` — **changed from required to
  Option** (round-3 M-5 fold). Clap semantics:
  `#[arg(long, conflicts_with = "plugin",
  required_unless_present = "plugin")]`. The positional
  `plugin` carries
  `#[arg(required_unless_present = "fixture",
  conflicts_with = "fixture")]`. Exactly one of the two must
  be supplied; setting both is a clap error before `run()`
  executes.
- `project_root: Option<PathBuf>` (round-4 B-2 fold) —
  matches the existing `rfl chat --project-root` and the new
  `rfl init --project-root` + `rfl audit --project-root`
  ergonomics. Defaults to `std::env::current_dir()`. Lets the
  J2 tmux script point every `rfl <subcommand>` at the same
  throwaway project from a single lab-worktree cwd.

Resolution order:

1. If `--fixture <path>` is set, current behaviour (m5a-
   ratified path, unchanged for fixture tests; the
   `package_dir` is the user-supplied directory and the
   manifest filename is `rafaello.toml` per
   `decisions.md` row 25 — same as live).
2. Else if `plugin` positional is set, resolve the **bundled
   source tree** at
   `<release-prefix>/share/rafaello/plugins/<plugin>/`
   (Phase F lays this out). The manifest filename inside that
   tree is **`rafaello.toml`** (canonical per `decisions.md`
   row 25). The resolution path is owner-judgment item 1 below.

In both arms, after the source `package_dir` is identified,
B1 **also copies the package tree** into
`${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/` per invariant
PP1 (round-3 B-1 fold); `digest` / `manifest_digest` are
computed over the copied tree. This unifies fixture-install
and positional-install runtime behaviour: in both cases
`rfl chat` resolves the plugin from `.rafaello/plugins/…/rafaello.toml`.

The release-layout resolution **requires** Phase F (the
expanded package output) — B1 lands the resolver but cites
the F-expanded layout it consumes; the B3 positional-install
integration test runs against a fixture release tree until F
lands.

**B2 — bundled-plugin source tree layout.** Each bundled plugin
(`rfl-mailcat`, `rfl-readfile`, `rafaello-fetch`,
`rfl-mockprovider`, `rfl-openai`) needs a discoverable
manifest + sidecar `openrpc.json` (per `decisions.md` row 31).
m5b shipped these inside the test fixture tree only; B2
promotes them to the release tree under
`<release-prefix>/share/rafaello/plugins/<name>/` with the
manifest named **`rafaello.toml`** (not `manifest.toml`) per
`decisions.md` row 25. Each plugin directory ships:

```
<release-prefix>/share/rafaello/plugins/<plugin>/
├── rafaello.toml
├── openrpc.json
├── bin/<plugin-bin>        # REAL FILE (round-4 B-1 — not symlink)
└── schemas/                # grant-match templates (if any)
```

Round-4 B-1 fold: `bin/<plugin-bin>` is an **actual file
inside the plugin's package directory**, not a symlink into
`<release-prefix>/bin/`. The `compile::resolve_entry` /
`validate_with_package::resolve_inside_package` containment
checks canonicalise the path and reject anything escaping
`package_dir`; a symlink whose target lives in
`<release-prefix>/bin/` would canonicalise out and trip
`EntryEscape`. The Phase F package output writes each plugin
binary directly into its plugin-specific
`share/rafaello/plugins/<plugin>/bin/` directory.

Top-level `<release-prefix>/bin/` carries only the
user-facing entry points: `rfl` (CLI) and `rfl-tui` (the
TUI subprocess spawned by `rfl chat` per `decisions.md` row
34). Plugin binaries do **not** live in `<release-prefix>/bin/`.

**B3 — tests.** Integration tests in
`rafaello/tests/rfl_install_positional_*.rs`:

- `rfl_install_positional_resolves_to_bundled_plugin.rs` —
  `rfl install rfl-mailcat` against a fixture release tree
  finds `share/rafaello/plugins/rfl-mailcat/rafaello.toml`,
  copies the package tree into
  `.rafaello/plugins/<topic-id>/` (PP1), and writes the lock
  entry. Asserts the post-install package dir exists.
- `rfl_install_fixture_flag_still_works.rs` — `--fixture <path>`
  path remains functional (m5a regression anchor); the
  package tree is also copied to `.rafaello/plugins/<topic-id>/`
  in the fixture arm.
- `rfl_install_positional_unknown_plugin_errors.rs` —
  `rfl install nonsense` exits non-zero with a "no bundled
  plugin named 'nonsense'" message; lock + package dir are
  unchanged.
- `rfl_install_requires_one_of_fixture_or_plugin.rs` (round-3
  M-5) — invoking `rfl install` with neither argument is a
  clap error; invoking with both is a clap error; both cases
  exit non-zero before `run()`.
- `rfl_install_project_root_flag.rs` (round-4 B-2) —
  `rfl install rfl-mailcat --project-root <tmpdir>` from a
  different cwd writes the lock + materialised package dir
  under `<tmpdir>/.rafaello/plugins/<topic-id>/`, not under
  the invoking cwd.
- `rfl_install_resolves_entry_against_canonicalised_package_dir.rs`
  (round-4 B-1) — after `rfl install rfl-mailcat` against a
  fixture release tree whose `bin/rfl-mailcat` is a real
  file, asserts `compile::resolve_entry(&plugin_dir,
  &manifest.entry)` returns `Ok(<canonicalised-path>)` and
  the canonicalised path lies inside the project's
  `.rafaello/plugins/<topic-id>/` (no `EntryEscape`).

Glossary candidate: `rfl install <plugin>` (positional argument
resolves against the bundled plugin tree).

### Phase C — syd-pty discovery fix (≈3 commits)

Per hard requirement #2 + pre-flight §"Hard requirement #1: the
syd-pty wall". Belt-and-braces approach (devshell export +
lockin sandbox plumbing). Pi B-6 raised the structural
weakness in round 1's "fallback to `sandbox/pty:off`" framing:
round 2 makes success require **actual `syd-pty` discovery on
the syd child command**; `pty:off` is recorded only as a
diagnostic residual-risk path, not a success criterion.

**C1 — devshell export of `CARGO_BIN_EXE_syd-pty`.**
`rafaello/nix/devenv.nix` exports `CARGO_BIN_EXE_syd-pty`
mirroring how it already exports `LOCKIN_SYD_PATH` (live at
line 7-9). The env var points at the same nix-store `syd-pty`
adjacent to the resolved `syd`. Covers the interactive
`rfl chat` case in the canonical devshell — but is **not**
sufficient on its own because Homebrew-installed `rfl` and
other future entrypoints never enter the rafaello devshell.

**C2 — lockin sandbox plumbing of `syd-pty` into the syd
child.** New function `resolve_syd_pty_path` in `lockin/
crates/sandbox/src/lib.rs` next to the existing
`resolve_syd_path` (live at `:209-232`). Resolution order:

1. `spec.syd_pty_path` explicit (analog of `spec.syd_path`).
2. `$CARGO_BIN_EXE_syd-pty` env var.
3. The directory containing the resolved `syd` (sibling lookup).
4. `PATH`.
5. Hard error — **no `pty:off` fallback** at this layer.

The resolved `syd-pty` path is set as `CARGO_BIN_EXE_syd-pty`
on the **syd child command** via `Command::env`. This is the
"solved at the right layer" requirement: lockin owns syd
child-command construction; the env var is set on the child,
not the rafaello process, so `direnv`/`nix develop` allowlist
filtering doesn't matter.

If round 2's "no `pty:off` fallback at the lockin layer" is
contentious (some operators may want a graceful-degradation
path), pi can argue back; default position is hard-error
because hard requirement #2 explicitly demands the right-layer
fix and a silent fallback would re-introduce the m5a wall.

The patch lands in **upstream lockin** (owner-judgment item 2
below — default: upstream lockin).

**C3 — regression tests.** Round-3 M-4 + round-4 M-1 fold:
introduce a **fake `syd` wrapper** Cargo binary at
`lockin/crates/sandbox/tests/bin/fake_syd.rs`, registered as a
test-only `[[bin]]` in `lockin/crates/sandbox/Cargo.toml`:

```toml
[[bin]]
name = "fake-syd"
path = "tests/bin/fake_syd.rs"
required-features = ["test-fixture"]
```

The `required-features = ["test-fixture"]` gate keeps the
fixture binary out of the default release build per the
project-wide `test-fixture` feature convention. Tests opt in
via `--features test-fixture` (already standard for the
rafaello test suite). The binary prints its `argv` and
`environ` to a sentinel file (path passed via
`RFL_FAKE_SYD_RECORD_PATH`), then `exec`s `true`. Integration
tests locate it via `env!("CARGO_BIN_EXE_fake-syd")` — the
same Cargo-injected env-var mechanism the live syd discovery
uses for `CARGO_BIN_EXE_syd-pty`.

Three tests in `lockin/crates/sandbox/tests/`:

- `fake_syd_records_cargo_bin_exe_env_when_set_explicitly.rs` —
  `spec.syd_pty_path = Some(<fixture-syd-pty-path>)`. Asserts
  the sentinel file contains
  `CARGO_BIN_EXE_syd-pty=<fixture-syd-pty-path>`.
- `fake_syd_records_cargo_bin_exe_env_from_sibling.rs` —
  spawning from a tempdir with `syd` and `syd-pty` placed
  side-by-side; `CARGO_BIN_EXE_syd-pty` env unset; no
  `spec.syd_pty_path`. Asserts the sentinel records
  `CARGO_BIN_EXE_syd-pty=<tempdir>/syd-pty` (sibling
  discovery arm).
- `fake_syd_resolution_fails_hard_when_pty_missing.rs` —
  spawning with `syd-pty` deliberately absent from all
  resolution paths. Asserts the sandbox build returns a
  typed `Err(SandboxError::SydPtyNotFound { … })`; no fallback
  to `pty:off`.

Plus one rafaello-side smoke test at
`rafaello/tests/rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty.rs`
(Linux + devshell-gated) — spawns `rfl chat` inside
`nix develop .#rafaello --impure --command` against the
`rfl-bus-fixture` plugin (real syd, real syd-pty) and asserts
the child plugin's recorded env contains
`CARGO_BIN_EXE_syd-pty=<absolute-path>`. The plugin's
recording mechanism: extend `rafaello-bus-fixture` with an
optional `--record-env <path>` mode (small, test-only) that
dumps `std::env::vars()` to a file before doing its normal
fixture work.

Round-3 framing: the tests assert `CARGO_BIN_EXE_syd-pty` was
**set on the syd child's environment with a specific value**,
not that the spawn succeeded silently. The fake-syd path
gives a mechanical proof without depending on a real PTY's
ANSI output or stderr-pattern absence.

Glossary candidate: `syd-pty discovery`.

### Phase D — `rfl audit` read CLI (≈3 commits)

m5b §5 row 8 routed this to m6. Round-2 rewrite against the
**live** `audit_events` schema (B-4 fix).

**D1 — `rfl audit` subcommand against live schema.** Extends
`RflChatCommand` with `Audit(AuditArgs)`. New
`crates/rafaello/src/audit_cli.rs` module. `AuditArgs`
includes `--project-root <path>` (round-3 B-2 fold) mirroring
the `rfl chat --project-root` + `rfl init --project-root`
ergonomics; defaults to `std::env::current_dir()`. Reads
`<project_root>/.rafaello/state/session.sqlite` `audit_events`
table whose live columns are `seq INTEGER PRIMARY KEY
AUTOINCREMENT, at TEXT NOT NULL, kind TEXT NOT NULL, request_id
TEXT, payload TEXT NOT NULL` (verified at `crates/rafaello-core/
src/audit/mod.rs:114-123`).

Default query: `SELECT seq, at, kind, request_id, payload FROM
audit_events ORDER BY seq ASC`. Rendered one row per line:

```
<seq>  <at>  <kind>  [<request_id>|-]  <payload-summary>
```

The `<payload-summary>` is the JSON `payload` truncated to ~80
columns; `--full` disables truncation.

**D2 — filter flags.**
- `--kind <kind>` (repeatable) — filters by `audit_events.kind`.
  The kind list is the live `AuditKind::as_str()` enum (m5b
  extended to include `confirm_request_taint_attached` per
  `decisions.md` row 58 and `plugin_publish_rejected_taint_superset`
  / `tool_request_taint_unioned_from_in_reply_to` per
  `decisions.md` rows 55–57; see `crates/rafaello-core/src/audit/
  mod.rs::AuditKind` for the authoritative list).
- `--since <duration>` — filters `at >= now() - duration`
  (parsed as `1h`/`30m`/`24h`).
- `--request-id <id>` — filters `audit_events.request_id = <id>`.
  Round-2 **drops** the round-1 `entries.call_id` join (B-4):
  the live `entries` schema (verified at
  `crates/rafaello-core/src/session/mod.rs:99-122`) has columns
  `id, seq, parent, kind, schema, payload, metadata, fallback,
  created_at` and **no `call_id` column**. A join against
  `entries` for full provenance is a follow-up that requires
  either a schema migration (add `call_id`) or a JSON-extract
  query over `entries.payload` for the embedded
  `request_id`/`call_id` fields; either is out of m6 scope.
- `--json` — emits one JSON object per row.
- `--full` — disables payload-summary truncation.

**D3 — tests.** Integration tests in
`rafaello/tests/rfl_audit_*.rs`:

- `rfl_audit_lists_all_rows_from_live_schema.rs` — opens an
  audit DB populated via `AuditWriter::open_for_install` +
  `record(AuditKind::*, …)` (the existing m5a/m5b helpers).
  Asserts the rendered output's row count + the first row's
  column order matches the spec.
- `rfl_audit_filters_by_kind.rs` — exercises `--kind`
  (repeatable).
- `rfl_audit_project_root_flag.rs` (round-3 B-2) — exercises
  `--project-root <path>`: populates an audit DB under
  `<tmpdir>/.rafaello/state/session.sqlite`, runs
  `rfl audit --project-root <tmpdir>` from a different cwd,
  asserts the output is the same as running with cwd set to
  `<tmpdir>`.
- `rfl_audit_filters_by_request_id_no_join.rs` — exercises
  `--request-id` and explicitly asserts the query path does
  NOT touch the `entries` table.
- `rfl_audit_filters_by_since.rs` — exercises `--since 1h`.
- `rfl_audit_empty_db.rs` — fresh `rfl init` lock; `rfl audit`
  exits 0 with a "no audit events" banner.
- `rfl_audit_json_emits_one_object_per_row.rs` — exercises
  `--json`.

Glossary candidate: `rfl audit`.

### Phase E — multi-turn `rfl-openai-stub` shape (≈2 commits)

m5b §5 row 1.

**E1 — scripted-turns env var + TOML schema.** Extends
`rafaello/crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`
(M-2 fix — pi-1 round-1 named the wrong crate) with
`RFL_OPENAI_STUB_SCRIPTED_TURNS = <path-to-toml>`. Backward-
compat: the existing single-turn env (m5a/m5b) still works
when the new env is unset. The TOML schema:

```toml
[[turn]]
match_user_message = "send <recipient> a hello note"
emit = "tool_call"
tool_name = "send-mail"
tool_args = { to = "<recipient>", subject = "hello", body = "hi" }

[[turn]]
match_in_reply_to = "<previous tool_request id>"
emit = "assistant_message"
content = "Done — mail to <recipient> sent."
```

The stub walks the turn list in order; first matching turn
fires; row marked consumed; exhaustion is a **hard panic**
(mirrors m5b multi-answer-hook semantics — `decisions.md` row
56). Mutual exclusion with the singular env is a stub startup
error.

**E2 — tests.** Unit tests in
`crates/rafaello-openai-stub/tests/rfl_openai_stub_scripted_turns.rs`:

- two-turn happy path (the send-mail flow above);
- exhaustion panics deterministically;
- `match_in_reply_to` plumbs the correct correlation id from
  the previous canonical `tool_request`;
- mutual-exclusion error fires when both envs are set.

Glossary candidate: `rfl-openai-stub scripted turns`.

### Phase F — `nix build .#rafaello` package repair (≈3 commits)

Per roadmap row + pi B-1 reframe. The output already exists
(`flake.nix:71-73` → `rafaello/nix/default.nix:10` →
`rafaello/nix/package.nix:16`); the gap is that
`cargoBuildFlags = [ "-p" "rafaello" ]` builds only the `rfl`
binary, so consumers like the Homebrew formula (Phase G) have
nothing to install for `rfl-openai`, `rfl-tui`, etc.

**F1 — expand `cargoBuildFlags` to the package build set.**
(Round-4 N-1 rename: "package build set" is the list of
Cargo **package** names passed via `-p` flags;
"release binary set" in the acceptance summary names the
**installed binaries** produced by those packages. The two
lists are 1:1 in m6 but conceptually distinct — Cargo
packages can produce multiple binaries; here each does
exactly one.)

The **package build set** (round-3 M-2 reconciliation;
`rafaello-bus-fixture` is test-shaped and **excluded** per
owner-judgment item 9 default):

- `rafaello`            → installs `rfl`
- `rafaello-tui`        → installs `rfl-tui`
- `rafaello-openai`     → installs `rfl-openai`
- `rafaello-openai-stub` → installs `rfl-openai-stub`
  *(included for `manual-validation.md` scripted demo +
  integration tests; owner-judgment item 13)*
- `rafaello-readfile`   → installs `rfl-readfile`
- `rafaello-mailcat`    → installs `rfl-mailcat`
- `rafaello-mockprovider` → installs `rfl-mockprovider`
- `rafaello-fetch`      → installs `rafaello-fetch`

Replace the live `[ "-p" "rafaello" ]` in
`rafaello/nix/package.nix:16` with the eight-package list
above. The bus-fixture crate stays inside the test tree and
never ships to end users.

**F2 — bundled plugin tree in the package output.** Phase B2
promotes the bundled plugin source trees to
`<release-prefix>/share/rafaello/plugins/<plugin>/` with the
manifest named **`rafaello.toml`** (per `decisions.md` row 25).
F2 plumbs them through `package.nix`'s `postInstall` step:

- Copies each plugin's manifest tree
  (`rafaello/crates/<plugin>/rafaello.toml`, the sibling
  `openrpc.json` per `decisions.md` row 31, and any
  `schemas/` directory) to
  `$out/share/rafaello/plugins/<plugin>/`.
- **Moves** each plugin binary (built by Cargo into
  `$out/bin/<bin>` per the standard `buildRustPackage`
  layout) into its plugin-specific
  `$out/share/rafaello/plugins/<plugin>/bin/<bin>` directory
  as an **actual file** (round-4 B-1 fold). Plugin binaries
  are **removed** from `$out/bin/`.
- Leaves only `$out/bin/rfl` and `$out/bin/rfl-tui` at the
  top level — the two user-facing entry points (`rfl` is
  the CLI; `rfl-tui` is the subprocess spawned by `rfl chat`
  per `decisions.md` row 34).

This ensures `compile::resolve_entry` /
`validate_with_package::resolve_inside_package` containment
checks pass against the plugin's own package directory after
the PP1 copy step (the binary's canonical path stays inside
the plugin dir; no symlink target escapes).

**F3 — CI matrix coverage.** GitHub Actions extends with a
`nix build .#rafaello` job on `ubuntu-latest` and
`macos-latest`. The macOS leg is the m4/m5a/m5b precedent
ratification gate. Per the m2 §5.7 lesson, the Phase C
syd-pty fix exercises against this CI matrix during m6
implementation, not at retrospective time.

### Phase G — Homebrew distribution (≈3 commits; default G.β is actionable)

Per roadmap row. Round-3 M-1 fold: **G.β is the actionable
default**; `commits.md` proceeds on the G.β shape without
waiting for owner reply. The owner may still pick G.α or G.γ
at ratification, in which case `commits.md` for Phase G
re-shapes accordingly (model-specific commit counts noted
below). Owner-judgment item 5 records the alternatives.

**Three distribution-model alternatives** (each owner-pickable
at item 5):

- **G.α — in-repo formula that builds from source via Cargo.**
  `homebrew/rafaello.rb` declares `depends_on "rust"` and runs
  `cargo install --path .` (or per-bin). Tap is in-repo;
  install path is the standard Homebrew prefix.
  - Pros: matches "formula matching scope/tempo" verbatim; no
    second repo; works on any macOS with the toolchain.
  - Cons: build time on user's machine; duplicates the
    Phase F Nix build with a Cargo build; user must keep
    a Rust toolchain installed.

- **G.β — separate `homebrew-rafaello` tap fetching Nix-built
  release tarballs.** Phase F output is packaged as a `.tar.gz`
  release artefact per arch; the tap's formula `url`-fetches
  the matching artefact and untars into the prefix.
  Architectures (round-3 M-3 narrowing): **`aarch64-darwin`,
  `aarch64-linux`, `x86_64-linux`** matching `flake.nix:24-28`'s
  `systems`. `x86_64-darwin` is **out of scope for m6** (no
  Nix builder for that arch in the root flake); see §"Out of
  scope" + the v2-expansion note in owner-judgment item 5.
  - Pros: zero-build install for users; reuses Phase F output;
    one canonical build path; matches typical pre-built
    formula model.
  - Cons: requires a release-tag automation step (CI job that
    runs `nix build` per arch + uploads to a GitHub release);
    separate tap repo to maintain; intel Mac users have no
    `brew install` path until v2 adds `x86_64-darwin`.

- **G.γ — Nix-based install script, NOT a Homebrew formula.**
  `scripts/install.sh` invokes `nix run github:luizribeiro/lab#rafaello`
  or builds via the flake. The README documents this as the
  install path; no Homebrew tap exists.
  - Pros: no duplication with Nix; honest about what backs the
    build.
  - Cons: roadmap row explicitly asks for a "Homebrew formula";
    G.γ trades a literal roadmap-row deliverable for less
    duplication. Pi B-7 flags this as a valid option but
    requires the roadmap text to acknowledge the trade.

**Default — G.β** (separate-tap fetching Nix-built tarballs).
Rationale: minimises duplication with the Phase F Nix build
(the tap consumes the same artefacts); feels like a Homebrew
formula to the user (no `nix` toolchain required on the user's
machine; clean `brew install` UX). The cost is the release-tag
automation (one CI job). `commits.md` drafts against this
default; owner can flip to G.α/G.γ at ratification.

**G.β commits** (the actionable default):

**G1 — `homebrew-rafaello` tap scaffold.** A new repo
`luizribeiro/homebrew-rafaello` with `Formula/rafaello.rb`.
The formula declares `url` and `sha256` for the per-arch
tarballs produced by G2, installs every release binary (the
F1 list) under `<prefix>/bin/`, and the bundled plugin tree
under `<prefix>/share/rafaello/plugins/<plugin>/` matching the
`rfl install <plugin>` discovery path (B1 resolution arm 2).
The tap creation itself is a one-time owner action recorded in
`manual-validation.md` §G; the formula file is committed in
the m6 branch as a fixture under `homebrew/rafaello.rb`
(symlinked into the tap repo after owner creates it).

**G2 — release-tag automation.** A GitHub Action triggered on
`v*` tags that runs `nix build .#rafaello` for each of the
three arches in `flake.nix:24-28`'s `systems`
(`aarch64-darwin`, `aarch64-linux`, `x86_64-linux`), packages
each output as `rafaello-<version>-<arch>.tar.gz`, and uploads
to the GitHub release. Lives at
`.github/workflows/rafaello-release.yml`. The aarch64-darwin
build runs on the `macos-14` (Apple-silicon) runner per
GitHub Actions' available macOS runners.

**G3 — install smoke test.** A manual-validation entry in
Phase J §G documents the clean macOS arm64 shell flow:
`brew tap luizribeiro/rafaello && brew install rafaello &&
rfl init && rfl install rfl-mailcat && rfl chat`.
Owner-judgment item 10 below asks whether a macOS-CI workflow
exercises `brew install` itself (default: manual validation
only).

**Model-specific commit counts** (for `commits.md` re-shaping
if owner flips item 5): G.α → 2 commits (no release
automation; in-repo formula + Cargo build flags doc);
G.β → 3 commits (the default above); G.γ → 1 commit
(`scripts/install.sh` + README banner).

### Phase H — README + CONTRIBUTING pass (≈2 commits)

Per roadmap row + hard requirements #4 + #5. (Round 1 had this
as Phase G; round-2 restores the pre-flight ordering at
Phase H — M-4 fix.)

**H1 — `rafaello/README.md` rewrite.** Replace the placeholder
with:

- One-paragraph project summary.
- The 5-line bootstrap snippet:

      cd ~/your/project
      nix develop .#rafaello --impure --command rfl init
      export LITELLM_API_KEY=…
      nix develop .#rafaello --impure --command rfl install rfl-mailcat
      nix develop .#rafaello --impure --command rfl chat

- Architecture-at-a-glance pointer to `plans/overview.md`.
- A **Troubleshooting** section. Primary remediation: "make
  sure you're inside `nix develop .#rafaello --impure` (which
  exports `CARGO_BIN_EXE_syd-pty`), or install the m6-or-newer
  release that ships the lockin sandbox `syd-pty` discovery
  fix." A separate **Pre-m6 workaround** subsection documents
  the manual `CARGO_BIN_EXE_syd-pty=$(which syd-pty)` recipe
  with a clear "use only against pre-m6 builds — m6+ does not
  need this" banner (N-2 framing).
- Installation instructions covering both the Nix flake path
  and the Homebrew path (chosen model per Phase G).

**H2 — `CONTRIBUTING.md` rewrite.** Replace the placeholder
with: dev-shell entry instructions
(`nix develop .#rafaello --impure`), the milestone / plans /
streams structure (one paragraph), the per-commit
code-reviewer agent expectation per `~/.claude/CLAUDE.md`, and
the rebase-no-force branch model (`decisions.md` row 33).

### Phase I — Coverage / regression-anchor sweep (≈3 commits)

Carryover backlog from m4/m5a/m5b §5.

**I1 — `core_tools_list_registered_before_provider_spawn.rs`**
(m5a §5 row 14 / m5b §5 row 12). Defence-in-depth regression
anchor for `decisions.md` row 49. Lives at
`crates/rafaello-core/tests/core_tools_list_registered_before_provider_spawn.rs`.
Conditional on whether m5b's c38c sequence landed it
(owner-judgment item 11; round 2 default: assume m5b did NOT
land it and m6 owes the test — the pre-flight calls it a m6
carryover unchanged).

**I2 — `load.triggers.kind = "tool"` lazy-load coverage**
(m4 §5.8 / m5a §5 row 8). The manifest field is plumbed but
never exercised end-to-end. m6 lands a fixture lock that uses
the trigger + an integration test at
`rafaello/tests/lazy_load_tool_trigger_spawns_on_first_call.rs`.
Owner-judgment item 6 — default: in scope.

**I3 — `#[allow(clippy::result_large_err)]` ratification
sweep** (m4 §5.5 / m5a §5 row 6 / m5b §5 row 13). Round-2 fix
of M-5: the **default-selected position is to ratify the
module-level allows** by **keeping them in place** + adding a
`decisions.md` row that explains the trade-off (1 commit,
edits comments next to the allows to point at the decision row,
does NOT delete the allows). Alternative: box `ReemitError` +
`AgentLoopError` (5+ commits, ripples through every `?` site).
Owner-judgment item 8 (renumbered from round 1's item 7) makes
this explicit.

### Phase J — Manual-validation transcript (≈2 commits + 1 retro)

Per hard requirement #3 + roadmap row demo. **The canonical
proof of life for v1.** Round-2 makes the tmux steps concrete
per pi B-3.

**J1 — `manual-validation.md` skeleton fill.** Extend the
existing 9-line m5b c15 file with seven sections:

- §1 — `rfl chat` cold-start walkthrough (the 5-line
  bootstrap, post-init).
- §2 — `rfl install rfl-mailcat` walkthrough (positional-arg
  shape from Phase B; declares the `send-mail` tool with
  `sinks = ["mail"]`).
- §3 — wire-shape note (preserved from m5b c15).
- §4 — macOS CI run URL (driver post-merge sweep — m5a §5 row
  9 / m5b §5 row 9).
- §5 — **the tmux-driven interactive recording** (see J2).
- §6 — audit-log inspection walkthrough using the new
  `rfl audit` CLI (Phase D).
- §7 — syd-pty failure-mode reproduction + the fix
  verification (hard requirement #5's "documented for
  posterity" half).
- §G — Homebrew install smoke (Phase G — model-dependent
  copy).

**J2 — the tmux-driven §5 recording.** Concrete tmux steps
(round-3 B-2 rewrite against live TUI overlay copy at
`rafaello-tui/src/confirm.rs:160-211` and gate-built summary
`<tool> via <plugin> — sinks: [<classes>]` at
`crates/rafaello-core/src/gate/mod.rs:374-378`):

```sh
# Round-4 B-2 fold: every `rfl <subcommand>` invocation runs
# from inside the lab worktree (where `.#rafaello` resolves)
# and points at the throwaway PROJECT via `--project-root`.

LAB_WORKTREE=/home/luiz/lab            # or current rafaello-v0.1 worktree
PROJECT=$(mktemp -d -t m6-demo-XXXX)
TRANSCRIPTS="$PROJECT/transcripts/section-5"
mkdir -p "$TRANSCRIPTS"

cd "$LAB_WORKTREE"

# Materialise the lock + bundled rfl-openai under $PROJECT.
nix develop .#rafaello --impure --command \
  rfl init --yes --project-root "$PROJECT"

export LITELLM_API_KEY=<dev-proxy-key-from-pass>

# Install rfl-mailcat under the same $PROJECT.
nix develop .#rafaello --impure --command \
  rfl install rfl-mailcat --project-root "$PROJECT"

# Open the tmux session that hosts rfl chat against $PROJECT.
tmux new-session -d -s rafaello-m6-demo \
  "cd '$LAB_WORKTREE' && nix develop .#rafaello --impure --command \
     rfl chat --project-root '$PROJECT'"

# Wait for the TUI to render the initial pane.
sleep 2
tmux capture-pane -t rafaello-m6-demo -p \
  > "$TRANSCRIPTS/01-after-launch.txt"

# Send a prompt that triggers the send-mail tool.
tmux send-keys -t rafaello-m6-demo \
  "Please email alice@example.com a one-line hello note." Enter
sleep 3
tmux capture-pane -t rafaello-m6-demo -p \
  > "$TRANSCRIPTS/02-modal.txt"
# Live overlay copy (confirm.rs:160-211):
#   - title border:  " confirm "
#   - summary line:  "send-mail via local:mailcat@0.0.0 — sinks: [mail]"
#   - args line:     "args: { … "alice@example.com" … }"
#   - sinks line:    "sinks: mail"
grep " confirm "          "$TRANSCRIPTS/02-modal.txt"
grep "send-mail via"      "$TRANSCRIPTS/02-modal.txt"
grep "sinks: mail"        "$TRANSCRIPTS/02-modal.txt"
grep "alice@example.com"  "$TRANSCRIPTS/02-modal.txt"

# Allow the call. Live binding (rafaello-tui/src/lib.rs:70):
#   KeyCode::Char('y') | KeyCode::Char('a') | KeyCode::Enter => Allow.
tmux send-keys -t rafaello-m6-demo "a"
sleep 3
tmux capture-pane -t rafaello-m6-demo -p \
  > "$TRANSCRIPTS/03-response.txt"
# The assistant-message line from the live LiteLLM model (or the
# multi-turn stub if running deterministically) acknowledges the
# send. Operator pastes the rendered line; no fixed substring
# asserted because real-model wording is non-deterministic.

# Quit cleanly. The m3 chat loop's quit binding is Ctrl-C
# (the TUI's input-mode loop doesn't bind 'q' as quit — see
# rafaello-tui/src/lib.rs input handling). Owner-judgment item
# 12 confirms the live binding at implementation time.
tmux send-keys -t rafaello-m6-demo "C-c"
sleep 1
tmux kill-session -t rafaello-m6-demo

# Audit + SQLite dumps.
nix develop .#rafaello --impure --command \
  rfl audit --project-root "$PROJECT" \
  > "$TRANSCRIPTS/04-audit.txt"
grep "confirm_request"   "$TRANSCRIPTS/04-audit.txt"
grep "confirm_allowed"   "$TRANSCRIPTS/04-audit.txt"

sqlite3 "$PROJECT/.rafaello/state/session.sqlite" \
  "SELECT seq, kind, request_id FROM audit_events ORDER BY seq" \
  > "$TRANSCRIPTS/05-sqlite-audit.txt"
sqlite3 "$PROJECT/.rafaello/state/session.sqlite" \
  "SELECT seq, kind FROM entries ORDER BY seq" \
  > "$TRANSCRIPTS/06-sqlite-entries.txt"

# Round-4 B-2: final copy of captured transcripts into the
# in-repo committed location.
REPO_TRANSCRIPTS="$LAB_WORKTREE/rafaello/plans/milestones/m6-polish-release/transcripts/section-5"
mkdir -p "$REPO_TRANSCRIPTS"
cp "$TRANSCRIPTS"/*.txt "$REPO_TRANSCRIPTS/"
```

Allow key (`a`) matches the live binding at
`rafaello-tui/src/lib.rs:70`. The quit binding for the TUI's
chat loop is `Ctrl-C` (round-3 correction — round 2's `q`
assumption was incorrect; `q` is not bound as quit in the
input-mode handler). Owner-judgment item 12 records the
implementation-time verification gate.

The six captured files
(`01-after-launch.txt`, `02-modal.txt`, `03-response.txt`,
`04-audit.txt`, `05-sqlite-audit.txt`, `06-sqlite-entries.txt`)
land under
`milestones/m6-polish-release/transcripts/section-5/`
(round-3 N-3 — replaces the Unicode `§5` path component with
ASCII-safe `section-5` for shell-quoting safety) and are
referenced from `manual-validation.md` §5. The transcript
files are the only required evidence (N-4: no asciinema; tmux
capture-pane outputs only).

**J3 — retrospective + decisions row appends + glossary
additions** (≈1 commit, **reserved in the budget** per N-3
fix — round 1 said "not counted"; round 2 reserves the
explicit retrospective commit because m6 cannot ratify without
it).

Pre-emptive `decisions.md` row candidates (placeholders;
actual numbers assigned at retrospective ratification time
per the append-only convention; current row tail is 58, so
m6 retro begins at row 59):

- row **59** — Roadmap-text reconciliation: m6 reads "install
  rfl-openai" as `rfl init` pre-installs the bundled provider;
  the operator's `rfl install` step is for tool plugins
  (refines `decisions.md` row 38 + the m6 roadmap row in
  `milestones/README.md`).
- row **60** — `rfl init` materialises the bundled `rfl-openai`
  lock entry against the dev-environment LiteLLM endpoint
  (canonical id `builtin:openai@0.0.0`); declining the prompt
  writes an empty lock.
- row **61** — `rfl install <plugin>` positional argument
  resolves against the bundled plugin tree at
  `share/rafaello/plugins/<plugin>/rafaello.toml` (refines
  `decisions.md` row 31 by pinning the discovery path).
- row **62** — Syd-pty discovery belt-and-braces: devshell
  exports `CARGO_BIN_EXE_syd-pty`; lockin sandbox resolves
  `syd-pty` adjacent to `syd` and sets the env var on the syd
  child command; no `pty:off` fallback at the lockin layer.
- row **63** — `rfl audit` read CLI semantics: default
  ordering, filter flag set, output format, JSON mode,
  no-join-against-entries (the m6 ratification of the audit-CLI
  contract).
- row **64** — `rfl-openai-stub` scripted-turns env-var
  (`RFL_OPENAI_STUB_SCRIPTED_TURNS`) + TOML schema +
  exhaustion-panics-deterministically + mutual-exclusion with
  the singular env.
- row **65** — `nix build .#rafaello` package output ships
  every workspace binary + the bundled plugin manifest tree;
  macOS CI green is the ratification gate (formalises the
  m3/m4/m5a/m5b precedent).
- row **66** — Homebrew distribution model (G.α / G.β / G.γ
  per owner-judgment item 5 final call).
- row **67** — `result_large_err` ratification: module-level
  `#![allow(clippy::result_large_err)]` retained on
  `reemit/mod.rs`, `agent/mod.rs`, plus any m5b sites;
  boxing the error hierarchy is post-v1 (refines the m4
  retro §5.5 punchlist; M-5 round-2 fix.).
- row **68** — m6 RATIFICATION closes `rafaello-v0.1 → main`
  merge; v1 demo-ready. Closes the v1 path opened by row 33.

---

## Out of scope

1. **§A9 superset narrowing** on `provider.<id>.assistant_message`,
   `frontend.<id>.confirm_answer`, `plugin.<a>.rpc_reply` (m5b
   §5 row 2; v2).
2. **Real-network `rafaello-fetch`** (m5b §5 row 3; post-v1).
3. **Substring-threshold tuning / Aho-Corasick** (m5b §5 rows
   4 + 6; v2).
4. **`TaintMatchMap` LRU cap** (m5b §5 row 5; v2).
5. **Laundered-flow taint / CaMeL** (m5b §5 row 7; v2).
6. **Helper plugins / external attach / patch ops / subprocess
   renderers** (`decisions.md` rows 26–29; design-phase
   deferrals).
7. **Public `rfl serve` (any flavour)** (`decisions.md` row 34;
   v2).
8. **`rfl provider tool <plugin>` CLI** (m5a §5 row 7; v2).
9. **`rfl grant` / `rfl revoke` / `rfl provider use` /
   `rfl update` as top-level CLI subcommands.** Slash-command
   equivalents exist in-session per m5a; the top-level CLI
   forms are not in the live `RflChatCommand` and m6 does not
   add them (M-1 correction; defer to v2 with the rest of the
   non-chat CLI surface).
10. **`rfl audit --request-id` cross-table join.** The live
    `entries` schema has no `call_id` column; m6 ships
    `--request-id` as a single-table filter on
    `audit_events.request_id`. A full provenance join is a
    v2 feature requiring a schema migration.
11. **`rfl init` reconfiguration UX.** m6 ships init but no
    `rfl init --endpoint=<url>` style override; operators on a
    non-LiteLLM deployment edit the lock manually for v1.
12. **Renderer-shape extensions, new built-in renderer kinds,
    new tool plugins.** Per roadmap row's "No opportunistic
    new tools": only the m4 (`read-file`), m5a (`send-mail`),
    m5b (`web-fetch`) tools are in the workspace; no new ones
    land in m6.
13. **Non-test workspace `#[allow(...)]` audits beyond
    `result_large_err`.**
14. **macOS-CI Homebrew install workflow.** Owner-judgment
    item 10 defaults to manual validation only; CI coverage
    of `brew install` itself is post-v1.
15. **`x86_64-darwin` Homebrew artefacts.** The root flake's
    `systems = [aarch64-darwin, aarch64-linux, x86_64-linux]`
    has no `x86_64-darwin` builder. m6 ships Homebrew tarballs
    for `aarch64-darwin` only; intel Mac users have no
    `brew install` path until v2 adds the arch. Round-3 M-3
    fold.

---

## Demo bar

Per `milestones/README.md` §"Demo bar per milestone": positive
+ negative tests, plus a `manual-validation.md` entry.

### Headline integrated demo (positive)

The 5-line bootstrap → `rfl chat` → `send-mail` tool call →
confirm modal → response → persist. **Single canonical tool:
`rfl-mailcat` declaring `send-mail` with `sinks = ["mail"]`**
(B-3 lock-in). Captured as both:

- **An integration test** at
  `rafaello/tests/rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`.
  Uses `rfl-openai-stub` (Phase E multi-turn) + the
  `RFL_TUI_TEST_CONFIRM_ANSWERS` hook (`decisions.md` row 56)
  to drive the flow deterministically:
  - `rfl init --yes` writes the lock with `rfl-openai`
    pre-installed.
  - `rfl install rfl-mailcat` (Phase B positional shape)
    appends the mailcat plugin entry to the lock.
  - `rfl chat` spawns; the stub scripts turn 1 (assistant
    proposes `send-mail`), the modal fires, the hook allows;
    the stub scripts turn 2 (assistant acknowledges).
  - Assertions: the `entries` SQLite table has the canonical
    `tool_call` + `tool_result` + assistant-message rows;
    the `audit_events` table has the `confirm_request` +
    `confirm_allowed` rows; the chat process exits cleanly on
    `Ctrl-C` (round-3 J2 correction).
- **The §5 tmux-driven recording** in `manual-validation.md`
  (Phase J2 — concrete tmux steps above). Same flow, run by
  hand against the LiteLLM proxy with the real `rfl-openai`
  plugin and the real `rfl-mailcat` tool plugin.

### Defensive negatives added by m6

- `rfl_init_idempotent_no_overwrite.rs` (Phase A4).
- `rfl_init_decline_writes_empty_lock.rs` (Phase A4).
- `rfl_install_positional_unknown_plugin_errors.rs` (Phase B3).
- `fake_syd_records_cargo_bin_exe_env_when_set_explicitly.rs`
  + `fake_syd_records_cargo_bin_exe_env_from_sibling.rs`
  + `fake_syd_resolution_fails_hard_when_pty_missing.rs`
  (Phase C3, round-3 M-4) — assert the syd child got
  `CARGO_BIN_EXE_syd-pty` set, **not** `pty:off` fallback.
  Plus the rafaello-side smoke test
  `rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty.rs`.
- `rfl_audit_empty_db.rs` (Phase D3).
- `rfl_audit_filters_by_request_id_no_join.rs` (Phase D3) —
  asserts the query path does not touch `entries`.
- `rfl_openai_stub_scripted_turns_panics_on_exhaustion.rs`
  (Phase E2).

m6 inherits the m5a/m5b security negatives unchanged; no new
security negatives are added.

### macOS CI green

Hard ratification gate per m3/m4/m5a/m5b precedent. The macOS
leg in Phase F3 runs `cargo test --workspace --features
test-fixture` + `nix build .#rafaello` and must be green. The
only exception is tests gated `#[cfg(target_os = "linux")]`.

---

## Single milestone vs split (owner-judgment item 0)

Pre-flight + round 1 + round 2 all converge: **single
milestone**, with an m6a/m6b ergonomics-vs-release fold-line
documented as a fallback. The headline demo is the integrated
flow; splitting forces the headline recording into m6b's
retrospective only, and the headline demo IS what
v1-demo-readiness is.

Fallback fold-line if pi pushes back at round 3+:

- **m6a — ergonomics** (Phases A, B, C, D, E, I, J).
  First-chat-from-cold works; install UX shipped; audit CLI
  lands; transcript captured; regression anchors closed.
- **m6b — release** (Phases F, G, H). `nix build` green on
  Linux + macOS; Homebrew works; README + CONTRIBUTING
  shipped. m6b is the milestone that closes the
  `rafaello-v0.1 → main` merge.

---

## Owner-judgment items

Numbered list — each item has a default-selected position; the
owner may override at convergence-round cost. **Round 3: 14
items, numbered 0–13** (round 2 had 13; round-3 N-1 corrected
the off-by-one count, and added new item 13 for the
`rafaello-openai-stub` release-bin inclusion question — see
F1).

0. **Single milestone vs m6a/m6b split.** Default: **single
   milestone**.

1. **`rfl install <plugin>` discovery path for bundled
   plugins** (B1 / B2). Default:
   `<release-prefix>/share/rafaello/plugins/<plugin>/rafaello.toml`
   (manifest filename per `decisions.md` row 25; round-3 B-1
   fold corrects round-2's `manifest.toml`). Alternative:
   `$out/libexec/rafaello/plugins/<plugin>/` (FHS-style).
   Round-3 default matches Homebrew + Nix convention for
   shared data.

2. **B2 lockin sandbox patch landing layer.** Default:
   **upstream lockin** (clean fix at the layer that owns the
   spawn). Alternative: rafaello-local lockin patch (faster to
   land, creates a fork of lockin to maintain). Round 2
   defaults to upstream.

3. **syd-pty discovery fix layers** (C1 + C2). Default:
   **belt-and-braces** (devshell export + lockin sandbox
   plumbing). Alternative A: C1 only — doesn't cover
   Homebrew-installed `rfl`. Alternative B: C2 only — defers
   devshell visibility. Pre-flight defaults to
   belt-and-braces; round 2 inherits.

4. **`pty:off` fallback policy at the lockin layer** (C2).
   Default: **no fallback** — hard error when `syd-pty`
   discovery fails, since hard requirement #2 demands the
   right-layer fix and a silent fallback would re-introduce
   the m5a wall. Alternative: graceful-degradation `pty:off`
   with a loud stderr warning (cheaper for operators on
   exotic deploys, but the C3 success criterion has to be
   "no warning fired" rather than "syd-pty resolved"). Round 2
   defaults to no-fallback per pi B-6.

5. **Homebrew distribution model** (Phase G). Default: **G.β —
   separate `homebrew-rafaello` tap that fetches Nix-built
   release tarballs**, arches `aarch64-darwin / aarch64-linux /
   x86_64-linux` (round-3 M-3 narrowing). Alternative A: G.α
   — in-repo formula that builds from source via Cargo (no
   Nix dependency for users, but duplicates the Phase F build).
   Alternative B: G.γ — Nix-based install script not labelled
   as a Homebrew formula (trades roadmap-row literalness for
   less duplication). Round-3 M-1 fold: `commits.md` proceeds
   on G.β without waiting for owner reply; owner may flip at
   ratification. **`x86_64-darwin` is a v2 expansion** if intel
   Mac demand surfaces — adding it requires a new flake system
   + a macOS-13 CI builder; the v1 budget excludes it.

6. **`load.triggers.kind = "tool"` lazy-load coverage**
   (Phase I2). Default: **in scope** (closes the m4-since
   carryover).

7. **`rfl init --force` semantics** (Phase A1). Default:
   **rewrites the lock byte-for-byte from defaults**, dropping
   user edits. Alternative: merge new defaults into the
   existing lock preserving user edits (much harder).

8. **`result_large_err` disposition** (Phase I3). Default:
   **ratify the module-level allow** — keep the allows in
   place + add `decisions.md` row 67 that names the trade-off.
   Alternative: box `ReemitError` + `AgentLoopError` (5+
   commits, ripples through every `?` site). Round 2 fixes
   M-5: ratify means **keep**, not delete.

9. **Bus-fixture inclusion in the release package** (Phase F1).
   Default: **exclude** — `rafaello-bus-fixture` is
   test-shaped (lockless fixture broker for unit tests) and
   should not ship to end users. Alternative: include (one
   fewer cargo flag).

10. **macOS-CI Homebrew install coverage** (Phase G3). Default:
    **manual validation only**. Alternative: CI workflow that
    `brew install`s the formula in a clean macOS container.

11. **Phase I1 inclusion** (`core_tools_list_registered_before_provider_spawn.rs`).
    Default: **in scope** (round 2 assumes m5b did NOT land it;
    verify against m5b commits list during round 3 if
    available; round-2 default is "we owe it").

12. **TUI quit binding for the chat loop** (Phase J2). Round-3
    correction: round 2 assumed `q`; the live TUI input-mode
    handler (`rafaello-tui/src/lib.rs`) does not bind `q` as
    quit — `Ctrl-C` is the m3-era chat-loop terminator. J2
    uses `Ctrl-C`. Owner-judgment recorded so the
    implementation-time verification gate is explicit: if a
    proper `q`-quit binding is desired for v1 polish, scope
    it as a tiny TUI patch + a unit test; otherwise leave the
    binding as-is. Default: leave as-is, use `Ctrl-C` in J2.
    (The confirm-allow binding `a` is confirmed correct at
    `rafaello-tui/src/lib.rs:70`.)

13. **`rafaello-openai-stub` in the release package** (F1).
    Default: **include** — the `manual-validation.md` §5
    deterministic-stub demo + the headline integration test
    both need a runnable stub binary inside the F-built tree.
    Alternative: exclude — keep `rafaello-openai-stub` as a
    test-only crate built only under `cargo test`; the
    integration test then builds the stub via `cargo build`
    before invoking it. Round-3 default: include (small
    binary, large ergonomic win for the scripted demo).

---

## Coverage / regression-anchor list

Verbatim from the §5 retrospectives, with the m6 disposition
inline.

| Source row | Item | File path | m6 phase | Disposition |
|---|---|---|---|---|
| m4 §5.5 / m5a §5 row 6 / m5b §5 row 13 | `#[allow(clippy::result_large_err)]` sweep | `crates/rafaello-core/src/{reemit,agent}/mod.rs:1` + any m5b sites | I3 | in scope (default: ratify-by-keeping) |
| m4 §5.8 / m5a §5 row 8 | `load.triggers.kind = "tool"` lazy-load exercise | `rafaello/tests/lazy_load_tool_trigger_spawns_on_first_call.rs` (new) | I2 | in scope (owner-judgment item 6) |
| m5a §5 row 9 / m5b §5 row 9 | macOS CI green hard gate | `manual-validation.md` §4 (URL) | F3 + J1 §4 | in scope |
| m5a §5 row 10 / m5b §5 row 10 | Interactive `rfl chat` recording | `manual-validation.md` §5 + `transcripts/section-5/` | J2 | in scope (hard requirement #3) |
| m5a §5 row 11 / m5b §5 row 11 | `manual-validation.md` skeleton fill | `manual-validation.md` §1–§7 + §G | J1 | in scope |
| m5a §5 row 14 / m5b §5 row 12 | `core_tools_list_registered_before_provider_spawn.rs` | `crates/rafaello-core/tests/core_tools_list_registered_before_provider_spawn.rs` | I1 | in scope (default; owner-judgment item 11) |
| m5b §5 row 1 | Multi-turn `rfl-openai-stub` shape | `crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs` (M-2 fix) | E1 + E2 | in scope |
| m5b §5 row 8 | `rfl audit` read CLI | `crates/rafaello/src/audit_cli.rs` (new) | D1–D3 | in scope (round-2 schema rewrite) |

Items routed to v2 (NOT m6): m5a-7, m5b-2, m5b-3, m5b-4,
m5b-5, m5b-6, m5b-7 (see §"Out of scope").

m5a §5 row 12 (`rfl_chat_eager_spawns_five_tree_…`) and m5a
§5 row 13 (`rfl_chat_spawns_inactive_provider_but_reemit_ignores_it`)
were both internal-split items in m5b's commits list. Round 2
treats them as **landed by m5b** unless round 3 verification
finds otherwise.

---

## Glossary additions

Proposed at retrospective time (no live `glossary.md` edits at
scope-drafting time per the plans/README.md authoring
convention):

- **`rfl init`** — materialises `rafaello.lock` with the
  bundled `rfl-openai` provider (canonical id
  `builtin:openai@0.0.0`) pre-installed against the
  dev-environment LiteLLM endpoint. Idempotent; `--force`
  rewrites; declining the install-time review writes an empty
  lock. Cites `crates/rafaello/src/init.rs` + `decisions.md`
  rows 59–60.
- **`rfl install <plugin>`** — positional argument resolves
  against the bundled plugin tree at
  `share/rafaello/plugins/<plugin>/rafaello.toml`. The
  existing `--fixture <path>` shape remains for fixture tests.
  Cites `crates/rafaello/src/install.rs` + `decisions.md` row
  61.
- **`rfl audit`** — read CLI over `audit_events` SQLite
  table. Live schema columns: `seq, at, kind, request_id,
  payload`. Default human-readable; `--json` for scripting.
  Filters: `--kind`, `--since`, `--request-id`. Cites
  `crates/rafaello/src/audit_cli.rs` + `decisions.md` row 63.
- **`syd-pty discovery`** — the spawning-side problem where
  `syd` invokes `setup_pty` and fails because `syd-pty` is
  not on `PATH` and `CARGO_BIN_EXE_syd-pty` is not set on the
  syd child command. v1 fix is belt-and-braces: devshell
  exports the env; lockin sandbox resolves `syd-pty` adjacent
  to `syd` and sets the env on the syd child command. Cites
  `rafaello/nix/devenv.nix` + `lockin/crates/sandbox/src/lib.rs`
  + `decisions.md` row 62.
- **`rfl-openai-stub scripted turns`** — N-turn TOML script
  consumed by the stub via `RFL_OPENAI_STUB_SCRIPTED_TURNS`.
  Deterministic-panic on exhaustion; mutually exclusive with
  the singular env (m5b). Cites
  `crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs` +
  `decisions.md` row 64.

The m6 retrospective is also expected to extend the existing
`rafaello.lock` glossary entry with a banner-pointer at the
`rfl init`-materialised default shape, and the existing
`Bundled provider` entry with an `rfl init` cross-reference.

---

## Internal split (driver guidance for `commits.md`)

Round-3 sizing: **28 commits implementation default, 30 max
+ 1 retro reservation**. Round 1 was 22; round 2 added
Phase B (install UX, +3) + G.β release automation (+1) + the
retro reservation (+1); round 3 keeps the same headline
total — the round-3 folds (PP1 package copy in A2/B1,
`--project-root` in D1, fake-syd fixture in C3) extend
existing rows rather than adding new commits.

| # | Section | Subject sketch | ~commits |
|---|---------|----------------|----------|
| 1 | A1 | `rfl init` CLI scaffold + idempotency invariant | 1 |
| 2 | A2 | `rfl-openai` default-entry materialisation against live lock schema | 1 |
| 3 | A3 | install-time review prompt (TTY + `--yes` + decline paths) | 1 |
| 4 | A4 | `rfl init` integration tests (4 tests incl. PP1 + `resolve_entry` assertion) | 1 |
| 5 | B1 | `rfl install` positional plugin-id resolution + `--fixture: Option` (clap conflicts/required-unless) + PP1 package-tree copy | 1 |
| 6 | B2 | bundled plugin source tree under `share/rafaello/plugins/<plugin>/` with manifest `rafaello.toml` | 1 |
| 7 | B3 | `rfl install <plugin>` integration tests (6 tests incl. clap-error, `--project-root`, `resolve_entry` containment) | 1 |
| 8 | C1 | devshell `CARGO_BIN_EXE_syd-pty` export | 1 |
| 9 | C2 | lockin sandbox `resolve_syd_pty_path` + env on syd child + fake-syd `[[bin]]` in `sandbox/Cargo.toml` | 1 |
| 10 | C3 | syd-pty discovery tests (3 fake-syd lockin tests + 1 rafaello-side smoke test) | 1 |
| 11 | D1 | `rfl audit` CLI scaffold against live `audit_events` schema + `--project-root` flag | 1 |
| 12 | D2 | filter flags (`--kind`, `--since`, `--request-id` no-join, `--json`, `--full`) | 1 |
| 13 | D3 | `rfl_audit_*` integration tests (7 tests incl. `--project-root`) | 1 |
| 14 | E1 | `RFL_OPENAI_STUB_SCRIPTED_TURNS` parser + dispatcher (correct crate path) | 1 |
| 15 | E2 | stub scripted-turns unit tests (incl. mutual-exclusion + exhaustion) | 1 |
| 16 | F1 | `cargoBuildFlags` expansion to every **release binary** (8-binary list; bus-fixture excluded per item 9) | 1 |
| 17 | F2 | bundled plugin source trees in package output (real binaries inside each plugin dir; `bin/` top-level retains only `rfl` + `rfl-tui`) | 1 |
| 18 | F3 | CI matrix `nix build` on Linux + macOS | 1 |
| 19 | G1 | Homebrew formula scaffold per chosen model (default G.β separate tap) | 1 |
| 20 | G2 | release-tag automation (G.β only — vacates if G.α or G.γ wins item 5) | 0-1 |
| 21 | G3 | manual-validation Homebrew install entry (folded into J1 §G) | folded |
| 22 | H1 | `rafaello/README.md` rewrite (5-line bootstrap + troubleshooting + Pre-m6 workaround subsection) | 1 |
| 23 | H2 | `CONTRIBUTING.md` rewrite | 1 |
| 24 | I1 | `core_tools_list_registered_before_provider_spawn.rs` (conditional; default: in scope) | 1 |
| 25 | I2 | `load.triggers.kind = "tool"` lazy-load fixture + test | 1 |
| 26 | I3 | `result_large_err` ratify (keep allows + decisions row 67) | 1 |
| 27 | J1 | `manual-validation.md` §1–§7 + §G skeleton + audit dump shape | 1 |
| 28 | J2 | tmux-driven §5 recording (transcripts under `transcripts/section-5/`; greps against live overlay copy) | 1 |
| 29 | J3 | retrospective + `decisions.md` rows 59–68 + glossary additions | 1 (reserved) |

Realistic total at default-selected positions:

- **28 commits implementation** (rows 1–28, with G2 landing
  per default G.β and row 21 folded into J1).
- **+ 1 retro reservation** (row 29) per pi N-3.

**30 max** if owner-judgment item 11 vacates I1 (-1) while
items 5 → G.α/G.γ alter Phase G's count (G.α: 1 commit only +
no release automation, -1; G.γ: 1 commit, -1) — but Phase B's
positional shape can split into "argparse" + "resolver" if
implementation surfaces a clean fold (+1). Pi-round budget:
**4–5** (round 1 + round 2 burned; m5b took 7 with a wider
security surface; m6 has fewer load-bearing invariants
left).

**Forced-monolithic commits:**

- Row 5 (B1 `rfl install` positional resolution) lands as one
  commit because the `InstallArgs` change + the resolver
  function + the error-mapping ripple are coupled at the
  `Cli::parse` layer.
- Row 9 (C2 lockin sandbox) lands as one commit because the
  `resolve_syd_pty_path` function + the `Command::env` call +
  the negative `pty:off` rejection are coupled at the
  child-command construction site.
- Row 16 (F1 cargoBuildFlags) lands as one commit because Nix
  evaluation is whole-flake.
- Row 11 + row 12 (D1 + D2) are kept separate because the
  scaffold + the filter flags exercise different test
  surfaces.

**Test ladder dependencies:**

- Row 4 (A4 tests) extends in row 27 (J1 §1 walkthrough) — A4
  covers happy path in isolation; §1 extends with the operator's
  tmux send-keys narrative.
- Row 18 (F3 CI) gates row 27 §4 (macOS CI URL capture).
- Row 14 (E1 multi-turn stub) is consumed by row 28 (the J2
  tmux recording integration test).
- Row 7 (B3 positional install) gates the row-28 J2 transcript
  because the §5 flow uses `rfl install rfl-mailcat`.

---

## Acceptance summary

m6 is done when:

- Every named deliverable in §"In scope" is implemented and its
  tests pass.
- `nix develop .#rafaello --impure --command cargo test
  --manifest-path rafaello/Cargo.toml --workspace --features
  test-fixture` green on Linux (M-6 fix: use the `.#rafaello`
  devshell, not the default monorepo shell).
- **macOS CI green is a hard ratification gate** (m3 / m4 /
  m5a / m5b precedent). Both `cargo test --workspace` and
  `nix build .#rafaello` on `macos-latest` must be green
  before retrospective ratification, with the only exception
  being tests gated `#[cfg(target_os = "linux")]`.
- `nix develop .#rafaello --impure --command cargo build
  --manifest-path rafaello/Cargo.toml --workspace --bins`
  green.
- `nix build .#rafaello` produces a binary tree on both Linux
  + macOS containing the **release-binary set** (Phase F1's
  8-binary list: `rfl`, `rfl-tui`, `rfl-openai`,
  `rfl-openai-stub`, `rfl-readfile`, `rfl-mailcat`,
  `rfl-mockprovider`, `rafaello-fetch` — `rafaello-bus-fixture`
  excluded per owner-judgment item 9 default) plus the
  bundled plugin tree under
  `share/rafaello/plugins/<plugin>/rafaello.toml`.
- Post-`rfl init --yes`,
  `${PROJECT_ROOT}/.rafaello/plugins/<topic-id-of-openai>/rafaello.toml`
  exists, `bin/rfl-openai` is present, and the lock's
  `digest` / `manifest_digest` match the copied tree (PP1
  acceptance, round-3 B-1 fold).
- `brew install <tap>/rafaello` (or the chosen-model
  equivalent per owner-judgment item 5) works in a clean
  macOS shell (manual validation per Phase J §G).
- The 5-line bootstrap (`cd …; nix develop .#rafaello
  --impure --command rfl init; export LITELLM_API_KEY=…;
  nix develop … rfl install rfl-mailcat; nix develop … rfl
  chat`) lands in `rafaello/README.md` verbatim and works
  against the dev LiteLLM endpoint.
- `manual-validation.md` records §1–§7 + §G with
  operator-witnessed evidence, including the §5 tmux-driven
  recording (hard requirement #3). The §5 transcript files
  live under `milestones/m6-polish-release/transcripts/section-5/`.
- `retrospective.md` written with the syd-pty narrative
  captured for posterity (hard requirement #5) + the
  `decisions.md` row appends (rows 59–68 placeholders) + the
  glossary additions + the v0.1 → main merge plan executed.
- All §"Owner-judgment items" resolved at convergence.

m6 RATIFIED ⇒ `rafaello-v0.1 → main` merge executes
immediately after per `decisions.md` row 33 ⇒ v1 demo-ready.

---

## References

- Roadmap row — `milestones/README.md` (last row).
- Driver pre-flight —
  `milestones/m6-polish-release/driver-preflight.md`.
- Round-1 pi review —
  `milestones/m6-polish-release/scope-pi-review-1.md`.
- Branch model — `decisions.md` row 33.
- Bundled `rfl-openai` provider — `decisions.md` row 38;
  `overview.md` §8.1.
- `[session].provider_active` selector — `decisions.md` row 44.
- `env.allow_secrets` — `decisions.md` row 46.
- `grant_match` template — `decisions.md` row 47.
- m5a/m5b split — `decisions.md` row 48.
- `core.tools_list` RPC — `decisions.md` row 49.
- Taint primitives + audit kinds — `decisions.md` rows 50–58.
- v1 scope cut — `overview.md` §16.
- m4 §5 carryovers —
  `milestones/m4-provider-agent-loop/retrospective.md` §5.5,
  §5.8.
- m5a §5 carryovers —
  `milestones/m5a-sinks-confirmation/retrospective.md` §5.
- m5b §5 carryovers —
  `milestones/m5b-taint-exfil/retrospective.md` §5.
- Live shapes — `flake.nix`, `rafaello/nix/{default,package,devenv}.nix`,
  `rafaello/crates/rafaello/src/{lib,install}.rs`,
  `rafaello/crates/rafaello-core/src/{audit,lock,session}/`,
  `lockin/crates/sandbox/src/lib.rs`,
  `rafaello/crates/rafaello-openai-stub/`.
- m2 §5.7 push-to-CI-early lesson —
  `milestones/m2-broker-spawn/retrospective.md` §5.7.

---

## Disagreements with pi (cumulative)

**Round 1**: none. **Round 2**: none. **Round 3**: none.
**Round 4**: none. Every B/M/N item across all four rounds is
folded as a direct text change; no items are being argued
back. The G.β model is made actionable (round-3 M-1 fold)
but remains owner-overridable at ratification — that's a
clarification of default semantics, not a disagreement.

---

## Changelog

- Round 1 → `scope-pi-review-1.md` (B/7 M/6 N/4, BLOCKING).
- Round 2 → `scope-pi-review-2.md` (B/2 M/5 N/3,
  BLOCKING-but-narrowing).
- Round 3 → `scope-pi-review-3.md` (B/2 M/1 N/1,
  BLOCKING-but-very-narrow).
- Round 4 → this commit. Folds every round-3 B/M/N. Awaiting
  pi round 4; target verdict CONVERGED.

---

*End of m6 scope round 4 draft. Claude-authored; awaiting pi
adversarial review per `plans/README.md` Phase 2.*
