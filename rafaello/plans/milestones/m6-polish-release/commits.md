# m6 — v1 polish + release readiness — commits

> **Status:** round 1 draft — claude-authored 2026-05-11,
> awaiting pi adversarial review. Built against the RATIFIED
> `scope.md` (round 5, owner sign-off at commit `a0764b3`).
>
> **Budget.** Scope §"Internal split" pins the m6 budget at
> **28 commits implementation default / 30 max + 1
> retrospective reservation**. This draft lands **27
> implementation commits + 1 retrospective reservation = 28
> slots** matching scope's row-1–28 table verbatim under the
> default-selected positions (G.β wins owner-judgment item 5;
> G3 install-smoke folded into J1 per scope's row 21 disposition;
> I1 in scope per item 11). The owner-judgment item-5 G.β
> default is locked per the ratification commit; if any future
> owner override flips item 5 to G.α (-1) or G.γ (-1), the
> retrospective reservation absorbs the slack and the implementation
> count stays inside the 28 ceiling.
>
> If pi-1 demands the literal 28-implementation interpretation
> from the driver prompt (rather than the scope-table reading
> of 27 + 1 retro), round 2 splits c05 (B1) into argparse-cutover
> + bundled-source-resolver per scope's "+1 if implementation
> surfaces a clean fold" escape hatch. The round-1 default
> matches the scope table.
>
> **Phase distribution.** A:4 · B:3 · C:3 · D:3 · E:2 · F:3 ·
> G:2 · H:2 · I:3 · J:2 · retro:1 = 27 + 1 = 28.
>
> **Workspace-wide cutovers explicitly called out** (m0 §4.1
> precedent):
>
> - **c05 (B1)** — `InstallArgs` clap cutover: `fixture: PathBuf`
>   becomes `fixture: Option<PathBuf>`, positional `plugin:
>   Option<String>` lands alongside, `project_root:
>   Option<PathBuf>` lands alongside, with `conflicts_with` /
>   `required_unless_present` clauses wiring exactly-one-of-two
>   semantics. The `run_install` body fans out across both
>   resolution arms in the same commit. Scope §"Internal split"
>   pins this as forced-monolithic (the `InstallArgs` change +
>   the resolver + the error-mapping ripple are coupled at the
>   `Cli::parse` layer).
> - **c16 (F1)** — `cargoBuildFlags` expansion: replaces the
>   live single `[ "-p" "rafaello" ]` with an eight-package
>   list driving the release binary set (`rfl`, `rfl-tui`,
>   `rfl-openai`, `rfl-openai-stub`, `rfl-readfile`,
>   `rfl-mailcat`, `rfl-mockprovider`, `rafaello-fetch`). Nix
>   evaluation is whole-flake so this lands as one commit;
>   scope §"Internal split" forced-monolithic list pins it.
> - **c09 (C2)** — lockin sandbox `resolve_syd_pty_path` +
>   `Command::env` plumbing + hard-error rejection of the
>   `pty:off` fallback path. Scope §"Internal split" pins this
>   as forced-monolithic (function + call site + negative
>   coupled at the child-command construction site).
>
> Round-1 changelog: this is the first round; nothing to fold.
>
> ---

## Reading order for per-commit agents

Every per-commit agent receives:

1. `rafaello/plans/overview.md` — §4.6 (reserved env vars),
   §8.1 (bundled `rfl-openai`), §15.1 (manifest), §16 (v1
   scope cut).
2. `rafaello/plans/decisions.md` rows **25** (manifest
   filename `rafaello.toml`), **31** (sidecar `openrpc.json`),
   **33** (branch model / v0.1 → main), **34** (`rfl-tui`
   subprocess + no public `rfl serve`), **38** (bundled
   `rfl-openai`), **44** (`[session].provider_active`),
   **46** (`env.allow_secrets`), **47** (`grant_match`),
   **49** (`core.tools_list` RPC), **50–58** (m5b taint /
   audit primitives the m6 audit CLI consumes).
3. `rafaello/plans/glossary.md` — `rafaello.lock`, `Bundled
   provider`, `Manifest`, `Audit log`, `topic_id`, `Sandbox`.
4. `rafaello/plans/milestones/m6-polish-release/scope.md`
   (round-5 RATIFIED) — every per-commit agent reads scope
   end-to-end so the §"Package-placement invariant PP1"
   block is in working memory before touching Phase A / B / F.
5. The **inlined row text below** — full prose, every
   acceptance bullet — passed verbatim in the per-commit
   prompt body. Per m1 §4.2 + plans/README.md "Patterns
   from prior milestones": the orchestrator does **not** cite
   by row number; the row is quoted into the agent prompt to
   keep granularity decisions on the orchestrator side and to
   guard against mid-implementation `commits.md` drift.

`tests-with-code`: every acceptance row names the test files
it adds. Per `~/.claude/CLAUDE.md`, tests land in the same
commit as the surface they cover unless explicitly called out
as a two-stage ladder (m0 retro §4.3 — two pairs called out
inline below: c01 → c04, c05 → c07).

---

## Phase ordering rationale

Phases land in alphabetical order with the following
cross-phase landing-order constraints:

- **A (init) precedes B (install) precedes F (Nix package)**
  on the **runtime resolver invariant PP1**. A2 / B1's PP1
  package-tree copy targets `${PROJECT_ROOT}/.rafaello/plugins/
  <topic-id>/`; the source tree it copies from is laid out by
  F2 inside `<release-prefix>/share/rafaello/plugins/<plugin>/`.
  The A1–B3 test rows use **fixture release trees** so they
  do not block on F; F's package-output test (F3) re-validates
  the integration. PP1 is documented in scope §"Package-placement
  invariant"; every Phase A / Phase B / Phase F per-commit
  agent quotes that block verbatim.
- **A1 (init CLI scaffold) precedes A2 (lock + PP1 copy)
  precedes A3 (review prompt) precedes A4 (tests).** Standard
  phase ladder; A2 lands the package-tree copy that A4's
  `rfl_init_materialises_package_dir.rs` asserts.
- **C1 / C2 (devshell + lockin sandbox plumbing) precede C3
  (tests).** C3's fake-syd `[[bin]]` test depends on C2's
  `resolve_syd_pty_path` shape; C3's rafaello-side smoke
  test depends on C1's devshell export.
- **D1 (audit CLI scaffold) precedes J2 (tmux script).** The
  J2 transcript flow shells out to `rfl audit --project-root
  "$PROJECT"`; D1 lands the `--project-root` flag (scope
  round-3 B-2 fold) consumed by J2's audit step.
- **E1 (multi-turn stub) precedes J2 and the demo-bar
  integration test.** The J2 tmux flow optionally runs the
  scripted stub for deterministic walkthroughs; the demo-bar
  integration test
  (`rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`,
  scope §"Demo bar" / §"Headline integrated demo") consumes
  E1's `RFL_OPENAI_STUB_SCRIPTED_TURNS` env var. That demo-bar
  test lands inside **J2** so the tmux recording and the
  programmatic flow share one body.
- **F1 / F2 (package output expansion) precede G1 (Homebrew
  tap formula) and J1 (manual-validation §G).** Both consume
  the Phase F layout (real plugin binaries inside
  `share/rafaello/plugins/<plugin>/bin/`, top-level
  `<prefix>/bin/` carrying only `rfl` + `rfl-tui`). G2 (release
  automation) consumes F's `nix build` invocation.
- **F3 (macOS CI matrix) precedes J1 §4 (macOS CI URL
  capture)** but J1 §4 only references the URL once available;
  J1 lands the skeleton with a placeholder URL, the
  retrospective ratification phase fills it after CI green
  on the rafaello-v0.1 → main merge candidate.
- **I3 (`result_large_err` ratify) is decisions-row-only +
  comment-pin: ordering is independent of all other rows.**
  It lands anywhere in the I phase window.
- **J1 (manual-validation skeleton) precedes J2 (§5 tmux
  recording).** J1 lays down §1–§7 + §G headings; J2 fills
  §5 with the captured transcripts under
  `transcripts/section-5/`.

---

## Commit table

### Phase A — `rfl init` (4 commits)

Lands the cold-start command per hard requirement #1. Phase A
is the load-bearing carrier of invariant PP1 on the
`rfl init`-side: A2 writes the lock entry AND copies the
bundled `rfl-openai` source tree into the project's plugin
directory. Without that copy, the lock validates but
`rfl chat` cannot resolve the manifest at runtime.

#### c01 — feat(rafaello): `rfl init` CLI scaffold + idempotency invariant

- **What.** Scope §A1. Extend `RflChatCommand` (live at
  `rafaello/crates/rafaello/src/lib.rs:57-69` — today exposes
  only `Chat`, `Install(InstallArgs)`, `Status`) with
  `Init(InitArgs)`. New module
  `rafaello/crates/rafaello/src/init.rs`. `InitArgs`:
  ```rust
  #[derive(Debug, clap::Args)]
  pub struct InitArgs {
      #[arg(long, default_value_t = false)]
      pub yes: bool,
      #[arg(long, default_value_t = false)]
      pub force: bool,
      #[arg(long)]
      pub project_root: Option<PathBuf>,
  }
  ```
  `run_init` body in this commit is the scaffold only: parse
  `--project-root` (defaulting to `std::env::current_dir()`);
  if `${PROJECT_ROOT}/rafaello.lock` exists and `--force` is
  not set, print `"lock already present at <path>"` and exit
  0; otherwise return a typed stub error
  `InitError::NotYetImplemented` so the per-commit green bar
  holds without writing a partial lock. The default-lock
  TOML emit + PP1 copy land in c02.
- **Why.** Scope §A1 hard requirement #1's cold-start UX
  needs the subcommand visible in `rfl --help` before any
  lock-writing logic. Idempotency lands here because it is a
  CLI-shape invariant (operators invoking `rfl init` twice
  from a script must not corrupt their lock). m4 c01 / m5a
  c02 precedent of "scaffold the subcommand, write logic
  next commit."
- **Depends on.** baseline (a0764b3, scope ratified).
- **Acceptance.** Tests in `rafaello/crates/rafaello/tests/`:
  - `rfl_init_help_lists_init.rs` — `rfl init --help` exits 0
    and prints `--yes`, `--force`, `--project-root`.
  - `rfl_init_with_existing_lock_idempotent.rs` — pre-create
    `<tmpdir>/rafaello.lock` with arbitrary bytes; run
    `rfl init --project-root <tmpdir>`; assert exit 0, lock
    bytes unchanged, stderr contains `lock already present`.
  - `rfl init` from a cwd without a pre-existing lock exits
    non-zero with `NotYetImplemented` (this assertion is
    **amended away** in c02 once the writer lands —
    two-stage ladder per m0 §4.3).
  - `cargo build -p rafaello` green.
- **Files touched.** `rafaello/crates/rafaello/src/lib.rs`
  (add `Init` variant + dispatch arm, ~10 lines);
  `rafaello/crates/rafaello/src/init.rs` (new module,
  ~60 lines); two new test files. Total ~150 lines.
- **Size.** small-to-medium.
- **Scope sections.** §A1.

#### c02 — feat(rafaello-core, rafaello): `rfl init` materialises default lock + PP1 bundled-plugin copy

- **What.** Scope §A2 + PP1 invariant. Implements
  `run_init`'s lock-emission body and the PP1 package-tree
  copy step. The default lock content is the TOML literal
  pinned by scope §A2 — single `[plugin."builtin:openai@0.0.0"]`
  table with `entry = "bin/rfl-openai"`, the
  `[plugin."…".grant.bundles.default.{network,env,env.set}]`
  subtables, `[plugin."…".bindings]` (`provider = true`,
  `provider_id = "openai"`, `load = "eager"`), and
  `[session].provider_active = "builtin:openai@0.0.0"`.
  Algorithm:
  1. Resolve the bundled `rfl-openai` source tree path —
     `<release-prefix>/share/rafaello/plugins/rfl-openai/`
     when invoked from a release-installed `rfl` (Phase F
     layout); for in-tree dev invocations, fall back to a
     `RFL_BUNDLED_PLUGINS_DIR` env var if set, then to a
     repo-relative resolve (`rafaello/crates/rafaello-openai/`
     adjacent to the `rfl` binary's nix-store sibling). The
     resolver lives in a new helper
     `rafaello/crates/rafaello/src/bundled.rs`.
  2. Compute `topic_id = topic_id::derive("builtin:openai@0.0.0")`
     using the existing `rafaello_core::topic_id` helper.
  3. Copy the source tree into
     `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`,
     **dereferencing symlinks** (per scope PP1 containment
     invariant — `compile::resolve_entry` rejects symlinks
     escaping `package_dir`). Manifest file is renamed to
     `rafaello.toml` (per `decisions.md` row 25); sibling
     `openrpc.json` and any `schemas/` directory carried
     verbatim.
  4. Compute `digest` (recursive content hash over the copied
     plugin directory) and `manifest_digest` (hash of the
     copied `rafaello.toml`) using
     `rafaello_core::digest::recompute` (the existing helper
     `install.rs` uses).
  5. Render the lock TOML with the computed digests + an
     `RFC3339`-formatted `granted_at = chrono::Utc::now()`,
     write to `${PROJECT_ROOT}/rafaello.lock`.
- **Why.** Scope §A2 + PP1 invariant. The lock alone is
  insufficient: `rfl chat`'s runtime resolver
  (`crates/rafaello/src/lib.rs:235-244`) opens
  `.rafaello/plugins/<topic-id>/rafaello.toml`; without the
  copy step the chat session fails on the first invocation.
  PP1's `actual-file-not-symlink` half is enforced by the
  copy implementation choosing `fs::copy` semantics (or
  `cp -L` for directories with sub-symlinks) — never
  `symlink_metadata`-preserving traversal.
- **Depends on.** c01.
- **Acceptance.** Tests in `rafaello/crates/rafaello/tests/`:
  - `rfl_init_writes_default_lock.rs` (scope §A4 happy path)
    — `rfl init --yes --project-root <tmpdir>` against a
    fixture bundled-plugin source tree (created by the test
    via `tempdir` + `RFL_BUNDLED_PLUGINS_DIR`); assert the
    rendered TOML round-trips through `Lock::from_toml`
    byte-stably (load → render → compare bytes).
  - `rfl_init_materialises_package_dir.rs` (scope §A4 round-3
    B-1 + round-4 B-1) — same setup; assert
    `<tmpdir>/.rafaello/plugins/<topic-id-of-openai>/rafaello.toml`
    exists and parses via `Manifest::parse`; assert
    `bin/rfl-openai` exists as a regular file
    (`fs::metadata(...).file_type().is_file()` true,
    `is_symlink()` false); assert the lock's `digest` /
    `manifest_digest` fields match
    `rafaello_core::digest::recompute(&plugin_dir)`; assert
    `compile::resolve_entry(&plugin_dir, "bin/rfl-openai")`
    returns `Ok(<canonical-path>)` with the canonical path
    inside `plugin_dir` (no `EntryEscape`).
  - `rfl_init_idempotent_no_overwrite.rs` (scope §A4) —
    `rfl init --yes` twice in succession leaves both the
    lock bytes and the package-dir tree unchanged on the
    second run.
  - `rfl_init_force_rewrites.rs` (scope §A4 + owner-judgment
    item 7) — pre-create a hand-edited lock with a garbage
    `[plugin."hand-edit:foo@0.0.0"]` entry; run `rfl init
    --yes --force`; assert the lock is rewritten
    byte-for-byte from defaults (no `hand-edit:foo` entry
    survives), the package dir for `<topic-id-of-openai>`
    is also rewritten.
  - The c01 `NotYetImplemented` assertion is **amended**
    in this commit to assert success on the previously-failing
    invocation (two-stage ladder per m0 §4.3).
- **Files touched.** `rafaello/crates/rafaello/src/init.rs`
  (body fill, ~120 lines); `rafaello/crates/rafaello/src/
  bundled.rs` (new helper, ~40 lines); four new test files
  in `rafaello/crates/rafaello/tests/`; the c01 stub-error
  assertion amended in `rfl_init_with_existing_lock_idempotent.rs`.
  Total ~280 lines.
- **Size.** medium (body-justified: the lock-rendering
  algorithm + the PP1 copy implementation are coupled at
  `run_init`'s top-level body; splitting would land a
  half-baked `run_init` that writes a lock without the
  package dir, which the c02 `materialises_package_dir`
  test cannot accept; m0 c08 / m5a c30 precedent of
  package-fixture atomicity).
- **Scope sections.** §A2, §"Package-placement invariant
  PP1".

#### c03 — feat(rafaello): `rfl init` install-time review prompt + decline-empty-lock path

- **What.** Scope §A3 + owner-judgment item 8 (declining
  the prompt writes an empty lock + no PP1 copy). Wraps c02's
  unconditional lock-write with a TTY prompt:
  1. After resolving the bundled source but **before**
     writing anything, print the default grant content (one
     paragraph per `grant.bundles.default.*` subtable —
     network, env, env.set, subscribes/publishes); prompt
     `"Proceed? [y/N]"` on stdin.
  2. If `--yes` is set, skip the prompt and treat as
     accepted.
  3. If accepted, run c02's writer (lock + PP1 copy);
     return `Ok(())`.
  4. If declined (or the input is empty / not `y`/`Y`),
     write a lock with **no** `[plugin."…"]` entries and
     **no** `[session].provider_active`. The `[session]`
     table is still emitted (with an empty body); the lock
     parses through `Lock::from_toml` to an empty
     plugin map. **No PP1 copy** runs. Print `"declined;
     wrote empty lock at <path>"` to stderr; exit 0.
- **Why.** Scope §A3 + owner item 8 ratification (default:
  empty-lock on decline). The empty-lock path is the safety
  valve for operators who want to hand-author a lock against
  a non-LiteLLM endpoint; m6 ships no `rfl init --endpoint
  <url>` (scope §"Out of scope" item 11). The TTY prompt
  follows the live `install`-time prompt convention from
  m5a's `trifecta` flow.
- **Depends on.** c02.
- **Acceptance.** Tests in `rafaello/crates/rafaello/tests/`:
  - `rfl_init_yes_skips_prompt.rs` — `rfl init --yes
    --project-root <tmpdir>` against a fixture source tree;
    assert stdin is not read (the test runs with stdin
    closed); assert the resulting lock has the
    `builtin:openai@0.0.0` entry and the PP1 dir exists.
  - `rfl_init_decline_writes_empty_lock.rs` (scope §A4
    decline arm) — feed `n\n` on stdin; assert lock has no
    plugin entries, no `[session].provider_active`,
    `.rafaello/plugins/` directory either absent or empty.
  - `rfl_init_eof_treated_as_decline.rs` — feed empty
    stdin (EOF on read); same expectations as the explicit
    decline.
- **Files touched.** `rafaello/crates/rafaello/src/init.rs`
  (prompt wrapper + decline-path branch, ~50 lines); three
  new test files. Total ~150 lines.
- **Size.** small-to-medium.
- **Scope sections.** §A3, §A4 (decline test), owner-judgment
  item 8.

#### c04 — test(rafaello): `rfl init` consolidated integration tests against PP1 + live lock schema

- **What.** Scope §A4 (closing assertion). Adds the two
  remaining A-phase integration tests that bind c01–c03 to
  the demo-bar surface contract:
  - `rfl_init_round_trip_byte_stable.rs` — generate the
    default lock, parse via `Lock::from_toml`, render via
    `Lock::to_toml`, compare bytes; second-pass also asserts
    the canonical id ordering (`BTreeMap` invariant) and
    grant-subtable ordering match the literal in scope §A2.
  - `rfl_init_against_live_bundled_openai_tree.rs` — invokes
    `rfl init --yes --project-root <tmpdir>` with
    `RFL_BUNDLED_PLUGINS_DIR` pointing at the **live**
    `rafaello/crates/rafaello-openai/` directory (or its
    fixture-mirror at
    `rafaello/fixtures/m6-bundled-plugins/rfl-openai/` —
    introduced by this commit as a one-time fixture pin so
    A4 doesn't depend on the live in-tree manifest's exact
    bytes). Asserts the post-init `.rafaello/plugins/<topic>/
    rafaello.toml` parses + the lock's `manifest_digest`
    field matches the fixture-mirror's `rafaello.toml` digest.
- **Why.** Scope §A4 closes Phase A's acceptance by binding
  the default-lock TOML literal in scope §A2 to byte-stable
  round-trip + to a real bundled-source tree. The fixture
  mirror under `rafaello/fixtures/m6-bundled-plugins/`
  insulates A4 from in-tree manifest drift; Phase F's tests
  use the live in-tree shape independently.
- **Depends on.** c02, c03.
- **Acceptance.** Both tests green; the fixture mirror
  directory exists and contains the same files Phase F2's
  `postInstall` step copies (sanity-checked by F3 against
  this fixture).
- **Files touched.** Two new test files in
  `rafaello/crates/rafaello/tests/`;
  `rafaello/fixtures/m6-bundled-plugins/rfl-openai/`
  (fixture mirror: `rafaello.toml`, `openrpc.json`, `bin/`
  with a placeholder shell-script `rfl-openai` that
  `exec`s `true` and the real `rfl-openai` binary used at
  test time via `env!("CARGO_BIN_EXE_rfl-openai")` when
  the test runs under `cargo test`). Total ~120 lines + ~3
  fixture files.
- **Size.** small.
- **Scope sections.** §A4.

### Phase B — `rfl install <plugin>` UX (3 commits)

Lands the install-time ergonomics polish so the README
5-line bootstrap and the J2 tmux script can run
`rfl install rfl-mailcat --project-root "$PROJECT"`.

#### c05 — feat(rafaello): `rfl install` positional plugin + `--fixture: Option<PathBuf>` + `--project-root` clap cutover + bundled-source resolver + PP1 copy

- **What.** Scope §B1 — the **clap cutover + resolver**
  combined commit (scope §"Internal split" forced-monolithic).
  Three coordinated edits in
  `rafaello/crates/rafaello/src/install.rs`:
  1. `InstallArgs` rewrite (live at lines 32-43):
     ```rust
     #[derive(Debug, clap::Args)]
     pub struct InstallArgs {
         #[arg(required_unless_present = "fixture",
               conflicts_with = "fixture")]
         pub plugin: Option<String>,
         #[arg(long, required_unless_present = "plugin",
               conflicts_with = "plugin")]
         pub fixture: Option<PathBuf>,
         #[arg(long)]
         pub project_root: Option<PathBuf>,
         #[arg(long)]
         pub lock: Option<PathBuf>,
         #[arg(long = "i-know-what-im-doing", default_value_t = false)]
         pub i_know_what_im_doing: bool,
         #[arg(long = "allow-credential-paths", default_value_t = false)]
         pub allow_credential_paths: bool,
         #[arg(long, default_value_t = false)]
         pub verbose: bool,
     }
     ```
  2. New helper `resolve_bundled_source(plugin: &str) ->
     Result<PathBuf, InstallError>` in a new
     `rafaello/crates/rafaello/src/install/bundled.rs`
     module. Resolution order:
     - `$RFL_BUNDLED_PLUGINS_DIR/<plugin>/` if set.
     - Adjacent to the `rfl` binary's nix-store sibling:
       `<rfl-binary-parent>/../share/rafaello/plugins/<plugin>/`
       (Phase F layout).
     - Hard error `InstallError::BundledPluginNotFound { name }`.
  3. `run_install` body fans out:
     - If `args.fixture` is `Some(path)`, current m5a-ratified
       behaviour (resolve manifest at `<path>/rafaello.toml`,
       compile, write lock).
     - Else `args.plugin` is `Some(name)`: resolve via the
       helper, treat that as `package_dir`, compile, write
       lock.
     - **Both arms** then perform the PP1 copy: copy
       `package_dir` (dereferencing symlinks) to
       `${project_root}/.rafaello/plugins/<topic-id>/`,
       where `project_root` is `args.project_root` or
       `std::env::current_dir()`. Compute `digest` /
       `manifest_digest` over the copied tree; pin them
       into the lock entry.
  Acknowledged forced-monolithic per scope §"Internal split"
  row 5 — the clap struct change, the new resolver helper,
  and the body fan-out are coupled at the
  `clap::Args::parse` layer; a clap-only intermediate state
  cannot compile because `run_install` cannot consume an
  `Option<PathBuf>` fixture without the matching plugin-arm
  resolver.
- **Why.** Scope §B1 — closes pi B-2 / B-3 (round 1 — the
  README cannot ship a fixture-only command shape; the
  canonical demo's `rfl install rfl-mailcat` requires a
  positional argument). Round-3 M-5 and round-3 B-1 folds
  add the clap conflicts/required_unless wiring and the
  PP1 copy step. Round-4 B-2 fold adds `--project-root`.
- **Depends on.** baseline (no Phase A dependency: c05 only
  reads the install-side; `rfl install` is independent of
  `rfl init`).
- **Acceptance.** Tests in `rafaello/crates/rafaello/tests/`:
  - `rfl_install_help_lists_positional_and_fixture.rs` —
    `rfl install --help` shows the positional `plugin` arg
    + `--fixture <path>` + `--project-root <path>`.
  - `rfl_install_fixture_flag_still_works.rs` (scope §B3
    regression anchor) — m5a-ratified `--fixture <path>`
    behaviour holds; the new PP1 copy materialises a
    `.rafaello/plugins/<topic-id>/` directory regardless
    of arm.
  - `rfl_install_positional_resolves_to_bundled_plugin.rs`
    (scope §B3) — `rfl install rfl-mailcat` against
    `RFL_BUNDLED_PLUGINS_DIR=<fixture-release-tree>`
    finds `share/rafaello/plugins/rfl-mailcat/rafaello.toml`,
    compiles, writes the lock entry, and materialises the
    PP1 dir.
  - `rfl_install_positional_unknown_plugin_errors.rs`
    (scope §B3) — `rfl install nonsense` exits non-zero
    with `BundledPluginNotFound` (clear "no bundled plugin
    named 'nonsense'" message); lock + PP1 dir unchanged.
  - `rfl_install_requires_one_of_fixture_or_plugin.rs`
    (scope §B3 round-3 M-5) — invoking with neither / both
    args triggers a clap error before `run_install` runs;
    exit non-zero with a clap-format usage message.
  - `rfl_install_project_root_flag.rs` (scope §B3 round-4
    B-2) — `rfl install rfl-mailcat --project-root
    <tmpdir>` from a different cwd writes lock + PP1 dir
    under `<tmpdir>`, not under the invoking cwd.
  - `rfl_install_resolves_entry_against_canonicalised_package_dir.rs`
    (scope §B3 round-4 B-1) — after `rfl install
    rfl-mailcat` against a fixture release tree whose
    `bin/rfl-mailcat` is a real file, asserts
    `compile::resolve_entry(&plugin_dir, &manifest.entry)`
    returns `Ok(<canonical-path>)` inside
    `.rafaello/plugins/<topic-id>/`.
- **Files touched.** `rafaello/crates/rafaello/src/install.rs`
  (clap struct rewrite + body fan-out, ~60 lines net);
  `rafaello/crates/rafaello/src/install/bundled.rs` (new
  helper, ~50 lines); seven new test files. Total ~350
  lines.
- **Size.** medium (body-justified by the forced-monolithic
  cutover; m0 c08 / m5a c14 precedent of clap-layer
  rippling).
- **Scope sections.** §B1, §"Internal split" forced-monolithic.

#### c06 — feat(rafaello-{mailcat,readfile,openai,mockprovider,fetch}): promote bundled plugin manifest trees + sidecar `openrpc.json`

- **What.** Scope §B2. Each bundled plugin crate ships its
  source tree with the manifest renamed to `rafaello.toml`
  (per `decisions.md` row 25) and a sibling `openrpc.json`
  (per `decisions.md` row 31). Inventory:
  - `rafaello/crates/rafaello-mailcat/rafaello.toml`
  - `rafaello/crates/rafaello-mailcat/openrpc.json`
  - `rafaello/crates/rafaello-readfile/rafaello.toml`
  - `rafaello/crates/rafaello-readfile/openrpc.json`
  - `rafaello/crates/rafaello-openai/rafaello.toml`
  - `rafaello/crates/rafaello-openai/openrpc.json`
  - `rafaello/crates/rafaello-mockprovider/rafaello.toml`
  - `rafaello/crates/rafaello-mockprovider/openrpc.json`
  - `rafaello/crates/rafaello-fetch/rafaello.toml`
  - `rafaello/crates/rafaello-fetch/openrpc.json`
  Each `rafaello.toml` matches the live
  `bindings.tool_meta.<tool>.*` lock-side projection
  convention (m5b c20 precedent: package-side manifest
  declares `[provides.tool.<tool>]`, lock-side `bindings`
  table projects `tool_meta`). Where a plugin already has
  a fixture-only manifest (`rafaello-mailcat`,
  `rafaello-fetch`, `rafaello-readfile` per m4/m5a/m5b),
  this commit **moves** the canonical copy into the crate
  directory and points the fixture lock at the in-tree
  path. The `openrpc.json` sidecar lists the plugin's RPC
  methods (live `openrpc.json` shape per m4 c15 / m5a c30 /
  m5b c20). For `rfl-openai-stub` (test-fixture crate),
  the same layout lands but the manifest declares
  `[provides.tool]` empty (the stub doesn't declare tools,
  only provider-shape).
- **Why.** Scope §B2 — Phase B1's positional resolver
  reads `share/rafaello/plugins/<plugin>/rafaello.toml`;
  the in-tree promotion gives Phase F2's `postInstall`
  step a canonical source to copy into
  `$out/share/rafaello/plugins/<plugin>/`. Manifest
  filename `rafaello.toml` per `decisions.md` row 25 is
  load-bearing for the runtime resolver match against
  what `rfl chat` opens.
- **Depends on.** c05.
- **Acceptance.** Tests:
  - `rafaello/crates/rafaello-mailcat/tests/bundled_manifest_parses.rs`
    + sibling tests for each of the five bundled plugin
    crates — asserts the in-tree `rafaello.toml` parses
    via `Manifest::parse` and the in-tree `openrpc.json`
    deserialises via the existing m5b openrpc helper.
  - The c05
    `rfl_install_positional_resolves_to_bundled_plugin.rs`
    test from c05 is **amended** in this commit to point
    `RFL_BUNDLED_PLUGINS_DIR` at a constructed
    `<tmpdir>/share/rafaello/plugins/rfl-mailcat/` that
    copies the in-tree files; the amend pins the
    happy-path resolver to the canonical in-tree shape
    (two-stage ladder).
- **Files touched.** Ten new manifest + sidecar files
  (one pair per plugin crate); five new
  `tests/bundled_manifest_parses.rs` files (one per
  plugin crate). Total ~80 LoC of manifests +
  ~10 LoC × 5 tests.
- **Size.** small-to-medium (file count is high but each
  is a small declarative manifest; m5b c20 fixture-package
  atomicity precedent justifies the bundled landing).
- **Scope sections.** §B2.

#### c07 — test(rafaello): `rfl install` integration suite extension + multi-plugin install acceptance

- **What.** Scope §B3 closer. Extends c05's positional
  install test coverage with the multi-plugin acceptance
  cases that bind c05–c06 to scope §"Demo bar":
  - `rfl_install_writes_lock_entry_for_each_bundled_plugin.rs`
    — table-driven test: for each of `rfl-mailcat`,
    `rfl-readfile`, `rafaello-fetch`, `rfl-mockprovider`,
    invoke `rfl install <name>` against a constructed
    fixture release tree (using c06's in-tree manifests
    copied to `<tmpdir>/share/rafaello/plugins/<name>/`);
    assert the lock contains the corresponding
    `[plugin."<canonical-id>"]` entry with non-empty
    `digest` and `manifest_digest`.
  - `rfl_install_init_then_install_smoke.rs` (consumes
    Phase A) — runs `rfl init --yes` then
    `rfl install rfl-mailcat` from the same `--project-root
    <tmpdir>`; asserts both `builtin:openai@0.0.0` and
    `local:mailcat@0.0.0` entries land in the lock and
    both PP1 dirs exist.
- **Why.** Scope §B3 binds Phase B's resolver to the
  Phase A init flow; the J2 tmux script + the demo-bar
  integration test require this pair to compose cleanly.
  Two-stage ladder closer for c05 (the smoke covers the
  end-to-end shape c05 stubbed out).
- **Depends on.** c05, c06; baselining on c02 / c03 for
  the init half.
- **Acceptance.** Both tests green; `cargo test
  -p rafaello --test rfl_install_init_then_install_smoke`
  green on Linux.
- **Files touched.** Two new test files. Total ~120 lines.
- **Size.** small.
- **Scope sections.** §B3.

### Phase C — syd-pty discovery fix (3 commits)

Per hard requirement #2 + scope §"Hard requirements" #2.
Belt-and-braces fix: devshell exports `CARGO_BIN_EXE_syd-pty`
**and** lockin sandbox resolves it on the syd child command.
Owner-judgment item 3 default — belt-and-braces. Item 4
default — no `pty:off` fallback at the lockin layer.

#### c08 — feat(rafaello-nix): devshell export of `CARGO_BIN_EXE_syd-pty`

- **What.** Scope §C1. Extend `rafaello/nix/devenv.nix`
  (live at line 7-9 — exports `LOCKIN_SYD_PATH =
  "${pkgs.sydbox}/bin/syd"` on Linux) with a sibling export:
  ```nix
  env.CARGO_BIN_EXE_syd-pty = lib.optionalString
    pkgs.stdenv.isLinux "${pkgs.sydbox}/bin/syd-pty";
  ```
  (Or equivalent under devenv's syntax — match the live
  `LOCKIN_SYD_PATH` export's exact form.) The env var is
  exported only on Linux (matching `LOCKIN_SYD_PATH`'s
  `isLinux` gate); the sydbox package ships `syd-pty`
  adjacent to `syd` in the nix store, so the path is the
  same nix-store sibling.
- **Why.** Scope §C1 + hard requirement #2. Covers the
  interactive `rfl chat` case in the canonical
  `nix develop .#rafaello` devshell. Insufficient on its
  own — Homebrew-installed `rfl` and future entrypoints
  never enter the devshell — so c09 lands the lockin-side
  fix in tandem.
- **Depends on.** baseline. Independent of c09 / c10.
- **Acceptance.**
  - `cd rafaello && nix develop .#rafaello --impure
    --command env | grep ^CARGO_BIN_EXE_syd-pty=` prints
    the absolute path (manual verification step recorded
    in the per-commit prompt; no test seam — the env-var
    export is a nix-evaluation property, not a Rust
    artifact).
  - `nix flake check` on `rafaello/flake.nix` green.
- **Files touched.** `rafaello/nix/devenv.nix` (~3 line
  addition).
- **Size.** small.
- **Scope sections.** §C1, owner-judgment item 3.

#### c09 — feat(lockin-sandbox): `resolve_syd_pty_path` + `Command::env` on syd child + hard-error rejection + fake-syd `[[bin]]` registration

- **What.** Scope §C2 + scope §"Internal split"
  forced-monolithic row 9. **Lands upstream in
  `lockin/crates/sandbox/`** (per owner-judgment item 2
  default). Three coordinated edits in
  `lockin/crates/sandbox/src/lib.rs` (and Linux-specific
  surface in `linux.rs`):
  1. New function (Linux-gated) mirroring the live
     `resolve_syd_path` at lines 209-232:
     ```rust
     #[cfg(target_os = "linux")]
     fn resolve_syd_pty_path(
         spec: &SandboxSpec,
         resolved_syd: &Path,
     ) -> Result<PathBuf> {
         if let Some(path) = &spec.syd_pty_path {
             return Ok(path.clone());
         }
         if let Some(val) = std::env::var_os(
             "CARGO_BIN_EXE_syd-pty"
         ) {
             let path = PathBuf::from(val);
             anyhow::ensure!(path.is_absolute(), …);
             return Ok(path);
         }
         if let Some(parent) = resolved_syd.parent() {
             let sibling = parent.join("syd-pty");
             if sibling.exists() {
                 return Ok(sibling);
             }
         }
         if let Some(p) = find_in_path("syd-pty") {
             return Ok(p);
         }
         Err(SandboxError::SydPtyNotFound { … }.into())
     }
     ```
     **No `pty:off` fallback** (owner-judgment item 4
     default — hard requirement #2 demands the right-layer
     fix; a silent fallback would re-introduce the m5a
     wall).
  2. `SandboxSpec` extended with `syd_pty_path:
     Option<PathBuf>` (mirroring the existing `syd_path`).
  3. In the syd child-command construction site (live
     `linux.rs` near `build_command`), call
     `resolve_syd_pty_path(spec, &resolved_syd)?` and
     set `CARGO_BIN_EXE_syd-pty=<resolved-path>` on the
     `Command::env` of the syd child.
  4. New typed error variant `SandboxError::SydPtyNotFound
     { searched_dirs: Vec<PathBuf>, last_error: String }`
     (or equivalent; matches the existing
     `SandboxError`/`anyhow::Error` shape — m6 keeps the
     surface tight by emitting through the existing error
     channel).
  5. **Fake-syd `[[bin]]` registration** in
     `lockin/crates/sandbox/Cargo.toml`:
     ```toml
     [features]
     default = []
     tokio = ["dep:tokio"]
     test-fixture = []           # net-new (scope round-5 M-1)

     [[bin]]
     name = "fake-syd"
     path = "tests/bin/fake_syd.rs"
     required-features = ["test-fixture"]
     ```
     The `tests/bin/fake_syd.rs` source itself ships in
     c10 (the test that consumes it); landing the
     `[[bin]]` entry here keeps c10's diff a tests-only
     delta. (Alternatively, the binary source ships in
     c09 if pi-1 prefers atomicity; round-1 default
     splits to keep c09's lib-side diff coherent and
     c10's test-side fan-out separate.)
- **Why.** Scope §C2 + hard requirement #2's right-layer
  framing. The env var is set on the **syd child**, not
  on rafaello's own process, so `direnv` / `nix develop`
  env-allowlist filtering does not apply: rafaello
  resolves `syd-pty` from its own process environment
  (which has the var via c08), or from the sibling of
  `syd` (which works for Homebrew-installed rafaello via
  Phase F's tree layout), then *injects* the absolute
  path into the syd child's environment unconditionally.
  Hard requirement #4 from m5a (no manual
  `CARGO_BIN_EXE_syd-pty=$(which syd-pty)`) holds because
  the lockin layer always sets the env on the child.
  Forced-monolithic per scope row 9 — function + spec
  field + call site + negative are coupled at the
  child-command construction site.
- **Depends on.** baseline.
- **Acceptance.**
  - `cargo build -p lockin-sandbox --all-features` green
    on Linux; `cargo doc` warning-free.
  - `cargo build -p lockin-sandbox --features test-fixture
    --bin fake-syd` produces a binary at
    `target/debug/fake-syd` (the test-fixture feature gate
    keeps it out of default release builds).
  - Compile-only assertion: the `Err` arm of
    `resolve_syd_pty_path` returns a `SydPtyNotFound`
    variant (any-error fallthrough is rejected at code
    review).
  - The `Cargo.toml` `[features]` block now contains
    `default = []`, `tokio = ["dep:tokio"]`, and
    `test-fixture = []`.
- **Files touched.** `lockin/crates/sandbox/src/lib.rs`
  (`resolve_syd_pty_path` + `SandboxSpec` field + error
  variant, ~60 lines); `lockin/crates/sandbox/src/linux.rs`
  (the `Command::env` call site, ~5 lines);
  `lockin/crates/sandbox/Cargo.toml` (`test-fixture`
  feature + `[[bin]] fake-syd` entry, ~6 lines). Total
  ~80 lines.
- **Size.** small-to-medium (forced-monolithic by scope's
  row 9; matches m4 c07 / m5a c14 cutover precedent).
- **Scope sections.** §C2, owner-judgment items 2/3/4.

#### c10 — test(lockin-sandbox, rafaello): fake-syd records `CARGO_BIN_EXE_syd-pty` on child + rafaello-side devshell smoke

- **What.** Scope §C3 (round-3 M-4 + round-4 M-1 + round-5
  M-1 closure). Four tests landing the syd-pty discovery
  acceptance:
  1. `lockin/crates/sandbox/tests/bin/fake_syd.rs` — the
     fake-syd binary itself (~30 lines). Reads
     `RFL_FAKE_SYD_RECORD_PATH` from env; writes a JSON
     blob to that path with `{ "argv": [...],
     "environ": [...] }`; then `execvp("true", &[])` (or
     just `std::process::exit(0)`). Registered as a
     `[[bin]]` in c09.
  2. `lockin/crates/sandbox/tests/fake_syd_records_cargo_bin_exe_env_when_set_explicitly.rs`
     — constructs a `SandboxSpec` with `syd_path =
     Some(env!("CARGO_BIN_EXE_fake-syd"))` and
     `syd_pty_path = Some(<fixture-syd-pty-path>)`. Builds
     the sandbox; spawns; asserts the fake-syd's sentinel
     file contains
     `CARGO_BIN_EXE_syd-pty=<fixture-syd-pty-path>`.
  3. `lockin/crates/sandbox/tests/fake_syd_records_cargo_bin_exe_env_from_sibling.rs`
     — tempdir with `fake-syd` and a fixture `syd-pty`
     binary placed side-by-side; `syd_path` points at the
     tempdir's `fake-syd`, no `syd_pty_path`,
     `CARGO_BIN_EXE_syd-pty` unset in process env;
     asserts the sentinel records the tempdir's
     `syd-pty` (sibling-discovery arm).
  4. `lockin/crates/sandbox/tests/fake_syd_resolution_fails_hard_when_pty_missing.rs`
     — tempdir with only `fake-syd`, no `syd-pty`,
     env unset, no `syd_pty_path`. Asserts
     `Sandbox::build(...)` returns
     `Err(SandboxError::SydPtyNotFound { … })`; **no**
     `pty:off` fallback path runs.
  5. `rafaello/crates/rafaello/tests/rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty.rs`
     (Linux + devshell-gated, scope §C3 closer) — spawns
     `rfl chat` inside `nix develop .#rafaello --impure
     --command` against an extended `rafaello-bus-fixture`
     plugin running in `--record-env <path>` mode. Asserts
     the plugin's recorded env contains
     `CARGO_BIN_EXE_syd-pty=<absolute-path>`. The plugin's
     `--record-env <path>` mode is added in this commit
     to `rafaello/crates/rafaello-bus-fixture/src/bin/`
     (small, test-only — dumps `std::env::vars()` to a
     file at startup, then resumes normal fixture work).
- **Why.** Scope §C3 + hard requirement #2 verification.
  The fake-syd mechanism gives a mechanical proof of the
  env-on-child invariant without depending on a real PTY's
  ANSI output. The rafaello-side smoke binds the lockin
  fix back to the rafaello devshell — the c08 + c09
  pair's combined behaviour is observable via the plugin
  subprocess's environment.
- **Depends on.** c08, c09.
- **Acceptance.** All four tests green; the rafaello-side
  smoke is gated `#[cfg(target_os = "linux")]` and runs
  only inside the rafaello devshell (the test invokes
  `nix develop .#rafaello --impure --command` itself via
  `std::process::Command` — needs `--impure` per the m0
  retrospective §4.6 gotcha).
- **Files touched.** `lockin/crates/sandbox/tests/bin/
  fake_syd.rs` (new, ~30 lines); three new test files
  in `lockin/crates/sandbox/tests/`;
  `rafaello/crates/rafaello-bus-fixture/src/bin/` extended
  with `--record-env <path>` mode (~15 lines); one new
  rafaello-side smoke test file. Total ~200 lines + the
  bus-fixture extension.
- **Size.** medium (5 test files + 1 fixture binary + 1
  fixture-mode extension; body-justified by syd-pty
  acceptance fan-out).
- **Scope sections.** §C3, owner-judgment item 4.

### Phase D — `rfl audit` read CLI (3 commits)

m5b §5 row 8 carryover. Round-2 schema rewrite against the
live `audit_events` shape (`seq, at, kind, request_id,
payload`); round-3 adds `--project-root` for J2 wiring.

#### c11 — feat(rafaello): `rfl audit` CLI scaffold against live `audit_events` schema + `--project-root` flag

- **What.** Scope §D1. Extend `RflChatCommand` with
  `Audit(AuditArgs)` (live at
  `rafaello/crates/rafaello/src/lib.rs:57-69`). New module
  `rafaello/crates/rafaello/src/audit_cli.rs` (matches the
  scope §D1 path). `AuditArgs`:
  ```rust
  #[derive(Debug, clap::Args)]
  pub struct AuditArgs {
      #[arg(long)]
      pub project_root: Option<PathBuf>,
      // Filter flags land in c12.
  }
  ```
  Body:
  1. Resolve `project_root = args.project_root.unwrap_or(
     std::env::current_dir()?)`.
  2. Open `<project_root>/.rafaello/state/session.sqlite`
     via the existing
     `rafaello_core::session::SessionStore::open_read_only`
     helper (or a new `audit_only_open` if the existing
     helper opens too much; m5b precedent has read-only
     SQLite access through a small wrapper — match the
     existing surface).
  3. Issue the default query
     `SELECT seq, at, kind, request_id, payload FROM
     audit_events ORDER BY seq ASC`.
  4. Render one row per line:
     ```
     <seq>  <at>  <kind>  [<request_id>|-]  <payload-summary>
     ```
     where `<payload-summary>` is the JSON `payload`
     truncated to ~80 columns (UTF-8-safe truncation —
     prefer chars to bytes).
  5. Empty-DB case: print `"no audit events"` banner to
     stderr; exit 0.
- **Why.** Scope §D1 + m5b §5 row 8 carryover. The CLI
  scaffold + the default query + the project-root flag
  give J2 its `rfl audit --project-root "$PROJECT"`
  invocation. Filter flags split into c12 to keep the
  scaffold diff coherent (different test surfaces per
  scope §"Internal split" row 11 vs row 12).
- **Depends on.** baseline.
- **Acceptance.** Tests in `rafaello/crates/rafaello/tests/`:
  - `rfl_audit_help_lists_project_root.rs` — `rfl audit
    --help` exits 0; usage prints `--project-root <PATH>`.
  - `rfl_audit_lists_all_rows_from_live_schema.rs` (scope
    §D3) — populate an audit DB via `AuditWriter::open_for_install`
    + `record(AuditKind::ConfirmRequest, …)` (existing
    m5a/m5b helpers); run `rfl audit --project-root
    <tmpdir>`; assert the rendered output's row count
    matches the inserted-row count and the first row's
    column order matches the spec
    `<seq>  <at>  <kind>  [<request_id>|-]  <payload-summary>`.
  - `rfl_audit_project_root_flag.rs` (scope §D3 round-3
    B-2) — populate the audit DB under
    `<tmpdir>/.rafaello/state/session.sqlite`; run
    `rfl audit --project-root <tmpdir>` from a different
    cwd; assert output matches running with cwd =
    `<tmpdir>`.
  - `rfl_audit_empty_db.rs` (scope §D3) — fresh
    `rfl init` lock; `rfl audit` exits 0 with stderr
    containing `"no audit events"`.
- **Files touched.** `rafaello/crates/rafaello/src/lib.rs`
  (Audit variant + dispatch arm, ~10 lines);
  `rafaello/crates/rafaello/src/audit_cli.rs` (new, ~120
  lines); four new test files. Total ~250 lines.
- **Size.** medium (body-justified by the scaffold + the
  default-query/render path coupled at `run_audit`'s
  top-level body).
- **Scope sections.** §D1.

#### c12 — feat(rafaello): `rfl audit` filter flags (`--kind`, `--since`, `--request-id`, `--json`, `--full`)

- **What.** Scope §D2. Extend `AuditArgs` with the filter
  surface:
  ```rust
  #[arg(long)]
  pub kind: Vec<String>,           // repeatable
  #[arg(long)]
  pub since: Option<String>,       // "1h", "30m", "24h"
  #[arg(long)]
  pub request_id: Option<String>,
  #[arg(long, default_value_t = false)]
  pub json: bool,
  #[arg(long, default_value_t = false)]
  pub full: bool,
  ```
  - `--kind` validation: each value is checked against
    `AuditKind::from_str` (or `AuditKind::as_str`
    membership — m6 does **not** add `FromStr`; per scope
    §"Glossary" the lookup uses iteration over
    `AuditKind::VARIANTS`-equivalent or a static lookup
    table maintained alongside `as_str`). Unknown kind
    exits non-zero with `"unknown audit kind: <foo>; see
    AuditKind::as_str table"`.
  - `--since` parsing: `1h`, `30m`, `24h`, `7d`; converts
    to a UTC threshold; query becomes `... WHERE at >=
    ?`. Invalid spec exits non-zero with a usage message.
  - `--request-id` query: `... WHERE request_id = ?`.
    **No join against `entries`** (scope §"Out of scope"
    item 10 — live `entries` schema has no `call_id`
    column).
  - `--json`: emit one JSON object per row with keys
    `seq, at, kind, request_id, payload` (payload as
    parsed JSON `Value`, not stringified).
  - `--full`: disables payload summary truncation in the
    default render path.
- **Why.** Scope §D2. Filter flags are the operator-facing
  shape J1 §6 documents. Scope §"Internal split" splits
  D1 from D2 because the two exercise different test
  surfaces (scaffold + default render in c11;
  filter-logic + SQL parameter binding in c12).
- **Depends on.** c11.
- **Acceptance.** Tests in `rafaello/crates/rafaello/tests/`:
  - `rfl_audit_filters_by_kind.rs` (scope §D3) —
    insert rows with mixed `AuditKind` values; assert
    `--kind confirm_request --kind confirm_allowed`
    returns the two-kind union.
  - `rfl_audit_filters_by_request_id_no_join.rs` (scope
    §D3 — **explicitly asserts the query does not touch
    `entries`**) — wrap the SQLite connection in an
    `sqlx::Sqlite::trace`-equivalent / `set_tracer`
    (or use the existing m5b session-store trace seam);
    assert the executed SQL contains `FROM audit_events`
    and does **not** contain the substring `entries`.
  - `rfl_audit_filters_by_since.rs` (scope §D3) — exercise
    `--since 1h`, `--since 30m`; verify row exclusion at
    the time boundary.
  - `rfl_audit_json_emits_one_object_per_row.rs` (scope
    §D3) — `--json` output round-trips through
    `serde_json::from_str` per line; the payload key is
    an object, not a string.
  - `rfl_audit_full_disables_truncation.rs` — insert a
    row with payload >1KB; assert `--full` output
    contains the full bytes, default output is
    truncated.
- **Files touched.** `rafaello/crates/rafaello/src/audit_cli.rs`
  (extend args + filter dispatch + json render, ~80
  lines); five new test files. Total ~250 lines.
- **Size.** medium (body-justified by the filter-flag
  fan-out + the SQL-trace assertion in the no-join test).
- **Scope sections.** §D2.

#### c13 — test(rafaello): `rfl audit` consolidated integration coverage + glossary update

- **What.** Scope §D3 closer + scope §"Glossary"
  candidate. The two remaining integration tests + a
  glossary entry:
  - `rfl_audit_renders_m5b_taint_kinds.rs` — populate
    rows with `AuditKind::ConfirmRequestTaintAttached`
    (m5b row 58), `PluginPublishRejectedTaintSuperset`
    (m5b row 55), `ToolRequestTaintUnionedFromInReplyTo`
    (m5b row 57); assert default render distinguishes
    each kind in the `<kind>` column.
  - `rfl_audit_filters_combine.rs` — `--kind
    confirm_request --since 1h --request-id <id>`
    composes correctly (AND semantics; SQL trace
    asserts a single combined `WHERE`).
  - Glossary update note: scope §"Glossary" lists
    `rfl audit` as a candidate; this commit does NOT
    write to `glossary.md` (scope-drafting-time
    convention per `plans/README.md`) — the candidate
    lands in J3's retrospective commit.
- **Why.** Scope §D3 binds Phase D to the m5b
  audit-kind variants (the audit-CLI's primary
  consumer is the headline-flow `confirm_request` /
  `confirm_allowed` pair + the m5b taint variants).
- **Depends on.** c11, c12.
- **Acceptance.** Both tests green.
- **Files touched.** Two new test files. Total ~120
  lines.
- **Size.** small.
- **Scope sections.** §D3.

### Phase E — multi-turn `rfl-openai-stub` (2 commits)

m5b §5 row 1 carryover.

#### c14 — feat(rafaello-openai-stub): `RFL_OPENAI_STUB_SCRIPTED_TURNS` parser + TOML schema + dispatcher

- **What.** Scope §E1. Extend
  `rafaello/crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`
  (the live binary — round-2 M-2 confirmed crate path)
  with a new env var
  `RFL_OPENAI_STUB_SCRIPTED_TURNS=<path-to-toml>`. TOML
  schema:
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
  Dispatcher:
  1. On startup, if the env var is set, load + parse the
     TOML into a `Vec<Turn>`. **Mutually exclusive** with
     the existing singular env (m5a/m5b
     `RFL_OPENAI_STUB_*` shape — match the live env name);
     if both are set, exit non-zero with `"both scripted
     and singular envs set; exactly one allowed"`.
  2. On each incoming `user_message` / `tool_result`
     event, walk `turns` in order; first turn whose
     `match_user_message` / `match_in_reply_to`
     predicate matches the event fires; the turn is
     **marked consumed** (an `AtomicUsize` cursor or a
     `Mutex<Vec<bool>>`).
  3. Exhaustion (event arrives with no remaining turn
     matches) is a **hard panic** with a deterministic
     message including the unmatched event payload
     (mirrors m5b multi-answer-hook semantics per
     `decisions.md` row 56).
  4. Emit dispatch: `emit = "tool_call"` shapes a canonical
     `tool_request` event via the existing m5a wire
     helpers; `emit = "assistant_message"` shapes a
     canonical `assistant_message`.
- **Why.** Scope §E1 + m5b §5 row 1 carryover + load-bearing
  for the J2 §5 transcript's deterministic walkthrough
  variant. Round-3 §"Internal split" splits E1 (parser +
  dispatcher) from E2 (tests) because the dispatcher's
  surface is well-defined enough to test independently
  of the parser; round-1 default keeps them together
  because they share the same file's source and pi
  hasn't yet reviewed the split. If pi-1 prefers further
  splitting, round-2 lifts the parser into a sibling
  `scripted_turns.rs` module.
- **Depends on.** baseline.
- **Acceptance.** Build-green only; behavioural tests in c15.
  - `cargo build -p rafaello-openai-stub` green.
  - `cargo build -p rafaello-openai-stub --tests` green.
- **Files touched.**
  `rafaello/crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`
  (parser + dispatcher additions, ~120 lines). Total
  ~120 lines.
- **Size.** small-to-medium.
- **Scope sections.** §E1.

#### c15 — test(rafaello-openai-stub): scripted-turns happy path + exhaustion-panic + mutual-exclusion

- **What.** Scope §E2. Unit tests in
  `rafaello/crates/rafaello-openai-stub/tests/rfl_openai_stub_scripted_turns.rs`
  (single test file with multiple `#[test]` functions
  per scope §E2 list):
  - `two_turn_happy_path_send_mail_flow` — write the
    scope §E1 TOML to a tempdir; invoke the stub binary
    with `RFL_OPENAI_STUB_SCRIPTED_TURNS=<path>`; feed
    a fixture `user_message` matching the first
    `match_user_message`; assert the stub emits a
    `tool_request` with `tool_name = "send-mail"` and
    `args.to = "<recipient>"`; feed a fixture
    `tool_result` whose `in_reply_to` matches the
    first turn's emitted id; assert the stub emits an
    `assistant_message` with `content = "Done — …"`.
  - `exhaustion_panics_deterministically` — script a
    single turn; consume it; send a second matching
    event; assert the stub process exits with a panic
    (non-zero exit, stderr contains the deterministic
    panic message).
  - `match_in_reply_to_plumbs_correlation_id` — assert
    the second turn's `match_in_reply_to` matches the
    canonical id of the first turn's emitted
    `tool_request` (the dispatcher must use the same
    id on emit and on subsequent match).
  - `mutual_exclusion_with_singular_env` — set both
    `RFL_OPENAI_STUB_SCRIPTED_TURNS` and the singular
    env; assert the stub exits non-zero with the
    mutual-exclusion error.
- **Why.** Scope §E2. Closes Phase E acceptance and
  pre-validates the J2 stub-mode invocation.
- **Depends on.** c14.
- **Acceptance.** All four `#[test]` cases green.
- **Files touched.** One new test file. Total ~180 lines.
- **Size.** small-to-medium.
- **Scope sections.** §E2.

### Phase F — `nix build .#rafaello` package repair (3 commits)

#### c16 — feat(rafaello-nix): `cargoBuildFlags` expansion to the package build set (8 packages)

- **What.** Scope §F1 + scope §"Internal split" row 16
  forced-monolithic. Replace the live
  `cargoBuildFlags = [ "-p" "rafaello" ]` in
  `rafaello/nix/package.nix:16` with the **package build
  set** (round-4 N-1 rename: this list names Cargo
  packages; the installed-binary set is a 1:1 derivation
  of these but conceptually distinct):
  ```nix
  cargoBuildFlags = [
    "-p" "rafaello"
    "-p" "rafaello-tui"
    "-p" "rafaello-openai"
    "-p" "rafaello-openai-stub"
    "-p" "rafaello-readfile"
    "-p" "rafaello-mailcat"
    "-p" "rafaello-mockprovider"
    "-p" "rafaello-fetch"
  ];
  ```
  `rafaello-bus-fixture` is **excluded** (owner-judgment
  item 9 default — test-shaped fixture, not user-facing).
  `rafaello-openai-stub` is **included** (owner-judgment
  item 13 — required by the J2 deterministic walkthrough +
  the headline integration test).
  No `postInstall` changes in this commit; that lands in
  c17. The result of this commit alone is: `nix build
  .#rafaello` produces a tree with all eight binaries
  flat in `$out/bin/` — wrong final layout, but a
  per-commit green-bar holds because c17 lands the
  `postInstall` reshape immediately.
- **Why.** Scope §F1. Forced-monolithic per scope row 16
  (Nix evaluation is whole-flake; the package list is
  one expression). Splitting into "rafaello + rafaello-tui"
  then "the rest" buys nothing — the build either
  expands fully or stays single-package.
- **Depends on.** baseline.
- **Acceptance.**
  - `nix build .#rafaello` succeeds on Linux + macOS
    (manual run inside the agent worktree; CI gate in
    c18).
  - `nix-store --query --references
    /nix/store/<rafaello-out>/bin` lists all eight
    binaries flat in `$out/bin/` (pre-c17 layout).
  - No tests in this commit — F1 is a Nix-evaluation
    delta; integration validation lands in c17 / c18.
- **Files touched.** `rafaello/nix/package.nix` (~10 line
  cargoBuildFlags rewrite).
- **Size.** small.
- **Scope sections.** §F1, owner-judgment items 9 + 13.

#### c17 — feat(rafaello-nix): bundled plugin trees in `postInstall` + plugin binaries moved into `share/rafaello/plugins/<plugin>/bin/`

- **What.** Scope §F2 + PP1 invariant (round-4 B-1
  closure). Extend `rafaello/nix/package.nix`'s
  `postInstall` (or add one if absent) to:
  1. For each of the bundled plugins
     (`rfl-mailcat`, `rfl-readfile`, `rfl-openai`,
     `rfl-mockprovider`, `rafaello-fetch`,
     `rfl-openai-stub`):
     - Create `$out/share/rafaello/plugins/<plugin>/`.
     - Copy the in-tree manifest tree (from c06):
       `rafaello.toml`, `openrpc.json`, any `schemas/`
       directory.
     - Move the Cargo-produced binary (currently at
       `$out/bin/<plugin-bin>`) to
       `$out/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`
       as a **real file** (not a symlink — scope PP1
       containment invariant; `compile::resolve_entry`
       rejects targets escaping `package_dir`). Use
       `mv` (or `cp` + `rm` if the binary is a
       store-path symlink) to preserve the canonical
       file shape.
  2. Final layout assertion (in the nix build itself or
     as a post-build script in c18): only `$out/bin/rfl`
     and `$out/bin/rfl-tui` remain at the top level;
     every other plugin binary lives inside its plugin
     directory.
- **Why.** Scope §F2 + PP1. The `rfl install` runtime
  resolver (c05's resolution arm 2) opens
  `share/rafaello/plugins/<plugin>/rafaello.toml`; the
  PP1 copy step copies the entire plugin tree
  (including `bin/<plugin-bin>`) into
  `.rafaello/plugins/<topic-id>/`, where
  `compile::resolve_entry` canonicalises the entry path
  and rejects anything escaping `package_dir`. A symlink
  into `$out/bin/` would canonicalise out, so F2 stores
  the real binary inside the plugin dir.
- **Depends on.** c16, c06 (the in-tree manifests).
- **Acceptance.** Tests:
  - `rafaello/tests/nix_build_layout.rs` (gated
    `#[cfg(target_os = "linux")]`, runs the
    `nix build .#rafaello` invocation) — asserts:
    - `$out/bin/` contains exactly `rfl` and `rfl-tui`.
    - For each of the six bundled plugins,
      `$out/share/rafaello/plugins/<plugin>/rafaello.toml`
      exists and parses.
    - `$out/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`
      exists and `fs::metadata(...)
      .file_type().is_file()` is true.
    - `compile::resolve_entry(
      $out/share/rafaello/plugins/<plugin>/,
      &manifest.entry)` returns `Ok(<canonical>)` inside
      the plugin dir.
  - Manual verification step recorded in the per-commit
    prompt: `nix build .#rafaello && ls -la
    ./result/bin/ ./result/share/rafaello/plugins/`.
- **Files touched.** `rafaello/nix/package.nix`
  (postInstall stanza, ~30 lines); one new test file.
  Total ~120 lines.
- **Size.** medium (body-justified: the postInstall
  block is one atomic Nix expression that must reshape
  the entire `$out` tree consistently; splitting per
  plugin would land intermediate broken layouts).
- **Scope sections.** §F2, PP1.

#### c18 — feat(rafaello-ci): macOS + Linux CI matrix for `nix build .#rafaello` + `cargo test --workspace --features test-fixture`

- **What.** Scope §F3 + scope §"Acceptance summary" macOS
  CI green hard gate (m3/m4/m5a/m5b precedent). Extend
  `.github/workflows/rafaello.yml` (or the live workflow
  filename) with a `nix-build` job matrix:
  ```yaml
  strategy:
    matrix:
      os: [ubuntu-latest, macos-latest]
  steps:
    - uses: cachix/install-nix-action@v25
    - run: nix build .#rafaello
    - run: nix develop .#rafaello --impure --command
            cargo test --manifest-path rafaello/Cargo.toml
            --workspace --features test-fixture
  ```
  macOS leg gates retrospective ratification (scope
  §"Acceptance summary" hard gate). Per scope §"Risks"
  and the m2 §5.7 push-to-CI-early lesson, the Phase C
  syd-pty exercise runs inside this CI matrix during m6
  implementation, not at retrospective time. Linux-only
  tests stay gated `#[cfg(target_os = "linux")]`; macOS
  must be green on the rest.
- **Why.** Scope §F3. The Phase C syd-pty fix is the
  m5a-RATIFIED carryover whose CI exercise has been
  deferred since m5a; m6 closes that punchlist by
  landing the fix and exercising it in CI in the same
  milestone.
- **Depends on.** c16, c17 (the macOS leg of `nix build
  .#rafaello` consumes c17's reshape).
- **Acceptance.**
  - `.github/workflows/rafaello.yml` (or the live name)
    has both `ubuntu-latest` and `macos-latest` jobs;
    both run `nix build .#rafaello`.
  - First push of the m6 branch triggers a CI run; both
    legs go green (manual operator confirmation; URL
    captured in J1 §4).
- **Files touched.** `.github/workflows/rafaello.yml`
  (job-matrix expansion, ~30 lines). Total ~30 lines.
- **Size.** small.
- **Scope sections.** §F3, §"Acceptance summary" hard
  gate.

### Phase G — Homebrew distribution (2 commits — G.β default)

Owner-judgment item 5 default locked at G.β (separate tap
fetching Nix-built tarballs). G3 install-smoke folded into
J1 §G per scope row 21.

#### c19 — feat(homebrew): `Formula/rafaello.rb` formula + tap-pointer in `homebrew/`

- **What.** Scope §G1 + round-5 N-2 layout fold. New file
  `homebrew/rafaello.rb` (committed in-repo as a
  fixture; the owner symlinks it into the
  `luizribeiro/homebrew-rafaello` tap repo at owner-action
  time per scope §G1). Formula content:
  ```ruby
  class Rafaello < Formula
    desc "v1 demo-ready CLI for the rafaello agent"
    homepage "https://github.com/luizribeiro/lab"
    version "<populated-by-G2-on-release>"

    on_arm do
      on_linux do
        url "<aarch64-linux tarball URL>"
        sha256 "<aarch64-linux sha>"
      end
      on_macos do
        url "<aarch64-darwin tarball URL>"
        sha256 "<aarch64-darwin sha>"
      end
    end

    on_intel do
      on_linux do
        url "<x86_64-linux tarball URL>"
        sha256 "<x86_64-linux sha>"
      end
    end

    def install
      bin.install "bin/rfl"
      bin.install "bin/rfl-tui"
      (share/"rafaello/plugins").install Dir["share/rafaello/plugins/*"]
    end

    test do
      system bin/"rfl", "--version"
    end
  end
  ```
  The formula installs **only `rfl` + `rfl-tui` into
  `<prefix>/bin/`**; the bundled plugin trees go under
  `<prefix>/share/rafaello/plugins/<plugin>/`, including
  each plugin's `bin/<plugin-bin>` as a real file inside
  that directory (round-5 N-2 layout). No plugin binaries
  in `<prefix>/bin/`. `x86_64-darwin` is omitted (scope
  §"Out of scope" item 15).
- **Why.** Scope §G1 — actionable G.β default per
  owner-judgment item 5. The in-repo fixture-copy gives
  m6 an artifact to test + version; the tap-repo is an
  owner-action follow-up captured in J1 §G.
- **Depends on.** c16, c17 (the tarball-source layout
  the formula installs).
- **Acceptance.**
  - `brew style homebrew/rafaello.rb` clean (manual
    verification step on macOS; recorded in the
    per-commit agent prompt).
  - `homebrew/rafaello.rb` parses as a Ruby file (CI
    can `ruby -c homebrew/rafaello.rb`).
- **Files touched.** `homebrew/rafaello.rb` (new, ~40
  lines).
- **Size.** small.
- **Scope sections.** §G1, owner-judgment item 5.

#### c20 — feat(rafaello-ci): release-tag automation — `nix build .#rafaello` per arch + tarball upload + formula SHA pin

- **What.** Scope §G2. New
  `.github/workflows/rafaello-release.yml` triggered on
  `v*` tags:
  ```yaml
  on:
    push:
      tags: ['v*']
  jobs:
    build-and-upload:
      strategy:
        matrix:
          include:
            - os: ubuntu-latest
              system: x86_64-linux
            - os: ubuntu-latest          # cross via nix
              system: aarch64-linux
            - os: macos-14               # Apple-silicon
              system: aarch64-darwin
      runs-on: ${{ matrix.os }}
      steps:
        - uses: actions/checkout@v4
        - uses: cachix/install-nix-action@v25
        - run: nix build .#packages.${{ matrix.system }}.rafaello
        - run: tar czf rafaello-${{ github.ref_name }}-${{ matrix.system }}.tar.gz -C result .
        - uses: softprops/action-gh-release@v2
          with:
            files: rafaello-${{ github.ref_name }}-${{ matrix.system }}.tar.gz
  ```
  After upload, a follow-up `update-formula` job (or a
  small Ruby helper script in `homebrew/update-shas.rb`)
  rewrites `homebrew/rafaello.rb`'s placeholder URLs +
  SHA256 fields with the tag's release artifacts; the
  rewritten formula is committed to the tap repo by an
  owner-action follow-up (J1 §G). The aarch64-linux
  job uses `aarch64-linux` via Nix's cross-build
  facility from `ubuntu-latest`; if the cross-build is
  unstable, swap to the `ubuntu-latest-arm64` runner
  (available in GitHub-hosted runners as of 2024).
- **Why.** Scope §G2 — closes G.β's "release-tag
  automation" deliverable. Three arches matching
  `flake.nix:24-28` (round-3 M-3 narrowing).
  `x86_64-darwin` deferred to v2 per scope §"Out of
  scope" item 15.
- **Depends on.** c16, c17, c19.
- **Acceptance.**
  - `.github/workflows/rafaello-release.yml` exists and
    is well-formed (CI workflow-syntax check via
    `actionlint` or the GitHub Actions linter).
  - The `update-formula` job (or `homebrew/update-shas.rb`
    script) is idempotent — re-running over a
    populated formula leaves it byte-stable.
  - End-to-end exercise of the workflow is deferred
    until the v0.1 → main merge cuts an actual `v*`
    tag; J1 §G captures the run URL.
- **Files touched.**
  `.github/workflows/rafaello-release.yml` (~60 lines);
  `homebrew/update-shas.rb` (~30 lines). Total ~100
  lines.
- **Size.** small-to-medium.
- **Scope sections.** §G2.

### Phase H — README + CONTRIBUTING pass (2 commits)

#### c21 — docs(rafaello): `rafaello/README.md` rewrite — 5-line bootstrap + troubleshooting + pre-m6 workaround subsection

- **What.** Scope §H1 + hard requirements #4 + #5.
  Replace the placeholder `rafaello/README.md` with:
  1. One-paragraph project summary (cite
     `plans/overview.md` §16 for v1 scope cut).
  2. The 5-line bootstrap (verbatim from scope §"Hard
     requirements" #4):
     ```
     cd ~/your/project
     nix develop .#rafaello --impure --command rfl init
     export LITELLM_API_KEY=…
     nix develop .#rafaello --impure --command rfl install rfl-mailcat
     nix develop .#rafaello --impure --command rfl chat
     ```
  3. Architecture-at-a-glance pointer to
     `plans/overview.md`.
  4. **Troubleshooting** section. Primary remediation:
     "make sure you're inside `nix develop .#rafaello
     --impure` (which exports `CARGO_BIN_EXE_syd-pty`),
     or install the m6-or-newer release that ships the
     lockin sandbox `syd-pty` discovery fix."
  5. **Pre-m6 workaround** subsection (round-2 N-2
     framing): documents the manual
     `CARGO_BIN_EXE_syd-pty=$(which syd-pty)` recipe
     under a clear banner "use only against pre-m6
     builds — m6+ does not need this."
  6. Installation instructions covering the Nix flake
     path (`nix develop .#rafaello --impure`) and the
     Homebrew path (`brew tap luizribeiro/rafaello &&
     brew install rafaello`, owner-judgment item 5
     default G.β).
- **Why.** Scope §H1 + hard requirements #4 + #5.
  Roadmap-row "documentation pass on
  `rafaello/README.md`" + the 5-line bootstrap
  literal-text deliverable.
- **Depends on.** c01–c10 (every flow the README
  describes), c19 (Homebrew install instructions).
- **Acceptance.**
  - `rafaello/README.md` exists and contains the
    verbatim 5-line bootstrap (per scope §H1).
  - Manual verification: the 5-line bootstrap executes
    against the dev LiteLLM endpoint and lands in a
    functioning `rfl chat` session (recorded as J2's §1
    walkthrough plus the §5 tmux recording).
  - `markdown-lint` (or the project-pinned md linter)
    clean.
- **Files touched.** `rafaello/README.md` (rewrite,
  ~180 lines from the ~30-line placeholder). Total
  ~180 lines.
- **Size.** small-to-medium.
- **Scope sections.** §H1, hard requirements #4 + #5.

#### c22 — docs(rafaello): `CONTRIBUTING.md` rewrite — dev-shell entry, plans structure, per-commit code-review expectation, rebase-no-force branch model

- **What.** Scope §H2. Replace the placeholder
  `CONTRIBUTING.md` with:
  1. Dev-shell entry instructions (`nix develop
     .#rafaello --impure` per the m0 §4.6 gotcha
     about `--impure`).
  2. The milestone / plans / streams structure (one
     paragraph; cite `plans/README.md` for the
     workflow).
  3. The per-commit code-reviewer agent expectation
     per `~/.claude/CLAUDE.md` (every per-commit agent
     runs `code-reviewer` before committing).
  4. The rebase-no-force branch model
     (`decisions.md` row 33; `rafaello-v0.1` is the
     integration branch; m6 RATIFIED merges `v0.1` →
     `main` per row 33's terminal condition).
  5. Test-running invocations: `cargo test
     --workspace --features test-fixture`,
     `nix flake check`.
- **Why.** Scope §H2.
- **Depends on.** baseline.
- **Acceptance.**
  - `CONTRIBUTING.md` exists with the four sections
    above.
  - `markdown-lint` clean.
- **Files touched.** `CONTRIBUTING.md` (rewrite, ~100
  lines from the ~30-line placeholder). Total ~100
  lines.
- **Size.** small.
- **Scope sections.** §H2.

### Phase I — Coverage / regression-anchor sweep (3 commits)

m4/m5a/m5b §5 carryovers.

#### c23 — test(rafaello-core): `core_tools_list_registered_before_provider_spawn.rs` regression anchor

- **What.** Scope §I1 + m5a §5 row 14 / m5b §5 row 12
  carryover (owner-judgment item 11 default: in scope).
  New integration test at
  `rafaello/crates/rafaello-core/tests/core_tools_list_registered_before_provider_spawn.rs`.
  Asserts the m4-ratified invariant that
  `core.tools_list` RPC is registered on the broker
  **before** any provider plugin spawns. Uses the
  existing m5b broker test-ordering hook (or a sibling
  shape if the m5b hook is broker-private) to record
  the sequence of broker calls during a fixture
  `run_chat`; assert
  `Broker::register_rpc("core.tools_list", _)` precedes
  `PluginSupervisor::spawn(<provider-plugin>)`.
- **Why.** Scope §I1 + `decisions.md` row 49 — the
  invariant is load-bearing for the m4/m5a tool-catalog
  design. Defence-in-depth regression anchor; closes
  the m5a/m5b carryover unconditionally per
  owner-judgment item 11.
- **Depends on.** baseline (m5b retro merged).
- **Acceptance.** Test green; failure mode (regressing
  the order) reproduces deterministically (manually
  verified by temporarily swapping the
  `register_rpc` / `spawn` ordering in a local diff
  and re-running the test).
- **Files touched.** One new test file. Total ~80
  lines.
- **Size.** small.
- **Scope sections.** §I1, owner-judgment item 11.

#### c24 — feat(rafaello): `load.triggers.kind = "tool"` lazy-load fixture + integration test

- **What.** Scope §I2 + m4 §5.8 / m5a §5 row 8
  carryover (owner-judgment item 6 default: in scope).
  New fixture lock under
  `rafaello/fixtures/m6-lazy-load-tool/rafaello.lock`
  that wires a plugin with
  `bindings.load.triggers = [{ kind = "tool",
  tool = "<name>" }]` (the live manifest field per
  `streams/c-manifest/rfc-manifest.md` §"load triggers"
  / `decisions.md` row 42 lazy-load). New integration
  test at
  `rafaello/crates/rafaello/tests/lazy_load_tool_trigger_spawns_on_first_call.rs`
  — spawns `rfl chat` against the fixture lock;
  asserts the trigger-bound plugin is **not spawned**
  at session startup (verified via a process-list
  check or a supervisor test seam); the
  test-confirm-answers hook (`decisions.md` row 56)
  drives a tool call to the trigger tool; asserts the
  plugin **then** spawns and serves the call.
- **Why.** Scope §I2 — the `load.triggers.kind =
  "tool"` field is plumbed but never exercised
  end-to-end. Closes the m4-since carryover.
- **Depends on.** baseline.
- **Acceptance.** Test green.
- **Files touched.** One new fixture lock + sidecar
  plugin manifest + one new test file. Total ~180
  lines (fixture-package atomicity per m5b c20
  precedent).
- **Size.** small-to-medium (body-justified by
  fixture-package atomicity).
- **Scope sections.** §I2, owner-judgment item 6.

#### c25 — refactor(rafaello-core): ratify `#[allow(clippy::result_large_err)]` allows + comment-pin to `decisions.md` row 67

- **What.** Scope §I3 + owner-judgment item 8 default
  (ratify-by-keeping). m4 §5.5 / m5a §5 row 6 / m5b §5
  row 13 carryover. Identify every module-level
  `#![allow(clippy::result_large_err)]` site in the
  workspace (initial inventory:
  `rafaello/crates/rafaello-core/src/reemit/mod.rs:1`,
  `rafaello/crates/rafaello-core/src/agent/mod.rs:1`,
  any m5b additions); **keep** the allows and add a
  single-line comment immediately adjacent:
  ```rust
  // Module-level result_large_err allow ratified by
  // m6 per decisions.md row 67 — boxing the error
  // hierarchy is post-v1.
  #![allow(clippy::result_large_err)]
  ```
  No code changes; no `Box<ErrorType>` rewrites. The
  decisions row append lands in J3 retro.
- **Why.** Scope §I3 + owner-judgment item 8. Round-2
  M-5 fix: ratify means **keep** the allows + name
  the trade-off in a decisions row, not delete them.
  Boxing burns 5+ commits across `?` sites for
  negligible win (m4 retro §5.5 estimate).
- **Depends on.** baseline.
- **Acceptance.**
  - `rg "allow\(clippy::result_large_err\)"` enumerates
    the same site set both before and after this
    commit (no allows added or removed).
  - Each allowed site has the new comment-pin line
    immediately above the `#![allow]` attribute.
  - `cargo clippy --workspace --all-features
    -- -D warnings` green.
- **Files touched.** 2–4 source files (per the
  inventory). Total ~5 lines per file × ≤4 = ~20
  lines net.
- **Size.** small.
- **Scope sections.** §I3, owner-judgment item 8.

### Phase J — Manual-validation transcript (2 commits + 1 retro)

#### c26 — docs(rafaello): `manual-validation.md` §1–§7 + §G skeleton + audit-dump shape

- **What.** Scope §J1. Extend the existing 9-line m5b
  c15 file at
  `rafaello/plans/milestones/m6-polish-release/manual-validation.md`
  (or whichever path m5b ratified — verify against
  m5b's commits list during agent-prompt construction)
  with seven sections + the Phase G install smoke
  (folded from scope row 21 per the default G.β
  layout):
  - **§1** — `rfl chat` cold-start walkthrough (the
    5-line bootstrap, post-init).
  - **§2** — `rfl install rfl-mailcat` walkthrough
    (positional-arg shape from Phase B; declares the
    `send-mail` tool with `sinks = ["mail"]`).
  - **§3** — wire-shape note (preserved from m5b
    c15).
  - **§4** — macOS CI run URL (driver post-merge
    sweep — m5a §5 row 9 / m5b §5 row 9; placeholder
    URL until first green merge candidate).
  - **§5** — placeholder for the J2 tmux recording
    (filled by c27).
  - **§6** — audit-log inspection walkthrough using
    the new `rfl audit` CLI (Phase D); concrete
    invocation:
    ```
    rfl audit --project-root <PROJECT> \
      --kind confirm_request --kind confirm_allowed
    ```
    Expected output shape: rows matching the
    `<seq>  <at>  <kind>  <request_id>  <payload-summary>`
    format from c11.
  - **§7** — syd-pty failure-mode reproduction + the
    fix verification (hard requirement #5's
    "documented for posterity" half). Concrete
    repro: in a clean shell without `nix develop`,
    invoke a release `rfl chat`; verify the
    `setup_pty` error fires; then run inside `nix
    develop .#rafaello --impure`; verify success.
    The post-m6 release narrative records that the
    lockin sandbox fix obviates the manual env-var
    recipe.
  - **§G** — Homebrew install smoke (per the
    chosen-model G.β default): `brew tap
    luizribeiro/rafaello && brew install rafaello &&
    rfl init && rfl install rfl-mailcat && rfl
    chat`. Owner-judgment item 10 confirms manual
    validation only (no CI workflow that
    `brew install`s).
- **Why.** Scope §J1 + m5a/m5b §5 row 11 carryovers.
  Lays the skeleton ahead of J2's §5 fill so that
  the manual-validation surface is testable in
  ratification order (operator can walk §1 → §G
  sequentially against the m6 build).
- **Depends on.** c11, c12, c13 (audit CLI), c19,
  c20 (Homebrew formula + release automation), c08,
  c09, c10 (syd-pty fix for §7).
- **Acceptance.** File exists with all eight
  sections; §5 contains a placeholder line "filled
  by c27"; `markdown-lint` clean.
- **Files touched.**
  `rafaello/plans/milestones/m6-polish-release/manual-validation.md`
  (rewrite, ~280 lines from m5b's 9-line baseline).
  Total ~280 lines.
- **Size.** medium (body-justified by skeleton
  fan-out across 8 sections; m4 c30 / m5a c40 manual-
  validation skeleton precedent).
- **Scope sections.** §J1, m5a/m5b §5 row 11.

#### c27 — docs(rafaello): tmux-driven §5 recording + transcripts under `transcripts/section-5/`

- **What.** Scope §J2 + hard requirement #3. Execute
  the tmux script verbatim from scope §J2 (round-4
  B-2 form — every `rfl <subcommand>` runs via
  `--project-root "$PROJECT"` from the lab worktree;
  final copy step lands captures under
  `rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`).
  Commit the six captured transcript files:
  - `01-after-launch.txt`
  - `02-modal.txt`
  - `03-response.txt`
  - `04-audit.txt`
  - `05-sqlite-audit.txt`
  - `06-sqlite-entries.txt`
  Fill `manual-validation.md` §5 with:
  - The exact tmux script (verbatim from scope §J2).
  - References to each of the six transcript files.
  - The expected substrings each `grep` step
    asserts (from scope §J2's grep block):
    `" confirm "`, `"send-mail via"`, `"sinks: mail"`,
    `"alice@example.com"`, `"confirm_request"`,
    `"confirm_allowed"`.
  - The `Ctrl-C` quit (owner-judgment item 12 default
    — TUI input-mode handler doesn't bind `q`).
  Also lands the demo-bar integration test that
  programmatically exercises the same flow:
  `rafaello/crates/rafaello/tests/rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`
  (scope §"Demo bar" / §"Headline integrated demo")
  — uses `rfl-openai-stub` (c14 multi-turn) +
  `RFL_TUI_TEST_CONFIRM_ANSWERS` (m5b row 56 hook) to
  drive `init → install → chat → confirm → persist`
  deterministically; asserts the `entries` table has
  the canonical `tool_call` + `tool_result` +
  assistant-message rows; asserts the `audit_events`
  table has the `confirm_request` + `confirm_allowed`
  rows; asserts the chat process exits cleanly on
  `Ctrl-C` (round-3 J2 correction).
- **Why.** Scope §J2 + hard requirement #3 — the v1
  canonical proof of life. The tmux capture is the
  manual evidence; the programmatic integration test
  is the regression-grade companion that runs in CI.
  Both share the same `rfl-mailcat` / `send-mail`
  demo tool (round-2 B-3 lock-in).
- **Depends on.** c01–c25 (every flow J2 exercises).
- **Acceptance.**
  - Six transcript files exist under
    `rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`.
  - Each `grep` step in scope §J2's script returns
    non-empty (transcript-file-existence is the
    operator-witnessed evidence; the per-commit agent
    runs the tmux script live during commit
    construction).
  - `rafaello/crates/rafaello/tests/rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`
    green.
  - `manual-validation.md` §5 references all six
    files by name.
- **Files touched.**
  `rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`
  (six new transcript files, ~200 lines of captured
  output total);
  `rafaello/plans/milestones/m6-polish-release/manual-validation.md`
  (§5 fill, ~80 lines); one new demo-bar integration
  test file (~180 lines). Total ~460 lines.
- **Size.** medium-to-large (body-justified by the
  headline-demo aggregation per scope §"Demo bar" +
  m5a c39 / m5b c23 EXFIL1-headline precedent: the
  tmux transcript files + the programmatic
  integration test are bound to the same operator-
  witnessed flow; splitting forces a partial
  ratification window where §5 is documented but
  un-tested or vice versa).
- **Scope sections.** §J2, §"Demo bar", hard
  requirement #3.

### Retrospective reservation (1 slot)

#### c28 — docs(rafaello-m6): retrospective + `decisions.md` rows 59–68 + glossary additions (RESERVED)

- **What.** Scope §J3 + scope §"Glossary additions".
  Reserved budget slot for the m6 retrospective phase.
  Per `plans/README.md` Phase 3, this slot lands after
  every implementation commit (c01–c27) ships and
  pi reviews the milestone diff. The retrospective
  commit body lands:
  - `retrospective.md` (claude + pi co-authored
    against `scope.md` + the m6 commit history).
  - `decisions.md` row appends — placeholders 59–68
    per scope §J3 (actual numbers assigned at
    retrospective ratification time per the
    append-only convention; current row tail is 58):
    - **59** roadmap-text reconciliation.
    - **60** `rfl init` bundled-`rfl-openai` lock
      entry + decline-empty-lock semantics.
    - **61** `rfl install <plugin>` bundled-tree
      discovery path.
    - **62** syd-pty discovery belt-and-braces (no
      `pty:off` fallback at lockin).
    - **63** `rfl audit` read CLI semantics (default
      ordering, filter set, no-join-against-entries).
    - **64** `rfl-openai-stub` scripted-turns env +
      TOML schema + exhaustion-panics + mutual
      exclusion.
    - **65** `nix build .#rafaello` package shape
      (release binary set excluding fixtures + PP1
      plugin trees with real binaries).
    - **66** Homebrew distribution model (G.β
      ratified default).
    - **67** `result_large_err` ratification (allows
      kept; boxing post-v1).
    - **68** m6 RATIFICATION closes `rafaello-v0.1
      → main` merge.
  - `glossary.md` additions per scope §"Glossary
    additions": `rfl init`, `rfl install <plugin>`,
    `rfl audit`, `syd-pty discovery`,
    `rfl-openai-stub scripted turns`; banner-pointers
    on the existing `rafaello.lock` and `Bundled
    provider` entries.
  - Stream A drift candidates folded inline (if any
    surface during implementation; m6 §"Inputs" notes
    nothing new known).
  - The `rafaello-v0.1 → main` ff-merge command,
    pinned with the m6-RATIFIED tip hash, executed by
    the milestone driver post-ratification.
- **Why.** Scope §J3 + `plans/README.md` Phase 3
  ratification flow. Reserved (not drafted in this
  round 1) — retrospective + decisions row text is
  authored adversarially with pi after every
  implementation commit lands, per the m0/m1/m2/m4/m5a/m5b
  retrospective-pi-review precedent.
- **Depends on.** c01–c27.
- **Acceptance.** Out of scope for round 1; lands in
  the retrospective phase. The slot is reserved here
  so the 28-slot budget closes.
- **Files touched.** TBD at retrospective time.
- **Size.** medium (m1/m2/m4/m5a/m5b retrospective
  precedent — typically 200–400 lines plus
  decisions/glossary appends).
- **Scope sections.** §J3, §"Glossary additions".

---

## Acceptance traceability appendix

Every scope §"In scope" acceptance bullet mapped to a
commit row. Used by pi to spot-check that nothing
dropped.

### Phase A — `rfl init`

| Scope acceptance | Commit |
|---|---|
| `rfl init --help` exposes `--yes`, `--force`, `--project-root` | c01 |
| `rfl init` with existing lock is idempotent (one-line notice, exit 0) | c01 |
| `rfl init --yes` writes default lock against live `Lock::from_toml` schema | c02 |
| `rfl init` materialises `${PROJECT_ROOT}/.rafaello/plugins/<topic-id-of-openai>/rafaello.toml` (PP1) | c02 |
| `bin/rfl-openai` inside the PP1 dir is a regular file (not symlink) | c02 |
| Lock's `digest`/`manifest_digest` match the copied tree | c02 |
| `compile::resolve_entry(plugin_dir, "bin/rfl-openai")` returns Ok inside `plugin_dir` (no `EntryEscape`) | c02 |
| `rfl init --force` rewrites lock + package dir byte-for-byte from defaults | c02 |
| `rfl init` declining the prompt writes an empty lock + no PP1 copy | c03 |
| Lock TOML round-trips byte-stably (`from_toml → to_toml → from_toml`) | c04 |
| Phase A integration tests pass against the in-tree bundled `rfl-openai` shape | c04 |

### Phase B — `rfl install <plugin>`

| Scope acceptance | Commit |
|---|---|
| `InstallArgs` carries optional `--fixture: Option<PathBuf>` + positional `plugin: Option<String>` + `--project-root: Option<PathBuf>` with clap `conflicts_with` / `required_unless_present` | c05 |
| `rfl install rfl-mailcat` resolves bundled source under `share/rafaello/plugins/rfl-mailcat/` | c05 |
| `--fixture <path>` arm still works (m5a regression anchor) | c05 |
| `rfl install nonsense` exits non-zero with `BundledPluginNotFound` | c05 |
| `rfl install` with neither/both args is a clap error | c05 |
| `rfl install rfl-mailcat --project-root <tmpdir>` writes lock + PP1 under `<tmpdir>` | c05 |
| `compile::resolve_entry` containment passes for installed plugin | c05 |
| Each bundled plugin crate (`rfl-mailcat`, `rfl-readfile`, `rfl-openai`, `rfl-mockprovider`, `rafaello-fetch`) ships `rafaello.toml` + `openrpc.json` | c06 |
| `rfl install` writes a valid lock entry for each of the four non-openai bundled plugins | c07 |
| `rfl init → rfl install` composes without conflict (PP1 dirs coexist) | c07 |

### Phase C — syd-pty discovery

| Scope acceptance | Commit |
|---|---|
| Devshell exports `CARGO_BIN_EXE_syd-pty` (Linux) | c08 |
| `lockin::sandbox::resolve_syd_pty_path` resolution order: spec → env → sibling → PATH → hard-error | c09 |
| Lockin sandbox sets `CARGO_BIN_EXE_syd-pty` on the syd child via `Command::env` | c09 |
| `SandboxError::SydPtyNotFound` returned on resolution failure; **no** `pty:off` fallback | c09 |
| `fake-syd` `[[bin]]` registered in `lockin/crates/sandbox/Cargo.toml` under `test-fixture` feature | c09 |
| `test-fixture` feature added to `[features]` block | c09 |
| Fake-syd records env explicitly-set arm | c10 |
| Fake-syd records env sibling-discovery arm | c10 |
| Fake-syd hard-error arm (no `pty:off`) | c10 |
| Rafaello-side smoke `rfl_chat_in_devshell_propagates_cargo_bin_exe_syd_pty.rs` green inside `nix develop .#rafaello --impure` | c10 |

### Phase D — `rfl audit`

| Scope acceptance | Commit |
|---|---|
| `rfl audit --project-root <PATH>` resolves DB under `<PATH>/.rafaello/state/session.sqlite` | c11 |
| Default query `SELECT seq, at, kind, request_id, payload FROM audit_events ORDER BY seq` | c11 |
| Render format `<seq>  <at>  <kind>  [<request_id>|-]  <payload-summary>` | c11 |
| Empty-DB banner `"no audit events"` | c11 |
| `--kind` repeatable, validates against `AuditKind::as_str` | c12 |
| `--since 1h`/`30m`/`24h` parses + thresholds | c12 |
| `--request-id` filters with no join against `entries` (SQL-trace asserted) | c12 |
| `--json` emits one JSON object per row | c12 |
| `--full` disables payload truncation | c12 |
| Renders m5b taint variants (`confirm_request_taint_attached` etc.) | c13 |
| Filter combinations AND-compose | c13 |

### Phase E — `rfl-openai-stub` scripted turns

| Scope acceptance | Commit |
|---|---|
| `RFL_OPENAI_STUB_SCRIPTED_TURNS` parses scope §E1 TOML schema | c14 |
| Mutual exclusion with singular env | c14 (build); c15 (test) |
| Two-turn happy path emits `tool_request` then `assistant_message` | c15 |
| Exhaustion panics deterministically | c15 |
| `match_in_reply_to` plumbs correlation id | c15 |

### Phase F — `nix build .#rafaello` repair

| Scope acceptance | Commit |
|---|---|
| `cargoBuildFlags` builds the 8-package release set (bus-fixture excluded) | c16 |
| `postInstall` reshapes `$out` to PP1 layout: `bin/` carries `rfl` + `rfl-tui`; plugins under `share/rafaello/plugins/<plugin>/bin/<plugin-bin>` as real files | c17 |
| `compile::resolve_entry` containment holds against the F-built plugin dirs | c17 |
| CI matrix runs `nix build .#rafaello` on `ubuntu-latest` + `macos-latest`; both green | c18 |
| macOS CI green is the ratification gate | c18 |

### Phase G — Homebrew (G.β default)

| Scope acceptance | Commit |
|---|---|
| `Formula/rafaello.rb` installs `rfl` + `rfl-tui` under `<prefix>/bin/`; bundled plugin trees under `<prefix>/share/rafaello/plugins/<plugin>/bin/<plugin-bin>` (round-5 N-2) | c19 |
| Three arches: `aarch64-darwin`, `aarch64-linux`, `x86_64-linux` | c19 + c20 |
| Release-tag automation builds + uploads tarballs per arch | c20 |
| Formula-SHA update step idempotent | c20 |
| `brew install` install smoke recorded in `manual-validation.md` §G | c26 (skeleton) + owner-action at v0.1 → main merge |

### Phase H — README + CONTRIBUTING

| Scope acceptance | Commit |
|---|---|
| `rafaello/README.md` carries the verbatim 5-line bootstrap | c21 |
| Troubleshooting section names the m6+ fix path | c21 |
| Pre-m6 workaround subsection banner-flags the manual recipe | c21 |
| Installation instructions cover Nix + Homebrew paths | c21 |
| `CONTRIBUTING.md` covers dev-shell, plans-structure, code-reviewer, branch model | c22 |

### Phase I — Coverage / regression anchors

| Scope acceptance | Commit |
|---|---|
| `core_tools_list_registered_before_provider_spawn` test green | c23 |
| `load.triggers.kind = "tool"` lazy-load fixture + spawn-on-first-call test | c24 |
| `result_large_err` allows retained with comment-pin to row 67 | c25 |

### Phase J — Manual validation

| Scope acceptance | Commit |
|---|---|
| `manual-validation.md` carries §1–§7 + §G | c26 |
| §6 audit-CLI walkthrough | c26 |
| §7 syd-pty failure-mode reproduction + fix verification | c26 |
| §5 tmux recording: six transcripts under `transcripts/section-5/` | c27 |
| Greps assert `" confirm "`, `"send-mail via"`, `"sinks: mail"`, `"alice@example.com"`, `"confirm_request"`, `"confirm_allowed"` | c27 |
| `Ctrl-C` quit per owner-judgment item 12 | c27 |
| Demo-bar integration test `rfl_chat_demo_bar_init_install_chat_confirm_persist.rs` | c27 |

### Retrospective

| Scope acceptance | Commit |
|---|---|
| `retrospective.md` written | c28 |
| `decisions.md` rows 59–68 appended | c28 |
| `glossary.md` additions (`rfl init`, `rfl install <plugin>`, `rfl audit`, `syd-pty discovery`, `rfl-openai-stub scripted turns`) | c28 |
| `rafaello-v0.1 → main` ff-merge executed | c28 |

---

## Cross-checks

- **Every scope §"In scope" item maps to ≥1 commit row.**
  §A1 → c01. §A2 → c02. §A3 → c03. §A4 → c04 (+ c02/c03
  per-commit assertions). §B1 → c05. §B2 → c06. §B3 →
  c07 (+ c05 per-commit assertions). §C1 → c08. §C2 →
  c09. §C3 → c10. §D1 → c11. §D2 → c12. §D3 → c13 (+
  c11/c12 per-commit assertions). §E1 → c14. §E2 →
  c15. §F1 → c16. §F2 → c17. §F3 → c18. §G1 → c19.
  §G2 → c20. §G3 → c26 (folded per scope row 21). §H1
  → c21. §H2 → c22. §I1 → c23. §I2 → c24. §I3 → c25.
  §J1 → c26. §J2 → c27. §J3 → c28 (reserved).
- **PP1 invariant (load-bearing across A2 / B1 / F2).**
  c02 lands the PP1 copy on the init side; c05 lands it
  on the install side (both fixture + positional arms);
  c17 lands the matching `postInstall` source layout
  with real plugin binaries inside the plugin dir.
  Every PP1-consuming row asserts
  `compile::resolve_entry` returns Ok inside
  `package_dir` (no `EntryEscape`).
- **Forced-monolithic rows justified inline.** c05
  (B1 `InstallArgs` cutover — scope §"Internal split"
  forced-monolithic row 5), c09 (C2 lockin sandbox —
  scope §"Internal split" forced-monolithic row 9),
  c16 (F1 `cargoBuildFlags` — scope §"Internal split"
  forced-monolithic row 16), c17 (F2 `postInstall`
  reshape — body-justified by tree-atomicity), c27
  (J2 transcripts + demo-bar test — m5a c39 / m5b
  c23 EXFIL1-headline precedent).
- **No synthetic-stub tests without successors** (m2
  retro §3.3). c01's stub-error assertion on the
  `NotYetImplemented` arm is **amended** in c02 to
  assert success (two-stage ladder, m0 §4.3). c05's
  `rfl_install_positional_resolves_to_bundled_plugin.rs`
  is amended in c06 to point at the in-tree
  bundled-plugin manifests (two-stage ladder).
- **Two-stage tests called out explicitly** (m0 retro
  §4.3). Two pairs:
  - c01 → c02 (`rfl_init_with_existing_lock_idempotent.rs`
    + the `NotYetImplemented` arm extended into success
    on the previously-failing invocation when c02
    lands the body).
  - c05 → c06 (`rfl_install_positional_resolves_to_bundled_plugin.rs`
    flips from synthetic fixture-release-tree to
    in-tree bundled-plugin manifests).
- **Per-commit agent prompts must inline the row text
  + every acceptance bullet verbatim** (m1 §4.2 / m5a
  operational guardrail; `plans/README.md` "Patterns
  from prior milestones"). The driver does NOT cite by
  row number.
- **Topic-id / env-var / manifest / lock paths match
  scope verbatim.** `builtin:openai@0.0.0`,
  `local:mailcat@0.0.0`, `topic_id::derive`,
  `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/rafaello.toml`,
  `<release-prefix>/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`,
  `CARGO_BIN_EXE_syd-pty`, `LOCKIN_SYD_PATH`,
  `RFL_OPENAI_STUB_SCRIPTED_TURNS`,
  `RFL_FAKE_SYD_RECORD_PATH`,
  `RFL_BUNDLED_PLUGINS_DIR`, `LITELLM_API_KEY`,
  `RFL_OPENAI_API_KEY_ENV`, `RFL_OPENAI_ENDPOINT_URL`,
  `RFL_OPENAI_MODEL = "vllm/qwen3.6-27b"`,
  `audit_events(seq, at, kind, request_id, payload)`,
  `confirm_request`, `confirm_allowed`,
  `confirm_request_taint_attached`,
  `plugin_publish_rejected_taint_superset`,
  `tool_request_taint_unioned_from_in_reply_to`. All
  spellings checked against scope.md round 5 RATIFIED.
- **No new workspace dep added by m6**. The
  `test-fixture` feature in
  `lockin/crates/sandbox/Cargo.toml` is a net-new
  feature flag, not a dep. The
  `rafaello-openai-stub` scripted-turns TOML parsing
  reuses the existing `toml` workspace dep (m5a/m5b
  precedent).
- **Workspace-wide cutover commits** (m0 §4.1
  precedent). c05, c09, c16 are the three explicit
  cutovers; bodies pin the forced-monolithic
  justification.
- **macOS CI green** is gated by c18 + carried through
  to retrospective ratification per scope §"Acceptance
  summary" hard gate.
- **`#[cfg(target_os = "linux")]` discipline.** Tests
  that require `syd` (c10's three fake-syd tests + the
  rafaello-side smoke) gate on Linux per scope
  §"Acceptance summary" exception clause.
- **Owner-judgment items resolution.** Items 0–13 are
  all ratified at default values per the `a0764b3`
  ratification commit. No commit row in this draft
  diverges from a ratified default. If a future round
  surfaces a default-revisit, the affected row gets
  re-shaped with an explicit body callout.

---

## Sizing summary

Round-1 sizing (CLAUDE.md `<100 lines / ≤5 files`
guideline applied, with body-justified larger rows
called out):

- **small** (≲50 LoC): 6 commits — c08, c13, c18,
  c22, c23, c25.
- **small-to-medium** (50–150 LoC): 8 commits — c01,
  c03, c06, c11 (NB borderline), c14, c15, c20, c24.
- **medium** (150–300 LoC, row-local body-justified):
  10 commits — c02, c05, c07, c09, c10, c12, c17, c19,
  c21, c26. Each carries an inline body justification
  pointing at the relevant scope §"Internal split"
  forced-monolithic clause or fixture-package-
  atomicity precedent.
- **medium-to-large** (300–500 LoC, body-justified):
  2 commits — c05 (B1 `InstallArgs` cutover), c27
  (J2 transcripts + demo-bar integration test).
- **large** (~500+ LoC, body-justified): 0 commits in
  round 1 default; if c05 or c27 grows past 500 LoC
  in implementation, the per-commit body retains the
  m5a c39 / m5b c23 precedent justification.

Total: 6 + 8 + 10 + 2 + 0 = **26 implementation
commits + 1 retrospective reservation**. Wait —
recount: c01–c25 are 25 commits, plus c26 + c27 = 27
implementation, plus c28 retro = 28 total. Sizing
buckets re-tallied:

- **small** (6): c08, c13, c18, c22, c23, c25.
- **small-to-medium** (8): c01, c03, c04, c06, c14,
  c15, c20, c24. (c04 added.)
- **medium** (11): c02, c05, c07, c09, c10, c11, c12,
  c17, c19, c21, c26.
- **medium-to-large** (2): not-named; c05 + c27 may
  drift here at implementation time.
- Plus c27 (medium-to-large body-justified) and c28
  (retrospective, sized at retrospective time).

Re-tallied total: 6 + 8 + 11 + 1 (c27) + 1 (c28
reserved) = **27 implementation + 1 retrospective =
28 slots**. ✓ matches scope row 1–29 with row 21
folded into J1 (=c26).

**Body-justified larger rows** (round 1 candidate
list; pi may push for further compression in round
2):

- **c05** (B1 `InstallArgs` cutover + bundled-source
  resolver + PP1 copy + 7 tests) — scope §"Internal
  split" forced-monolithic row 5.
- **c09** (C2 lockin sandbox `resolve_syd_pty_path` +
  spec field + call site + error variant + fake-syd
  `[[bin]]` registration) — scope §"Internal split"
  forced-monolithic row 9.
- **c17** (F2 `postInstall` `$out`-reshape across six
  bundled plugins) — body-justified by Nix-evaluation
  atomicity.
- **c27** (J2 §5 tmux transcripts + demo-bar
  integration test) — m5a c39 / m5b c23
  EXFIL1-headline precedent.

**Unsplittable cutovers** (m0 c08 / m4 c07 / m5a c14
precedent): c05, c09, c16, c17 (per the bodies). All
four carry the inline forced-monolithic justification.

Pi round budget on `commits.md`: **3–5 rounds** is the
round-1 expectation. m5b took 6; m6 has fewer
load-bearing invariants (no taint primitives, no §A9
fallback), so 3–5 is reasonable. Round 1 fold sentinel:
this commit lands the round-1 artifact and pings pi.

---

*End of m6 commits.md round 1 — claude-authored,
awaiting pi adversarial review. Phase distribution:
A:4 · B:3 · C:3 · D:3 · E:2 · F:3 · G:2 · H:2 · I:3 ·
J:2 · retro:1 = 27 implementation + 1 retro = 28
slots, matching scope row 1–29 with row 21 folded.
Three workspace-wide cutovers explicitly called out:
c05 (`InstallArgs` clap rewrite), c09 (lockin sandbox
syd-pty plumbing), c16 (`cargoBuildFlags` 8-package
expansion). PP1 invariant load-bearing across c02 /
c05 / c17.*
