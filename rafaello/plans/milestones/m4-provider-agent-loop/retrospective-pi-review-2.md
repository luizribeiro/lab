# m4 retrospective.md — pi review round 2

Review target: `rafaello/plans/milestones/m4-provider-agent-loop/retrospective.md` at `d04a625`.

Verdict: **not ratifiable yet**. Round 2 fixed the round-1 cargo-doc and stale-Linux-evidence problems, and the anticipated drift commits are now present. I still found two document-vs-code mismatches that would mislead future m5 authors: the newly-ratified `PublisherIdentity::Provider` docs omit the live `topic_id` field, and the clippy/`#[allow]` retrospective still under-reports m4 suppressions.

## Blocking findings

### B1. `PublisherIdentity::Provider` is documented without its live `topic_id` field

The round-2 follow-up docs that were supposed to close the Stream A / decisions drift record the provider wire identity as only `{ canonical, provider_id }`:

- `retrospective.md` §2.2 says Stream A now includes `PublisherIdentity::Provider { canonical, provider_id }`.
- `decisions.md` row 44 says `PublisherIdentity::Provider { canonical, provider_id: String }` lands in m4.
- `streams/a-security/rfc-security-model.md` §5 banner says m4 promotes `Provider { canonical, provider_id }` onto the live wire schema.

The code and the c07 schema test disagree. The live enum is:

```rust
Provider {
    canonical: String,
    provider_id: String,
    topic_id: String,
}
```

and `rafaello-core/tests/bus_event_serializes_provider_publisher_identity.rs` explicitly asserts the serialized provider publisher includes the three payload fields `canonical`, `provider_id`, and `topic_id`.

This is not just wording polish: row 44 is now the ratified on-disk decision future milestones will cite for the m4 wire shape. Fix the retrospective §2.2, `decisions.md` row 44, and the Stream A banner to include `topic_id`, or explicitly explain why the code/test should be changed instead.

### B2. §5.5 still under-reports `#[allow]` / clippy suppressions introduced in m4

Round 1 caught the false “no `#[allow]`” claim. Round 2 now says exactly one `#[allow(...)]` was introduced in Phase 3: c25's test-helper `#[allow(clippy::too_many_arguments)]`, and says no production `#[allow]` was introduced.

`git blame` shows more m4-introduced suppressions, including production clippy suppressions:

- `rafaello-core/src/reemit/mod.rs:1` — `#![allow(clippy::result_large_err)]`, introduced by c18 (`61209e7`).
- `rafaello-core/src/agent/mod.rs:1` — `#![allow(clippy::result_large_err)]`, introduced by c19 (`9036e22`).
- `rafaello-core/src/bus.rs:101` — `#[allow(dead_code)]`, introduced by c09 (`f2c07ad`).
- Several m4 test/common modules add `#![allow(dead_code)]`, e.g. `provider_test_kit.rs` (c10), `reemit_test_kit.rs` (c18), `agent_test_kit.rs` (c19), `mock_provider_handle.rs` (c21), `read_file_tool_handle.rs` (c23), `m4_lock_fixture.rs` (c24/c25), and `m4_install.rs` (c25).

If §5.5 is intended to cover only non-`dead_code` clippy suppressions, it still needs to list the c18/c19 production `result_large_err` module-level allows. If it is intended to cover all `#[allow]`, the section needs a fuller inventory and rationale. As written, the round-2 retrospective remains materially false on the same class of claim as round 1 B3.

## Non-blocking notes / polish

### N1. `manual-validation.md` overstates owner acceptance of mechanical demo coverage

`manual-validation.md` §5 says “Owner accepts mechanical coverage in lieu of a screen-recording (m3-precedent).” The retrospective is more cautious: §5.3 says the owner *may* accept the c27 headline test as substitute coverage, but the default expectation is a recorded run, and the acceptance table correctly leaves “interactive demo + macOS CI URL” pending.

Align the companion file with the retrospective unless there is an actual owner decision to cite. The current wording risks making a still-pending scope acceptance item look accepted.

### N2. Minor wording: decisions row 44 says the same canonical can be installed twice with different provider ids

`decisions.md` row 44 says `provider_id` is needed because “the same canonical can in principle be installed twice with different per-instance `provider_id` segments.” If current lock validation forbids duplicate canonicals or if this is only a future/v2 possibility, soften the rationale. The concrete m4 need is simpler: provider topics use the public provider-id namespace, while canonical ids identify installed packages.

## Confirmation table

| Claim | Verified | Notes |
|---|---:|---|
| Round-1 B1 cargo-doc warning was fixed. | ✅ | Re-ran `nix develop --impure --command cargo doc --manifest-path rafaello/Cargo.toml --workspace --no-deps` in this worktree; warning-free. `/tmp/m4-doc.log` also has zero `warning:` lines. |
| Round-1 B2 stale Linux evidence was fixed. | ✅ | `manual-validation.md` now records Linux test/build/doc transcripts and the post-`0a0e824` demo-bar status. |
| Round-1 B3 false no-allow claim was fully fixed. | ❌ | The c25 `too_many_arguments` allow is documented, but c18/c19 production `clippy::result_large_err` allows and other m4 allows are omitted. |
| Round-1 B4 drift follow-up commits landed. | ✅ | `c222087`, `9bd24e3`, `d51caba`, `3a3a917`, `63f6997`, `152813a` are present after `928df3e`. |
| Round-1 B5 `[load]` drift closure is now backed by on-disk records. | ✅ | `decisions.md` row 45 and `overview.md` §5.3 now document `load = "eager"` / `"lazy"`; Stream F residual examples are explicitly left to the standard RFC-not-rewritten rule. |
| Linux acceptance run aggregate remains 608 / 0 / 0. | ✅ | Parsed `/tmp/m4-acceptance.log`: 608 passed, 0 failed, 0 ignored. |
| On-disk test inventory is 382 top-level `tests/*.rs` files. | ✅ | `find` across the five rafaello test dirs returns 382 files. |
| Build gate log is green. | ✅ | `/tmp/m4-build.log` ends with `Finished dev profile`. |
| Demo-bar headline appears green in acceptance log. | ✅ | `/tmp/m4-acceptance.log` contains `test rfl_chat_demo_bar_read_file ... ok`. |
| macOS CI green. | ⏳ | Still correctly pending post-retrospective branch push. |
| Interactive `rfl chat` recording. | ⏳ | Still pending; companion wording should not imply owner acceptance unless that decision exists. |
| Provider wire identity docs match code. | ❌ | Code/test include `topic_id`; retrospective / Stream A / decisions row 44 omit it. |

## Items checked vs items found

- Rechecked the two formerly-false cargo evidence claims (`doc`, `manual-validation`) and the drift follow-up commits.
- Re-parsed acceptance counts and test inventory.
- Audited the provider publisher identity schema against code and its schema test.
- Audited all current `allow(` sites with `git blame` to identify m4-introduced suppressions.
- Issues raised: 4 total — 2 blocking, 2 non-blocking.
