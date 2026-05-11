# m5a commits.md round-4 pi review

> Verdict: blocking.
> Counts: B/2 M/1 N/1

Reviewed the round-4 `commits.md` at `258e4de`, the round-3 review, the `dc8da82..HEAD` diff, ratified `scope.md`, and live source under `rafaello/crates/`. Round 4 closes the specific round-3 items: c34 now uses live `session.provider_active`, c11's `confirm_resolved` positive is broker-level, c34 builds the combined `ToolSchemaCatalog`, and the stale c37 row-body references were changed to c38.

Two blockers remain from a live-schema re-attack: the plan treats `session.tool_owner` as an active tool-owner pinning table. In live m1 it is only a conflict-resolution table, and redundant entries are rejected by `validate::lock`.

## Findings

### B1 — c34's canonical m5a fixture lock sets redundant `session.tool_owner` entries that live validation rejects

Anchor: c34 (`rfl-openai` manifest + lock fixture).

Round 4 says the complete fixture lock sets:

> `session.tool_owner.send-mail = "local:mailcat@0.0.0"`, and `session.tool_owner.read-file = "local:readfile@0.0.0"`.

It also adds an acceptance file asserting those tool owners are pinned.

Live validation does not allow that unless there is an actual tool-name conflict. `crates/rafaello-core/src/validate/mod.rs` builds `tool_claims`, then for every `lock.session.tool_owner` entry returns `ValidationError::ToolOwnerRedundant` when `claim_count <= 1`. The ratified m1 scope says the same: redundant owner entries are rejected so dead state is not retained.

The c34 four-plugin fixture has exactly one owner for `send-mail` (mailcat) and exactly one owner for `read-file` (readfile). `openai` and `mockprovider` are providers and do not declare those tools. Therefore `m5a_fixture_lock_validates_and_compiles.rs` cannot pass as written: `validate::lock` rejects the lock before compile/catalog checks run.

Smallest fix: remove the `session.tool_owner.send-mail` and `session.tool_owner.read-file` requirements from c34 and rename/drop `m5a_fixture_lock_session_pins_provider_active_and_tool_owners.rs` so it asserts only `session.provider_active` for this fixture. Keep `tool_owner` only for tests/fixtures with two plugins claiming the same tool.

### B2 — c18 defaults `/grant <tool>` through `session.tool_owner[tool]`, but single-owner tools have no such entry

Anchor: c18 (`SlashHandler`).

c18 says a `/grant` command's optional `plugin` field:

> defaults to the lock's `session.tool_owner[tool]` canonical

That is the same live-schema mistake as B1. For the normal m5a lock, `session.tool_owner` must be empty for `send-mail` and `read-file` because each tool has one claimant. If c34 is fixed by removing the redundant entries, `/grant send-mail to=alice@example.com` has no `session.tool_owner["send-mail"]` value to default from. If c34 is not fixed, the lock does not validate.

Live `broker_acl::compile` already computes the correct routing table for both single-owner tools and conflict-resolved tools: `BrokerAcl.tool_routes: BTreeMap<String, CanonicalId>`. That is the value slash handling should use for the default plugin pin.

Smallest fix: change c18 (and c38 construction text if needed) so `SlashHandler` receives the resolved tool route map (or a lookup wrapper over `BrokerAcl::tool_route`) and defaults `plugin` from that resolved route. Add an acceptance assertion for `/grant send-mail ...` in the four-plugin m5a lock with an empty `session.tool_owner` table.

## Major findings

### M1 — c38's inactive-provider text mixes provider canonical ids with provider topic ids

Anchor: c38 round-4 spawn-order bullet.

The new inactive-provider bullet says to spawn every other provider whose:

> `provider_id` [is] distinct from `session.provider_active`

and says the re-emit router stays subscribed only to:

> `provider.<session.provider_active>.**`

Live types do not line up that way. `session.provider_active` is a canonical id such as `builtin:openai@0.0.0`; `bindings.provider_id` is the topic segment such as `openai`; provider events are under `provider.<provider_id>.*`. The existing `ReemitRouter::new` correctly accepts the active provider canonical and internally looks up `PluginAcl.provider_id` before subscribing to `provider.openai.**`.

Smallest fix: in c38, say the inactive spawn loop excludes the active provider by comparing the map key/canonical id to `session.provider_active`, and say re-emit remains scoped to the active provider's `provider_id` topic (`provider.openai.**` in the fixture). The acceptance test already uses the correct topic wording.

## Nit findings

### N1 — Round-4 tail sections still label themselves as round 3

The implementation-critical row bodies are renumbered, but the tail still says:

- `## Cross-checks (round 3 self-audit)`
- `Round 3 sizing — each commit is in exactly one category`
- `Pi-2 expects one more round after round 3 before ratification.`

Smallest fix: update those labels to round 4 / current convergence wording.

## Round-3 verification table

| r3 finding | status | verification |
|---|---|---|
| B1 c34 `active_provider` field | closed for that field | c34 now says `session.provider_active`, matching `crates/rafaello-core/src/lock/session.rs`. New c34 live-schema blockers are B1/B2 above (`tool_owner`). |
| M1 c11 positive `confirm_resolved` placement | closed | c11 now defines the positive as a direct broker/internal-subscriber wire-shape test; c24 owns the gate-publisher positive. |
| M2 c38 inactive-provider spawn loop | mostly closed | c38 now explicitly adds an inactive-provider spawn loop and a regression test. The new wording has a provider-id/canonical-id mixup tracked as M1. |
| M3 c34 combined catalog build | closed | c34 now calls `ToolSchemaCatalog::build` against the combined lock and asserts exactly `send-mail` + `read-file`. |
| N1 stale c37 references | closed | The c14/c31/c34 stale row-body references are now c38. Remaining c37 references refer to the real TUI test-hook row or c39's dependency. |

## Convergence call

Not clean yet: **two blockers**. Both are rooted in the same m1 lock invariant: `session.tool_owner` is conflict-resolution state, not a required active-tool pin table. Fixing c34 to leave `tool_owner` empty for the four-plugin fixture, and c18 to default `/grant` via resolved broker routes, should unblock the fixture and demo path.
