# m6.1 scope.md — pi review round 5

Verdict: CONVERGED

Counts: B/0 M/0 N/0

## Blockers

None.

## Majors

None.

## Nits

None.

## Items the scope handles correctly (confirmation)

- Round-4 N-1 is folded. The internal-split c01 row now includes `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)` **env-arm + release-arm + dev-fallback** coverage, matching the normative §A0 test list.
- The round-4 banner accurately describes the fold, and I see no new issue introduced by the edit.
- Prior resolved items remain stable: `resolve_plugin_dir(name)` is left unchanged for `rfl install`, the `BundledPluginNames` sister resolver is scoped to `rfl init`, C2's conservative `CARGO_TARGET_DIR` skip is explicit, and C4 still follows the cK5 wrapper/tmux pattern.

## Out-of-scope checks performed (negative coverage)

- Re-checked only the round-5 delta plus the adjacent c01 test-summary text. No residual blockers, majors, or nits remain.
