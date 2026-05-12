# m6 scope.md round-5 pi review

> Verdict: CONVERGED
>
> Counts: B/0 M/0 N/0

Round 5 is the convergence point. The remaining round-4 major and nits are folded surgically, with no new contradictions found in the changed sections. A fresh commits.md-drafting agent should be able to derive the implementation commit list from the current scope.

## Round-4 follow-up

- **M-1 fake-syd feature gate:** closed. C3 now scopes `test-fixture = []` as a net-new `lockin/crates/sandbox/Cargo.toml` feature alongside the live `default` and `tokio` entries, and keeps `[[bin]] name = "fake-syd" ... required-features = ["test-fixture"]`. The Cargo mechanics are now sound for tests using `env!("CARGO_BIN_EXE_fake-syd")`.
- **N-1 stale binary-list wording:** closed. J3 row 65 now says “release binary set excluding fixtures”, and internal split row 16 says “package build set”. The remaining “every release binary” text is only preserved prior-round changelog/traceability, not active scope.
- **N-2 Homebrew prefix layout:** closed. G1 now installs only `rfl` and `rfl-tui` under `<prefix>/bin/` and preserves bundled plugin package directories under `<prefix>/share/rafaello/plugins/<plugin>/`, including each plugin's `bin/<plugin-bin>` real file.

## Spot-checks

- Live `lockin/crates/sandbox/Cargo.toml` currently has only `default = []` and `tokio = ["dep:tokio"]`; round 5 correctly calls the sandbox `test-fixture` feature net-new rather than pre-existing.
- The release-tree / PP1 containment fix from round 4 remains intact: plugin binaries are real files inside plugin package dirs, satisfying the live `validate_with_package` and `compile::resolve_entry` canonical-path containment checks.
- The J2 tmux script remains executable in shape: it runs from `LAB_WORKTREE`, targets `$PROJECT` via `--project-root`, creates transcript dirs, and copies captures into the in-repo `transcripts/section-5/` path.

## Verdict

CONVERGED. No blockers, majors, or nits remain.
