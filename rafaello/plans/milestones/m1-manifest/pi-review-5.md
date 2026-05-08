# Pi review 5 — m1 manifest / lock / grant / compiler scope

Reviewed `rafaello/plans/milestones/m1-manifest/scope.md` round-5 draft in
`/home/luiz/lab-wt/m1-pi-scope-5` after the round-4 fixes.

Cross-checks: `overview.md`, `decisions.md` rows 5/12/16/17/26–32/36,
`milestones/README.md`, Stream A security RFC, Stream F manifest RFC, Stream E
renderer RFC, and the current `lockin`, `lockin-config`, `outpost`, and
`fittings` crate surfaces in this worktree.

## Overall verdict

**Not ready for owner ratification yet.**

Round 5 correctly resolves the three round-4 blockers: `.active_bundles` is gone
and row-17 union flattening is restored; path templates are split from package
relative paths; and the W fittings cutover is now named as a second deliverable.
It also handles the round-4 high-priority cleanup (`always_confirm`, renderer
prefixes, `allow_hosts` outside proxy, cwd-explicit commands).

The remaining blockers are mostly lock-vs-manifest hardening gaps. The scope is
excellent at validating the manifest, but m1's runtime/compiler boundary reads
the lock snapshot, not the live manifest. Several manifest-only security checks
must be repeated against hand-edited locks before m2 can safely consume the
`CompiledPlugin` / `BrokerAcl` values.

## Blocking findings

### 1. Lock-side publish authority is not revalidated before emitting `BrokerAcl`

The scope has strong manifest validation for publish namespaces:

- V2 rejects foreign `plugin.<topic-id>.*` publishes.
- V2 rejects provider namespace mismatches.
- V1 rejects `core.*` and `frontend.*` publishes.
- The negative matrix has manifest tests for all of those cases.

But the compiler and broker ACL consume the **lock**, and the lock can be
hand-edited. On the lock path, G2 only says publish topics and subscribe patterns
are rechecked against the grammar. V3 does not re-run namespace authority checks.
That means an implementation following the scope could emit a `BrokerAcl` whose
`publish_topics` include, for example:

- `core.session.tool_result`;
- `frontend.tui.confirm_answer`;
- `plugin.<other-topic-id>.tool_result`;
- `provider.openai.tool_request` for a plugin whose `bindings.provider_id` is
  `anthropic`.

m2's broker should still enforce principal namespace rules, but m1's stated job
is to produce the structured ACL table m2 consumes and to catch lock corruption /
hand edits. Emitting an ACL with impossible authority is the same class of
runtime rug-pull that L2 fixed for `.entry`.

**Required fix:** add lock-level publish-authority validation, either in
`validate::lock` or as a hard refusal inside `broker_acl::compile` before output
is returned:

- `core.*` and `frontend.*` are never plugin publish grants;
- `plugin.<topic-id>.*` must match `topic_id::derive(canonical)` for that lock
  entry;
- `provider.<id>.*` requires `bindings.provider == true` and
  `bindings.provider_id == Some(id)`.

Add lock/compile negative tests mirroring the manifest cases, e.g.
`lock_publishes_core_topic.rs`, `lock_publishes_frontend_topic.rs`,
`lock_publishes_other_plugin_namespace.rs`, and
`lock_provider_namespace_mismatch.rs`.

### 2. `tool_owner` can point at a non-owner unless V3 says otherwise

V3 says conflicting tool names are a hard error unless
`[session].tool_owner.<name>` resolves the conflict to one specific plugin. The
positive matrix covers the happy case where both plugins claim `grep` and the
owner points at one of them.

Missing: the error cases where the owner entry points to an installed plugin that
does **not** declare that tool, or to a plugin id not installed in the lock. That
is not just an edge-case nicety. `BrokerAcl.tool_routes` is the m2 dispatcher
source of truth, and `CompiledPlugin.tool_meta` is filtered by owner resolution.
A bad owner target can route a tool call to a plugin that lacks the corresponding
`ToolMeta` / sink metadata, which is exactly the metadata m5's confirmation gate
will rely on.

**Required fix:** V3 must validate every `session.tool_owner.<tool>` entry:

- the referenced canonical id parses and is installed;
- the referenced plugin's `bindings.tools` contains `<tool>`;
- preferably, the key is only accepted when there is an actual conflict for that
  tool (or the scope explicitly says redundant owner entries are allowed).

Add negative tests such as `lock_tool_owner_unknown_plugin.rs` and
`lock_tool_owner_plugin_does_not_declare_tool.rs`.

### 3. `allow_hosts` outside proxy mode is fixed for manifests, not locks

Round 5 adds V1 manifest validation and a negative test for
`allow_hosts` with `network.mode = "deny"` / `"allow_all"`. Good.

But the runtime authority is the lock. A hand-edited lock can still put
`allow_hosts = ["x.example"]` next to `mode = "allow_all"` or `mode = "deny"`
unless V3 / compile rejects it. The scope only has `compile_invalid_allow_hosts`
for syntactically invalid proxy-mode hosts.

This recreates the same silent-ignore problem pi-4 finding 8 was trying to
close, just one artifact later in the pipeline.

**Required fix:** apply the `allow_hosts requires proxy mode` rule to lock grant
bundles as well. Add a lock/compile negative test such as
`lock_allow_hosts_outside_proxy.rs` or `compile_allow_hosts_outside_proxy.rs`.

### 4. The lockin config API names in Inputs / C1 / C7 are wrong

The scope repeatedly refers to `lockin::config::...` APIs:

- `lockin::config::resolve_network_plan`;
- `lockin::config::apply_config_to_builder`;
- m2 calling `lockin::config::apply_env` after `command(...)`.

The current crates in this worktree do not expose a `config` module from the
`lockin` crate. Those functions and types live in the separate package
`lockin-config`, imported in Rust as `lockin_config`:

- `lockin_config::resolve_network_plan`;
- `lockin_config::apply_config_to_builder`;
- `lockin_config::apply_env`.

This matters because S2 deliberately says m1 has **no `lockin-config` dep**.
That is fine for m1's own structured plan boundary, but the text must not teach
m2 implementers to call a nonexistent `lockin::config` module.

**Required fix:** replace the `lockin::config::*` references with
`lockin_config::*` when describing the existing lockin-config crate, while
keeping the m1 boundary clear: `rafaello-core` does not depend on
`lockin-config`; m2 may.

## High-priority findings

### 5. The W acceptance command is too narrow for a source-breaking fittings enum cutover

W changes the public `FittingsError::MethodNotFound` struct variant. The scope
acknowledges this is source-breaking for direct struct literals and some named
field patterns.

Manual validation currently requires only:

```sh
cargo test --manifest-path fittings/Cargo.toml --test method_not_found_typed_method_round_trip
```

That command compiles the workspace libraries and the one selected integration
test. It does **not** compile the existing integration tests under
`fittings/tests/` that pattern-match `MethodNotFound { message, data }`, nor does
it run the existing unit-test coverage in the fittings crates. The repository's
fittings CI runs the full workspace test suite; the milestone acceptance should
not be weaker than the known source-breaking blast radius.

**Required fix:** if W stays in m1, require the full fittings workspace suite in
manual validation / acceptance, e.g.

```sh
cargo test --manifest-path fittings/Cargo.toml --workspace
```

The named `method_not_found_typed_method_round_trip` test can still be called out
as the new targeted regression, but it should not be the only fittings command.

### 6. Lock-side binding snapshot validation is underspecified beyond `.entry`

Round 5 is careful with lock `.entry`, but other manifest-derived binding fields
are still described as plain values in the lock without equivalent lock-side
validation:

- `bindings.tool_meta.<n>.grant_match: Option<PathBuf>` should be the same
  package-relative `SafePath` as the manifest field, or m5 can inherit an unsafe
  hand-edited path.
- `bindings.tools` / `bindings.tool_meta` keys should use the tool-name grammar
  and be mutually consistent.
- `bindings.provider_id` should use the provider-id grammar and be present iff
  `provider == true`.
- `bindings.renderer_kinds` should retain the M7 built-in / prefixed-kind rules,
  even if plugin renderer dispatch is inert in v1.

Some of this is future-consumer hardening rather than immediate m1 compile
output, but m1 is the lock-schema foundation. If these checks are left to m5,
then m2–m4 can already persist and route from malformed lock metadata.

**Required fix:** add an explicit lock-level binding validation bullet in V3 (or
L4/L9) and enough negative tests to pin the most security-relevant cases:
unsafe `grant_match`, bad provider id / provider bool mismatch, and tool-meta key
not declared in `bindings.tools`.

### 7. Capability path canonicalisation is ambiguous for non-existent write paths

C3 says post-expansion containment checks use canonicalisation. That is clear for
existing read dirs and for the package `entry`, but capability grants include
write paths / dirs that may legitimately not exist yet. Standard filesystem
canonicalisation fails on a non-existent leaf.

If implementation agents infer "all capability paths must exist", m1 will reject
reasonable grants such as writing a future output file under `${project}/target`.
If they infer lexical normalisation only, symlink escapes through existing
ancestors need a separate rule.

**Required fix:** define the capability path resolver precisely. A typical rule:
resolve existing ancestors with symlink checks, lexically normalise the
non-existent suffix, and then enforce `${project}` / `${plugin}` containment on
the resulting absolute path. Add one positive test for a non-existent write leaf
inside `${project}` and one negative where an existing symlinked ancestor escapes.

## Medium-priority cleanup

### 8. The `lockin` dependency / `SandboxBuilder` public-surface rationale is muddy

S2 says m1 depends on `lockin` because m2 needs `SandboxBuilder` publicly and the
"type re-export is part of the public m1 surface". But C1's output is a pure
`CompiledPlugin` plan with m1-owned plan types; no in-scope API takes or returns
`SandboxBuilder`, and out-of-scope explicitly says m1 does not apply env or spawn.

Either remove the `lockin` dep from m1, or add the exact public API that requires
it. If the intent is only "m2 will separately depend on lockin", say that instead
and keep `rafaello-core` free of an unnecessary sandbox dependency.

### 9. Absolute source paths in Inputs are stale / non-portable

Inputs cite `/home/luiz/lab/lockin/...`; this review worktree is
`/home/luiz/lab-wt/m1-pi-scope-5`, and the repo-local paths are
`lockin/crates/...`. Prefer repo-relative paths so future worktrees do not point
agents at the wrong checkout.

### 10. Topic minimum segment count is not pinned in the scope

Security RFC §5.1 uses `topic := segment ("." segment)+`, i.e. at least two
segments. Scope V1 only says segment-by-segment grammar. If single-segment topics
are rejected, add the rule and a small negative. If they are accepted in m1,
state that this intentionally differs from the security RFC.

### 11. `MethodNotFound` data conflict semantics should be explicit

W2 says encode synthesises `data.method` from the typed `method` field and also
preserves caller-supplied `data` keys. It should say what happens if caller data
already contains a `method` key that conflicts with the typed field, and what
encode does when `method == None` but `data.method` is present. Pick typed-field
precedence or reject the conflict; do not leave it to implementation taste.

## Round-4 finding status

- `.active_bundles` removed / full bundle union restored: **resolved**.
- Placeholder spelling + `SafePath` vs `CapabilityPathTemplate` split:
  **resolved**, modulo the non-existent-write-path resolver ambiguity above.
- W cutover named in goal/manual/acceptance: **partially resolved**; the targeted
  test is named, but the command is too narrow for the source-breaking fittings
  change.
- Private-state topic-id drift flagged for retrospective: **resolved**.
- `milestones/README.md` m1 row patched away from builder calls: **resolved**.
- `always_confirm` round-trip coverage: **resolved**.
- Renderer kind prefix rule + test: **resolved**.
- `allow_hosts` outside proxy: **resolved for manifests, incomplete for locks**.
- Cwd-explicit rafaello commands: **resolved**.

## Summary

The draft is close, but the lock is the runtime authority. Any manifest-side
security rule that affects the compiled plan or broker ACL needs a lock-side
counterpart, or hand-edited locks can bypass m1's carefully-specified manifest
validator. Fix lock-side publish ACL validation, `tool_owner` target integrity,
lock `allow_hosts` consistency, and the lockin-config API references before owner
ratification. Then strengthen the W validation command to run the full fittings
workspace suite.
