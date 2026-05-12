# m6 scope.md round-4 pi review

> Verdict: blocking
>
> Counts: B/0 M/1 N/2

Round 4 closes both round-3 blockers. The package-placement story now respects the live containment checks (`canonicalize` + `starts_with(package_dir)`), and the J2 script now runs from a flake-bearing lab worktree while targeting a temp project via `--project-root`.

I am not converged because the fake-syd Cargo mechanics are still one line short: the scope adds a `[[bin]] required-features = ["test-fixture"]` target under `lockin/crates/sandbox`, but live `lockin/crates/sandbox/Cargo.toml` has no `test-fixture` feature. Cargo will not build the target under `--features test-fixture` until that feature exists in `[features]`, so the C3 proof is not yet mechanically sound as written.

## Round-3 follow-up

- **B-1 release plugin tree containment:** closed. PP1 now requires real files inside each plugin `bin/` directory, not symlinks to `$out/bin`; A4/B3 acceptance asserts `compile::resolve_entry` succeeds inside `.rafaello/plugins/<topic-id>/`.
- **B-2 J2 working directory / `--project-root`:** closed. Phase B scopes `rfl install --project-root`; J2 runs from `LAB_WORKTREE`, uses `--project-root "$PROJECT"` for init/install/chat/audit, creates `$TRANSCRIPTS`, and copies captures into the in-repo `transcripts/section-5/` directory.
- **M-1 fake-syd buildability:** open as M-1 below. The `[[bin]]` target and `env!("CARGO_BIN_EXE_fake-syd")` are the right shape, but the feature gate is not actually present in live `lockin/crates/sandbox/Cargo.toml` and is not explicitly added by the scope.
- **N-1 package vs binary names:** mostly closed. F1 now says “package build set” with a 1:1 mapping to installed binaries. Two stale references remain as nits below.

## Major

### M-1. `fake-syd` is gated on a non-existent lockin feature

**Anchor:** `scope.md:772-790`; live `lockin/crates/sandbox/Cargo.toml:7-10`, `:23-26`.

Round 4 scopes:

```toml
[[bin]]
name = "fake-syd"
path = "tests/bin/fake_syd.rs"
required-features = ["test-fixture"]
```

and says this uses an “existing `test-fixture` feature gate”. That feature exists in some rafaello crates, but not in `lockin/crates/sandbox/Cargo.toml`, whose features are currently only `default = []` and `tokio = ["dep:tokio"]`.

Cargo `required-features` is the right mechanism, but the feature must also be declared in the same package. Add the explicit scope line:

```toml
[features]
default = []
tokio = ["dep:tokio"]
test-fixture = []
```

(or choose a different existing lockin feature). Otherwise `cargo test --features test-fixture` for the sandbox crate fails feature resolution, and tests using `env!("CARGO_BIN_EXE_fake-syd")` cannot compile/run.

## Nits

### N-1. Two stale “release/workspace binary” references survived the F1 rename

**Anchor:** `scope.md:1361`, `scope.md:1723`.

F1 itself is fixed, but the J3 row-65 placeholder still says the package output ships “every workspace binary”, and the internal split table still describes row 16 as `cargoBuildFlags` expansion to every “release binary”. Prefer “release binary set excluding fixtures” for row 65 and “package build set” for row 16.

### N-2. G1 still says Homebrew installs every release binary under `<prefix>/bin/`

**Anchor:** `scope.md:1083-1087`; round-4 F2 says plugin binaries live only under `share/rafaello/plugins/<plugin>/bin/` and top-level `bin/` keeps only `rfl` + `rfl-tui`.

This is probably just stale wording after the B-1 fix. Say the formula installs `rfl`/`rfl-tui` under `<prefix>/bin/` and preserves the bundled plugin trees under `<prefix>/share/rafaello/plugins/<plugin>/bin/<plugin-bin>`.

## What's working

The two round-3 blockers are addressed tightly, and commits.md readiness is otherwise good: each phase has concrete subjects, tests, and acceptance. After adding the lockin `test-fixture` feature line and cleaning the stale wording, I would expect convergence.
