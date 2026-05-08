# Pi review 1 — m1 manifest / lock / grant / compiler scope

Review target: `rafaello/plans/milestones/m1-manifest/scope.md` as the
round-1 draft.

Verdict: **do not ratify as-is**. The milestone is pointed at the right
layer — pure manifest/lock/grant/compiler data transformation before any
runtime spawn — but the current scope has several implementation-level
contradictions. If implemented literally, m1 either cannot compile against
`lockin`'s public API, or lands a lock/compiler surface that m2 cannot use
without immediate breaking rewrites.

## Summary of blocking findings

1. **The compile output shape does not match `lockin`'s Rust API.** The
   scope asks m1 to return a pre-populated `SandboxBuilder`, to assert an
   env/command call sequence, and to encode `allow_hosts`; `lockin` cannot
   do that on the builder value as described.
2. **The lock schema omits data the compiler requires.** `entry`, named
   capability bundles, the active bundle set, `[session].tool_owner`, and
   digest-check inputs are referenced later but absent from the schema/API.
3. **Tool/method/bundle naming is internally inconsistent.** Tool names are
   constrained to one topic segment, while the positive examples and bundle
   tests use dotted method names like `rust.format`.
4. **Several validation/compile APIs lack the context their own rules need.**
   `compile_plugin` has no active-bundle or recomputed-digest inputs, and
   `trifecta::evaluate` has no `PathContext` even though it expands paths.
5. **Carve-out semantics contradict themselves.** K2 says broad write grants
   covering carve-outs are refused; the negative matrix says
   `${PROJECT_ROOT}` writes are decomposed around `rafaello.lock`.
6. **The dependency policy is impossible/incomplete for the stated APIs.**
   The workspace has no shared dependency table yet, the scope forbids new
   top-level deps while requiring them, and it omits at least `semver`.
7. **Some acceptance rows cannot pass against the stated parser.** The
   manifest RFC worked examples still contain fields m1 must reject, and the
   env scrubber test row has contradictory expected output.

## Blocking findings in detail

### 1. `CompiledPlugin` / `SandboxBuilder` is not a viable API as written

Scope citations:

- Inputs bind m1 to lockin's builder API and list `network_*` /
  `command(...)` as the public surface.
- C1 returns `sandbox_builder: lockin::SandboxBuilder`; m2 then calls
  `command(entry).spawn()`.
- C4 says the compiler call sequence is `network mode → allow_hosts → ...
  → env → limits → command(entry)`.
- `compile_default_bundle.rs` asserts recorded calls including `env` and
  `command(entry)`.

This does not match current `lockin`:

- `SandboxBuilder::command(self, program)` consumes the builder and returns
  a `SandboxedCommand`. If m1 calls `command(entry)`, it cannot also return
  a `SandboxBuilder` for m2 to call later.
- Env is not a `SandboxBuilder` concern. `lockin::config::apply_env` takes a
  `&mut lockin::SandboxedCommand`, i.e. after `command(...)` has consumed the
  builder.
- `allow_hosts` is not a builder method. Lockin's config layer resolves
  `allow_hosts` to an `outpost::NetworkPolicy`; the caller must start an
  outpost proxy and pass the resulting loopback port to
  `SandboxBuilder::network_proxy(port)`.

Recommended fix:

- Choose one compile boundary and make the whole scope follow it:
  - **Builder-only boundary:** m1 returns `SandboxBuilder` plus separate
    `entry_absolute`, `EnvPlan`, and `NetworkPlan/allow_hosts` data for m2 to
    apply when it creates the `SandboxedCommand`; tests stop asserting
    `env`/`command(entry)` as builder calls.
  - **Command-ready boundary:** m1 returns a `SandboxedCommand` (or a trait
    abstraction over it), applies env after `command(entry)`, and no longer
    says m2 calls `command` itself.
- For proxy mode, explicitly model the outpost step. A lock grant of
  `network.mode = "proxy"` + `allow_hosts` cannot be represented as just a
  `SandboxBuilder` until something starts the proxy and supplies a port.

### 2. The lock schema is not sufficient to compile a plugin

The scope says the compiler reads a lock entry and produces
`entry_absolute = ${plugin}/<entry>`, flattens capability bundles, checks
content/manifest digests, and resolves conflicting tool owners. The lock
schema bullets do not carry the corresponding data:

- L1-L5 do not include the plugin `entry` path. The compiler cannot compute
  `entry_absolute` from `Lock + CanonicalId + PathContext` alone.
- L2 is a flat `.grant` table. There is no representation for
  `default` plus named capability bundles, but C2 and the tests require
  scoped bundle union.
- C2 says the test surface takes an active-bundle set as input, but
  `compile::compile_plugin(lock, plugin_id, ctx)` has no such parameter.
- V2 says conflicting tool names are allowed with a
  `[session].tool_owner.<name>` decision, but L1 defines `[session]` as only
  `provider_active`.
- D3 says callers pass recomputed content and manifest digests via
  `PathContext`, but C3 defines `PathContext` only as project/home/plugin/cache/state
  directories.

Recommended fix:

- Expand the lock schema before ratification. At minimum, a plugin entry
  needs `entry` and either a bundle-aware grant shape or a deliberately
  flattened grant shape with scoped bundles removed from m1.
- Add `session.tool_owner: BTreeMap<String, CanonicalId>` if V2 is in scope.
- Add active bundles and recomputed digest inputs to the compiler API (or move
  those checks to a distinct validation function whose signature includes the
  required inputs).

### 3. Tool names, methods, and bundle keys are conflated

M5 says `provides.tools` are validated as a single topic segment
`[a-z0-9_-]+`. Later rows use dotted method names:

- `manifest_parse_worked_example.rs` expects the RFC `rust-tools` example and
  scoped `[capabilities."rust.format".filesystem]` to decode.
- V1 cross-validates `load.command = ["foo"]` against `provides.tools`.
- C2 and `compile_scoped_bundle_union.rs` use an active bundle named
  `"rust.format"`.

Those cannot all be true if tool names are single segments. Either
`rust.format` is a tool/method name and the grammar must allow dotted names,
or tool names are segments and the examples/bundle keys must be rewritten to
use segment names such as `format` / `rust_format`.

Recommended fix:

- Pick a single vocabulary:
  - **Tool names are single routing names** (`read-file`, `grep`): update the
    worked examples, load triggers, and bundle tests away from `rust.format`.
  - **Tools are OpenRPC-style dotted method names**: update M5/L6/tool_meta
    key handling and stop justifying the grammar as a topic segment.
- Separately define capability bundle key grammar. A bundle key that may be a
  topic/pattern or a dotted method name is not the same grammar as a topic
  segment.

### 4. APIs omit context required by their own validation rules

Examples:

- `trifecta::evaluate(lock, plugin_id)` computes whether `read_dirs`/
  `read_paths` are outside `${PROJECT_ROOT}` after placeholder expansion, but
  it receives no `PathContext`.
- `carveout::compile_against(grant, ctx, allow_credential_paths)` implements
  private-state and hidden-directory behaviour that also needs the plugin id
  to name `.rafaello-plugin-data/<plugin-id>` unless that is hidden inside
  `PathContext`.
- `validate::manifest(&Manifest, &PathContext)` cannot fully validate
  `plugin.<topic-id>.*` self-namespace publishes until the canonical id/source
  is known. The prose mentions a hook, but the API surface does not name the
  hook or the lock-bound manifest validation function.

Recommended fix: make context dependencies explicit in the API list, not just
in prose. The future m2 caller should be able to tell from the signatures what
must be supplied.

### 5. Carve-out refusal vs decomposition is contradictory

K2 says write grants covering any carve-out are refused unless
`allow_credential_paths` is set. The negative matrix says
`carveout_lockfile_path.rs` expects:

- `write_dirs = ["${PROJECT_ROOT}"]` covering `${PROJECT_ROOT}/rafaello.lock`
  is decomposed; but
- direct `write_paths = ["${PROJECT_ROOT}/rafaello.lock"]` is refused.

Those are different policies. The security RFC also leans toward refusing broad
write ancestors, while the milestone overview allows “refusing or decomposing.”
This must be pinned before implementation because it changes both security UX
and test fixtures.

Recommended fix: choose one rule and make K2 + tests match it. If broad
workspace writes are decomposed, spell out exactly when write decomposition is
allowed and which carve-outs still force refusal. If broad writes are refused,
change `carveout_lockfile_path.rs` accordingly.

### 6. Dependency constraints are currently impossible

S2 says dependencies are pinned via the workspace `Cargo.toml` and “no new
top-level deps the workspace doesn't already use,” but `rafaello/Cargo.toml`
currently has no `[workspace.dependencies]` and only the binary crate is a
member. The expected m1 dependency set also omits dependencies that the scope
requires:

- M1 and L6 require semver / semver-req parsing; L6 says to use the
  workspace's semver crate, but S2 does not list one.
- L5 requires RFC3339 parsing/formatting; the scope names no `time`/`chrono`
  dependency or string-only validation rule.
- Tests will likely need `tempfile` or an equivalent fixture helper for
  package/openrpc/digest/carve-out trees.

Recommended fix: change S2 to explicitly add a `[workspace.dependencies]`
section and list all required crates, including `semver` and the chosen time
crate or a deliberate string-only RFC3339 validator. Do not keep the “no new
deps the workspace doesn't already use” sentence; m1 necessarily introduces
new dependencies to the rafaello workspace.

### 7. Acceptance matrix has self-contradictory / stale examples

Positive manifest tests cite Stream F worked examples that are known stale:

- RFC §9.1 and §9.2 include `runtime` and `[rpc]`, both rejected by m1.
- RFC §9.3 includes `runtime`, `[rpc]`, and `${secret:...}` in `env.set`, while
  m1 rejects `runtime`/`[rpc]` and explicitly excludes secret interpolation.
- Some worked examples omit the required top-level `rafaello` field.

The test rows say “RFC §9.x decodes,” but M2/M3/M10 say those same inputs must
not decode. The fixture text should be explicitly “post-simplification
rewrites of RFC §9.x,” not the RFC examples as printed today.

The env scrubber positive row is also internally contradictory: it first says
`["GITHUB_TOKEN", "MY_API_KEY", "AWS_REGION", "PATH"]` scrubs to
`AWS_REGION` + `PATH`, then says `AWS_REGION` is stripped because `AWS_*`
matches and only `PATH` survives.

Recommended fix: rewrite the fixture descriptions so every positive fixture is
valid under m1's parser, and fix the env scrubber expected output.

## High-priority non-blocking findings

### 8. Reserved-field errors need a parsing strategy

M1 says `deny_unknown_fields` is set on every struct, while M2/M3/M4 require
specific `ManifestError::ReservedField` variants for `runtime`, `[rpc]`, and
`helper_for`. Plain serde unknown-field handling will normally report those as
unknown fields before custom reserved-field logic sees them.

This is implementable, but the scope should say parsers pre-scan the TOML table
for reserved fields (or use a custom deserializer) so implementers do not land
only generic unknown-field errors and fail the negative matrix.

### 9. Sink defaults from the security model are missing

Security RFC §7.2.5 says missing sink metadata defaults from the grant:
network implies `network`, workspace writes imply `workspace_write`. m1 validates
explicit sink strings and snapshots `bindings.tool_meta`, but it does not say
where default sink inference happens or how lock corruption/omitted sink metadata
is handled.

If m1 produces the lock/broker ACL surfaces that m4/m5 will trust, this default
must be either implemented now or explicitly deferred to the install flow with a
validation error for lock entries that omit `tool_meta.<tool>.sinks`.

### 10. Reserved env vars are described in two incompatible ways

C7 correctly says `RFL_BUS_FD` and `RFL_PLUGIN` are stripped from the parent
environment before `env.pass`. Sc3 then tests that listing them in `env_pass`
“survives the strip.” The security intent is: they are exempt from the secret
scrubber, but user/parent values are not allowed to reach the plugin; core
injects authoritative values later.

Recommended fix: split the tests:

- `scrubber::strip` does not classify `RFL_BUS_FD` / `RFL_PLUGIN` as secrets;
- compiler/env application removes parent-supplied values before pass matching;
- m2 spawn injection sets the core-owned values after env policy is applied.

## Scope-size note

The draft estimates 30–45 sequential commits. After resolving the builder/env/
network-plan boundary and the bundle-aware lock schema, that estimate may still
be plausible, but only if `commits.md` splits parser/lock/compiler work very
aggressively. If the chosen fix is “return command-ready output” plus outpost
proxy planning, m1 may need an m1a/m1b split earlier than group 9.

## Ratification bar

Before owner ratification, update `scope.md` so it is internally consistent on:

- whether m1 returns a builder-only plan or command-ready object;
- how env and proxy `allow_hosts` are represented against lockin's API;
- the bundle-aware lock schema and active-bundle compiler input;
- `entry`, `session.tool_owner`, and digest-check inputs;
- tool/method/bundle naming grammar;
- carve-out write decomposition vs refusal;
- dependency list and workspace dependency policy;
- positive fixture examples and env scrubber expected results.
