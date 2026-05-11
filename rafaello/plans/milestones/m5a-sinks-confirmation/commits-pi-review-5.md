# m5a commits.md round-5 pi review

> Verdict: zero blockers.
> Counts: B/0 M/1 N/1

commits.md is ready for owner ratification.

Reviewed the round-5 `commits.md` at `aa6682d`, the round-4 review, the `258e4de..HEAD` diff, ratified `scope.md`, and live source under `rafaello/crates/`. Round 5 closes the two round-4 blockers: c34 no longer writes redundant `session.tool_owner` entries, and c18 now resolves `/grant`'s default plugin through the resolved tool-route table rather than through `session.tool_owner`. The c38 provider canonical-vs-topic wording is also corrected.

## Major findings

### M1 — c18 introduces `grant_failed`, an audit kind not listed in the audit inventory

Anchor: c18 (`SlashHandler` unknown-tool path).

Round 5 adds an unknown-tool `/grant` path that publishes `command_result {ok: false, kind: "grant", message: "no plugin provides tool '<tool>'"}` and audits:

> `grant_failed`

But ratified scope §AL1's slash-command audit-kind list is only:

> `grant_added`, `grant_revoked`, `grant_list`, `slash_unknown`

and c08 says `AuditKind` contains every variant scope §AL1 lists. Scope §SL3 likewise says slash handling audit-logs only `grant_added` / `grant_revoked` / `grant_list` / `slash_unknown`.

This is not a blocker because c18 can mechanically extend `AuditKind` when it adds the new path, but it is a schema-inventory drift: an audit reader using the §AL1 kind list will not expect `grant_failed`.

Smallest fix: either (a) use an existing kind, likely `slash_unknown`, for the unknown-tool grant failure and keep the detailed error in the payload, or (b) explicitly add `grant_failed` to c08's `AuditKind` inventory, c18's audit list, and the cross-check as an intentional commits-level audit-kind extension.

## Nit findings

### N1 — Sizing summary still says round 3

The round-5 tail fixed the cross-check heading and convergence note, but the sizing section still says:

> Round 3 sizing — each commit is in exactly one category

and later:

> round-3 lands at 41

Smallest fix: change those to round 5 / current draft wording.

## Round-4 verification table

| r4 finding | status | verification |
|---|---|---|
| B1 c34 redundant `session.tool_owner` entries | closed | c34 now requires no `session.tool_owner` entries and asserts `session.tool_owner.is_empty()`. This matches live `validate::lock`, which rejects redundant owner entries when only one plugin claims a tool. |
| B2 c18 defaulted `/grant` via `session.tool_owner[tool]` | closed | c18 now gives `SlashHandler` an `Arc<BrokerAcl>` and defaults through `BrokerAcl::tool_route(tool)` over the live `tool_routes` map. `broker_acl::compile` already populates that map for both single-owner and conflict-resolved tools. |
| M1 c38 provider canonical-vs-topic mixup | closed | c38 now excludes inactive providers by canonical-id comparison with `session.provider_active`, and describes re-emit's provider topic scope as `provider.<active.provider_id>.**`, matching live `ReemitRouter::new` lookup through `PluginAcl.provider_id`. |
| N1 round labels in tail | mostly closed | Cross-check and convergence note are updated to round 5. Sizing summary still has round-3 wording (N1). |

## Re-attack notes

- The `BrokerAcl::tool_route` direction is live-source-compatible. The current live API has `Broker::tool_route` and a public `BrokerAcl.tool_routes` map; adding a small `BrokerAcl` lookup wrapper in c18 is a trivial local extension.
- The c34 fixture no-conflict invariant now matches live m1 validation and `broker_acl::compile` routing.
- The c38 inactive-provider spawn text now distinguishes canonical ids from provider topic ids, and the acceptance test uses the correct `provider.mock.*` vs `provider.openai.*` topics.
