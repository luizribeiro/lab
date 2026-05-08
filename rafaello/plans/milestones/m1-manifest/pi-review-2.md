# Pi review 2 — m1 manifest / lock / grant / compiler scope

Reviewed `rafaello/plans/milestones/m1-manifest/scope.md` at
`cc70526` (`docs(rafaello-m1): scope.md round-2, address pi review-1`).

Primary scope: the m1 scope draft itself, cross-checked against
`overview.md`, `decisions.md`, `milestones/README.md`, and the current
lockin/outpost crate surfaces where the draft names concrete APIs.

## Overall verdict

**Not ready for owner ratification yet.**

The round-2 draft is much more concrete than the first draft: the
structured-plan decision is right, the lock schema is no longer
hand-wavy, and the test matrix is adversarial enough to be useful.
However, the draft still has several implementability and security
blockers. The worst issues are path-escape ambiguity in identity/path
fields, sink-default state that is specified in one section but absent
from the lock schema, an ambiguous `MethodNotFound` scope decision, and
conflict-resolution state (`tool_owner`) that validation checks but the
compiled routing outputs do not consume.

## Blocking findings

### 1. Path-bearing identity and manifest fields are not escape-safe

Several m1 fields are later used as filesystem paths, but the scope only
specifies weak syntactic parsing:

- `CanonicalId::source` allows `[a-z0-9._/-]+` (L8). That admits
  `..`, `../x`, leading `/`, double slashes, and other path-shaped
  values.
- The canonical id is used raw in path contexts: C5 adds
  `${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/`, and C3 says m2's
  install layout uses `${plugin_root}/<source>:<name>@<version>/`.
  If `<source>` may contain arbitrary slash/dot segments, a hand-edited
  lock can escape or alias unexpected directories unless every consumer
  re-sanitises it.
- `Manifest.entry` is described as “path inside the package” (M1), and
  `grant_match` is “relative path” (M3), but no in-scope validator/test
  rejects absolute paths, `..`, symlink escapes, directories, or missing
  files for either field. M10 only checks `openrpc.json` sibling
  presence.

This is not just polish: m1 emits `entry_absolute`, and m2 will spawn it.
The lock snapshot is explicitly trusted over the live manifest, so m1
must make the snapshot shape safe and validate corrupted/hand-edited
locks.

Required fix:

- Either encode canonical ids before using them in paths (e.g. topic-id
  or a path-id) or restrict `CanonicalId::source` to no leading slash,
  no `..` segments, no empty segments, and no path traversal after
  normalisation.
- Add `validate_with_package` checks for `entry` and `grant_match`:
  relative, normal, inside package after canonicalisation, expected file
  type, and existing where the scope says install-time existence is
  required.
- Add lock/compiler validation that the lock-side `.entry` snapshot is
  still relative/inside the installed plugin dir before emitting
  `entry_absolute`.
- Add negative tests for `entry = "../evil"`, absolute `entry`, escaping
  `grant_match`, and dangerous canonical-id sources.

### 2. Sink-default inference requires state the lock schema does not define

The Si section adds sink-default inference and drift detection, but the
M/L/C schemas do not carry the information Si needs.

Conflicts:

- M3 defines `[provides.tool.<name>] sinks: Vec<String>`, not
  `Option<Vec<String>>`, while Si1 takes `declared: &Option<Vec<String>>`
  and needs to distinguish “omitted” from “declared empty”.
- L4 defines `ToolMeta = { sinks: Vec<String>, grant_match:
  Option<PathBuf>, always_confirm: bool }`.
- Si2 later says the lock carries
  `bindings.tool_meta.<n>.sinks_inferred: bool`, but L4 never includes
  that field, and C1's `tool_meta` output shape does not mention it.

As written, implementers cannot know whether `sinks = []` means “author
explicitly declared no sinks” or “installer inferred no sinks from the
then-current grant”. Therefore `SinkInferenceDrift` cannot be implemented
soundly.

Required fix: update M3/L4/C1/test fixtures to one consistent model,
e.g. manifest `sinks: Option<Vec<String>>`, lock `ToolMeta { sinks,
sinks_inferred, ... }`, compiled `ToolMeta` carrying enough provenance or
a clear decision to drop provenance after validation.

### 3. `FittingsError::MethodNotFound` is simultaneously out-of-scope and default-in-scope

The draft points in both directions:

- Inputs include decision row 36's “m1 follow-through”.
- Out of scope says **`FittingsError::MethodNotFound { method:
  Option<String> }` cutover** is out of scope.
- The same out-of-scope bullet then says the default is to include it as
  a trailing m1 commit unless size analysis shows it is non-trivial.
- Risks §7 and internal split group 14 repeat the “driver call” framing.

This leaves `commits.md` without a ratified boundary. A breaking public
API cutover cannot be both excluded from the milestone and expected by
default.

Required fix: choose one:

1. Make it in-scope for m1, add a small W section and a named test, and
   remove it from “Out of scope”; or
2. Defer it to m2/fittings-follow-up explicitly and remove it from m1
   inputs/internal split.

### 4. `session.tool_owner` is validated but not reflected in compiled routing outputs

L1/L3/V3 correctly add `[session].tool_owner` to resolve conflicting tool
names. The negative test even says that with `tool_owner.grep =
"<plugin-A>"`, “only the named plugin gets the routing”.

But the outputs m2 will consume do not encode that:

- C1 emits `tool_meta: BTreeMap<String, ToolMeta>` from the plugin's
  lock entry, with no mention of filtering non-owned conflicting tools.
- G1 emits ACL fields but no tool-routing table and no filtering rule for
  tools owned by a different plugin.
- V3 only validates the conflict is resolved; it does not produce a
  resolved routing table.

If m2 naively registers every compiled plugin's `bindings.tools`, the
losing plugin in a resolved conflict can still appear routable.

Required fix: specify where routing resolution lives. Either
`compile_plugin` filters `tool_meta`/tools for non-owned conflicts, or
`broker_acl::compile` emits a resolved `tool_routes: BTreeMap<ToolName,
CanonicalId>`, or a separate m1 output does. Add a test where two plugins
claim `grep`, `tool_owner` names plugin A, and the compiled/routing output
contains only A for `grep`.

## High-priority findings

### 5. Reserved env var handling contradicts itself

C7.1 says `compile_plugin` **strips** `RFL_BUS_FD` and `RFL_PLUGIN` from
`env.set`, but **refuses** them in `env.pass`. The tests say something
else:

- `compile_env_set_passes_through.rs` says reserved keys in `env.set` are
  “stripped by C7.1 with a typed error”.
- `compile_reserved_env_in_set.rs` expects `env.set = { RFL_PLUGIN = ... }`
  to return `CompileError::ReservedEnvVarRequested`.

“Strip and continue” vs “reject with error” is a security/UX decision and
must be one rule. I recommend rejecting both `env.pass` and `env.set` for
reserved names; silent stripping can hide a malicious or broken manifest.

### 6. Network allow-host validation names an API m1 cannot import under the listed deps

S2 says m1 depends on `lockin-config` to reuse network-plan parsing and
avoid reimplementing `outpost::NetworkPolicy`. Risks §2 then says m1
calls `outpost::NetworkPolicy::from_allowed_hosts(...)` directly while
“m1 does not import it directly”.

Rust crates cannot use transitive dependencies directly unless they are
re-exported, and `lockin-config` does not re-export `outpost`. The current
lockin API offers `lockin_config::resolve_network_plan(config)`, but that
requires constructing lockin's config shape; it is not the direct
`allow_hosts` validator described here.

Required fix: either add an explicit `outpost` workspace dependency and
say m1 uses it directly, or specify a small wrapper in `lockin-config` / a
constructed `lockin_config::Config` path that m1 actually calls. Add an
`InvalidAllowHosts` test that proves malformed host patterns are rejected.

### 7. Carve-out diagnostics are asserted but absent from `CompiledPlugin`

The negative test `carveout_lockfile_path_explicit.rs` asserts that an
explicit project-class carve-out leaf is dropped and recorded in
`dropped_carveouts: Vec<PathBuf>`. C1's `CompiledPlugin` has no such
field, and K2 does not include it in `DecomposedGrant` either.

Required fix: add an explicit diagnostics field to the relevant output
(`CompiledPlugin`, `FilesystemPlan`, or `DecomposedGrant`) or change the
test/behaviour to a refusal. Silent drop with no typed diagnostic is too
surprising for a grant compiler.

### 8. Topic-id collision helper visibility is still muddled

T3 says the collision helper has no test seam and `derive(...)` remains
the only hash entry point. Risks §4 then says the helper is `pub(crate)`
while also “reachable from integration tests via a small `pub fn
collisions_with_prefixes` re-export under the `topic_id` module”, and that
this is the “stable public surface”.

Integration tests cannot call `pub(crate)` items. If the helper is public,
it is part of the API surface, not an internal helper. This is easy to fix:
declare `collisions_with_prefixes(...)` as an intentional public testable
API, or move the forced-collision test into an in-crate unit test and keep
the helper private.

### 9. `grant_match` existence is promised but not tested

M3 says `grant_match` path syntax and file existence are validated at
install time, but the scope has no API/test that does this. M10 only names
`openrpc.json`. If m1 is the paper-first install-time validation layer,
this should land here, not be rediscovered in m2.

Required fix: extend `manifest::validate_with_package(...)` to validate
`grant_match` paths and add positive/missing/escape tests.

## Medium-priority findings

### 10. Renderer manifest acceptance is misleading with subprocess renderers deferred

M7 parses and accepts non-built-in plugin renderer registrations, while
Out of scope says subprocess-renderer dispatch is deferred and m1 emits no
wiring. That may be okay for forward compatibility, but the scope should
state what m2/m3 do with accepted renderer registrations in v1. If they
are inert until v2, say so explicitly and ensure no runtime treats them as
available.

### 11. `content_digest` wording is internally inconsistent on directories

D1 says the algorithm walks “files only”, then says empty directories
contribute their relative path. Choose one precise algorithm. Including
empty directories is fine, but then the digest is over filesystem entries,
not files only. Also specify symlink-to-directory behaviour.

### 12. The milestone is probably too large to keep as one scope

The draft estimates **35–45 sequential commits** and includes parser,
lock schema, digesting, validation, carve-outs, trifecta, compiler,
broker ACL, docs/manual validation, and possibly a fittings API cutover.
That is larger than m0's already-large 32-commit milestone and will make
`commits.md` review hard.

I would strongly prefer splitting now at the natural boundary the draft
already identifies:

- m1a: crate skeleton, manifest/lock/canonical id/topic-id/digests/sink
  inference;
- m1b: validation, carve-outs, trifecta, compiler plans, broker ACL.

If the owner wants one milestone anyway, `commits.md` should include an
explicit checkpoint after group 7 with a go/no-go to split before group 8.

### 13. The referenced round-1 review file is absent

The status banner and change log cite `pi-review-1.md`, but the m1
directory currently contains only `scope.md` and this review. For
traceability, either add the round-1 review file or change the references
to say the round-1 findings were chat-only.

## Summary

The shape is close, but ratifying this draft would bake in ambiguous
contracts in exactly the parts m1 is supposed to make boring and
mechanical. Fix the path-safety rules, sink-inference state, method-field
scope boundary, tool-owner routing output, and reserved-env/network-plan
contradictions before moving to `commits.md`.
