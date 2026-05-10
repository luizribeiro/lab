# m2 retrospective.md — pi review round 1

Review target: `rafaello/plans/milestones/m2-broker-spawn/retrospective.md` at `a719748`.

Verdict: **not ratifiable yet**. The draft is useful and catches real issues, but it still overclaims coverage and contains several factual mismatches against the landed tree / `scope.md` / `commits.md`. I expect at least one more review round after these are fixed.

## Blocking findings

### B1. Coverage section claims scope-named file presence that is false

`retrospective.md:34-36`, `:83-84`, `:112`, `:130-131`, and `:772-776` repeatedly say every scope-named positive/negative test file landed and that `ls tests/` confirms file presence for every scope-named row.

That is not true in the current tree:

- `scope.md` names `bus_event_schema_round_trip.rs`; landed file is `bus_wire_types_schema.rs` per `commits.md` c06.
- `scope.md` names `supervisor_lifecycle_drop_kills_child.rs`; landed file is `supervisor_drop_kills_managed_children.rs` per c26.
- `scope.md` names `supervisor_spawn_reserved_env_helper_refused.rs`; landed coverage is folded into the table-driven `supervisor_spawn_reserved_env_in_{set,pass}_refused.rs` files per c16.

The draft currently records only one rename/disambiguation (`supervisor_spawn_fixture_lifecycle.rs` vs `supervisor_spawn_fixture_happy_path.rs`). It needs an explicit scope-vs-commits reconciliation table for all renamed/merged behaviours, and the broad claim should be changed from “every named test file landed” to “every named behaviour is covered, with these file-name changes/merges.”

Related inventory mismatch: `retrospective.md:37-39` says the on-disk listing has 27 `supervisor_*.rs` and 6 `fixture_*.rs`; the current tree has 32 supervisor files and 7 fixture files. Re-run the inventory and update it.

### B2. Deleted unwind coverage is understated: there were three deleted tests, not two

`retrospective.md:116-128`, `:386-391`, `:427-466`, and `:663-684` describe exactly two deleted unwind tests (`supervisor_spawn_unwinds_after_register.rs` and `supervisor_spawn_post_register_reaps_child.rs`). But `5378718` also deleted:

- `supervisor_spawn_unwinds_after_socketpair.rs`

Evidence: `git show --name-status 5378718 -- rafaello/crates/rafaello-core/tests/supervisor_spawn_unwinds_after_socketpair.rs` reports `D`.

That test was not the same post-register window; it covered the earlier Phase-B unwind after socketpair / sandbox-builder setup, including the Linux fd-count return-to-baseline assertion. The proposed §5.1 m3 follow-up only injects faults after `register_plugin`, so it would not restore the pre-child-spawn/socketpair cleanup coverage.

Fix: update §1/§3/§5 to account for all three deleted tests. Either document why the socketpair unwind coverage has an adequate successor, or add a separate follow-up/injection point for the pre-register Phase-B failure window (after socketpair/proxy/tokio_command but before child/register).

### B3. `PublisherIdentity` is documented with variants that do not exist in m2

`retrospective.md:216-224` says m2 fixes `PublisherIdentity` as:

> `Core | Plugin(CanonicalId) | Provider(ProviderId) | Frontend(AttachId)` per scope §B6

The landed code has only:

```rust
pub enum PublisherIdentity {
    Core,
    Plugin { canonical: String, topic_id: String },
}
```

And `scope.md` §B8 lists `Provider` / `Frontend` only as future commented variants, not m2 variants. This matters because the planned Stream A banner would over-document the live m2 wire schema.

Fix §2.4 and the follow-up banner wording to say m2’s live `PublisherIdentity` is exactly `Core` plus `Plugin { canonical, topic_id }`; provider/frontend identities are reserved/future m3–m5 work.

### B4. Proposed provider-rejection decisions row does not match the landed error shape

`retrospective.md:167-169` proposes recording:

> `SpawnError::InvalidPlan { reason: ProviderNotInM2 }`

The landed `InvalidPlanReason` variant is `ProviderNotInM2 { provider_id: String }`, and c16 acceptance expects the provider id in the error. The row should preserve that payload; otherwise the decision log will not match the public error surface.

Fix row 39 wording to use `ProviderNotInM2 { provider_id }` (and, if desired, state that the refusal is detected from the compiled ACL/provider metadata for lock entries with provider bindings).

### B5. Several strong empirical claims lack evidence or use the wrong command range

The draft makes claims that are stronger than the recorded evidence:

- `retrospective.md:708-716` says “Three full reruns of the 357-test suite” found 0 flakes. `manual-validation.md` records one aggregated run; I did not find the three-run evidence in `manual-validation.md` or `driver-notes.md`.
- `retrospective.md:680-682` says the m3 fault-injection fix is “Owner-blessed”; I did not find a written owner-blessing citation in the milestone docs.
- `retrospective.md:718-722` cites `git log -p HEAD~31..HEAD | grep ...` for clippy suppressions. At current `HEAD`, `HEAD~31` is `f553036`, so that range excludes c01/c02 and includes the retrospective/docs tail rather than exactly the m2 implementation range. It is probably harmless here, but the command does not prove what the sentence says.

Fix by either adding citations/transcripts to `driver-notes.md` / `manual-validation.md`, or weakening these to “not observed in the recorded run”, “proposed m3 follow-up”, and using an exact range such as `8ea4502^..1d68b5b` (or whatever range is intended).

### B6. Acceptance-status checkmarks are too optimistic while required follow-ups are still pending

The draft correctly says at `retrospective.md:378-379` that the retrospective does not ratify until follow-up items 1–7 land or are waived. But the final checklist marks as done:

- `retrospective.md:788-791`: “retrospective.md written with the eight anticipated drift items addressed”

while the very next bullets mark Stream A, reserved-env/decisions, provider staging, and `request_id` patches pending. This is easy for an owner to misread as “drift addressed” rather than “drift recorded with fixes pending.”

Fix the checklist language to distinguish “recorded in this retrospective” from “addressed by follow-up patch,” and avoid ✅ on any acceptance item whose required code/docs patch is still pending.

### B7. Commit-count/status banner is internally inconsistent

`retrospective.md:3-5` says “all 31 m2 git commits (`8ea4502` → `1d68b5b`) landed.” But `retrospective.md:21-28` then says there are 31 plan-row commits plus an additional docs-only restructure commit `d7c7705` in that same span. The git log from `8ea4502` through `1d68b5b` contains 32 commits if the restructure commit is included.

Fix the banner to say “31 plan-row commits plus one docs-only restructure commit landed before the retrospective,” or otherwise define the range precisely.

## Non-blocking notes / polish

- `retrospective.md:197-203` cites `scope §B9` for both `publish_rejected` and `boot`; `boot` is specified under the `publish_boot` bullet earlier in §B1, with the acceptance summary later calling out the schema. Cite both locations or avoid implying `boot` is in §B9.
- The “Tests added beyond the matrix” list omits some commits.md-ratified non-matrix tests (`m2_error_surface_compiles.rs`, `bus_wire_types_schema.rs`, `supervisor_spawn_{starts_proxy_for_proxy_plan,skips_proxy_for_deny_plan}.rs`). If the section is meant to be exhaustive, add them; if not, label it as examples.
- The c22/c23 restructure narrative is good and should stay; after B2 is fixed, consider generalising the lesson to “synthetic stub tests must have a planned successor or explicit deletion rationale.”
