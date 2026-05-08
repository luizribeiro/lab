# Pi review 4 — m1 manifest / lock / grant / compiler scope

Reviewed `rafaello/plans/milestones/m1-manifest/scope.md` round-4 draft in
`/home/luiz/lab-wt/m1-pi-scope-4` after the round-3 fixes.

Cross-checks: `overview.md` (especially §5.2 and §5.5),
`decisions.md` rows 16/17/31/36, `milestones/README.md`, the Stream F
manifest RFC, the Stream E renderer RFC, and the current lockin/outpost crate
surfaces in this worktree.

## Overall verdict

**Not ready for owner ratification yet.**

Round 4 fixes the concrete round-3 blockers: dependency paths now resolve,
`NetworkPlan` ownership is m1-local, lock-side `entry` is safe, sink-default
inference is over the effective tool grant, `openrpc.json` is required for every
plugin, topic-id private state is used in the test matrix, and the stale risk
paragraphs were cleaned up.

However, the new draft still has three ratification blockers:

1. the `.active_bundles` model contradicts the ratified v1 capability-flattening
   decision;
2. placeholder/path-template handling is internally inconsistent;
3. the in-scope fittings `MethodNotFound` cutover contradicts the goal/demo bar
   that says m1 is exercised exclusively by `rafaello-core` tests.

The rest of the findings are cross-doc drift / precision issues that should be
fixed before `commits.md` so implementation agents do not copy stale wording into
code.

## Blocking findings

### 1. `.active_bundles` contradicts decision row 17 and `overview.md` §5.2

The scope now defines a lock field and compiler model around active bundles:

- L6: `.active_bundles: Vec<String>` is "the set of named-bundle keys that are
  currently active for spawn".
- L6 also says "m5 is the consumer that flips a named bundle on per-call".
- C2 says the compiler flattens `default ∪ active_bundles`.
- V3/K text says lock validation and carve-out enforcement run per active bundle.
- The fixture default is normally `active_bundles = []`, i.e. default-bundle-only
  spawn.

That is not the architecture ratified in `overview.md` §5.2 / `decisions.md` row
17. The ratified model is:

> In v1, the compiler unions [capability bundles] into a single spawn-time policy
> because lockin does not support live in-process policy switching; per-method
> enforcement above the sandbox layer is core's responsibility.

There is no per-call sandbox policy switch in v1. A long-running plugin cannot
safely or usefully have m5 "flip" a named bundle for a single tool invocation
unless m5 also tears down/re-spawns the plugin under a different policy, which is
not specified and would be a much larger runtime contract.

This creates concrete implementation contradictions:

- A tool-scoped bundle such as `format` can contribute `write_dirs`; Si1 rightly
  infers `workspace_write` from `default ∪ format`, but C2 may compile a sandbox
  with only `default` if `active_bundles` is empty. The metadata says the tool is
  a sink for authority the running sandbox does not actually have.
- Conversely, if m5 mutates `active_bundles` in the lock per call, that is a lock
  mutation / spawn-policy mutation path that the overview explicitly avoided.
- m2 cannot consume this cleanly: it needs one spawn-time policy for the plugin,
  not a hidden future per-call policy knob.

**Required fix:** choose one model and make the scope match it.

Recommended: remove `.active_bundles` from m1, and make the compiler flatten the
union of every granted bundle (`default` plus all named granted bundles) into the
single spawn-time `CompiledPlugin` policy. Keep per-tool enforcement in core
above the sandbox, per row 17. Sink inference can still use `default ∪ <tool>`
for per-tool metadata.

Alternative: if the owner wants active bundles, append/revise a decision that
explicitly reverses row 17 and specify the runtime lifecycle (per-call respawn,
policy cache, lock mutation rules, and validation semantics). That seems out of
scope for m1 and probably v1.

### 2. Placeholder names and path-template validation are inconsistent

The scope uses two incompatible placeholder vocabularies.

M8 defines the closed placeholder set as lowercase:

- `${project}`
- `${home}`
- `${plugin}`
- `${cache}`
- `${state}`

C3 and `compile_unknown_placeholder.rs` say unknown placeholders are rejected.
But K1/K3 and many tests use uppercase placeholders:

- `${PROJECT_ROOT}/rafaello.lock`
- `${PROJECT_ROOT}/.rafaello/**`
- `${HOME}/.ssh/**`
- `read_dirs = ["${PROJECT_ROOT}"]`
- `write_dirs = ["${HOME}"]`

As written, the carve-out and test fixtures are using placeholders that M8/C3
would reject as unknown. Implementation agents will have to invent aliases or
change tests.

There is a second staging contradiction in M11/C3:

- M11 says every path-bearing manifest field, including capability paths after
  placeholder expansion, is parsed through `SafePath::parse`, which rejects `..`
  segments at any position.
- C3 and the negative test `compile_path_escape_after_expansion.rs` expect a
  grant such as `read_dirs = ["${project}/../../etc"]` to reach compile time and
  fail as `CompileError::PathEscape` after placeholder expansion.

Both cannot be true. If `SafePath` rejects `..` for capability path templates,
that fixture fails at parse/load time, not compile time. If compile-time escape
checking is the intended contract, capability path templates need a different
parser from package-relative fields like `entry` and `grant_match`.

**Required fix:** split the path vocabulary explicitly, for example:

- `RelativePackagePath` / `SafePath` for `entry` and `grant_match`: relative
  only, no `..`, no absolute paths, no empty segments.
- `CapabilityPathTemplate` for grant paths: allows the canonical placeholder
  syntax and possibly absolute host paths such as `/usr/bin/rustfmt`; performs
  placeholder expansion and root-containment checks at compile time.

Also pick one placeholder namespace. I recommend using the lowercase M8 names
throughout K/tests (`${project}`, `${home}`), or explicitly defining uppercase
aliases and adding tests for both. Do not leave both spellings implicit.

### 3. The in-scope fittings W cutover contradicts the m1 goal/demo bar

The W section makes `FittingsError::MethodNotFound { method: Option<String>, ...
}` in scope for m1, with a new test under `fittings/tests/`.

But the top of the scope still says:

- the deliverable is a new `rafaello-core` crate;
- it is "exercised exclusively by `cargo test -p rafaello-core`";
- nothing outside the `rfl` binary gains behaviour in m1.

The demo/manual validation sections also only require `cargo test -p
rafaello-core`, `cargo doc -p rafaello-core`, and release/nix variants of the
same. Those commands do not run the new fittings test. From the repo root there
is not even a top-level `Cargo.toml`, so the command needs either `cd rafaello`
or `--manifest-path rafaello/Cargo.toml`; for the W section it needs the
`fittings` manifest instead.

This leaves the W cutover in an awkward state: it is in scope, but the milestone
success commands do not exercise it.

**Required fix:** either:

1. keep W in scope and update Goal / Demo bar / Manual validation / Acceptance
   summary to say m1 has two deliverables:
   - `cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core`;
   - `cargo test --manifest-path fittings/Cargo.toml --test method_not_found_typed_method_round_trip`
     (or the relevant full fittings workspace command); or
2. move W out of m1 into a separate fittings follow-up and remove row-36
   follow-through from the m1 scope inputs.

I still think option 1 is fine — the W change is small — but the acceptance
commands must name it.

## High-priority findings

### 4. Private-state path changed from `<plugin-id>` to `<topic-id>` without an architecture-doc update

Round 4 correctly uses the hashed topic-id form for the private-state directory
inside m1:

- C5: `${PROJECT_ROOT}/.rafaello-plugin-data/<topic-id>/`.
- `compile_private_state_grant.rs`: asserts topic-id form, not raw canonical id.

That is the safer implementation choice. But the architecture docs still say
`<plugin-id>`:

- `overview.md` §5.5;
- `decisions.md` row 16;
- `glossary.md` "Per-plugin private state".

Given the glossary has both "Plugin id, canonical" and "Plugin id, topic-id",
plain `<plugin-id>` is ambiguous and likely means the canonical id in older
text. The scope is therefore changing a path-bearing architectural contract.

**Required fix:** before or at ratification, add an explicit note that in path
contexts the private-state directory key is the topic-id, or append a decision
row refining row 16. At minimum, add this to the m1 retrospective's required
architecture-drift list so it cannot be missed.

### 5. `milestones/README.md` still says m1 emits lockin builder calls

The scope now consistently says m1 emits a structured plan and m2 applies it to
`SandboxBuilder` / `SandboxedCommand`. That is the right boundary.

But `milestones/README.md` still describes m1 as:

> Grant compiler that produces lockin builder calls from a granted lock entry.

That is stale and points future milestone drivers at the round-1 model pi
rejected.

**Required fix:** patch the m1 row in `milestones/README.md` to say "produces a
structured compile plan consumed by m2" rather than "builder calls".

### 6. `always_confirm` has no explicit acceptance coverage

M3 parses `always_confirm`, L4 stores it in `ToolMeta`, and C1 exposes
`tool_meta`, so the intended path is present. But the test matrix does not name a
positive assertion that `always_confirm = true` survives manifest → lock →
compiled metadata.

This field is load-bearing for m5's confirmation gate. Without a named test, it
is easy for an implementation to parse it but drop it during lock or compile
projection.

**Required fix:** add one positive test row, e.g.
`tool_meta_always_confirm_round_trip.rs`, asserting that `always_confirm` is
preserved through the lock and appears in `CompiledPlugin.tool_meta` for the
owning routed tool.

### 7. Renderer kind grammar is under-specified and one positive fixture looks stale

`manifest_parse_renderer_example.rs` accepts two non-built-in kinds:
`mermaid:diagram` and `code.diff`.

Stream E §8 says plugin kinds must be prefixed, with examples like
`mermaid:diagram` and `myorg:trace`. The scope does not define a renderer-kind
grammar beyond rejecting built-in kinds, and `code.diff` is neither built-in nor
prefixed in the Stream E sense.

Since plugin renderers are inert in v1, this is not a runtime blocker, but m1 is
where the manifest schema lands. Accepting an unprefixed `code.diff` now may
create a v2 compatibility trap.

**Required fix:** either define the v1 parser as intentionally permissive for
forward compatibility, or validate plugin renderer kinds against the Stream E
prefix rule and change the fixture to a prefixed kind such as `diff:code`.

## Medium-priority cleanup

### 8. `NetworkPlan` should say what happens to `allow_hosts` outside proxy mode

The scope dry-runs `outpost::NetworkPolicy::from_allowed_hosts(...)` for proxy
mode, but it does not state whether `allow_hosts` is rejected when
`network.mode = "deny"` or `"allow_all"`.

Lockin-config rejects `allow_hosts` outside proxy mode because otherwise the
field is silently ignored. m1 owns its own `NetworkPlan`, so it should make the
same decision explicitly.

Recommended: reject non-empty `allow_hosts` unless `mode = "proxy"`, with a
small negative test. This is mostly UX / lock-corruption hardening, not a core
security blocker.

### 9. Cargo commands should be cwd-explicit

The scope repeatedly says `cargo test -p rafaello-core`. In this monorepo there
is no root `Cargo.toml`; CI uses `--manifest-path rafaello/Cargo.toml`. The
milestone driver may run commands from repo root, from `rafaello/`, or from a
worktree root depending on the session.

Recommended: write acceptance/manual commands in cwd-explicit form, e.g.

```sh
cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core
cargo doc  --manifest-path rafaello/Cargo.toml -p rafaello-core --no-deps
```

and add the fittings command if W remains in scope.

## Round-3 finding status

- Dependency paths / `NetworkPlan` ownership: **resolved**.
- Lock-side `.entry` safety: **resolved**.
- Sink inference over effective tool grant + unconditional drift check:
  **resolved**, modulo the broader active-bundle contradiction above.
- `openrpc.json` for every plugin: **resolved**.
- Private-state test row switched to topic-id: **resolved locally**, but needs
  architecture-doc reconciliation.
- Topic-id collision helper public API wording: **resolved**.
- Digest symlink cycle handling: **resolved**.
- Manifest name grammar: **resolved**.
- `load.event` pattern matching: **resolved**.

## Summary

This draft is close, but the `.active_bundles` model is a real architectural
regression against row 17, and the placeholder/path-template inconsistency will
cause implementation churn immediately. Fix those, make the W acceptance path
explicit, and do the small cross-doc cleanup before owner ratification.
