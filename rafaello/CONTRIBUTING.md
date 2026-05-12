# Contributing to rafaello

## Development environment

The repo uses a [devenv](https://devenv.sh/)-managed Nix shell. With
[direnv](https://direnv.net/) installed, `cd` into the repo and the
shell loads automatically. Otherwise, enter the shell manually:

```
nix develop .#rafaello --impure
```

The `--impure` flag is required: `devenv` asserts on the caller's
current directory, and a pure `nix develop` invocation fails that
assertion (`plans/README.md` "Recurring operational gotchas",
referencing m0 retrospective §4.6 and `manual-validation.md` §4).
CI invokes `nix develop` with `--impure` for the same reason.

The shell provides the pinned Rust toolchain (see repo-root
`rust-toolchain.toml`) and every system dependency rafaello's crates
need at build and test time.

## Building and running tests

From the repo root, inside the dev shell:

```
cargo test --workspace --features test-fixture
nix flake check
```

`cargo test --workspace --features test-fixture` exercises the
rafaello crates with the in-tree fixture plugin built in; this is
the canonical per-commit local test invocation, matching what CI
runs. `nix flake check` validates every flake output (the
`rafaello` package, the dev shell, formatter, and any per-crate
checks) and is the closest local approximation of the CI matrix.

To build the `rfl` binary:

```
cargo build --release        # from the repo root
nix build .#rafaello          # produces a reshaped result/ tree
```

## Plans, milestones, and streams

rafaello is paper-first: every load-bearing design decision lives
under [`plans/`](./plans/) before code lands. The single source of
truth for the v1 architecture is `plans/overview.md`; per-subsystem
RFCs live in `plans/streams/`; the append-only architecture decision
log is `plans/decisions.md`; and per-milestone scope, commit plans,
and retrospectives live in `plans/milestones/m<N>-<topic>/`. The
full design-review-implement-retrospect workflow — including the
claude+pi review loop on `scope.md` and `commits.md`, the per-commit
worktree-based implementation phase, and the merge model — is
described in [`plans/README.md`](./plans/README.md); read that
before making non-trivial changes.

## Per-commit code review

Every per-commit agent — and every contributor landing a commit by
hand — runs the `code-reviewer` agent on the staged diff before
committing, per the global Claude Code guidelines in
`~/.claude/CLAUDE.md`. The review catches DRY violations, missing or
worthless tests, overly large commits, and stale "what" comments
before they enter the history. Commits land one logical change at a
time, with tests in the same commit as the code they test; see
`~/.claude/CLAUDE.md` for the full commit-size and test-coverage
expectations.

## Branch model: rebase, no force-push

Milestone branches accumulate on the long-running integration
branch `rafaello-v0.1`, not on `main`. Per-commit work lands on
`agents/m<N>/c<NN>` worktrees, gets fast-forward-merged onto the
milestone branch, and the milestone branch then rebases onto
`rafaello-v0.1` with linear history. `main` stays at the
`rafaello-design` merge until v1 is demo-ready, at which point
`rafaello-v0.1` merges to `main` in one pass; m6's RATIFIED
disposition is that terminal merge, per
[`plans/decisions.md`](./plans/decisions.md) row 33.

All merges are rebase-based. No merge commits, no force-pushes
to shared branches (`main`, `rafaello-v0.1`, milestone branches).
A per-commit branch may be rebased and force-pushed only while it
is still solely owned by its agent worktree; once it has been
merged it is immutable. Pre-commit hooks always run — never
`--no-verify`. If a hook fails on pre-existing breakage, surface
it rather than working around it.
