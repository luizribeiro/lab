# m6 scope.md round-2 pi review

> Verdict: blocking
>
> Counts: B/2 M/5 N/3

Round 2 is a substantial improvement: the round-1 live-code contradictions are mostly folded, the demo tool is single-valued, the audit schema is corrected, and the size budget is much more honest. I am not converged yet because the new install/release-tree story still does not line up with the live runtime resolver, and the now-concrete tmux script contains commands/assertions not scoped by the phases.

The remaining blockers are narrower than round 1. They should be fixable in one more scope round without reopening the milestone shape. I still agree with a single m6 and the 28-ish implementation commit budget, modulo the fixes below.

## Round-1 follow-up

- **B-1 package output already exists:** closed. Phase F is now package repair, not adding `.#rafaello`.
- **B-2 `rfl install <tool>` missing:** partially closed, but reopened as B-1 below. Phase B exists, but its release-tree/package-root contract is not compatible with live `rfl chat` package resolution.
- **B-3 demo not single/concrete:** mostly closed. `rfl-mailcat` / `send-mail` is now canonical and J2 has concrete tmux steps. Remaining script defects are B-2 below.
- **B-4 audit SQL wrong:** closed in Phase D.
- **B-5 init lock shape wrong:** closed on TOML shape; runtime package placement is covered by new B-1.
- **B-6 syd-pty fallback success:** closed on policy. Round 2 chooses no lockin-layer fallback.
- **B-7 Homebrew conflation:** mostly closed with G.α/G.β/G.γ + default G.β; see M-1/M-3 for remaining actionability/arch issues.
- **M-1 CLI inventory overstated:** closed.
- **M-2 stub crate path:** closed.
- **M-3 size estimate:** closed enough. 28 implementation + 1 retro reservation is plausible.
- **M-4 phase letters:** closed.
- **M-5 `result_large_err` ratify wording:** closed.
- **M-6 devshell selector:** closed.
- **N-1 audit-kind anchor:** closed.
- **N-2 README workaround framing:** closed.
- **N-3 retro budget:** closed.
- **N-4 asciinema:** closed.

## Blockers

### B-1. Bundled plugin discovery does not produce package dirs the live runtime can spawn

**Anchor:** `scope.md:308-357`, `scope.md:390-415`, `scope.md:634-640`, `scope.md:1196-1201`; live `rafaello/crates/rafaello/src/lib.rs:235-276`, `install.rs:84-114`, `decisions.md` row 25.

**Issue:** Round 2 pins `rfl install <plugin>` to `<release-prefix>/share/rafaello/plugins/<plugin>/manifest.toml`, and `rfl init` writes a `builtin:openai@0.0.0` lock entry. But live `rfl chat` does not resolve bundled plugins from a release prefix. It maps **every** lock entry to `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`, then reads `rafaello.toml` from that package dir and recomputes digests there. Live `rfl install --fixture` also reads `package_dir.join("rafaello.toml")`, not `manifest.toml`; row 25 canonically names the manifest `rafaello.toml`.

As scoped, cold-start can write a valid-looking lock, and `rfl install rfl-mailcat` can write another lock entry, but `rfl chat` will still look for `.rafaello/plugins/<topic-id>/rafaello.toml` unless m6 either copies bundled package dirs into the project install root or changes runtime package-dir resolution. This directly threatens hard requirement #1.

**Recommendation:** Add an explicit package-placement/runtime-resolution invariant. Smallest fix: `rfl init` copies the bundled `rfl-openai` package tree to `.rafaello/plugins/<topic-id>/`, and `rfl install rfl-mailcat` copies the resolved bundled package tree there too; keep `entry = "bin/..."` relative to that copied package. Use the canonical filename `rafaello.toml` everywhere. Alternative: extend the lock/runtime resolver with a bundled-source path class, but then scope the lock-schema/runtime changes and tests.

### B-2. The concrete J2 script is still not executable as scoped

**Anchor:** `scope.md:512-552`, `scope.md:820-858`; live `rafaello/crates/rafaello-tui/src/confirm.rs:156-213`, `rafaello/crates/rafaello/src/lib.rs:57-69`.

**Issue:** J2 runs `rfl audit --project-root "$PROJECT"`, but Phase D never scopes a `--project-root` flag for `AuditArgs`; the only commands currently scoped with project-root are `chat` and new `init`. J2 also asserts exact TUI strings (`grep "rafaello chat"`, `grep "Allow this call?"`) that are not anchored in the live TUI overlay: the confirm overlay renders title `confirm`, summary/args/sinks/taint/provenance, not that literal prompt string.

That makes the manual-validation proof brittle/non-runnable for a fresh driver. Hard requirement #3 specifically asks for a real tmux recording, so the script must be executable without interpreting missing CLI flags or inventing UI copy.

**Recommendation:** Either add `--project-root` to `rfl audit` in Phase D (and tests), or change J2 to `cd "$PROJECT" && nix develop ... rfl audit`. Replace expected greps with scoped/live strings: e.g. `grep " confirm "`, `grep "sinks: mail"`, `grep "alice@example.com"`, plus whatever assistant ack text the stub/real model is expected to produce. If new modal copy is desired, scope it as a TUI docs/test change.

## Major

### M-1. Homebrew has a default, but Phase G says commits.md cannot finalise without owner input

**Anchor:** `scope.md:649-657`, `scope.md:695-721`, `scope.md:1104-1113`.

Round 2 gives a clear default (G.β), then says `commits.md` for Phase G cannot finalise until owner resolves the question. That defeats the owner-judgment default convention. Either make G.β actionable for commits.md absent owner reply, or mark the whole scope owner-blocked on item 5.

### M-2. Release package acceptance contradicts the bus-fixture default

**Anchor:** `scope.md:625-632`, `scope.md:920-923`, `scope.md:1131-1135`, `scope.md:1331-1333`.

Owner item 9 defaults to excluding `rafaello-bus-fixture`, but J3 row 65 and acceptance say the package ships “every workspace binary.” Those cannot both be true. Use “every release binary” and list the included bins, or default item 9 to include the bus fixture.

### M-3. G.β promises macOS x64 tarballs, but the root flake does not build x86_64-darwin

**Anchor:** `scope.md:672-681`; live `flake.nix:24-28`.

The default Homebrew model says release artefacts include “macOS arm64, macOS x64.” The flake systems are `aarch64-darwin`, `aarch64-linux`, and `x86_64-linux`; there is no `x86_64-darwin`. Either add x86_64-darwin to Phase F/G scope or narrow the Homebrew artefact promise to supported macOS architecture(s).

### M-4. C3 still lacks a mechanically reliable proof that `syd-pty` was used

**Anchor:** `scope.md:479-503`.

Round 2 correctly removes the fallback success path, but the default test assertion is “stderr absence.” Absence of `setup_pty` / `pty:off` text is a weak proxy for “syd child got `CARGO_BIN_EXE_syd-pty`.” Prefer a fake/wrapper `syd` in tests that records its environment and asserts the exact env var value, plus a sibling-discovery tempdir test. If real syd is required, keep the test Linux/devshell-gated and explicitly assert the env handoff in logs.

### M-5. Phase B does not explicitly make `--fixture` optional

**Anchor:** `scope.md:390-401`; live `install.rs:32-35`.

B1 says “extends `InstallArgs` with `plugin: Option<String>`,” but live `fixture: PathBuf` is required. The resolution order only works if `fixture` becomes `Option<PathBuf>`. Spell that out, with clap conflict/required-unless semantics, so implementers do not add a positional arg while leaving `--fixture` required.

## Nits

### N-1. Owner-judgment item count is off

**Anchor:** `scope.md:1060-1068`.

The text says round 2 has 12 items, but the list is numbered 0 through 12 (13 items).

### N-2. A3 mentions a nonexistent `--no` answer

**Anchor:** `scope.md:359-365`.

“Decline (or `--no` TTY answer)” reads like a flag. Use “answering no at the TTY prompt” unless `--no` is being scoped.

### N-3. The `§5` path component is cute but operationally awkward

**Anchor:** `scope.md:874-879`, `scope.md:1165-1167`, `scope.md:1342-1345`.

Unicode paths work, but they make shell quoting/tooling more fragile. Prefer `transcripts/section-5/` for artefacts.

## What's working

The revised scope does the important round-1 repair work: live-code facts are now in the document, the demo tool is consistently `send-mail`, audit is aligned to the actual schema, and the syd-pty policy is stronger. The single-milestone stance still looks right; the remaining issues are mostly about making the newly introduced install/release layout precise enough that cold-start chat can actually run.
