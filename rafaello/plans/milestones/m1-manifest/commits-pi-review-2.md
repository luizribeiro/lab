# Pi review 2 — m1 manifest commits

Review target: `rafaello/plans/milestones/m1-manifest/commits.md` round-2 draft, checked against `scope.md` round 7 and `commits-pi-review-1.md`.

Verdict: **do not ratify as-is**. Round-1 issues are mostly addressed, but the draft still has several per-commit greenness and sequencing traps. The most serious are an explicit numeric-order back-edge (`c26` depends on `c30`), scope tests placed before the APIs they assert exist, and parse-vs-validation phase drift that will produce the wrong public error surface for named tests.

## Blocking findings

### 1. `c26` depends on `c30` while the plan claims an ordered sequential commit list

The conventions say this is an “Ordered commit list”, commits land sequentially on `agents/m1/c<NN>`, and “Depends on” cites lower-numbered commits. `c26` breaks all three rules:

- `c26` depends on `c30`.
- The note says the driver actually lands `c27 → c30 → c26 → c31`.
- `c26` is in the V3 group but consumes a compiler-path resolver from the later compiler group.

This is not just cosmetic. Per-commit agents, branch naming, review order, and greenness reproduction will all naturally assume numeric order. The current wording asks them to violate the file's own process contract.

**Fix:** either renumber so actual land order is monotonic, or extract the C3 resolver into an earlier neutral path/validation utility commit consumed by both V3 and compile. Then make `c26` depend only on earlier commits.

### 2. `manifest_parse_minimal.rs` is assigned to `c04` before its scoped assertions can pass

Scope's positive matrix says `manifest_parse_minimal.rs` exercises both `Manifest::parse` and `Manifest::canonical_bytes`, and the minimal manifest includes an empty `[provides]` block. In `commits.md`:

- `c04` assigns `manifest_parse_minimal.rs`.
- `c04` has only M1/M2 top-level fields; `[provides]` does not land until `c05`.
- `Manifest::canonical_bytes()` does not land until `c10`.

So an implementation agent for `c04` cannot write the named scope test without either inventing future APIs early or weakening the test below its scoped meaning.

**Fix:** move the full `manifest_parse_minimal.rs` home to `c10`, or explicitly split it into an early parse-only test with a different name and extend the canonical scope test in `c10`.

### 3. Load trigger cross-reference tests are placed before renderer types exist

`c08` lands `[load]` and assigns `manifest_validate_load_trigger_cross_refs.rs`. Scope says that test covers declared methods, topics, **and kinds**. But renderer declarations and renderer-kind grammar do not land until `c09`.

`c09` says “c08's `kind` cross-ref now resolves against the declared renderer set”, which confirms the `c08` implementation cannot satisfy the full scoped test yet.

**Fix:** keep `c08` to command/event checks, then land the full cross-ref test (or its kind half) in `c09`; or move renderer type stubs before `c08`.

### 4. Manifest `exec_paths`-inside-project refusal is sequenced before the resolver/context needed to implement it

`c10` assigns `manifest_exec_path_inside_project.rs` and says `validate_with_package(manifest_path, package_dir, manifest)` refuses `exec_paths` / `exec_dirs` resolving inside `${project}`. Scope says this check uses C3-style placeholder expansion plus existing-ancestor canonicalisation.

At `c10`:

- the C3 resolver does not exist until `c30`;
- `validate_with_package` has no `PathContext` / `project_root`, so it cannot decide whether an absolute host path is inside the project root;
- rejecting only literal `${project}/...` templates would be a narrower syntactic rule than the scoped “resolving inside project” rule.

**Fix:** either add an earlier shared resolver plus a manifest validation context containing `project_root`, or move this test/rule later and make the API explicit. If the intended rule is syntactic-only for manifests, update `scope.md` before committing to this plan.

### 5. Several manifest negative tests are assigned to parse-time commits even though scope requires `ValidationError` from V1/V2

The commit plan repeatedly says parser/newtype decoding rejects shapes that the scope's negative matrix classifies as validation failures:

- `c05` parses/rejects `manifest_dotted_tool_name.rs` and sink-class errors, but scope says `ValidationError::IllegalToolName` / V1 sink validation.
- `c06` says publishes on `core.*` / `frontend.*` are rejected “at parse time”, but scope says V1 returns publish-namespace `ValidationError`s.
- `c09` parser rejects built-in/unprefixed renderer kinds, while scope's negative rows name `ValidationError` classes.

If implemented literally, named tests will either call `Manifest::parse` and assert the wrong public phase, or call `validate::manifest_standalone` after parse but never reach it because parse failed.

**Fix:** decide the phase boundary and align commits/tests with it. For ratified scope as written, parsing should decode enough structure to let V1/V2 return the named `ValidationError`s for these cases; parse-time rejection should be reserved for TOML/schema/path-syntax errors explicitly scoped as `ManifestError`.

### 6. Lock-side capability-template rejection is moved to lock parse, but scope says it is a V3 mirror

`c12` says lock capability path fields parse through `CapabilityPathTemplate` and assigns `lock_capability_path_relative.rs` there. Scope's V3 bullet says those fields are re-parsed/checked by `validate::lock`, and the negative test expects `ValidationError::LockCapabilityPathRelative`.

A parse-time `LockError` would not satisfy the scoped runtime-authority mirror: the point is that V3 loudly rejects hand-edited lock authority through its validation surface.

**Fix:** either store raw/string templates in the lock and validate them in V3, or explicitly make `Lock::from_toml` surface the scoped `ValidationError` (less clean). Do not assign `lock_capability_path_relative.rs` to a pure schema round-trip commit unless the expected error phase changes in `scope.md`.

## High-priority corrections

### 7. `c11` canonical-id source grammar omits the `.` segment rejection

Scope L8 rejects any source segment that is `.` or `..`. `c11` only calls out `..` plus leading/trailing/double slash. A canonical id like `github:./grep@1.0.0` risks slipping through the implementation plan.

**Fix:** add “segment equal to `.`” to `c11` and its invalid/path-traversal acceptance matrix.

### 8. Later compile/broker precondition commits under-declare dependencies on V3 mirror commits

`c28` describes detecting V3 invariants including foreign-namespace publish, but depends only on `c21` and `c27`; lock-side publish authority lands in `c23`. `c34`'s broker ACL has the same V3-must-run-first contract but depends only on `c14` and `c21`, even though grammar/publish/tool-owner invariants are extended through `c25`.

Sequential order hides this, but the dependency bullets are meant to tell agents which prior code/types they consume.

**Fix:** either narrow the examples in `c28`/`c34` to invariants available at their declared deps, or add explicit deps on the V3 extension commits they rely on (`c23`–`c25`, and `c26` if exec-path assumptions matter).

### 9. The traceability table is not actually mechanical for the largest drift surface

The new trace table is useful for positives, but the negative matrix is collapsed to “(every negative scope test) | (per the negative-tests rows above)”. Given how many phase/order issues above are about negative tests, this misses the main value of the table.

**Fix:** add a second explicit negative-test table (`test file → commit`) or make the existing table exhaustive. This would have made the c12/c26/parse-vs-validate mismatches much easier to catch.

## Minor cleanup

- `c16` returns `DigestError` but only depends on `c01`; if dependency bullets are meant to cite referenced types, include `c02` (or clarify that the ubiquitous error-surface dependency is implicit).
- `c20` similarly returns `CompileError` but depends only on `c01`.
- `c01` says `tempfile` is in `[workspace.dependencies]` “as a dev-dep”; Cargo has no dev-only entry inside `[workspace.dependencies]`. Say `tempfile = "3"` is listed there and added under `rafaello-core`'s `[dev-dependencies]` when tests first need it, or create the crate dev-dep in `c01`.
