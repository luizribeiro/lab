# m1 — manifest+lock — manual validation

> Captured 2026-05-08, after c34/c35 (and the c36 `MethodNotFound`
> typed-method cutover) landed, on the `agents/m1/c36` worktree off
> `rafaello-v0.1`.

This document records the milestone-level manual validation called
out in `scope.md` §"Manual validation in `manual-validation.md`".
Per scope, the macOS leg is delegated to CI under the m0 precedent —
m1 has no platform-specific code, so only the Linux leg is exercised
here.

## Environment

| Item | Value |
|------|-------|
| Host OS | `Linux 6.12.84 x86_64` (NixOS) |
| Rust | `rustc 1.94.0 (4a4ef493e 2026-03-02)` (workspace `rust-toolchain.toml`) |
| Branch | `agents/m1/c36` (off `rafaello-v0.1`) |
| HEAD at capture | `72d3a90 feat(fittings): MethodNotFound typed `method` field cutover (W1–W5)` |

## 1. `cargo test -p rafaello-core` green

```
$ cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core
... 269 tests passed; 0 failed; 0 ignored ...
```

Across 137 binaries (the integration-test suite plus the lib + doc
units), every assertion in scope.md §C / §G / §V / §D / §K / §L /
§M / §S / §T / §B / §Sc / §W is exercised by this run. No test ran
longer than ~1 s individually; the whole run completes in well under
the 30 s scope budget. No flake observed across two back-to-back
runs.

The repo has no root `Cargo.toml`, so `--manifest-path` is mandatory
(pi review-4 finding 9).

## 2. `cargo test --manifest-path fittings/Cargo.toml --workspace`

c37 capture (single run):
```
$ cargo test --manifest-path fittings/Cargo.toml --workspace
... 229 passed; 1 failed; 0 ignored ...
```

m1-retrospective re-run (3 attempts):
```
$ for i in 1 2 3; do cargo test ... -p mcp-server stdio_e2e_runtime_registry_mutation_emits; done
run 1: FAILED
run 2: FAILED
run 3: ok
```

The single failure is the pre-existing m0 flake documented in
`rafaello/plans/milestones/m0-fittings/manual-validation.md` §2:

```
---- stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list stdout ----
assertion `left == right` failed
  left: ["add", "add_with_details", "echo", "long_running_demo", "progress_demo"]
 right: ["add", "add_with_details", "echo", "long_running_demo", "progress_demo", "runtime_tool"]
```

Same race as m0: `tools/list` is observed before the preceding
`tools/register` is processed by the server. The test pre-dates m0,
is not in either m0 or m1's acceptance test matrix, and has nothing
to do with the §W enum cutover that motivates this validation step
(the cutover targets `MethodNotFound::method`, not server-side
registry mutation). All other 228 fittings tests pass — including
the targeted regression test for the §W enum cutover
(`method_not_found_typed_method_round_trip.rs`) — confirming the
cutover's blast radius across the fittings workspace is clean.

**Acceptance posture (pi retrospective review-1+2+3+4):** the
ratified `scope.md` §"Acceptance summary" requires this command
to be green. The retrospective records this as a **waiver request
pending explicit owner ratification** rather than a unilateral
pass — the m0 retrospective §5.2 documented the same flake and
proposed a fix (read-then-write in the mcp-server test harness)
that has not yet landed. The §W cutover in m1 doesn't touch the
flaky test; the flake is a pre-existing harness bug owned by
mcp-server's test infrastructure. The retrospective owner
ratification step is the natural place to either (a) approve the
waiver, or (b) require the fix land before m1 closes.

## 3. `cargo doc -p rafaello-core --no-deps`

```
$ cargo doc --manifest-path rafaello/Cargo.toml -p rafaello-core --no-deps
    Finished `dev` profile [unoptimized + debuginfo] target(s) in <…>s
```

Warning-free. The c37 capture had one broken-intra-doc-links warning
on `src/error.rs:6`'s ``[`Error`]`` link (rustdoc couldn't
disambiguate the enum from the `thiserror::Error` re-export); fixed in
the m1 retrospective sweep (commit `823e8bb`) by changing to
``[`enum@Error`]``. Pi review-1 of the retrospective flagged the
warning as a then-pending acceptance gap; pi-2 confirmed it landed.

## 4. `cargo test -p rafaello-core --release`

```
$ cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core --release
... 269 tests passed; 0 failed; 0 ignored ...
```

Same 269 tests as §1, recompiled with `--release`. Green. This
specifically exercises the digest module's deterministic-walk
algorithm (scope §D1) and the carve-out decomposition worst-case
behaviour (scope §K) under optimised codegen — both areas where a
release-only divergence (e.g. iteration order via a `HashMap` rather
than `BTreeMap`) would have shown up as a non-deterministic digest
or a different decomposition. None observed.

## 5. `nix develop --impure -L --command cargo test -p rafaello-core`

```
$ nix develop --impure -L --command \
    cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core
... 269 tests passed; 0 failed; 0 ignored ...
```

Green inside the project's `devenv` shell. `--impure` is
load-bearing per the m0 retrospective gotcha §4.6 (`devenv` asserts
on the current directory, which only works in impure mode); without
it, `nix develop` aborts before any compilation begins.

Same 269-test set as §1 and §4, identical pass count, no flake.
Confirms the m1 surface compiles + tests cleanly against the
toolchain pinned by `flake.nix` / `rust-toolchain.toml`, not just
against whatever `cargo` happens to be on `$PATH`.

The macOS leg is delegated to CI per scope.

## 6. Test surface dump

Scope §"Manual validation" was reworded in the retrospective sweep
(post-pi review-2 finding 3) to ask for `find
rafaello/crates/rafaello-core/tests -type f -name '*.rs' | sort`
rather than `tree .../tests/fixtures` — the latter pointed at a
directory that doesn't exist. m1 tests carry their fixture material
inline (string literals + `serde_json::json!` + `tempfile::TempDir`)
rather than as on-disk fixture files; that's the deliberate v1
choice (no separate `tests/fixtures/` directory).

The closest available surface is the integration-test directory
itself, which is the analogue artefact m2 would consume to discover
which manifest+lock shapes m1 considers in-spec:

```
$ ls rafaello/crates/rafaello-core/tests/
```

136 entries, comprising:

- `common/mod.rs` — shared test helpers.
- 12 `carveout_*` integration tests (scope §K).
- 19 `compile_*` integration tests (scope §C).
- 6 `digest_*` integration tests (scope §D).
- 3 `env_scrubber_*` integration tests (scope §Sc).
- 1 `error_surface_compiles.rs` integration test (scope §G).
- 33 `lock_*` integration tests (scope §L).
- 44 `manifest_*` integration tests (scope §M).
- 4 `sinks_*` integration tests (scope §Sn).
- 2 `tool_*` integration tests (scope §V / §C).
- 3 `topic_id_*` integration tests (scope §T).
- 2 `trifecta_*` integration tests (scope §V).
- 2 `validate_lock_*` integration tests (scope §V).
- 2 `broker_acl_*` integration tests (scope §B).
- 1 `capability_path_template_parse.rs` (scope §C).
- 1 `safepath_parse.rs` (scope §C).

The full file listing (one binary per test target) sits next to this
file in git; reading it back from there is the canonical inventory.

## Follow-ups discovered while exercising this

1. **F1: `cargo doc` broken-intra-doc-links warning in
   `crates/rafaello-core/src/error.rs:6`.** Module docstring writes
   ``[`Error`]`` but rustdoc sees both the enum `Error` and the
   `thiserror::Error` derive-macro re-export in scope. One-character
   fix: change to ``[`enum@Error`]``. Lands in m1 retrospective
   sweep, not c37. Not a public-API-surface regression.

2. **F2: scope wording on the fixtures directory.** §"Manual
   validation" calls for `tree
   rafaello/crates/rafaello-core/tests/fixtures`, but per scope
   §"Out of scope" m1 deliberately keeps fixtures inline rather than
   on-disk. The two scope sections drift; the §"Manual validation"
   bullet should either be reworded ("dump
   `tests/`") or dropped, alongside a note pointing at the inline-
   fixtures rationale. m1 retrospective territory.

3. **F3: pre-existing m0 flake
   `stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`**
   tripped here too (§2). Already filed in m0
   manual-validation.md §2 / m0 retrospective; carrying the
   reference forward so future m1 readers do not re-debug it.
   Fix lives in `mcp-server` test-harness, not in the m1 surface.

None of the above blocks the c37 acceptance; all three are recorded
for the m1 `retrospective.md`.
