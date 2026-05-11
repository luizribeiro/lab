# m5a scope.md round-6 pi review

> Verdict: ratification-ready
>
> Counts: B/0 M/0 N/1

Round 6 mechanically folds the three round-5 majors and the five round-5 nits. I found no blockers and no majors. The only remaining item is a trivial wording inconsistency in the exact unused-`allow_secrets` warning string; it is not implementation-blocking because §Tr1/§Tr4 now contain the operative install algorithm and test.

## Findings

## Blockers

None.

## Major

None.

## Nits

### N-1. OP6 and Tr1 use two different unused-`allow_secrets` warning strings

**Anchor:** §Tr1 step 10, §Tr4 test list, §OP6 validation rules.

Round 6 correctly adds the missing installer-side warning step to §Tr1 and lists `rfl_install_warns_on_unused_allow_secrets_entry.rs` in §Tr4. The operative §Tr1/§Tr4 string is:

```text
warning: unused allow_secrets entry '<name>' (no matching env.pass entry)
```

§OP6 still says the installer prints:

```text
unused allow_secrets entry: <name>
```

Smallest polish: make §OP6 refer back to §Tr1's exact stderr line (or explicitly say the exact string is owned by §Tr1). This is a nit because the actual install algorithm and named test are now present and precise.

## Round-5 verification table

| Round-5 finding | Round-6 disposition | Verification |
|---|---|---|
| M-1 `mark_session_grant_requested` error/race handling unspecified | Resolved | §CT5 step 5 now handles `Err(MarkError::NotActive)` by re-reading `prior_outcome`, auditing `confirm_late` / `confirm_duplicate` / `confirm_unknown`, and not publishing `core.session.confirm_reply`. New race test is named. |
| M-2 unused-`allow_secrets` warning not integrated into install algorithm | Resolved | §Tr1 step 10 computes unused names after validation and before lock write; §Tr1 step 12 records them in audit; §Tr4 lists the stderr/audit test. Only nit is OP6's older alternate wording for the stderr string. |
| M-3 `compile.rs::effective_grant` described as default+named | Resolved | §OP6 now says live `compile.rs::effective_grant` iterates over every `grant.bundles` value and m5a unions `allow_secrets` across the same all-bundles set. Live source confirms `for bundle in grant.bundles.values()`; `sinks::effective_grant` remains default ∪ tool and env-free. |
| N-1 CT5 introduction says special arm adds a `UserGrant` | Resolved | §CT5 intro now says re-emit marks `session_grant_requested`; CG4 remains the grant owner. |
| N-2 Re-emit test name says creates grant | Resolved | Test renamed to `reemit_confirm_answer_always_allow_session_marks_state_and_emits_allow.rs`; the gate-side `creates_grant` test name remains appropriate. |
| N-3 CT0 implication 6 says `resolve` | Resolved | CT0 implication 6 now says `try_resolve`. |
| N-4 OP6 manifest snippet omits live `Serialize` derive | Resolved | §OP6 snippet now includes `Deserialize, Serialize`; live `manifest/capabilities.rs` derives both. |
| N-5 OP5 lock-snippet comment says snapshotted into bindings | Resolved | §OP5 now says the list is snapshotted into the lock grant env at `grant.bundles.<bundle>.env.allow_secrets`. |

## Live-source spot checks

- `rafaello/crates/rafaello-core/src/compile.rs`: `effective_grant` iterates `for bundle in grant.bundles.values()`, unions/dedups `env.pass`, and merges `env.set` across all bundles.
- `rafaello/crates/rafaello-core/src/sinks.rs`: sink inference's `effective_grant` is the separate default ∪ tool path and `union_bundle` does not touch env.
- `rafaello/crates/rafaello-core/src/manifest/capabilities.rs`: `EnvCapabilities` currently derives both `Deserialize` and `Serialize`, matching the round-6 snippet expectation.
- `rafaello/crates/rafaello-core/src/lock/grant.rs`: `GrantEnv` currently has `pass` and `set`; the planned additive `allow_secrets` field belongs there.
- `rafaello/crates/rafaello-core/src/scrubber.rs`: `reject_reserved` checks only `env.pass` and `env.set`, so §OP6's explicit reserved-name validation for `allow_secrets` remains necessary.

## Convergence call

Blocking count: **0**. Major count: **0**. Nit count: **1**.

I found **zero blockers and zero majors**. The sole nit is an exact-string wording mismatch that can be handled during commits.md drafting or a tiny owner-side editorial pass. **scope.md is ready for owner ratification.**

Owner-judgment items still worth surfacing:

1. m5a/m5b split: m5 is not closed until m5b ships the verbatim-exfil negative.
2. `grant_match` JSON-Schema-as-template-shape-contract: `/grant`-time validation, runtime structural-subset matching.
3. `env.allow_secrets`: additive manifest/lock schema and scrubber-signature change, with installer-side warnings for unused deployment-flexibility names.
