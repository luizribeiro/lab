# m6 scope.md round-3 pi review

> Verdict: blocking
>
> Counts: B/2 M/1 N/1

Round 3 closes most of the round-2 deltas: the runtime package-placement invariant is now explicit, `rfl audit --project-root` is scoped, the J2 greps match the live confirm overlay, G.β is actionable, Homebrew no longer promises x86_64-darwin, `--fixture` is made optional, and the nits are folded.

I am not converged because two new/mechanically exposed issues still make the scope non-executable for the v1 proof: the release plugin-tree layout uses entry symlinks that violate the live package-entry containment checks (and would break after PP1 copy), and the concrete tmux script runs `nix develop .#rafaello` from an empty `mktemp` project with no flake and no transcript directory.

## Round-2 follow-up

- **B-1 package output/runtime discovery:** mostly folded, but still open as B-1 below. PP1 is the right invariant; the remaining problem is the concrete `share/rafaello/plugins/<plugin>/bin/<bin>` symlink layout.
- **B-2 concrete J2 script:** partially folded, but still open as B-2 below. `rfl audit --project-root`, live overlay greps, quit key, and SQLite schema are now right; the script still cannot run from the documented cwd.
- **M-1 Homebrew default actionable:** closed. Phase G says commits.md proceeds on G.β absent owner reply.
- **M-2 release binary list vs bus fixture:** closed in acceptance. The bus fixture is excluded and the release binary set is explicit; see N-1 for a small wording cleanup in F1.
- **M-3 Homebrew architectures:** closed. x86_64-darwin is explicitly v2/out of scope.
- **M-4 syd-pty proof:** mostly closed; upgraded to a major below because the fake-syd binary location is named but not how Cargo builds it.
- **M-5 `--fixture: Option<PathBuf>`:** closed. Clap `required_unless_present` / `conflicts_with` semantics and clap-error test row are present.
- **N-1 owner-judgment count:** closed. The list is now explicitly 14 items, numbered 0-13, after adding item 13.
- **N-2 `--no` wording:** closed.
- **N-3 Unicode transcript path:** closed. Paths now use `transcripts/section-5/`.

## Blockers

### B-1. The packaged plugin `bin/` symlink layout violates live entry containment and breaks PP1 copies

**Anchor:** `scope.md:359-388`, `scope.md:566-574`, `scope.md:849-858`; live `rafaello-core/src/manifest/validate_with_package.rs:18-39`, `:100-117`, `rafaello-core/src/compile.rs:440-460`.

Round 3 correctly says `entry = "bin/..."` is relative to the copied package directory. But B2/F2 also say each release plugin dir contains `bin/<plugin-bin>` as a relative symlink into `$out/bin/`.

That is incompatible with live validation/runtime:

- `validate_with_package` canonicalizes the manifest entry and rejects entries whose canonical target is outside `package_dir` (`EntryEscape`). A symlink from `share/rafaello/plugins/<plugin>/bin/<bin>` to `$out/bin/<bin>` escapes the package dir before install ever gets to PP1.
- `compile::resolve_entry` repeats the same containment check at chat time, so even a lock written by some bypass would fail before spawn.
- If PP1 copies the package tree into `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/` while preserving symlinks, the copied `bin/<bin>` points at the wrong place (or outside the copied package). If it dereferences symlinks, the scope needs to say so and the digest/acceptance needs to assert the executable target is inside the copied tree.
- The literal `../../../bin/<plugin-bin>` depth is also wrong for the documented release path: from `$out/share/rafaello/plugins/<plugin>/bin/`, `../../../bin` resolves to `$out/share/rafaello/bin`, not `$out/bin`.

This is load-bearing for cold-start chat. Fix by making the package entry an actual file inside each copied package tree (or a symlink whose canonical target remains inside the package), and make the release-tree compactness story separate from the installable package validation path. Acceptance should assert `compile::resolve_entry`/spawn sees an executable inside `.rafaello/plugins/<topic-id>/bin/...`, not merely that a `bin/...` path exists.

### B-2. J2 still is not executable from the scripted working directory

**Anchor:** `scope.md:1064-1126`, `scope.md:1140-1144`; live `rafaello/src/install.rs:32-43`, `rafaello/src/lib.rs:58-69`.

The round-3 J2 setup does:

```sh
PROJECT=$(mktemp -d)
cd "$PROJECT"
nix develop .#rafaello --impure --command rfl init --yes
...
```

`$PROJECT` is an empty temp directory, so `.#rafaello` has no flake. The later tmux command repeats the same problem inside the session. `rfl chat` and new `rfl init` have `--project-root`, but Phase B does not add `--project-root` to `rfl install`, so the script currently cannot cleanly separate “repo whose flake provides the devshell” from “temp project whose lock/state is under test”.

The transcript redirects also write to `$PROJECT/transcripts/section-5/...` without `mkdir -p`, while the prose says the landed artefacts live under `milestones/m6-polish-release/transcripts/section-5/`.

The grep strings, allow key, `rfl audit --project-root`, and SQL statements now line up with live code; this blocker is only about making the command sequence runnable. Use a temp checkout/worktree that actually contains the flake, or run `nix develop /path/to/lab#rafaello --command ...` and add/use `--project-root "$PROJECT"` consistently for init/install/chat/audit. Also create/copy the transcript directory explicitly.

## Major

### M-1. The fake-syd fixture is named as Rust source but not made buildable

**Anchor:** `scope.md:654-676`, `scope.md:1538-1542`; live `lockin/crates/sandbox/src/linux.rs:15-31`.

The fake-syd design is the right proof shape: `linux::build_sandbox_command` runs `Command::new(syd)` with env on that child, so a fake `syd` that records `argv`/`environ` can mechanically prove `CARGO_BIN_EXE_syd-pty` was set.

The scope says the fake binary lives at `lockin/crates/sandbox/tests/fixtures/fake_syd.rs` and that tests point `spec.syd_path` at the compiled binary. Cargo does not automatically compile arbitrary Rust files under `tests/fixtures/` into executables. Make the build mechanism explicit: e.g. add a test-only `[[bin]] fake-syd` and use `CARGO_BIN_EXE_fake-syd`, generate a small shell-script fixture instead of Rust source, or have the integration test compile the fixture with `rustc` before use.

## Nits

### N-1. F1 still calls package names a “release-binary list”

**Anchor:** `scope.md:825-841`, `scope.md:1546`, `scope.md:1624-1630`.

Acceptance has the correct installed binary names (`rfl`, `rfl-tui`, `rfl-openai`, ...). F1's `cargoBuildFlags` list uses package/crate names (`rafaello-tui`, `rafaello-openai`, ...). That is probably what `-p` needs, but call it the package build set and separately list installed binary names to avoid reintroducing the round-2 binary-list confusion.

## What's working

PP1 is the right direction and now matches the live `rfl chat` resolver path and canonical `rafaello.toml` filename. The audit CLI schema is aligned with the live table, the confirm overlay greps are realistic, and the Homebrew/default-owner semantics are much tighter. If the package-entry layout and J2 working-directory mechanics are fixed, I expect the next round to be convergence or nits-only.
