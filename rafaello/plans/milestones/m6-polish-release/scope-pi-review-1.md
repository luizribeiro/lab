# m6 scope.md round-1 pi review

> Verdict: blocking
>
> Counts: B/7 M/6 N/4

I reviewed `scope.md` against the m6 driver pre-flight, the roadmap row, overview/decisions/glossary, every stream RFC, the m4/m5a/m5b §5 carryovers, the m5b review precedent, and live code under `rafaello/crates`, `rafaello/nix`, `flake.nix`, and `lockin/`.

Round 1 has the right intent — cold-start chat, syd-pty, audit CLI, release packaging, docs, and the overdue manual recording are the correct m6 surfaces. It is not implementation-ready yet. The blockers are mostly live-code contradictions: the package output already exists but is incomplete, the advertised 5-line `rfl install <tool>` command is not the live install shape, the audit SQL targets columns/tables that do not exist, the `rfl init` lock sketch uses the wrong table shape, and the syd-pty test can pass by disabling PTY rather than proving discovery. The headline demo also needs to be made single-valued and concrete enough to drive with tmux.

## Blockers

### B-1. Phase E is scoped as adding `.#rafaello`, but live flake already exposes it — incompletely

**Anchor:** `scope.md:107-109`, `scope.md:167-169`, `scope.md:430-448`, `scope.md:991-993`; live `flake.nix:71-73`, `rafaello/nix/default.nix:10-12`, `rafaello/nix/package.nix:16`.

**Issue:** The draft says there is no `.#rafaello` package output and scopes E1 as adding `packages.rafaello`. Live code already merges `rafaello.packages` into the root flake and `rafaello/nix/default.nix` already defines `packages.rafaello`. The real gap is that `package.nix` builds only `-p rafaello`, so the output likely lacks `rfl-tui`, `rfl-openai`, `rfl-readfile`, `rfl-mailcat`, `rfl-mockprovider`, `rfl-openai-stub`, `rfl-bus-fixture`, and `rafaello-fetch`.

**Recommendation:** Reframe Phase E as **repair/complete the existing package output**, not add a new output. Acceptance should assert `nix build .#rafaello` today is evaluated, then m6 expands the package to install every required binary and the Homebrew formula consumes that layout.

### B-2. The 5-line bootstrap uses `rfl install <tool>`, but live `rfl install` is fixture-only and m6 does not scope install UX

**Anchor:** `scope.md:81-85`, `scope.md:160-162`, `scope.md:493-497`, `scope.md:1058-1061`; live `rafaello/crates/rafaello/src/install.rs:32-43`.

**Issue:** Hard requirement #4 and README/acceptance require `nix develop --impure --command rfl install <tool>`. Live `InstallArgs` has no positional plugin/source argument; it requires `--fixture <path>`. The driver pre-flight had a Phase B for `rfl install <plugin>` ergonomics, but the scope dropped that phase and treats existing `rfl install` as sufficient.

**Recommendation:** Add an explicit install-UX phase or change the bootstrap line to the live supported command. For v1 demo readiness, the better fix is to scope `rfl install <tool>` as a real command shape with tests, because the README cannot ship a fixture-only command as the user bootstrap.

### B-3. The headline demo is not single-valued or concrete enough for the hard tmux recording gate

**Anchor:** `scope.md:51-54`, `scope.md:72-78`, `scope.md:561-585`, `scope.md:685-714`.

**Issue:** The roadmap demo says `init → install rfl-openai → install one tool`; Phase A says `rfl-openai` is pre-installed and never installed; Phase I §2 names `read-file`; Phase I §5 uses `send-mail`; the integrated demo says “Lock-bump installs `rafaello-fetch`” then says round 1 proposes `send-mail`. The tmux recording itself is still prose (“send-keys the bootstrap, capture-pane the modal...”) rather than concrete commands, prompt text, pane captures, and SQLite dump commands.

**Recommendation:** Pick one canonical demo flow and make every section match it. If the v1 proof is sink confirmation, use `send-mail` as the installed tool everywhere; if the roadmap `install rfl-openai` text is superseded by `rfl init` preinstall, explicitly reconcile that once at the top. Add literal tmux steps: session name, exact `tmux send-keys` commands, prompt string, allow key, quit key, and exact SQLite / `rfl audit` commands.

### B-4. `rfl audit` is specified against non-existent SQLite columns/joins

**Anchor:** `scope.md:357-374`; live `rafaello/crates/rafaello-core/src/audit/mod.rs:114-123`, `rafaello/crates/rafaello-core/src/session/mod.rs:99-122`.

**Issue:** C1 selects `seq, ts_unix_ms, kind, request_id, payload`, but the live `audit_events` schema is `seq, at, kind, request_id, payload`. C2 says `--request-id` joins `entries` on `call_id`; the live `entries` table has `id, seq, parent, kind, schema, payload, metadata, fallback, created_at` and no `call_id` column.

**Recommendation:** Scope `rfl audit` to the live schema: use `at` (or add a migration to introduce `ts_unix_ms`, with backward-compat), and define `--request-id` as filtering `audit_events.request_id` unless m6 explicitly adds a supported provenance join. Add tests that open a DB created by current `SessionStore` / `AuditWriter` migrations.

### B-5. The `rfl init` lock sketch uses the wrong lock table shape

**Anchor:** `scope.md:250-264`, `scope.md:291-301`, `scope.md:932-937`; live `rafaello/crates/rafaello-core/src/lock/lock_file.rs:18-34`, fixture locks under `rafaello/fixtures/m5b-locks/rafaello.lock`.

**Issue:** The draft says `[plugins.rfl-openai]`, but the live lock serializes as `[plugin."<canonical-id>"]` via `#[serde(rename = "plugin")]`. Grants are under `grant.bundles.default.*`, not the flat sketch shown in A2. If implementers follow the draft literally, `Lock::from_toml` will reject the m6-generated lock.

**Recommendation:** Replace A2 with a TOML snippet that matches the live lock schema, including canonical id, `entry`, `digest`, `manifest_digest`, `grant.bundles.default.network`, `grant.bundles.default.env.pass`, `grant.bundles.default.env.allow_secrets`, `[plugin."...".bindings]`, and `[session].provider_active`.

### B-6. The syd-pty fallback can satisfy the test by disabling the thing m6 is supposed to prove

**Anchor:** `scope.md:313-344`, `scope.md:942-948`; live `rafaello/nix/devenv.nix:7-9`, `lockin/crates/sandbox/src/lib.rs:209-232`, repo-wide `rg "CARGO_BIN_EXE_syd-pty|syd-pty|setup_pty"`.

**Issue:** B2 resolves `syd-pty` and then “falls back to `sandbox/pty:off` with a one-line stderr warning.” B3 only asserts the plugin still spawns without `CARGO_BIN_EXE_syd-pty`. That can pass by disabling PTY instead of solving discovery, which violates hard requirement #2 (“solved at the right layer”) and #1 (“No workaround”). Live lockin currently only resolves `syd` via explicit path / `LOCKIN_SYD_PATH` / PATH; there is no existing `syd-pty` plumbing to anchor the fallback.

**Recommendation:** Make success require actual `syd-pty` discovery: set `CARGO_BIN_EXE_syd-pty` on the `syd` child after resolving it, and test that the PTY path is active (or at least that no pty-off fallback warning fired). If a degraded pty-off mode is retained, make it an explicit negative/residual-risk path, not the success criterion for the hard requirement.

### B-7. The release package/Homebrew path cannot be a Nix-build-under-Brew formula as written

**Anchor:** `scope.md:466-480`, `scope.md:824-835`, `scope.md:1048-1057`.

**Issue:** F1 says `homebrew/rafaello.rb` installs binaries under `/usr/local/bin`; owner item 3 defaults to a separate tap repo; owner item 4 defaults to `brew install nix` + `nix build .#rafaello` under the hood. Those are three different distribution models. A Homebrew formula that shells out to Nix to build the same flake is not a normal “formula matching scope/tempo,” and it does not install the already-built Phase-E output unless the tap also knows where to fetch source/artefacts.

**Recommendation:** Choose one Homebrew story in scope before commits.md. Either (a) in-repo formula that builds from source with Cargo and installs all bins, (b) separate tap that fetches release tarballs/bottles, or (c) document a Nix-based install script and do not call it a Homebrew formula. Acceptance and manual validation should match the chosen path.

## Major

### M-1. Live CLI inventory is overstated

**Anchor:** `scope.md:158-162`; live `rafaello/crates/rafaello/src/lib.rs:57-69`.

The draft says `rfl grant`, `rfl revoke`, `rfl provider use`, and `rfl update` are implemented. The live top-level `RflChatCommand` has only `chat`, `install`, and `status`. Slash-command grants exist, but these top-level CLIs do not. Correct the inventory so future phases do not rely on commands that are not there.

### M-2. Multi-turn stub paths name the wrong crate

**Anchor:** `scope.md:398-425`, `scope.md:905`, `scope.md:949-955`; live path `rafaello/crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`.

The draft points at `crates/rafaello-openai/src/bin/rfl_openai_stub.rs`. The stub is its own crate. Fix the path in Phase D, the coverage table, and glossary candidates.

### M-3. Size estimate is still too optimistic after the blockers are included

**Anchor:** `scope.md:237-238`, `scope.md:764-777`, `scope.md:967-1009`.

The table already lists 24 rows, not 22, before adding the missing install-UX work, package-output repair, Homebrew-model decision, audit-schema compatibility, and a non-handwavy tmux transcript. I agree with keeping m6 single for cohesion, but the honest default looks closer to 26-30 commits than 22.

### M-4. Phase letters no longer match the pre-flight candidate shape

**Anchor:** `scope.md:234-238`; pre-flight “Likely phase shape” has Phase B = install polish, C = syd-pty, D = audit, E = stub, F = nix build, G = Homebrew, H = docs, I = coverage, J = manual validation.

Round 1 says letters match the pre-flight, but they do not: install polish disappeared and every later phase shifted. Either restore the install phase (see B-2) or drop the “letters match” claim and update cross-references.

### M-5. The `result_large_err` ratify path is described backwards

**Anchor:** `scope.md:540-550`, `scope.md:626-628`, `scope.md:999-1000`.

H3 says the default is “ratify-and-allow,” but then describes that as “1 commit (delete the allows + add a decision row).” Ratifying the module-level allow means keeping the allows and adding a decision row (possibly tightening comments), not deleting them.

### M-6. Acceptance uses the default devshell for rafaello-specific gates

**Anchor:** `scope.md:1044-1053`; live `flake.nix:77-124`.

The acceptance commands use `nix develop --impure --command ...` with no `.#rafaello`. The hard requirements and m2 lessons are specifically about the `.#rafaello` devshell exports. Use `nix develop .#rafaello --impure --command ...` for the rafaello cargo/build gates unless there is an intentional reason to test the monorepo default shell.

## Nits

### N-1. `rfl audit` kind reference has a typo and weak anchor

**Anchor:** `scope.md:365-368`.

“m5b c-row 1'' table” is not a usable anchor. Point to `AuditKind::as_str()` and/or `decisions.md` rows 55/58.

### N-2. README troubleshooting recommends the workaround the hard requirement is trying to eliminate

**Anchor:** `scope.md:500-506`.

It is okay to document emergency diagnosis, but the user-facing README should not normalize `CARGO_BIN_EXE_syd-pty=$(which syd-pty)` as a fix for the canonical path. Make the primary remediation “upgrade/enter the fixed devshell or install the fixed release”; put manual env export under “temporary workaround for pre-m6 builds.”

### N-3. Phase J is called “not counted” but acceptance requires it before ratification

**Anchor:** `scope.md:595-630`, `scope.md:1065-1069`.

Fine to exclude retrospective from the core implementation budget, but the commit budget should reserve it explicitly because m6 cannot ratify without it.

### N-4. `manual-validation.md §5 captures asciinema` is new and not required elsewhere

**Anchor:** `scope.md:711-714`.

Hard requirement #3 asks for tmux send-keys/capture-pane evidence. If asciinema is required too, scope its install/availability; otherwise remove it and stick to tmux transcript files.

## What's working

The draft correctly identifies the m6 hard surfaces: `rfl init`, syd-pty discovery, real manual recording, `rfl audit`, multi-turn stub, all-bin packaging, Homebrew/docs, and the m4/m5a/m5b carryovers. The owner-judgment list is unusually good: every question has a default. I also agree with the single-milestone stance for now; the headline demo is integrated enough that splitting would make the v1 proof harder to reason about, provided the commit budget is made honest.
