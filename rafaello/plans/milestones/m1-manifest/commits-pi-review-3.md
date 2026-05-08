# Pi review 3 — m1 manifest commits

Review target: `rafaello/plans/milestones/m1-manifest/commits.md` round-3 draft, checked against `scope.md` round 7 and prior pi commit-plan reviews.

Verdict: **do not ratify as-is**. The round-2 phase-boundary rewrite is directionally right, but the file still has sequencing contradictions that will break per-commit agents. Most importantly, commit numbering is not executable, the c27→c31 back-edge remains despite the claimed fix, and several named scope tests still have conflicting or impossible homes.

## Blocking findings

### 1. Commit numbering is not a valid ordered commit list

`commits.md` has no `c11` section and uses `c12` twice:

- `c12` = manifest validate-with-package / canonical bytes.
- `c12` = `CanonicalId` parser/formatter.
- The trace tables and change log repeatedly point manifest package tests at `c11`.

This breaks the conventions at the top of the file: per-commit branches `agents/m1/c<NN>`, sequential landing, and dependency bullets all become ambiguous. It also makes dependencies like `c13` depending on `c12` impossible to interpret.

**Fix:** renumber to a single monotonic sequence. The manifest validate-with-package commit appears intended to be `c11`; the canonical-id commit can remain `c12`, with all later references audited.

### 2. The round-2 numeric back-edge was not removed

Round 3 says the resolver moved to `c03` and the old out-of-order note is gone, but `c27` still says it uses the `c31` path resolver, depends on `c31`, and lands after `c31` via an explicit out-of-order sequence.

That directly violates the file's “ordered commit list” and “Depends on cites lower commit numbers” rules, and it contradicts the “What changed” section.

**Fix:** either:

- make `c27` consume the `paths::resolve_under_root` helper from `c03` and remove the out-of-order note; or
- renumber so the actual landing order is monotonic.

Also make `c31` clearly reuse/adapter-wrap the `c03` resolver rather than re-introducing a second C3 resolver that `c27` depends on.

### 3. Early manifest commit still owns tests/APIs that cannot be green there

`c04` still assigns `manifest_parse_minimal.rs`, `manifest_unknown_field.rs`, and `manifest_invalid_name.rs`.

Those do not fit `c04`:

- `manifest_parse_minimal.rs` requires an empty `[provides]` block and `Manifest::canonical_bytes()`. `[provides]` lands in `c05`; `canonical_bytes()` lands in the later validate-with-package commit.
- Scope's `manifest_unknown_field.rs` includes an unknown nested key under `[provides]`; before `c05`, `c04` can only reject `[provides]` as an unknown top-level table, not exercise the nested shape.
- `c04` says `name` is grammar-validated during parse, but the round-3 phase boundary and `c10` say `manifest_invalid_name.rs` is V1/`ValidationError` coverage.

**Fix:** keep `c04` to top-level schema/reserved-field parsing. Move the full `manifest_parse_minimal.rs` and nested `[provides]` unknown-field assertion to the commit that has their APIs, and remove parse-time `name` grammar validation if the ratified phase is V1.

### 4. Manifest exec-under-project refusal still lacks the context needed to implement the stated rule

The validate-with-package commit says `manifest::validate_with_package(manifest_path, package_dir, manifest)` rejects `exec_paths` / `exec_dirs` resolving inside `${project}` using `paths::resolve_under_root`.

But that API has no `PathContext` or `project_root`. `package_dir` is the plugin package dir, not the user's project root. With the current signature, an implementation can at most reject syntactic `${project}/...` templates; it cannot resolve absolute host paths or symlink/ancestor cases “inside project” as the text claims.

**Fix:** either add an explicit project context to the manifest package-validation API, or narrow the planned manifest-side rule/test to a syntactic `${project}` placeholder refusal and update the wording accordingly. Do not claim to use the full resolver without supplying its root context.

### 5. Default/tool-meta tests still do not cover the scoped manifest/defaulting behaviour

The plan avoids inventing an m1 manifest→lock projection API, which is correct, but the named tests now under-cover the scope wording:

- `tool_table_omitted_uses_defaults.rs` in scope says a manifest with no `[provides.tool.grep]` table parses, validates, and lock-snapshots default `ToolMeta`. `c18` only describes a programmatic lock fixture compared to `infer_defaults`; it does not explicitly test that V1 accepts the omitted manifest table/defaults.
- `tool_meta_always_confirm_round_trip.rs` is reframed as a programmatic lock round-trip plus compile extension. That avoids an unscoped projection, but it should still explicitly cover that manifest parsing preserves `always_confirm = true` somewhere (or state that the named test is fixture-only and scope wording is being intentionally narrowed).

**Fix:** make these tests two-part fixtures: parse/validate the manifest half where relevant, then construct the corresponding programmatic lock snapshot for the lock/compile half. No projection API is needed, but the scope-required defaulting/manifest metadata must not disappear.

## High-priority corrections

### 6. `manifest_malformed_sinks.rs` has a phase ambiguity

The phase-boundary text says serde type mismatches are parse-time `ManifestError`s, while grammar checks are V1 `ValidationError`s. But `c10` assigns `manifest_malformed_sinks.rs` wholly to V1 even though the scoped case includes `sinks = [42]`, which cannot deserialize into `Vec<String>`.

**Fix:** split the test expectations explicitly: non-string `sinks` is parse-time schema/type failure, uppercase string is V1 sink-class grammar failure; or change the raw decode type so V1 deliberately owns both errors.

### 7. Traceability table is still not mechanical

The positive trace table points many manifest tests at nonexistent `c11`, duplicates `manifest_grant_match_present.rs` with two different commits, and omits named scope tests that do have acceptance homes elsewhere, including:

- `topic_id_collision_detection.rs`
- `compile_capability_path_nonexistent_write_leaf.rs`
- `broker_acl_tool_owner_resolves_routing.rs`
- `method_not_found_typed_method_round_trip.rs` (under `fittings/tests/`)

**Fix:** after renumbering, regenerate both positive and negative trace tables from the scope matrix and make every named test appear exactly once, with intentional aliases called out separately.

### 8. Dependency bullets still contradict the typed-error surface

The change log says `c17` and `c21` now depend on `c02`, but the actual commits still say:

- `c17` returns `DigestError` and depends only on `c01`.
- `c21` returns `CompileError` from `reject_reserved` and depends only on `c01`.

**Fix:** add `c02` to both dependency bullets, or define those error types locally in the same commits (not recommended; E1 is already centralized in `c02`).

## Minor cleanup

- `c01` still says the `tempfile` crate dev-dep lands when “c10's fixture tests” first need it. After renumbering and moving package fixtures, update this to the actual commit that first adds fixture/tempdir tests.
- The “What changed from prior drafts” section should be audited after fixes. It currently claims the out-of-order resolver issue and `c17`/`c21` dependency issues were fixed when the body says otherwise.

After the numbering/back-edge and phase/test-home fixes above, the plan should be close to ratifiable.
