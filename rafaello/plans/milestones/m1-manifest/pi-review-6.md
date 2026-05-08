# Pi review 6 — m1 manifest / lock / grant / compiler scope

Reviewed `rafaello/plans/milestones/m1-manifest/scope.md` round-6 draft in
`/home/luiz/lab-wt/m1-pi-scope-6` after the round-5 fixes.

Cross-checks: `overview.md` (§5 and §15.1), `decisions.md` rows
5/12/16/17/26–32/36, `milestones/README.md`, Stream A security RFC
(especially §6.9 and §7), Stream F manifest RFC, and the current `lockin`,
`lockin-config`, `outpost`, and `fittings` crate surfaces in this worktree.

## Overall verdict

**Not ready for owner ratification yet.**

Round 6 resolves the pi-5 blockers it claims to resolve: lock-side publish ACLs,
`tool_owner` target integrity, lock-side `allow_hosts` mode checks,
`lockin_config::*` naming, the full fittings workspace acceptance command, and
capability-path resolution for non-existent leaves are all materially improved.

The remaining problems are not another broad rewrite. They are mostly the last
places where the draft still says "lock is the runtime authority" but leaves a
manifest-only concept out of the lock/compiled surface, or where a lock-level API
is under-specified for the multi-plugin data it has to validate.

## Blocking findings

### 1. `validate::lock(lock, ctx)` has one `PathContext`, but lock validation is multi-plugin

V3 is a lock-level pass over every installed plugin. It now calls path-sensitive
logic per plugin:

- `trifecta::evaluate(lock, canonical, ctx)` needs to expand that plugin's grant
  paths, including `${plugin}`, `${cache}`, and `${state}`;
- `carveout::compile_against(..., ctx, ...)` likewise evaluates each plugin's
  grant against path roots;
- C3 defines `PathContext` as carrying `plugin_dir` for **this** plugin.

But V3's public API is still:

```rust
validate::lock(lock: &Lock, ctx: &PathContext) -> Result<()>
```

A single `plugin_dir` / cache / state root cannot be correct for a lock with two
plugins installed in different directories. An implementation following the
scope either applies plugin A's directory to plugin B's `${plugin}` grants, or
silently avoids validating `${plugin}`-anchored lock paths in V3. Both violate the
"APIs name every context dependency" rule that m1 explicitly adopted.

**Required fix:** replace the V3 context with a lock-level context, e.g.
`LockValidationContext { project_root, home, plugin_dirs: BTreeMap<CanonicalId,
PathBuf>, cache_dirs/state_dirs ... }`, or make V3 take an explicit per-plugin
context map/callback. Add a test with two plugins whose `${plugin}` paths would
resolve differently, so using one context for both fails.

### 2. `[load]` is parsed and validated, but never snapshotted into the lock or emitted for m2

M6 parses the manifest `[load]` block and V1 validates `event` / `command` /
`kind` cross-references. The test matrix has load-trigger tests. But the lock
schema has no `.load` / `bindings.load` / `lifecycle` field, and neither
`CompiledPlugin` nor `BrokerAcl` carries load policy.

That contradicts the overview's one-way artifact model:

- `overview.md` §5.1 says the manifest declares lazy-load triggers;
- §5.4 says core reads the **lock**, registers stubs, spawns `eager` plugins,
  spawns `boot` plugins, and lazy-spawns on `event` / `command` / `kind`;
- the same section says the spawn path reads the lock snapshot, not the live
  manifest.

As scoped, m1 can validate a manifest's load triggers at install time and then
throw them away. m2/m3 would have to consult the live manifest to know what to
spawn, or introduce a lock-schema change immediately after m1.

**Required fix:** add a lock-side snapshot of `load` and an m2-facing compiled
surface for it (either on `CompiledPlugin`, `PluginAcl`, or a separate lifecycle
plan). Re-validate lock-side load triggers against the lock snapshot fields
(`bindings.tools`, `bindings.renderer_kinds`, `.grant.subscribes`) so hand-edited
locks cannot smuggle invalid lifecycle state.

### 3. Several manifest-side grant-shape rules still lack lock-side counterparts

Pi-5 fixed several important lock-side mirrors, but V3 still does not explicitly
mirror all manifest-side rules for data the compiler consumes from the lock:

- **Grant bundle keys.** Manifest V1 rejects capability bundle keys that are not
  `default` or a declared tool name. Lock L3/V3 does not say the same for
  `.grant.bundles`. Because C2 unions **every** named grant bundle into the
  spawn-time policy, a hand-edited `[grant.bundles.typo]` is live authority, not
  inert metadata.
- **Capability path templates.** M11 defines `CapabilityPathTemplate` for
  `read_paths` / `read_dirs` / `write_*` / `exec_*` and rejects relative paths
  without a placeholder prefix. L3 does not say lock grant paths are parsed
  through the same type. C3 covers expansion and containment, but not the
  lock-load rule that rejects a bare `read_dirs = ["relative"]`.
- **Binding tool and sink grammars.** V3 checks `tool_meta` membership in
  `bindings.tools`, but does not explicitly validate the `bindings.tools` values
  themselves or `session.tool_owner` keys against the tool-name grammar. It also
  omits a lock-side mirror of the sink-class grammar for
  `bindings.tool_meta.<n>.sinks`.

These are exactly the same class of issue pi-5 found for publish ACLs and
`allow_hosts`: the manifest validator is strong, but the runtime authority is the
lock.

**Required fix:** add V3 lock-side validation bullets and negative tests for:

- unknown lock grant bundle key;
- relative/no-placeholder lock capability path;
- illegal lock `bindings.tools` value / `tool_owner` key;
- malformed lock `tool_meta.<n>.sinks` value.

### 4. Security RFC §6.9's project-workspace `exec_paths` refusal is missing

The security RFC's attack scenario §6.9 says:

> The grant compiler additionally refuses `exec_paths` entries that point into
> the project workspace, because user-controllable binaries-as-execs are a
> category of footgun we don't want even the user accidentally enabling.

The round-6 scope lists `exec_paths` / `exec_dirs` as capability fields and even
adds an `exec` sink class, but no M/V/C/K rule refuses executable grants under
`${project}`. There is no negative test for `exec_paths = ["${project}/scripts/tool"]`
or `exec_dirs = ["${project}/bin"]`.

Because the scope's Inputs explicitly include Stream A §6 attack scenarios that
map onto compiler refusals, this is not optional drift unless the owner reverses
that security-RFC line.

**Required fix:** either add a compiler/validation refusal for `exec_paths` and
`exec_dirs` resolving inside `${project}` (with tests for placeholder and
symlink-resolved cases), or add an explicit decision/overview delta deferring or
reversing the §6.9 refusal.

## High-priority findings

### 5. Tool metadata table presence contradicts `overview.md` §15.1 defaults

Scope V1 says every `provides.tools` entry must have a matching
`[provides.tool.<name>]` table and rejects missing tables. But `overview.md`
§15.1 normative delta says missing tables default to:

```toml
{ sinks = [], grant_match = absent, always_confirm = false }
```

The overview wording is itself awkward ("must have" and "missing tables default"
in the same bullet), but it is the ratified source of truth that m1 names as an
input. The scope should not silently pick the stricter interpretation without
calling it out as a normative change.

**Required fix:** either accept omitted `[provides.tool.<name>]` tables and apply
the overview defaults, or explicitly update the overview/decision log to make the
stricter table-presence rule ratified.

### 6. `content_digest` symlinked-directory semantics are still ambiguous enough to cause non-portable hashes

D1 says directory symlinks are followed and cycle detection uses a visited set
keyed by canonical absolute path. It does not say how to handle two distinct
in-package paths that point at the same canonical directory, e.g. `src/` and
`vendor_src -> src/`.

Two plausible implementations differ:

- hash both logical relative paths (`src/lib.rs` and `vendor_src/lib.rs`), because
  the digest is over package contents by relative path;
- treat the second canonical target as already visited and skip/error, because a
  global visited set sees the same canonical directory twice.

The second interpretation is not a cycle, and silently skipping it would violate
D1's deterministic relative-path hashing model. The first interpretation needs
cycle detection to be a recursion-stack check, not a global "seen target" skip.

**Required fix:** pin the rule: duplicate symlinked directories either contribute
under every logical relative path, or are rejected with a specific error. Add a
fixture test. If they contribute, specify recursion-stack cycle detection rather
than a global visited-set skip.

## Medium-priority cleanup

### 7. Public compile/ACL APIs rely on callers to have run V3, but the failure boundary is not stated

Several sections say V3 rejects invalid locks "before this code runs":
`BrokerAcl::compile` relies on resolved tool conflicts, and `compile_plugin`
relies on digest / validation invariants. Other sections say the compiler itself
"refuses" bad locks.

Before m2 consumes this API, the scope should choose the contract: either
`compile_plugin` / `broker_acl::compile` call the necessary lock validation (or a
cheap validation subset) internally, or their docs must say they require a
previous successful `validate::lock(...)` and return undefined/typed errors if
called without it. The current text leaves room for m2 to accidentally skip V3
and still receive a structured-looking ACL.

## Summary

Round 6 is close, but not ratifiable yet. The required fixes are targeted:
make lock validation context per-plugin, persist/emit load policy from the lock,
finish the remaining lock-side mirrors for compiler-consumed grant data, and
resolve the missing `exec_paths` refusal from the security RFC. After those, the
remaining items are precision cleanup rather than architecture churn.
