# m4 retrospective.md — pi review round 3

Review target: `rafaello/plans/milestones/m4-provider-agent-loop/retrospective.md` at `9b116fa`.

Verdict: **not ratifiable yet**. Round 3 fixed the specific `topic_id` omission in `PublisherIdentity::Provider` and aligned the companion demo-smoke wording, but the new follow-up text still contains live-surface mismatches that would mislead m5 authors. The `#[allow]` inventory also remains incomplete after claiming to be complete.

## Blocking findings

### B1. `decisions.md` row 44 now misstates what `session.provider_active` pins

Round 3 softened row 44's rationale, but the landed wording now says:

> The `provider_id` segment is the runtime-active provider identity (the value the lock's `active_provider` pins), not the canonical id ...

The live lock path does the opposite: `session.provider_active` stores a canonical plugin id string, which `rfl chat` parses into a `CanonicalId` and uses to look up the active provider plan:

- `rafaello/crates/rafaello/src/lib.rs` reads `lock.session.provider_active`, parses it with `CanonicalId::parse`, and looks up `compiled_plugins.get(&provider_canonical)`.
- `rafaello/crates/rafaello-core/src/lock/session.rs` exposes `provider_active: Option<String>`; validation treats it as a plugin/canonical reference.
- `provider_id` is taken from the lock entry's bindings (`entry.bindings.provider_id`) into `PluginAcl.provider_id` in `broker_acl::compile`, then used as the public `provider.<provider-id>.*` namespace segment.

So the correct distinction is: the active provider is selected by canonical id; the provider id is the public namespace segment associated with that provider entry. Row 44 is a ratified decision row, so this needs correction before future milestones cite it.

### B2. The Stream A m4 banner still documents non-live provider APIs / lock shape

The round-3 retrospective says the Stream A banner was corrected to the live provider schema. The `topic_id` field is now present, but the same banner still has other live-surface errors:

- It says `Broker::register_provider(canonical, provider_id) -> ProviderGuard`. The live API is `Broker::register_provider(canonical: CanonicalId, peer: PeerHandle) -> Result<RegisteredProvider, BrokerError>`; the provider id is derived from `BrokerAcl.plugins[canonical].provider_id`, not passed by the caller.
- It names `ProviderGuard`, but the public RAII type is `RegisteredProvider`.
- It says `PluginAcl.provider_id` is populated for plugins declared as `class = "provider"` in the lock. The live lock shape is `bindings.provider = true` plus `bindings.provider_id`; `broker_acl::compile` checks `entry.bindings.provider`.
- It says c13 adds the provider-id mismatch check at `BrokerAcl::compile`; the live check is in `Broker::new` ACL revalidation (`BrokerError::InvalidTopic`), as exercised by `broker_construct_with_provider_publish_id_mismatch_rejected.rs`.

Because this banner is the deliberate "live schema overlay" on a historical RFC, these are not harmless old-RFC drift. Either fix the banner or explicitly say only the `PublisherIdentity::Provider` wire schema was updated and leave these API claims out.

### B3. §5.5 still omits an m4-introduced production `#[allow]`

Round 3 says §5.5 is the complete m4-introduced inventory and lists **3 production sites**. `git blame` still finds another m4 production suppression:

- `rafaello/crates/rafaello-core/src/supervisor.rs:176` — `#[allow(dead_code)]` on `SpawnRegistration`, introduced by c14 (`c0cb6e2`), with the inline rationale `Held only for RAII drop; never read.`

That is the same production-code class as the documented `bus.rs:101` `ProviderConn` allow and should be recorded. Also, the test inventory says `m4_lock_fixture.rs` has module-level `#![allow(dead_code)]`; the live file has two item-level `#[allow(dead_code)]` function suppressions instead. The latter is minor, but the missing c14 production allow means the inventory is still materially incomplete.

## Non-blocking notes / polish

### N1. A few round labels remain stale

`retrospective.md` still says the pending pi gate is review of "this round-2 document" even though the file is round 3. `manual-validation.md`'s header also says "Status: round 2" despite the round-3 wording patch. This is polish, not a ratification blocker.

### N2. The retrospective itself uses the stale `ProviderGuard` name

Even outside Stream A, `retrospective.md` §3.2 and §5.5 refer to `ProviderGuard`. The live type is `RegisteredProvider`. If B2 is fixed, align these mentions too.

## Confirmation table

| Claim | Verified | Notes |
|---|---:|---|
| pi-r2 B1 `PublisherIdentity::Provider` includes `topic_id`. | ✅ | `decisions.md` row 44 and Stream A §5 banner now include `{ canonical, provider_id, topic_id }`; code/test agree. |
| pi-r2 N1 manual-validation owner-acceptance wording softened. | ✅ | `manual-validation.md` §5 now says owner *may* accept mechanical coverage; interactive recording remains pending. |
| pi-r2 N2 row 44 rationale softened. | ⚠️ | It no longer claims duplicate canonicals, but now falsely says `provider_id` is what `active_provider` pins. |
| pi-r2 B2 complete `#[allow]` inventory. | ❌ | Missing c14 `supervisor.rs:176` production `#[allow(dead_code)]`; `m4_lock_fixture.rs` site kind is also misdescribed. |
| Linux acceptance aggregate remains 608 / 0 / 0. | ✅ | Parsed `/tmp/m4-acceptance.log`: 397 result blocks, aggregate 608 passed / 0 failed / 0 ignored. |
| On-disk top-level `tests/*.rs` inventory remains 382. | ✅ | `find` across the five rafaello test dirs returns 382 files. |
| Build/doc logs remain green. | ✅ | `/tmp/m4-build.log` ends with `Finished dev profile`; `/tmp/m4-doc.log` has no `warning:` lines. |
| macOS CI and interactive recording are still pending. | ✅ | Correctly left pending in the retrospective / companion acceptance tables. |
| Stream A m4 live banner matches provider API. | ❌ | Banner has stale `register_provider(canonical, provider_id) -> ProviderGuard`, `class = "provider"`, and `BrokerAcl::compile` claims. |

## Items checked vs items found

- Rechecked the round-2 blockers/nits against the round-3 patch.
- Re-audited all `allow(` sites with `git blame` for m4-introduced suppressions.
- Re-parsed the Linux acceptance log and test-file inventory.
- Compared the provider identity / registration docs against live code.
- Issues raised: 5 total — 3 blocking, 2 non-blocking.
