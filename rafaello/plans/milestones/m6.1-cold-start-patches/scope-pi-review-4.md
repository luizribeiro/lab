# m6.1 scope.md — pi review round 4

Verdict: NON-BLOCKING

Counts: B/0 M/0 N/1

## Blockers

None.

## Majors

None.

## Nits

### N-1: Internal split c01 still omits the `resolve_plugin_dir_for_bundled` env-arm test

Round-4 folded pi-3 N-1 correctly in the main §A0 text: step 2 now specifies the env arm as `<RFL_BUNDLED_PLUGINS_DIR>/<names.dev_crate>` (scope.md:338-350), and the nearby c01 unit-test list includes `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)` env-arm + release-arm + dev-fallback coverage (scope.md:380-385).

However, the later “Internal split” c01 row still lists `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)` tests as only “release + dev arms” (scope.md:905-909). That is stale relative to the fold and could cause `commits.md` to drop the env-arm test when translating the scope to commit rows.

Concrete fix: change scope.md:907-908 to “env / release / dev arms” (or “env-arm + release + dev arms”) for `resolve_plugin_dir_for_bundled(&OPENAI_NAMES)`.

This is non-blocking because the normative §A0 test list is already correct, but it is the only remaining consistency nit I see.

## Items the scope handles correctly (confirmation)

- pi-3 N-1 is substantively folded in §A0: the env-arm join is now explicit (`<RFL_BUNDLED_PLUGINS_DIR>/<names.dev_crate>`) and tied to C1's `<env-root>/openai/` fixture shape (scope.md:338-350). The main §A0 c01 unit-test list includes the env-arm case (scope.md:380-385). See N-1 only for the stale internal-split summary.
- pi-3 N-2 is folded: §C2 now consistently describes the conservative guard as “skip whenever `CARGO_TARGET_DIR` is set,” not only for external target dirs (scope.md:591-606). The round-4 banner also records the conservative-skip rationale (scope.md:15-23).
- No new material issue was introduced by the round-4 edits. The `BundledPluginNames`/sister-function shape still preserves `rfl install` because `resolve_plugin_dir(name)` remains unchanged for `install.rs:96` (scope.md:362-369), while `rfl init` gets the release-dir-aware source resolver.
- The A1 env-var axis remains reasonable: deriving `RFL_BUNDLED_BIN_OPENAI` from `names.dev_crate` keeps the explicit override stable and avoids the noisy `RFL_BUNDLED_BIN_RFL_OPENAI` form (scope.md:394-404).
- C4 remains aligned with cK5: wrapper-as-pane-command, `exec`, stderr-log polling, Drop guard, Ulid nonce, and `session_alive` polling are all still specified (scope.md:665-738).

## Out-of-scope checks performed (negative coverage)

- Re-checked the round-4 changes only for newly introduced scope creep; none found.
- Re-checked the conservative `CARGO_TARGET_DIR` skip for hidden dependency changes; it remains a test-coverage tradeoff, not a behavior change.
- Re-checked PP1 release-layout interaction after the env-arm wording addition; no new issue beyond the stale internal-split test-summary nit.
