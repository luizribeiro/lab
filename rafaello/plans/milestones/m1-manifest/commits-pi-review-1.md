# Pi review 1 — m1 manifest commits

Review target: `rafaello/plans/milestones/m1-manifest/commits.md` as the
round-1 commit-plan draft.

Verdict: **do not ratify as-is**. The draft is close to the ratified
`scope.md` shape, but several commits cannot land green in the stated order:
acceptance tests require APIs that have not landed yet, dependency bullets omit
load-bearing commits, and a few scope tests have no unambiguous home. Fix these
before handing the file to per-commit implementation agents.

## Blocking findings

### 1. Tool-table-default / `always_confirm` tests require a manifest→lock projection that is not in scope or in sequence

`c08` accepts `tests/tool_table_omitted_uses_defaults.rs`, and `c11` is a
“tests-only” commit asserting the manifest → lock projection of
`always_confirm`. Neither can be implemented from the commits landed at that
point:

- `c08` is before the lock schema (`c10`) and before sink inference (`c16`),
  but the scope's `tool_table_omitted_uses_defaults.rs` assertion is explicitly
  about a lock snapshot: `tool_meta.grep = ... sinks_inferred: true`.
- `c11` depends only on `c10`, but no commit has introduced an install/projection
  API that converts a `Manifest` plus grant into a `Lock` entry. The milestone
  scope says m1 fixtures construct locks programmatically; it does not define a
  manifest→lock compiler/install function.

If implemented literally, agents either invent an unratified projection API or
write tests that cannot express their stated assertions.

**Fix:** choose one of:

- add an explicit, scoped manifest→lock snapshot/projection helper before these
  tests, including sink-default inference inputs; or
- move the lock-snapshot portions to the first commit that has the required
  pieces (`c16` for sink inference and `c33` for compiled `tool_meta`), and keep
  `c08` limited to parse/validate defaults; or
- rewrite `c11` as pure fixture/round-trip coverage that does not claim a
  manifest→lock projection exists.

### 2. `validate::manifest_standalone` is both used before it exists and reintroduced later

Early manifest commits require V1 behaviour:

- `c04` says `validate::manifest_standalone` rejects reserved publish
  namespaces.
- `c05` says bundle-key validation runs in `validate::manifest_standalone` and
  lands `manifest_unknown_bundle_key.rs` there.
- `c06`/`c07` land load-trigger and renderer-kind cross-reference tests.

But `c13` later says it “exposes `validate::manifest_standalone` as a single
API” and moves deferred checks there. That is ambiguous enough to break
per-commit greenness: if the public API is not available until `c13`, tests from
`c04`–`c08` cannot call it; if it is available earlier, `c13` is not the first
API exposure and must be described as consolidation only.

**Fix:** introduce the public V1 entry point in the first commit whose tests need
it (probably `c04`) and make `c13` explicitly a consolidation/coverage commit;
or move all public-V1 acceptance tests to `c13` and keep earlier commits parse-
only.

### 3. Lock schema commit is under-dependent on capability/bundle types

`c10` defines `grant.bundles: BTreeMap<BundleKey, GrantBundle>` and lock-side
filesystem/network/env/limits grants, but it depends on `c09` and `c06` only.
The `BundleKey` vocabulary and capability section shapes land in `c05`; `c06`
does not depend on `c05`.

**Fix:** make `c10` depend on `c05` (and say it reuses the manifest capability
vocabulary), or move the shared bundle/grant structs earlier into a neutral
module before both manifest capabilities and lock schema consume them.

### 4. `PathContext` / placeholder resolver dependencies are missing for Tr/K/V3 work

Several commits take or need `PathContext` and C3-style path resolution before
the commit plan has made that surface available to them:

- `c17` defines `trifecta::evaluate(..., ctx: &PathContext)` but depends only on
  `c10`; the placeholder expansion/context surface is introduced by `c08`.
- `c18` defines `carveout::compile_against(..., ctx: &PathContext, ...)` but
  does not depend on `c08` either.
- `c25` implements lock-side `exec_paths`/`exec_dirs` under `${project}` using
  expansion plus existing-ancestor canonicalisation, but the shared resolver is
  not introduced until `c28`.

This invites duplicated or incompatible path-resolution code and makes the
stated dependencies false.

**Fix:** introduce `PathContext` and the reusable resolver before Tr/K/V3 need
it, or explicitly split the resolver into an earlier validation/path module and
have `c28` reuse it for compile-side plan emission. Update `c17`, `c18`, and
`c25` dependencies accordingly.

### 5. Compile section commits under-declare dependency on bundle flattening

`c31` builds the network plan and `c32` builds the env plan, but both depend only
on `c27` plus their local helper commits. Correct network/env output depends on
`c29`'s full-bundle union and deterministic post-flatten ordering. Without that,
proxy mode, `allow_hosts`, `env.pass`, and `env.set` are ambiguous when default
and named bundles both contribute.

**Fix:** make `c31` and `c32` depend on `c29` and state that they consume the
post-flatten effective grant produced there.

### 6. Scope test coverage has naming/home drift

Two positive tests named by `scope.md` have no clear commit home:

- `compile_network_proxy_plan.rs` — scope says this asserts that
  `NetworkPlan::Proxy { allow_hosts }` records the list verbatim. `c31` only
  names `compile_network_proxy_allow_hosts_validates.rs`.
- `compile_digest_match.rs` — scope names it separately from
  `digest_match_compiles.rs`; `commits.md` only names `digest_match_compiles.rs`
  and repeats that filename in `c33`.

The milestone-wide acceptance says every named scope test must land, but
per-commit agents need an explicit home.

**Fix:** either add these test names to the relevant commits (`c31` and `c33`) or
update `scope.md`/`commits.md` to a single canonical set of test names and make
clear which file covers which behaviour.

## High-priority corrections

### 7. E1 typed-error surface has no explicit commit

`scope.md` includes E1: `thiserror`-driven module-local errors plus a top-level
`rafaello_core::Error` unifying `ManifestError`, `LockError`,
`ValidationError`, `CompileError`, `DigestError`, `CarveOutError`, and
`TrifectaError`. `commits.md` uses these error types throughout but never lands
the unifying error surface as acceptance.

**Fix:** add E1 to an early infrastructure commit (`c02` is a natural home for
`error::ManifestError`, or add a small explicit error-surface commit) and include
a build-only assertion that `rafaello_core::Error` exposes all module errors.

### 8. `c06` should pin the negative event-pattern case when it introduces event matching

`scope.md`'s `manifest_load_event_pattern_match.rs` includes both acceptance of
`core.session.**` matching `core.session.started` and rejection of an unrelated
event. `c06` names the positive file but only lists
`manifest_load_trigger_unknown_command.rs` as negative.

**Fix:** explicitly say `manifest_load_event_pattern_match.rs` includes the
unrelated-event rejection in `c06`, or add a separate negative test file there.

### 9. `compile_without_validate_lock_errors.rs` overclaims what can be detected

`c27` says calling `compile_plugin` “when `validate::lock` was not called first”
returns `ValidationNotRun`. The function signature has no validation token or
proof that V3 ran; it can only detect invalid invariants itself and map them to
`ValidationNotRun`.

**Fix:** reword the acceptance to “a lock violating a V3 invariant returns
`CompileError::ValidationNotRun` from `compile_plugin`” unless the design adds a
validated-lock token/newtype.

### 10. The m1a/m1b checkpoint rationale is stale

The checkpoint text says it sits after `c19` with “sink default inference +
digests + topic-id + lock + parsers landed,” but by `c19` the plan has also
landed trifecta, carve-out, and scrubber. `scope.md`'s internal-split prose says
the natural boundary is after group 7 / before carve-out + trifecta + compiler.

**Fix:** either move the checkpoint to the parser/lock/digest/topic-id/sinks
boundary, or update the rationale to say the first tranche intentionally includes
trifecta/carve-out/scrubber too.

## Minor cleanup

- `c19` lists `env_scrubber_override.rs` under “Negatives”; it is a positive
  override-preserves-input test.
- `c33` repeats `digest_match_compiles.rs` in its positive acceptance list.
- `c18` should spell out the `allow_credential_paths = true` override behaviour
  in the “What” section, not only imply it via the function parameter and tests.
- Consider adding a compact trace table at the end of `commits.md`: each named
  test file from `scope.md` → commit number. That would make these drift checks
  mechanical in later review rounds.
