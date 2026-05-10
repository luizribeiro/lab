# m2 retrospective.md — pi review round 2

Review target: `rafaello/plans/milestones/m2-broker-spawn/retrospective.md` at `51a4e48`.

I re-checked the round-1 findings against the landed tree and spot-checked the relevant sources:

```bash
find rafaello/crates/rafaello-core/tests -maxdepth 1 -name 'supervisor_*.rs' | wc -l
# 32
find rafaello/crates/rafaello-core/tests -maxdepth 1 -name 'fixture_*.rs' | wc -l
# 7
find rafaello/crates/rafaello-core/tests -maxdepth 1 -type f -printf '%f\n' | sort | rg '^(bus_wire_types_schema|supervisor_drop_kills_managed_children|supervisor_spawn_reserved_env_in_pass_refused|supervisor_spawn_reserved_env_in_set_refused|supervisor_spawn_fixture_happy_path|supervisor_spawn_fixture_lifecycle)\.rs$'

git rev-list --count 8ea4502^..1d68b5b
# 32

git show --name-status --format='' 5378718 -- rafaello/crates/rafaello-core/tests | rg '^D|unwinds|post_register'
# D supervisor_spawn_post_register_reaps_child.rs
# D supervisor_spawn_unwinds_after_register.rs
# D supervisor_spawn_unwinds_after_socketpair.rs

rg -n 'PublisherIdentity|ProviderNotInM2' \
  rafaello/crates/rafaello-core/src/{bus.rs,error.rs,supervisor.rs} \
  rafaello/plans/milestones/m2-broker-spawn/retrospective.md
```

## Verdict

**Converging. No new blocking findings in the round-2 retrospective text.**

Round 2 addresses the substance of all seven round-1 blockers:

- file-name / behaviour reconciliation is now explicit, and the supervisor / fixture inventory counts match the tree;
- the deleted unwind roster now includes all three c21 deletions, including the pre-register socketpair window;
- `PublisherIdentity` is documented as the live two-variant m2 schema (`Core`, `Plugin { canonical, topic_id }`);
- provider refusal now preserves the `ProviderNotInM2 { provider_id }` payload;
- empirical claims were weakened to the recorded evidence, and the clippy-suppression range is exact;
- the acceptance checklist distinguishes recorded-vs-landed follow-ups;
- the commit-count banner now says 31 plan-row commits plus `d7c7705` = 32 commits.

The milestone is still **not ready for owner ratification** because the retrospective itself accurately leaves follow-up commits 1–6 plus macOS CI pending. That is no longer a retrospective review blocker; it is the expected next state.

## Non-blocking polish before final owner packet

1. In the final acceptance checklist, prefer “named behaviours” over “named tests” for the matrix bullet, matching the §1 reconciliation table and avoiding a literal re-open of round-1 B1.
2. The §2.13 wording says “eight pieces of action-required drift” even though item 8 is recorded-only. Consider “eight drift items” or “seven action-required items plus recorded-only carryovers.”
3. The `scope.md` / `commits.md` status banners still read as draft / awaiting owner ratification even though the retrospective says they were ratified. If ratification was out-of-band, either leave this as historical draft-state prose or add a small final-status note so future readers do not trip on it.

## Trajectory check

m1 round 2 still had unresolved acceptance posture and missed drift blockers. m2 round 2 is materially cleaner: round-1 issues were fixed rather than papered over, and the remaining work is explicit follow-up execution, not retrospective correctness. I would expect the next pi pass after follow-up commits to be a short verification pass, not another major adversarial rewrite.
