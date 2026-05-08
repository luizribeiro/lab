# Pi review 3 — m1 manifest / lock / grant / compiler scope

Reviewed `rafaello/plans/milestones/m1-manifest/scope.md` round-3 draft in
`/home/luiz/lab-wt/m1-pi-scope-3`.

Cross-checks: `overview.md`, `decisions.md`, `milestones/README.md`, the
current `lockin` / `lockin-config` / `outpost` crate surfaces in this
worktree, and the round-2 findings.

## Overall verdict

**Not ready for owner ratification yet.**

Round 3 resolves most of the big round-2 issues: the structured-plan boundary
is now clear, reserved env vars are rejected consistently, `tool_owner` has a
compiled routing output, helper/frontend/renderer deferrals are reflected, and
the path-safety story is much better than round 2.

However, there are still ratification blockers. The worst ones are concrete
compile-time dependency paths that will not resolve in this workspace, a
`NetworkPlan` type mismatch with `lockin-config`, the lock-side `entry` snapshot
still being escape-unsafe, and sink inference still being underspecified for
the security-critical default-sink path. There are also a few stale-test / stale
-risk paragraphs that are exactly the kind of drift `commits.md` drivers will
copy into implementation.

## Blocking findings

### 1. Workspace dependency paths and `NetworkPlan` ownership are wrong

S2 lists these workspace deps:

- `lockin = { path = "../../../lockin/crates/sandbox" }`
- `lockin-config = { path = "../../../lockin/crates/config" }`
- `outpost = { path = "../../../outpost" }`

From `rafaello/Cargo.toml`, those paths resolve outside this worktree
(`/home/luiz/lockin`, `/home/luiz/outpost`) and do not exist. The paths that
match the current repo layout are:

- `../lockin/crates/sandbox`
- `../lockin/crates/config`
- `../outpost/crates/outpost`

The `outpost` path in S2 also points at the outpost workspace root rather than
the library package (`outpost/crates/outpost`).

There is a second mismatch in the same area: S2 says `lockin-config` is used for
the `NetworkPlan` re-export, but the actual `lockin_config::NetworkPlan` is:

```rust
Deny | AllowAll | Proxy { policy: outpost::NetworkPolicy }
```

while m1's C1 output requires:

```rust
Deny | AllowAll | Proxy { allow_hosts: Vec<String> }
```

Those are not the same type or the same ownership boundary. If m1's output must
preserve `allow_hosts` and let m2 start the proxy, then `rafaello-core` needs its
own `NetworkPlan` type and should use `outpost::NetworkPolicy::from_allowed_hosts`
only as dry-run validation.

Risks §2 is also stale: it still says the outpost dep is transitive via
`lockin-config` and that m1 does not import it directly, contradicting S2.

**Required fix:** correct the path deps, point `outpost` at the actual library
crate, and make the network-plan type ownership explicit. Either remove the
`lockin-config NetworkPlan` wording or change C1 to use the real
`lockin_config::NetworkPlan` shape. Risks §2 must match the chosen model.

### 2. Lock-side `.entry` remains escape-unsafe

Round 2 required lock/compiler validation that the lock-side `.entry` snapshot
is still relative and inside the installed plugin directory before emitting
`entry_absolute`. Round 3 added strong package validation for the manifest's
`entry`, but the compiler is explicitly supposed to read the lock snapshot, not
the live manifest.

Current gaps:

- L2 defines `.entry: PathBuf` but does not give it the same `SafePath` rule as
  manifest `entry`.
- C1 emits `entry_absolute` but C2/C3 do not state that lock `.entry` is checked
  for absolute paths, `..`, symlink escape, directory-vs-file, or existence.
- The negative matrix has `lock_missing_entry.rs`, but no
  `lock_entry_traversal.rs`, `lock_entry_absolute.rs`, or
  `compile_lock_entry_escape.rs`.

A hand-edited lock can therefore potentially set `entry = "../../evil"` or
`entry = "/bin/sh"` unless the implementation invents a rule not in the scope.
That is exactly the rug-pull class m1 is meant to close.

**Required fix:** make lock `.entry` a safe relative package path (same rule as
M11), validate it during lock load or compile, and add negative tests for
relative traversal, absolute entry, symlink escape, missing file, and directory
entry from the lock snapshot path.

### 3. Sink-default inference is still not implementable enough for a security gate

Round 3 adds `sinks: Option<Vec<_>>` and `sinks_inferred`, which fixes the
round-2 state-carrier problem, but Si still has two contradictions/holes.

First, Si2 says validation calls `sinks::infer_defaults` for entries whose
`sinks` field is empty **and** whose provenance was “no `sinks` declared”; then
it says locks with `sinks_inferred = true` and an inferred set differing from the
snapshot are rejected. Those cannot both be true. If the inferred snapshot is
`["network"]` or `["workspace_write"]`, `sinks` is not empty, so the drift check
would be skipped.

Second, Si1 takes a single `GrantBundle`, but the scope does not define which
bundle is used for a tool. For a tool-scoped bundle, the effective capabilities
are `default ∪ <tool-name>` per decision row 17. Sink defaults must be inferred
from the same effective capability set that can run for that tool; otherwise a
write-only named tool bundle can fail to infer `workspace_write`, and a
network-bearing tool bundle can fail to infer `network`.

**Required fix:** define sink inference over the effective tool grant
(`default ∪ tool-named bundle`, or another explicit choice) and run drift
validation for every `tool_meta` entry with `sinks_inferred = true`, regardless
of whether the snapshotted `sinks` vec is empty. Add a test where the default
bundle has no sinks but a tool-named bundle adds network/write authority and the
inferred sink list changes accordingly.

### 4. `openrpc.json` sibling requirement conflicts with decision row 31

Decision row 31 says the v1 manifest drops `[rpc]` and requires an
`openrpc.json` sibling at the manifest parent directory. Scope M10 narrows that
to “if `provides.tools` is non-empty”. That may be a reasonable product choice,
but it is not what the ratified decision currently says, and it leaves provider
and renderer-shaped manifests without the sibling the decision says v1 validates.

**Required fix:** choose one boundary before ratification:

1. require `openrpc.json` for every plugin package, matching row 31; or
2. explicitly narrow row 31 for m1/v1 (append a decision or call out the drift)
   and state why non-tool provider/renderer packages do not need the sibling.

## High-priority findings

### 5. Private-state path wording regressed in the test matrix

C5 correctly switches the private-state directory to
`${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/`, avoiding raw canonical ids
as path segments. But the positive test row `compile_private_state_grant.rs`
still expects `<canonical-id>`.

This is not harmless: tests are the implementation contract for this milestone,
and the stale expectation would reintroduce the path-safety regression round 2
was trying to close.

**Required fix:** change the test row to `<topic-id>`. Also consider noting the
overview/decision-row wording drift (`<plugin-id>` vs hashed topic-id) so the
m1 retrospective patches the canonical architecture docs.

### 6. Topic-id collision helper visibility still contradicts itself

T3 now correctly says `topic_id::collisions_with_prefixes(...)` is an
intentional public API. Risks §4 still says the helper is `pub(crate)` and
reachable from integration tests via a public re-export.

Integration tests cannot call `pub(crate)` items, and a public re-export is a
public API. This was round-2 finding 8 and should not survive into
`commits.md`.

**Required fix:** rewrite Risks §4 to match T3: the helper is a public,
documented testable API, and production `collisions(...)` delegates to it.

### 7. Directory symlink handling in `content_digest` can loop

D1 says symlinks are followed when their target resolves inside the package,
and directory-typed symlinks recurse normally. It does not define cycle
handling. A package containing `loop -> .` or `a/b -> ../a` is inside the package
and can recurse forever unless the implementation invents a visited-set rule.

**Required fix:** specify either “directory symlinks are refused” or “directory
symlinks are followed with canonical-target cycle detection and a typed
`DigestError::SymlinkCycle` / equivalent”. Add a negative digest test for an
inside-package symlink cycle.

### 8. Manifest `name` grammar is not pinned even though canonical ids require it

L8 requires canonical-id `name` to match `[a-z0-9_][a-z0-9_-]*`, but M1 only
says the manifest has a `name`; it does not validate the same grammar. Since the
canonical id is derived from source + manifest name + version at install time,
this should be caught at manifest validation time, not later as a lock parsing
surprise.

**Required fix:** add a manifest-name grammar rule and a negative test such as
`manifest_invalid_name.rs` for uppercase, slash, dot, and empty-name cases.

## Medium-priority findings

### 9. `load.event` cross-validation should say literal vs pattern match

V1 says a `load.event = ["x.y"]` trigger referencing a topic not in
`bus.subscribes` is rejected. It does not say whether a subscribed pattern such
as `core.session.**` covers `load.event = ["core.session.started"]`, or whether
the event must be listed literally in `bus.subscribes`.

**Required fix:** state the rule. I recommend “the load event topic must be
matched by at least one subscribe pattern,” because that follows the broker
semantics and avoids pointless duplicate literal subscriptions.

## Summary

Round 3 is close, but the remaining issues are not just editorial. Fix the
workspace dependency paths/API ownership, lock-entry safety, sink-default
inference semantics, and row-31 `openrpc.json` boundary before moving to
`commits.md`. Then do one stale-text sweep over Risks and the test matrix so the
implementation agents do not copy old contradictions into code.
