# m4 retrospective.md — pi review round 4

Review target: `rafaello/plans/milestones/m4-provider-agent-loop/retrospective.md` at `917dfda`.

Verdict: **ratifiable**. Round 4 closes the round-3 blockers: the row-44 provider-id/canonical-id distinction is now aligned with live code, the Stream A §5 live-schema banner matches the current provider registration and ACL surfaces, and the `#[allow]` inventory now accounts for the c14 production suppression plus the live `m4_lock_fixture.rs` item-level shape.

No blockers found. I found two small polish notes that do not change ratifiability because the canonical acceptance table, decision row, and Stream A live banner are correct.

## Non-blocking notes / polish

### N1. One phase-summary bullet still says c13 checked at `BrokerAcl::compile`

`retrospective.md` §3.2 still summarizes c13 as:

> defence-in-depth provider publish-id check at `BrokerAcl::compile` (B11).

The canonical round-4 closure text and Stream A banner correctly say the live check is in `Broker::new` ACL revalidation and surfaces as `BrokerError::InvalidTopic`, exercised by `broker_construct_with_provider_publish_id_mismatch_rejected.rs`. Live code and the original c13 diff agree: `bbd840c` edits `rafaello-core/src/bus.rs`, not `broker_acl.rs`.

This is not blocking because the load-bearing locations for future authors (`Round-4 patch summary`, the acceptance table, Stream A §5 banner, and `decisions.md` row 44) are already correct, but the phase-summary bullet should be cleaned up opportunistically.

### N2. `manual-validation.md` tail still says pi ratification of round 2

The companion header was correctly refreshed to round 3 after the §5 owner-acceptance wording patch, and its macOS / interactive placeholders remain accurate. Its final sentence still says:

> Ratification additionally waits on pi ratification of `retrospective.md` round 2.

That should say the current retrospective round, but it is a tail-label nit only; the manual acceptance table correctly leaves macOS CI and interactive recording pending.

## Confirmation table

| Claim | Verified | Notes |
|---|---:|---|
| pi-r3 B1: `decisions.md` row 44 no longer says `provider_id` is what the active-provider lock field pins. | ✅ | Row 44 now says `session.provider_active` is parsed as a canonical id and used to look up `compiled_plugins`; live `rafaello/src/lib.rs` does exactly that. |
| pi-r3 B1: `provider_id` is the public provider namespace segment populated from lock bindings into `PluginAcl.provider_id`. | ✅ | `lock/bindings.rs` exposes `bindings.provider` + `provider_id`; `broker_acl::compile` copies `entry.bindings.provider_id` only when `bindings.provider = true`. |
| pi-r3 B2: Stream A banner names the live provider registration API. | ✅ | Banner says `Broker::register_provider(canonical: CanonicalId, peer: PeerHandle) -> Result<RegisteredProvider, BrokerError>`; live `bus.rs` matches. |
| pi-r3 B2: Stream A banner uses the live RAII type name. | ✅ | Banner and retrospective now say `RegisteredProvider`; `ProviderGuard` no longer appears as a live type claim. |
| pi-r3 B2: Stream A banner uses the live lock/ACL provider shape. | ✅ | Banner says `bindings.provider: bool` + `bindings.provider_id: Option<String>`, no `class = "provider"`; live lock and ACL code match. |
| pi-r3 B2: Stream A banner locates the c13 provider publish-id mismatch check correctly. | ✅ | Banner says `Broker::new` ACL revalidation → `BrokerError::InvalidTopic`; live `bus.rs` and test `broker_construct_with_provider_publish_id_mismatch_rejected.rs` match. |
| Stream-A diff scope for `41768a5`. | ✅ | `git show 41768a5 -- rafaello/plans/streams/a-security/rfc-security-model.md` changes only the §5 banner block lines; no body rewrite. |
| pi-r3 B3: §5.5 production `#[allow]` inventory count is now complete. | ✅ | §5.5 lists 4 production m4 sites, including `supervisor.rs:176` `SpawnRegistration`; `git blame` confirms c14 (`c0cb6e2`). |
| pi-r3 B3: `m4_lock_fixture.rs` site shape is now described correctly. | ✅ | §5.5 says two item-level function `#[allow(dead_code)]` suppressions; live file has exactly those at lines 10 and 22. |
| pi-r3 N1/N2: round label and stale provider-guard naming fixes landed. | ✅ / ⚠️ | Retrospective is round 4 and live naming is `RegisteredProvider`; companion header is refreshed, with only the tail-label nit above. |
| Linux acceptance aggregate remains 608 / 0 / 0. | ✅ | Parsed `/tmp/m4-acceptance.log`: 397 result blocks, aggregate 608 passed / 0 failed / 0 ignored. |
| On-disk scoped test inventory remains 382 top-level `tests/*.rs` files. | ✅ | Counted across the five rafaello test dirs named by the retrospective. |
| Build/doc logs remain green. | ✅ | `/tmp/m4-build.log` ends with `Finished dev profile`; `/tmp/m4-doc.log` has zero `warning:` lines. |
| macOS CI URL and interactive recording remain pending, not over-claimed. | ✅ | Both `retrospective.md` and `manual-validation.md` leave these as ⏳ pending post-retrospective driver sweep. |

## Items checked vs items found

- **Round-3 findings rechecked:** 5/5 (3 blockers + 2 nits). Closure verified for all five; one tail-label polish note remains in the companion file.
- **Live-code/provider-surface claims checked:** 8 claims across `register_provider`, `RegisteredProvider`, `provider_active`, `provider_id`, `broker_acl::compile`, and `Broker::new`; 0 blocking mismatches found.
- **Stream-A diff-scope check:** 1 commit (`41768a5`) inspected; changed lines are confined to the §5 banner block.
- **Evidence/log checks:** 4 checks (acceptance aggregate, scoped test count, build log, doc log); all pass.
- **Issues raised:** 2 total — 0 blocking, 2 non-blocking polish notes.

## Closing note

Round 4 is ratifiable. Recommend the driver merge to `rafaello-v0.1` after the macOS CI URL and interactive `rfl chat` recording land in `manual-validation.md`; both are correctly marked ⏳ today.
