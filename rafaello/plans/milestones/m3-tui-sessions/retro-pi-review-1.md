# m3 retrospective.md — pi review round 1

Review target: `rafaello/plans/milestones/m3-tui-sessions/retrospective.md` at `f4275bc`.

Verdict: **not ratifiable yet**. The draft has the right high-level structure, but it currently overclaims coverage, cites acceptance evidence that is not archived, and treats pending drift patches as already addressed. I expect another review round after the reconciliation/evidence pass and the pre-named follow-up commits land.

## Blocking findings

### B1. The “no file-name deviations” coverage claim is false

`retrospective.md:34-49` says every named scope test is implemented and that there were “No 1:1 deviations from the scope-named test files.” That does not match the landed tree.

Comparing `scope.md`’s §I positive/negative matrix filenames with `crates/**/tests/**/*.rs`, these scope-named files are not present under their scoped names:

- `renderer_pipeline_built_in_kinds.rs` — landed as the eight split `renderer_builtin_{text,heading,code_block,thinking,tool_call,tool_result,image,error}.rs` tests per `commits.md` c11/c12.
- `tui_subscribes_to_core_session_events.rs` — the closest landed TUI behavior is `tui_test_mode_logs_bus_events_to_stderr.rs`; there is also a core-layer `frontend_subscribes_to_core_session_events.rs`, but the scope-named TUI file does not exist.
- `frontend_register_with_broker.rs` — no file by that name exists; registration happy-path behavior is only indirectly covered by other frontend tests.
- `frontend_register_duplicate_rejected.rs` — landed as `broker_register_frontend_duplicate_rejected.rs`.

The retrospective also names non-existent/globbed files in the coverage table, e.g. `renderer_pipeline_path_*` at `retrospective.md:70`; the actual c10 tests are the two unknown-kind tests, panic/Err fallback tests, and the two capability-downgrade tests.

Fix: replace the “name-for-name” claim with an explicit scope-vs-landed reconciliation table. It is fine to conclude behavior coverage is complete, but only after documenting the split/rename/indirect-coverage cases above.

### B2. Negative matrix verification is incomplete and contains wrong file names

`retrospective.md:79-92` says “The §I negative matrix is satisfied,” but the table is not the §I negative matrix:

- It omits the scope negative rows for all four frontend publish-rejection tests (`frontend_publish_on_reserved_namespace_rejected.rs`, `frontend_publish_two_segment_topic_rejected.rs`, `frontend_publish_unknown_namespace_rejected.rs`, `frontend_publish_outside_grant_rejected.rs`).
- It omits the frontend spawn Phase-A negative rows (`frontend_spawn_invalid_attach_id_rejected.rs`, relative path, control chars, not executable, reserved env in pass/set).
- It omits the session-store negative rows (`session_store_concurrent_open_errors.rs`, `session_store_locked_unknown_holder_errors.rs`, `session_store_schema_mismatch_errors.rs`).
- It lists names that do not exist in the landed tree, e.g. `bus_publish_unknown_publisher_kind_rejected.rs` and `bus_register_frontend_not_in_acl_rejected.rs`; the landed names are `frontend_publish_unknown_namespace_rejected.rs` and `broker_register_frontend_unknown_attach_id_rejected.rs`.
- It mixes in CLI error tests and `frontend_handle_drop_*`, which may be valuable coverage but are not substitutes for an exhaustive §I negative-matrix reconciliation.

Fix: make the negative table exhaustive against the 15 negative-matrix filenames in `scope.md`, with landed path/commit and any rename explanation. Put CLI negative tests in a separate “additional/CLI matrix” table if desired.

### B3. Acceptance evidence is missing or cited with the wrong command/status

The retrospective claims a captured Linux run of **516 tests / 0 failed / 0 ignored** (`retrospective.md:39-42`, `:428-430`), plus build/doc success (`:520-521`), but `plans/milestones/m3-tui-sessions/manual-validation.md` does not exist at this revision. The companion is referenced at `retrospective.md:15-16` and marked “writing alongside” at `:522`, but no transcript/URL artifact is available for review.

There are also command/status problems:

- `retrospective.md:39` cites `cargo test --workspace --features rafaello-core/test-fixture`, while `scope.md`’s Linux/macOS acceptance command is `cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture`.
- `retrospective.md:117-121` says the Linux test run satisfies the acceptance summary’s “first three bullets,” but the third acceptance bullet is **macOS CI green**, which `retrospective.md:410-423` and `:519` correctly say is still pending.
- The final checklist omits `--manifest-path rafaello/Cargo.toml`; from the worktree root `/home/luiz/lab`, the unqualified `cargo test --workspace` / `cargo doc --workspace` commands do not describe the scoped acceptance command.

Fix: create/update `manual-validation.md` with the exact Linux test/build/doc transcripts, the interactive `rfl chat` evidence, and the macOS CI URL/status. In the retrospective, quote the exact scope commands and do not say any Linux run satisfies the macOS gate.

### B4. Pending drift fixes are recorded as “addressed” before they land

`retrospective.md:263-267` says all nine drift items are addressed by follow-up commits or already-landed code, and §2 repeatedly says “m3 lands” banners/decision rows. But at `f4275bc`, the five follow-up commits listed at `retrospective.md:477-491` have not landed yet.

This is especially important because `scope.md` acceptance requires the retrospective/drift phase to land the Stream E banner, Stream A banner, overview §10.1 capabilities wording, replay decision row, and BrokerError decision row. The final paragraph (`retrospective.md:537-541`) correctly says ratification waits for these commits, but the §2 verdict language is too easy to read as already accepted.

Fix: distinguish “recorded here as pending follow-up” from “addressed/landed.” After the five docs commits land, update §2 and the acceptance table from pending to done with commit hashes.

### B5. Manual-validation scope items are not yet represented

`scope.md` requires `manual-validation.md` to record: Linux/macOS cargo-test evidence, a real interactive `rfl chat` run with `RFL_HARNESS_FIXTURES=1`, terminal restoration/quit behavior, second-chat lock behavior, and CI green. The retrospective currently has no companion file and no citation for the interactive smoke.

Fix: add the missing `manual-validation.md` before the next retrospective review, or explicitly mark the retrospective as pre-evidence and not ratifiable.

## Non-blocking notes / polish

- `retrospective.md:96` mentions `m3_types_compile.rs` patterns, but no file by that exact name exists. Use the actual landed names, e.g. `m3_frontend_error_surface_compiles.rs`, or keep the wording clearly generic.
- `retrospective.md:65` collapses `frontend_handle_*` into four tests and includes “timeout/post-ready”; the landed c20/core handle tests have more precise names, while the post-ready abnormal CLI path lands in c30. Spell these out to avoid hiding coverage boundaries.
- `retrospective.md:520-521` says build/doc were verified during c25/c27/post-c31. Acceptance should be evidenced by a final post-c31 exact-command run, not inferred from intermediate commit gates.
- The “Tests added beyond the matrix” section is labelled “examples,” which is good. Keep it non-exhaustive unless it is reconciled against `commits.md` acceptance rows.
