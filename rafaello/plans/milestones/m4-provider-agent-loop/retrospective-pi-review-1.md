# m4 retrospective.md — pi review round 1

Review target: `rafaello/plans/milestones/m4-provider-agent-loop/retrospective.md` at `d3fb08b`.

Verdict: **not ratifiable yet**. The coverage reconciliation is mostly sound, but the draft overclaims acceptance evidence in two places (`cargo doc` and `#[allow]`), leaves the companion `manual-validation.md` stale after the carveout fix, and marks drift/acceptance work as addressed before the required follow-up commits and CI/manual evidence land.

## Blocking findings

### B1. `cargo doc` warning-free is false

`retrospective.md:862-867` and the acceptance table row at `retrospective.md:955` say `cargo doc --workspace --no-deps` is warning-free and that `/tmp/m4-doc.log` contains no warnings. It does not.

`/tmp/m4-doc.log` contains:

```text
warning: public documentation for `subscribe_internal` links to private item `Self::notify_internal_subscribers`
   --> crates/rafaello-core/src/bus.rs:589:40
...
warning: `rafaello-core` (lib doc) generated 1 warning
```

Fix: either fix the rustdoc private intra-doc link and rerun the exact scope command, or mark the doc gate ❌/pending. As written, the scope acceptance bullet “warning-free” is not satisfied.

### B2. The companion evidence file is stale and contradicts the retrospective

The draft points at `manual-validation.md` as the companion (`retrospective.md:20`) and says post-fix verification restored the headline locally (`retrospective.md:649-652`). But `manual-validation.md` still says the driver must capture the Linux test/build/doc gates later (`manual-validation.md:22-26`, `:35-36`, `:58-63`) and still records the Landlock failure as current (`manual-validation.md:167-204`).

That is not just “interactive recording pending”; it leaves the canonical companion artifact in a pre-`0a0e824` state while the retrospective relies on post-`0a0e824` evidence. Ratification should wait until `manual-validation.md` is updated with the exact final Linux transcripts, corrected post-fix demo-bar status, and CI URLs.

### B3. “No `#[allow(...)]` was introduced by an agent during Phase 3” is false

`retrospective.md:855-860` claims no `#[allow(...)]` was introduced by a Phase-3 agent. `git blame` disproves that:

- `rafaello/crates/rafaello/tests/common/m4_install.rs:93` has `#[allow(clippy::too_many_arguments)]`, introduced by c25 (`8dbdfbb`).

This may be an acceptable suppression in test helper code, but the retrospective must not claim none were introduced. Either document the c25 allow as an accepted suppression or remove the claim.

### B4. Drift/ratification status is over-marked as addressed

The draft correctly lists pending follow-up commits, but then its final acceptance table marks `retrospective.md` / anticipated drift as ✅ (`retrospective.md:963`). At this revision, the required Stream A banners, decisions rows, and overview §4.6 patch have not landed (`retrospective.md:957-962` are still ⏳). The drift verdict also says “remaining four” while listing five sections (`§2.1`–`§2.5`) at `retrospective.md:405-406`.

Fix: mark the “anticipated drift addressed” acceptance item ⏳ until the follow-up commits land, and fix the drift arithmetic. After the docs commits land, update the table with commit hashes.

### B5. The §2.7 `[load]` drift is not closed by “recorded-only” status

`retrospective.md:364-370` says the post-retrospective sweep should grep `overview.md` / Stream F and patch or decide if the same `[load]` table syntax is found. The grep does find stale-ish references now:

- `rafaello/plans/overview.md:574` still lists ``[load]`` as the manifest field.
- `rafaello/plans/streams/f-manifest/rfc-manifest-schema.md` still contains `[load]` examples.

The draft then says §2.7 is “recorded-only — no follow-up commit needed in m4” (`retrospective.md:923-930`). That is inconsistent with its own recommended follow-up. At minimum, keep the overview grep/decision as an explicit pending pre-merge item; if overview wording is actually acceptable, record why it is not the stale `[load] eager = true` shape.

## Non-blocking notes / polish

### N1. “382 test binaries” is the `tests/*.rs` file count, not the Cargo test-run count

`retrospective.md:71-75` says the acceptance log has 608 tests across 382 test binaries. I verified 608/0/0, but parsing `/tmp/m4-acceptance.log` yields 397 `test result: ok` blocks and 398 `Running ...` lines including unit/doc-test targets. The 382 number matches the top-level integration-test file inventory (338+8+6+6+24), not Cargo test binaries. Reword to “382 top-level `tests/*.rs` files” or provide the exact Cargo target count.

### N2. The draft says the c27 test “passed in CI” before the CI gate exists

`retrospective.md:375-379` says the test passed in CI because CI did not enforce the Landlock dir-only rule, while the same document says branch push and macOS CI are still pending (`retrospective.md:20-23`, `:953`). If this refers to an agent-side run, do not call it CI; if an actual CI run exists, cite its URL in `manual-validation.md`.

## Confirmation table

| Claim | Verified | Notes |
|---|---:|---|
| 28 plan-row commits landed 1:1, plus follow-up fix `0a0e824`. | ✅ | `git log --reverse` shows c01 `8c4a1f1` through c28 `462f8e7`, then `0a0e824`, then the draft commit `d3fb08b`. |
| Scope converged after 6 pi rounds; commits after 3 rounds. | ✅ | `scope.md` status says round 6 converged; `commits.md` status says round 3. |
| All scope-named positive/negative behaviours land. | ✅ | I checked the retro’s named test files against `rafaello/crates/*/tests`; no unexplained missing matrix file. |
| Positive matrix mapping is satisfied. | ✅ | 45 positive table rows exist on disk and appear in `/tmp/m4-acceptance.log`. |
| Negative matrix mapping is satisfied. | ✅ | Six negative classes + six provider symmetry tests exist and appear in `/tmp/m4-acceptance.log`. |
| CLI orchestration negatives all land. | ✅ | The five scope C14 files plus `rfl_chat_tool_spawn_failure_propagates.rs` exist and appear in the acceptance log. |
| On-disk `tests/*.rs` inventory is 338 / 8 / 6 / 6 / 24. | ✅ | Verified with `find <crate>/tests -maxdepth 1 -type f -name '*.rs' | wc -l`. |
| Linux acceptance run is 608 passed / 0 failed / 0 ignored. | ✅ | Parsed `/tmp/m4-acceptance.log`: aggregate 608 / 0 / 0. |
| Linux acceptance run is across 382 “test binaries.” | ❌ | 382 is the top-level `tests/*.rs` file count. The log has 397 `test result` blocks / 398 `Running` lines including unit/doc-test targets. |
| Demo-bar headline test is green in acceptance log. | ✅ | `/tmp/m4-acceptance.log` contains `test rfl_chat_demo_bar_read_file ... ok`. |
| `cargo build --workspace --bins --features rafaello-core/test-fixture` green. | ✅ | `/tmp/m4-build.log` ends with `Finished dev profile`. Still should be archived in `manual-validation.md`. |
| `cargo doc --workspace --no-deps` warning-free. | ❌ | `/tmp/m4-doc.log` contains a rustdoc private intra-doc-link warning for `subscribe_internal`. |
| macOS CI green. | ⏳ | Correctly pending branch push / CI URL. |
| `manual-validation.md` records interactive demo + macOS CI URL. | ⏳ | Correctly pending in the retro, but the companion file is also stale about Linux gates and the fixed Landlock failure. |
| Stream A §10 banner follow-up. | ⏳ | No follow-up commit after `d3fb08b`; retro marks pending. |
| `PublisherIdentity::Provider` Stream A banner follow-up. | ⏳ | No follow-up commit after `d3fb08b`; retro marks pending. |
| `decisions.md` row for `BusEvent.request_id`. | ⏳ | No follow-up commit after `d3fb08b`; retro marks pending. |
| `decisions.md` row for `Publisher::Provider`. | ⏳ | No follow-up commit after `d3fb08b`; retro marks pending. |
| `overview.md` §4.6 reserved-env table patch. | ⏳ | No follow-up commit after `d3fb08b`; retro marks pending. |
| `retrospective.md` written with anticipated drift addressed. | ❌ | The draft is written, but the drift items are not addressed yet; acceptance row should be ⏳. |
| m1 lock-side unknown-namespace gap re-filed, no failure surfaced. | ⏳ | Re-file is documented; “no user-facing failure surfaced” is process evidence I cannot independently prove from the tree. |
| Fixture manifests use `load = "eager"`. | ✅ | Both `rafaello/fixtures/rafaello-mockprovider/rafaello.toml` and `rafaello/fixtures/rafaello-readfile/rafaello.toml` use the string shorthand. |
| §2.7 `[load]` drift needs no m4 follow-up. | ❌ | The draft’s own recommended grep finds overview/Stream F references; keep as pending or justify. |
| Carveout decomposition fix landed. | ✅ | `0a0e824` updates `decompose_dir`; `carveout_default_workspace_decomposition.rs` covers the partition. |
| No Phase-3 test deletions without same-commit successor. | ✅ | Verified the named row-39 deletion/successor (`supervisor_spawn_provider_lock_refused.rs` removed; `provider_plugin_spawns_through_supervisor.rs` present). I did not exhaustively audit every deleted test across all 28 commits. |
| No flakes observed in the recorded Linux run. | ✅ | `/tmp/m4-acceptance.log` shows a single clean run; no retry evidence in the log. |
| No `#[allow(...)]` introduced by Phase-3 agents. | ❌ | c25 introduced `#[allow(clippy::too_many_arguments)]` in `m4_install.rs`. |
| Lazy-load path intentionally not exercised. | ✅ | Fixture manifests are eager; no `rfl plugin start --skip-eager` m4 path found in the matrix. |
| Stale plugin `tool_result` correlation remains owner-accepted out-of-scope. | ✅ | Scope out-of-scope text and retro §5.1 agree. |

## Items checked vs items found

- **Test-name claims verified:** 83 unique test-binary filename claims from the retro’s tables/examples were present on disk and in `/tmp/m4-acceptance.log`; 2 helper-module names were present under `tests/common`; 1 deleted-stub name was intentionally absent with the documented successor present.
- **Acceptance/drift claims checked:** 28 rows in the confirmation table above.
- **Issues raised:** 7 total — 5 blocking, 2 non-blocking.
