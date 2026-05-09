Reviewed `rafaello/plans/milestones/m1-manifest/retrospective.md` at `4c1e31f` (`agents/m1-retro/pi-2`).

I ran:

```bash
awk '/^### Positive integration tests/,/^### Negative integration tests/' \
  rafaello/plans/milestones/m1-manifest/scope.md | grep -c '^|.*\\.rs.*|'
# 37
awk '/^### Negative integration tests/,/^### Manual validation/' \
  rafaello/plans/milestones/m1-manifest/scope.md | grep -c '^|.*\\.rs.*|'
# 86
cargo doc --manifest-path rafaello/Cargo.toml -p rafaello-core --no-deps
# warning-free
find rafaello/crates/rafaello-core/tests -maxdepth 2 -type d -print
rg -n 'lockin\.toml|builder calls|tests/fixtures|requires_confirmation' \
  rafaello/plans rafaello/crates fittings
```

## Round 2 findings

### 1. Blocking: final acceptance status is still internally inconsistent

The round-1 blockers were mostly addressed in substance: Stream A banners landed, matrix counts are now reproducible (37 / 86), and `cargo doc` is warning-free at current HEAD.

But `retrospective.md` still has three incompatible final states:

- §2.7 says all follow-up commits landed before ratification.
- “Follow-up commits on this branch” still says “planned, pending pi review” with four `⏳` items, including items that are already landed and one fixture-scope cleanup that is not landed.
- The acceptance summary says `fittings` is only waived because of the m0 flake, then ends with “the full fittings workspace cutover green has landed” and “m1 is done pending the four follow-up commits above.”

This is not just stale prose. The ratified scope required `cargo test --manifest-path fittings/Cargo.toml --workspace green`; the only captured `manual-validation.md` evidence is still `229 passed; 1 failed`. The retrospective now claims a later 3-attempt result (`FAIL, FAIL, PASS`), but that evidence is not in `manual-validation.md`, and there is no owner/scope waiver recorded. Either capture the clean rerun in manual validation, or explicitly record an owner-approved waiver. Do not simultaneously call the gate green, waived, and pending.

### 2. Blocking: overview/glossary still contain m1-relevant `lockin.toml` drift

The retrospective's drift section misses a substantive docs/code mismatch introduced by the m1 implementation shape:

- `overview.md:601` still says the grant compiler “produces a per-plugin `lockin.toml` and a broker ACL table”.
- `glossary.md:38` says Lockin “consumes a per-spawn `lockin.toml` policy”.
- `glossary.md:50` defines “Sandbox policy” as “the compiled, ephemeral `lockin.toml`”.

That contradicts decision row 32 and the ratified m1 scope, where m1 emits a structured `CompiledPlugin` / plan and m2 applies it via lockin's Rust API at spawn time. This is exactly the kind of `overview.md` / `glossary.md` drift the retrospective is supposed to reconcile before merge. Patch these docs or explicitly justify why the old `lockin.toml` wording is still normative (I do not think it is).

Related nit in the newly-added Stream F banner: it says row 32 means “compiler emits lockin builder calls”; m1 intentionally does not emit builder calls, it emits a structured plan consumed by m2. Use the same precise wording as `scope.md`.

### 3. Medium: fixture-directory cleanup is contradictory and partly false

The retrospective says the scope fixture-tree bullet should be fixed in a follow-up, but that follow-up has not landed: `scope.md:1271` still asks for `tree rafaello/crates/rafaello-core/tests/fixtures`.

Also, two new/remaining statements point readers to a non-existent fixture directory:

- `retrospective.md:711` says “only `tests/fixtures/` exists for the c11 validate-with-package fixture trees”. It does not exist.
- `streams/f-manifest/rfc-manifest-schema.md:32` says post-simplification fixtures live under `rafaello/crates/rafaello-core/tests/fixtures/`. They do not; tests are inline / tempdir-based.

`find rafaello/crates/rafaello-core/tests -maxdepth 2 -type d -print` shows only `tests/` and `tests/common/`. Either create/populate the fixture directory (probably wrong per scope’s inline-fixture intent) or update scope + Stream F banner + retrospective to point at the actual integration test files.

### 4. Low: `requires_confirmation` paragraph still contains the false grep claim it says was corrected

§2.2 first says:

> `grep -rn requires_confirmation rafaello/ fittings/` is empty; the only hit in the tree is `overview.md:1193`

Then the same paragraph says the corrected state is that the security RFC and CaMeL RFC still contain the term. The latter is closer, but still not the whole tree (`milestones/README.md`, `driver-notes.md`, old overview reviews, and the retrospective itself also contain it). Rephrase to “no code references” and use a code-only search path, or just delete the stale grep sentence.

### 5. Low: stale commit-plan trace rows remain; avoid overclaiming full trace-table cleanliness

`commits.md` still has conflicting trace rows for `manifest_grant_match_present.rs` (`c11` at line 1051 and `c10` at line 1079). The retrospective’s coverage conclusion is fine because the file exists and passes, but the “trace tables were verified mechanically” wording should be scoped to file presence, not exact commit-row mapping, unless the stale trace rows are fixed.

## Verdict

Do not sign off yet. The main remaining blockers are the unresolved final acceptance posture (especially fittings evidence/waiver vs “green”) and the missed `lockin.toml` overview/glossary drift. The rest is cleanup, but it is cleanup in the retrospective/doc sweep itself, not something to leave for m2 to rediscover.
