# m1 — manifest / lock / grant / compiler foundation — scope

> **Status:** ratified by owner 2026-05-08 after six pi review
> rounds (`pi-review-1.md` through `pi-review-6.md`). `commits.md`
> is also ratified; Phase 3 per-commit agent work begins on the
> `rafaello-v0.1` branch — see `driver-notes.md`.

## Goal

Land the **paper-first data-transformation** layer that takes a
plugin manifest plus a user grant and produces a structured
`CompiledPlugin` plan that m2's plugin supervisor can apply to a
`lockin::SandboxBuilder` + `SandboxedCommand` at spawn time —
without yet wiring up any spawn, broker, network proxy, or CLI.
m1 is intentionally pure: every output is computable from its
inputs; every test is a function-level assertion. m2 is the first
consumer (it spawns subprocess plugins from m1's compile output).

The deliverable is a new in-tree library crate `rafaello-core`,
exercised primarily by `cargo test --manifest-path
rafaello/Cargo.toml -p rafaello-core`. Plus a small Stream B
fittings cutover (§W — `FittingsError::MethodNotFound` typed
`method` field per `decisions.md` row 36 — exercised by
`cargo test --manifest-path fittings/Cargo.toml --workspace`
since the enum-shape change is source-breaking). Nothing under
`rafaello/crates/rafaello/` (the `rfl` binary) gains any new
behaviour in m1. Pi review-4 finding 3 forced the explicit
two-deliverable acceptance gate.

## Inputs

- `rafaello/plans/overview.md` §3, §4 (esp. §4.2–§4.5), §5, §6,
  §15.1, §16.
- `rafaello/plans/decisions.md` rows **5, 12, 17, 25, 26, 27, 30,
  31, 32** (load-bearing for m1) plus row 36's m1 follow-through
  on `FittingsError::MethodNotFound`.
- `rafaello/plans/streams/f-manifest/rfc-manifest-schema.md` end
  to end. v1 reads it through the simplifications pinned in rows
  30/31/32 and the §15.1 normative-delta items 1–4 of `overview.md`.
  These deltas are not yet folded into the RFC body; m1's
  retrospective patches the drift (per `plans/milestones/README.md`
  §"Stream RFC drift").
- `rafaello/plans/streams/a-security/rfc-security-model.md`
  §3 (artifacts), §5 (bus ACL — topic / pattern grammar, namespaces,
  publish authority), §6 (attack scenarios that map onto compiler
  refusals), §7 (the grant compiler — trifecta, taint, sinks,
  carve-outs, env scrubber, private state).
- `rafaello/plans/streams/a-security/rfc-camel-on-v1.md` is **not**
  load-bearing for m1; the v1 deferrals (rows 26–29) eliminate every
  m1 dependency on it.
- `rafaello/plans/glossary.md`.
- Lockin's public Rust API for the v1 sandbox backend
  (repo-relative paths: `lockin/crates/sandbox/src/lib.rs`
  and `lockin/crates/config/src/lib.rs`):
  - `lockin::Sandbox::builder() -> SandboxBuilder` exposes
    `read_path` / `read_dir` / `write_path` / `write_dir` /
    `exec_path` / `exec_dir` / `network_*` / `inherit_fd` /
    `max_*` / `disable_core_dumps` / `allow_kvm` /
    `allow_interactive_tty` / `allow_non_pie_exec` /
    `raw_seatbelt_rule` builder methods.
  - `SandboxBuilder::command(self, program: &Path) ->
    Result<SandboxedCommand>` **consumes the builder** and
    returns the command. Env (`apply_env`) is then applied to
    `&mut SandboxedCommand`, not the builder. `allow_hosts`
    is **not** a builder method — it lives in
    `lockin_config::resolve_network_plan` which uses
    `outpost::NetworkPolicy` and yields `NetworkPlan::Proxy
    { policy }`; the caller starts an `outpost` proxy and
    passes the resulting loopback port to
    `SandboxBuilder::network_proxy(port)`.
  - `lockin_config::apply_config_to_builder(builder, config,
    config_dir) -> Result<SandboxBuilder>` is the existing
    file→builder bridge; m1's compiler is a structural mirror
    that **does not depend on `lockin_config`** (config
    consumes a `lockin.toml`, which `decisions.md` row 32
    rules out for v1).

  **Decision row 32 binds m1 to emitting structured plans the
  m2 supervisor applies — *not* a pre-populated `SandboxBuilder`
  that m2 only has to call `command(...)` on. Builder vs. plan
  mismatch was pi review-1 finding 1.**

## In scope

The work decomposes into modules of the new `rafaello-core` crate.
Each in-scope item names a public API surface plus the negatives
the test matrix asserts; per-commit granularity is the driver's
call when drafting `commits.md`.

### S — crate skeleton

- **S1.** New library crate `rafaello-core` under
  `rafaello/crates/rafaello-core/`, added to the workspace
  `[workspace.members]`. `lib.rs` only — no binary, no
  `rafaello`-bin wiring. Public re-exports limited to the modules
  named below; everything else stays crate-private.
- **S2.** A `[workspace.dependencies]` table is **introduced** in
  the rafaello workspace `Cargo.toml` (it does not exist today —
  pi review-1 finding 6) and m1's deps are declared there:
  - `serde = { version = "1", features = ["derive"] }`
  - `toml = "0.8"`
  - `serde_json = "1"`
  - `sha2 = "0.10"`
  - `data-encoding = "2"`  (base32-no-pad-lower for topic-id)
  - `thiserror = "1"`
  - `semver = { version = "1", features = ["serde"] }`
  - `chrono = { version = "0.4", features = ["serde"] }`  (RFC 3339 parsing for `granted_at`; matches the workspace family default)
  - **No `lockin` dep on `rafaello-core`** (pi review-5
    medium 8). m1 emits a structured `CompiledPlugin` plan
    using m1-owned plan types (`FilesystemPlan`,
    `NetworkPlan`, `EnvPlan`, `LimitsPlan`); m1's public API
    never takes or returns `lockin::SandboxBuilder`. m2
    imports `lockin` separately and applies the plan at
    spawn time. Keeping `rafaello-core` free of the sandbox
    dep also helps the future capsa swap (decision row 2).
  - `outpost = { path = "../outpost/crates/outpost" }` —
    direct dep (pi review-2 finding 6 + pi review-3 finding 1:
    m1's `compile_invalid_allow_hosts.rs` calls
    `outpost::NetworkPolicy::from_allowed_hosts(...)` for
    dry-run `allow_hosts` validation). Path is workspace-local
    relative to `rafaello/Cargo.toml` (`lab/outpost/crates/outpost`).
  - **No `lockin-config` dep** — pi review-3 finding 1 caught
    that `lockin_config::NetworkPlan` is `Proxy { policy:
    outpost::NetworkPolicy }`, while m1's own `NetworkPlan`
    is `Proxy { allow_hosts: Vec<String> }` (m2 starts the
    proxy and resolves the policy at spawn time). The two are
    different types at different boundaries; m1 owns its
    `NetworkPlan` outright and uses `outpost` only for the
    parse-time dry-run.
  - `tempfile = "3"` as a `[dev-dependencies]` entry for the
    fixture trees the digest / carve-out tests build.
  m1 introduces no other top-level deps. The "no new top-level
  deps" wording from the round-1 draft was wrong; this revision
  drops it (pi review-1 finding 6).
- **S3.** No public API leaks from `rafaello-core` outside the
  modules named in M / L / T / C / D / V / E / G / Tr / K / Sc
  below. The `rafaello`-bin entry point
  (`crates/rafaello/src/main.rs`) is unchanged in m1.

### M — manifest schema and parser (`rafaello_core::manifest`)

Parse `rafaello.toml` into a typed `Manifest` value. The schema
matches Stream F **post-simplifications**: no `runtime`, no
`[rpc]`, no `helper_for`. The `[provides]` block follows the
§15.1 normative delta in `overview.md`.

- **M1.** Top-level required fields: `schema = 1`, `name`
  (must match the topic-segment grammar `[a-z0-9_][a-z0-9_-]*`
  per L8 — pi review-3 finding 8 caught that the round-3 draft
  validated the lock-side canonical-id `name` but not the
  manifest-side `name`), `version` (`semver::Version`),
  `entry` (relative path inside the package — see M11 for the
  path-safety rule), `rafaello = ">=0.1, <0.2"`
  (`semver::VersionReq`). Optional
  metadata: `description`, `authors`, `license`, `homepage`.
  All structs use `#[serde(deny_unknown_fields)]` so unknown TOML
  keys are a typed parse error.
- **M2.** Reserved-field handling (pi review-1 finding 8). The
  three keys `runtime`, `rpc`, and `helper_for` are reserved per
  `decisions.md` rows 30 / 31 / 26. `Manifest::parse` runs a
  **pre-scan** of the raw TOML document (using `toml::Table`)
  before the typed deserialise: if any of those keys is present
  at its expected location (top-level `runtime`, top-level
  `helper_for`, the `[rpc]` table), parse fails with
  `ManifestError::ReservedField { field, deferred_to:
  ReservedReason }` *before* `serde`'s `deny_unknown_fields`
  fires. The hint string explains that `runtime` is post-row-30,
  `[rpc]` is post-row-31 (use the `openrpc.json` sibling), and
  `helper_for` is deferred to v2. Other unknown fields fall
  through to serde and produce `ManifestError::UnknownField`.
- **M3.** `[provides]` block per §15.1 item 1:
  - `tools: Vec<String>` (zero or more **tool names** —
    `[a-z0-9_][a-z0-9_-]*`, single-segment, lower-snake/kebab
    only; no dots; pi review-1 finding 3 forced this commitment).
    Examples: `grep`, `read-file`, `format`. The earlier draft's
    use of `rust.format` as a tool name is wrong — that's an
    OpenRPC method name and is not the manifest-side tool
    routing key.
  - `provider: Option<String>` (zero or one provider id; same
    grammar — provider id appears literally in
    `provider.<id>.*` topics).
  - `[provides.tool.<name>]` table per declared tool:
    - `sinks: Option<Vec<String>>` — known classes `"network"`,
      `"vcs_push"`, `"mail"`, `"workspace_write"`, `"exec"`;
      custom strings allowed but lower-snake-case-validated
      (`[a-z0-9_]+`). **`None` (key absent) is distinct from
      `Some(vec![])` (key present, empty list)**: the install
      flow infers defaults (Si1) only when the manifest omits
      the key; an explicit empty list is the author's
      affirmative declaration "no sinks". Pi review-2 finding
      2 forced this distinction.
    - `grant_match: Option<PathBuf>` — relative path to a
      JSON-Schema describing the `user_grants` matcher
      (subject to M11's path-safety rule). Absent → "any
      invocation" matcher; the schema file itself is **not**
      parsed in m1 (its consumer is m5); v1 only validates
      the path syntax + that the file exists at install time
      (M10).
    - `always_confirm: bool` — UX gate per §15.1 item 1;
      defaults `false`.
- **M4.** `[bus]` block:
  - `subscribes: Vec<String>` — list of subscribe **patterns**
    drawn from the §5.1 pattern grammar (`*` / `**` allowed;
    `**` only as final segment).
  - `publishes: Vec<String>` — list of **topics** (no
    wildcards; pattern in publish position is rejected).
- **M5.** `[capabilities]` block, with the bundle-and-section
  grammar from manifest RFC §5: `[capabilities.<bundle>.{filesystem,network,env,limits}]`.
  Bundle name `default` plus zero or more named bundles; each
  named bundle's key **must** be a tool name from
  `provides.tools` (pi review-1 finding 3 — bundle keys are
  not free-form). Each section is a typed inner struct with the
  fields the manifest RFC enumerates: `read_paths`, `read_dirs`,
  `write_paths`, `write_dirs`, `exec_paths`, `exec_dirs`,
  `network.mode`, `network.allow_hosts`, `env.pass`, `env.set`,
  `limits.max_cpu_time`, `limits.max_open_files`,
  `limits.max_address_space`, `limits.max_processes`.
- **M6.** `[load]` block per manifest RFC §7.2. Either a string
  (`"eager"` / `"boot"` / `"manual"` / `"lazy"` sugar) or a
  table with one or more of `event` / `command` / `kind`.
- **M7.** `[[renderers]]` array per manifest RFC §6: `kind`,
  optional `priority` (default 100), optional `method`. Two
  rejection classes:
  - **Built-in kind names** (`text`, `code_block`,
    `tool_call`, `tool_result`, `error`, `heading`,
    `thinking`, `image`) are reserved and rejected — built-in
    registration is hard-coded in core (m3 territory).
  - **Plugin kind grammar** per Stream E §8 (pi review-4
    finding 7): plugin-supplied kinds must be **prefixed**
    `<vendor-prefix>:<kind-name>`, where `<vendor-prefix>`
    matches `[a-z][a-z0-9_-]*` and `<kind-name>` matches the
    same. Examples: `mermaid:diagram`, `myorg:trace`,
    `diff:code`. Unprefixed plugin kinds (e.g. `code.diff`)
    are rejected as `ValidationError::UnprefixedRendererKind`
    so a v2 stream-E pattern collision can't bite.
  Non-built-in plugin renderer registrations parse and
  round-trip into the lock for forward compatibility, but
  `decisions.md` row 29 defers subprocess `renderer.render`
  dispatch to v2; m3's renderer router is built-in-only and
  **ignores plugin renderer registrations entirely** (pi
  review-2 finding 10). m1 records a doc comment on the
  parsed type pointing the next reader at row 29 so the
  inertness isn't accidentally relied on as a v1 feature.
- **M8.** Placeholder substitution table per manifest RFC §5.1,
  exposed as `manifest::placeholders::expand(input: &str,
  ctx: &PathContext) -> Result<String, ManifestError>`.
  Closed set: `${project}`, `${home}`, `${plugin}`, `${cache}`,
  `${state}`. No env-var interpolation, no `${secret:...}`.
- **M9.** Canonical-form normalisation: `Manifest::canonical_bytes()`
  emits a deterministic byte representation used for hashing
  the manifest snapshot at install time (D2). Implemented as
  "TOML re-emit with sorted keys at every table level" via
  `toml::Table` — no semantic transforms.
- **M10.** **Package-level validation**, per `decisions.md`
  row 31 + pi review-2 findings 1 and 9 + pi review-3 finding 4.
  The single entry point
  `manifest::validate_with_package(manifest_path, package_dir,
  manifest)` performs every check that requires the on-disk
  package layout:
  - **`openrpc.json` sibling**: `<package_dir>/openrpc.json`
    must exist as a regular file for **every** plugin
    (matching `decisions.md` row 31's unqualified text — pi
    review-3 finding 4 caught the round-3 narrowing to
    `provides.tools` non-empty as drift from the ratified
    decision). Provider plugins, renderer-only plugins, and
    pure-bus plugins all ship an `openrpc.json` sibling; for a
    plugin that exposes no methods, the file lists no methods
    but still satisfies row 31's manifest-decoupling intent
    (Stream F can describe its surface in OpenRPC instead of
    re-stating it in TOML). Absent → `ManifestError::MissingOpenRpc`.
  - **`entry` resolution**: the manifest's `entry` field, after
    canonicalisation against `package_dir`, must point at an
    existing regular file *inside* `package_dir` (no traversal
    escape). Failures: `ManifestError::EntryEscape`,
    `ManifestError::EntryNotFound`, `ManifestError::EntryNotFile`.
  - **`grant_match` resolution** for every `[provides.tool.<name>]`
    table that sets it: same rule as `entry` — relative,
    canonicalised against `package_dir`, must exist as a regular
    file inside the package. Failures:
    `ManifestError::GrantMatchEscape`,
    `ManifestError::GrantMatchNotFound`,
    `ManifestError::GrantMatchNotFile`. v1 still does not
    *parse* the JSON-Schema; presence is the only gate (m5
    parses + uses).
  v1 does **not** parse openrpc.json; that contract belongs to
  fittings + m2's spawn path.

- **M11.** **Two distinct path vocabularies in the manifest**
  (pi review-2 finding 1 + pi review-4 finding 2 split):
  - **`SafePath` (relative-package paths).** Used by `entry`
    and `[provides.tool.<n>].grant_match`. Parsed through
    `manifest::SafePath::parse(s: &str) -> Result<Self,
    ManifestError>` which rejects: leading `/`, `..` segments
    at any position, empty path segments (consecutive `/` or
    trailing `/`), control characters and non-UTF-8 bytes,
    `\` separators. These fields are always relative to the
    package directory and never carry placeholders.
  - **`CapabilityPathTemplate` (placeholder-or-absolute
    paths).** Used by `read_paths` / `read_dirs` /
    `write_paths` / `write_dirs` / `exec_paths` /
    `exec_dirs`. Parsed through
    `manifest::CapabilityPathTemplate::parse(s: &str) ->
    Result<Self, ManifestError>` which accepts a closed
    placeholder set from M8 as a prefix (`${project}`,
    `${home}`, `${plugin}`, `${cache}`, `${state}`) **or**
    an absolute host path (e.g. `/usr/bin/rustc`); rejects
    relative paths without a placeholder prefix (no implicit
    cwd anchor), control chars, and non-UTF-8 bytes. `..`
    segments are *parser-allowed* in capability templates;
    C3's post-expansion root-containment check catches
    escapes for `${project}` / `${plugin}`-anchored paths
    (this is what makes `compile_path_escape_after_expansion.rs`
    reachable — pi review-4 finding 2 caught the round-3
    contradiction where uniform `SafePath` would have
    rejected `..` at parse time).

### L — lock schema (`rafaello_core::lock`)

Mirror of the manifest plus the bindings + grant view per security
RFC §3.2. Pi review-1 finding 2 forced expansion of this surface
to carry every input the compiler needs.

- **L1.** Top-level shape:
  - Per-plugin tables keyed by canonical id `<source>:<name>@<version>`,
    e.g. `[plugin."github:acme/grep@1.4.2"]`.
  - Per-plugin sub-tables `.entry`, `.grant`, `.bindings`,
    `.flags`.
  - `[session]` table with:
    - `provider_active: Option<String>` (string or absent)
    - `tool_owner: BTreeMap<String, String>` — per security RFC
      §5.4 ("conflicting tool bindings are a lock-time error;
      the user resolves with `rfl provider tool grep
      <plugin-id>`. The choice is persisted in the lock");
      m1 round-trips and respects this table.
- **L2.** **`.entry: SafePath`** per plugin (pi-1 finding 2 +
  pi-3 finding 2). The manifest's `entry` field is snapshotted
  into the lock at install time; the compiler reads this field
  to resolve `entry_absolute = ${plugin_dir}/<entry>`. The
  lock-side copy is what makes the spawn path independent of
  the on-disk manifest (security RFC §3.2). The lock loader
  parses `.entry` through the same M11 `SafePath` rule as the
  manifest (rejects absolute paths, `..`, empty segments,
  control chars, `\` separators). The compiler additionally
  checks at compile time that
  `${plugin_dir}/<lock-entry>` resolves (after canonicalisation
  + symlink follow-up) to a regular file inside `plugin_dir` —
  failures: `CompileError::EntryEscape`,
  `CompileError::EntryNotFound`, `CompileError::EntryNotFile`.
  These mirror the manifest-time M10 checks but run against
  the lock snapshot to defend against a hand-edited lock that
  bypasses install-time validation.
- **L3.** **`.grant.bundles`** — bundle-aware grant shape. The
  grant block is a map `BTreeMap<BundleKey, GrantBundle>` where
  `BundleKey ∈ { Default, Named(String) }` and `GrantBundle`
  carries the per-bundle `read_paths` / `read_dirs` /
  `write_paths` / `write_dirs` / `exec_paths` / `exec_dirs`
  / `network` / `env_pass` / `env_set` / `limits`. The lock
  also carries the cross-bundle fields `subscribes:
  Vec<String>` (patterns) and `publishes: Vec<String>` (topics)
  at the `.grant` level (these are not bundle-scoped). Pi-1
  finding 2 made the bundle representation explicit.
- **L4.** `.bindings` fields per security RFC §3.2 (manifest-derived
  authority snapshotted at install time):
  - `provider: bool`,
  - `provider_id: Option<String>` (present iff `provider == true`),
  - `tools: Vec<String>`,
  - `renderer_kinds: Vec<String>`,
  - `load: LoadPolicy` — snapshot of the manifest's `[load]`
    block (pi review-6 finding 2). Without this, the spawn
    path would have to re-read the live manifest, breaking
    "spawn reads the lock snapshot" from security RFC §3.2.
    `LoadPolicy` is an enum mirroring M6:
    `Eager | Boot | Manual | Lazy { event: Vec<String>,
    command: Vec<String>, kind: Vec<String> }`.
  - `tool_meta: BTreeMap<String, ToolMeta>` where
    `ToolMeta = { sinks: Vec<String>, sinks_inferred: bool,
    grant_match: Option<PathBuf>, always_confirm: bool }`.
    `sinks_inferred` (added per pi review-2 finding 2) records
    whether the lock's snapshotted sink list came from the
    manifest's explicit declaration (`false`) or was inferred
    by the install-time defaults (`true`); Si2 uses it to
    detect drift when the underlying grant changes.
  - **No `helpers` / `helper_for` fields** (deferred per row 26).
- **L5.** `.flags` fields — boolean lock-level overrides matching
  the security RFC's loud overrides:
  - `i_know_what_im_doing: bool` (suppresses trifecta refusal +
    env-scrubber strip per §7.1 / §7.4).
  - `allow_credential_paths: bool` (suppresses the carve-out
    refusal class per §7.3 rule 5).
  Both default `false`. Both are surfaced by the broker / runtime
  in `rfl status` (m2+ work; m1 only has to round-trip the field).
- **L6.** *(removed in round 5 per pi review-4 finding 1.)* The
  earlier `.active_bundles` field assumed per-call sandbox
  policy switching, which contradicts `decisions.md` row 17
  and `overview.md` §5.2: v1 unions every granted bundle
  (`default` plus every named bundle in `grant.bundles`)
  into a single spawn-time policy at compile time. There is
  no active-bundle selection knob in v1; per-method
  enforcement above the sandbox is core's responsibility.
  Sink inference still uses the per-tool effective grant
  (`default ∪ <tool-name>`) for metadata snapshotting (Si1)
  — that's a metadata computation, not a runtime sandbox
  switch, so no contradiction.
- **L7.** Identity fields per `[plugin.<canonical-id>]`:
  `digest = "sha256:<hex>"`, `manifest_digest = "sha256:<hex>"`,
  `granted_at` (RFC 3339, parsed with `chrono::DateTime<Utc>`).
- **L8.** Canonical-id parser/formatter
  (`lock::CanonicalId::parse(&str) -> Result<Self, _>` /
  `Display`) with a stable round-trip. The form is
  `<source>:<name>@<version>` where:
  - `source` is a `/`-separated sequence of
    **non-empty** segments matching `[a-z0-9._-]+`
    (`github.com/acme` / `crates.io` / `local` are valid;
    `..`, leading `/`, trailing `/`, double `/`, empty
    segments, and any segment that is `.` or `..` are all
    rejected — pi review-2 finding 1).
  - `name` matches the topic-segment grammar `[a-z0-9_][a-z0-9_-]*`.
  - `version` is parsed via the `semver` crate (not
    re-implemented).
  The compiler additionally **never** uses the canonical id
  literally as a path segment in C5 / C3: the per-plugin
  private state dir uses `<topic-id>` (the hashed form from
  T1, which is path-safe by construction). Pi review-2
  finding 1: the install-layout convention
  `${plugin_root}/<source>:<name>@<version>/` from m2 is
  out of m1 scope; if m2 needs a path-safe layout id, it
  uses the topic-id form.
- **L9.** Round-trip `to_toml` / `from_toml` with deterministic
  ordering (sorted plugin keys, sorted scalar arrays where the
  RFC permits). `serde`-driven so adding a field later doesn't
  churn the formatter.

### T — topic-id derivation (`rafaello_core::topic_id`)

- **T1.** `topic_id::derive(canonical_id: &str) -> String` returning
  `id_<base32-no-pad-lower(sha256(canonical_id))[0..16]>` per
  `decisions.md` row 5 / security RFC §5.1. The 16 chars correspond
  to the leading 80 bits of the digest.
- **T2.** `topic_id::collisions(plugins: &[CanonicalId]) ->
  Result<(), CollisionError>` — checks an entire installed set
  for topic-id duplicates. The carrier is the lock-level
  validation step in V (below); the compiler refuses to compile
  any lock that fails this check.
- **T3.** Topic-id collision **testability** without a
  feature-gated test seam: `topic_id::collisions_with_prefixes(
  pairs: &[(CanonicalId, String)]) -> Result<(), CollisionError>`
  is an **intentional public API** (pi review-2 finding 8 —
  earlier draft's `pub(crate)` wording was inconsistent with
  "reachable from integration tests"). Production code
  `topic_id::collisions(plugins: &[CanonicalId])` computes
  prefixes via `derive(...)` then calls the public helper.
  Integration tests construct synthetic colliding-prefix
  fixtures and call the public helper directly. `derive(...)`
  stays the only hash entry point.

### V — single-plugin and lock-level validation (`rafaello_core::validate`)

Two passes; APIs name every context dependency explicitly (pi-1
finding 4).

- **V1.** `validate::manifest_standalone(manifest: &Manifest) ->
  Result<()>` — per-manifest checks not requiring the canonical
  id:
  - **Pattern-vs-topic discipline** in `bus.{publishes,subscribes}`:
    publishes must be topics (no `*`/`**`), subscribes may be
    patterns. Unknown wildcards (e.g. in-segment `*` like
    `grep.*foo`) rejected.
  - **Topic grammar** segment-by-segment: only `[a-z0-9_-]+`
    per segment (and `*`/`**` permitted only in subscribe-pattern
    positions). Topics and patterns require **at least two
    segments** per security RFC §5.1's `topic := segment ("."
    segment)+` (pi review-5 medium 10): single-segment topics
    like `"core"` rejected as `ValidationError::TopicTooFewSegments`.
  - **Topic-ACL respect** in `bus.publishes` for the
    canonical-id-independent classes: rejects `core.*` and
    `frontend.*` outright.
  - **Reserved built-in renderer kinds** in `[[renderers]]` per
    M7.
  - **`provides.tool.<name>` table optional, defaults
    applied** (pi review-6 finding 5 — overview §15.1 says
    "missing tables default to `{ sinks = None,
    grant_match = absent, always_confirm = false }`", which
    is the ratified normative). Round-6's "must have a
    matching table" wording was stricter than the
    overview; this revision aligns: every tool name in
    `provides.tools` either has a `[provides.tool.<name>]`
    table OR gets the overview's defaults. A
    `[provides.tool.<name>]` table whose name is **not**
    in `provides.tools` is still rejected as
    `UnknownToolTable` (no orphan tool tables).
  - **Sink class** values: must be either a known class
    (`network`/`vcs_push`/`mail`/`workspace_write`/`exec`) or
    match the custom-class grammar `[a-z0-9_]+`.
  - **`network.allow_hosts` requires proxy mode** (pi review-4
    finding 8): for any bundle's
    `[capabilities.<bundle>.network]`, a non-empty
    `allow_hosts` is rejected unless `mode = "proxy"`. Mirror
    of `lockin_config::resolve_network_plan`'s rule; prevents
    a silently-ignored field on hand-edited manifests.
  - **Lazy-load triggers** (`[load]`) cross-validated against
    declared methods/topics/kinds:
    - `command = ["foo"]` trigger referencing a tool not in
      `provides.tools` → rejected.
    - `event = ["x.y"]` trigger requires that **at least one
      pattern in `bus.subscribes` matches the trigger topic**
      under the §5.1 pattern grammar (pi review-3 finding 9 —
      pattern-match, not literal-equality, so a subscribe of
      `core.session.**` covers a load trigger of
      `core.session.started` without forcing the manifest to
      duplicate the literal).
    - `kind = ["k"]` trigger referencing a kind not in
      `[[renderers]]` → rejected.
  - **Capability bundle keys** match `default` or a tool name
    from `provides.tools`.
- **V2.** `validate::manifest_with_id(manifest: &Manifest,
  canonical: &CanonicalId) -> Result<()>` — checks that depend
  on the canonical id:
  - **Self-namespace publish**: `plugin.<topic-id>.*` publishes
    are valid only when the `<topic-id>` matches
    `topic_id::derive(canonical.to_string())`. Foreign topic-ids
    rejected as `PublishOnForeignTopicId`.
  - **Provider namespace publish**: `provider.<id>.*` publishes
    are valid only when `<id>` matches `provides.provider`.
- **V3.** `validate::lock(lock: &Lock, ctx:
  &LockValidationContext) -> Result<()>` where
  ```rust
  pub struct LockValidationContext {
      pub project_root: PathBuf,
      pub home:         PathBuf,
      pub plugin_dirs:  BTreeMap<CanonicalId, PathBuf>,
      pub cache_root:   PathBuf,
      pub state_root:   PathBuf,
  }
  ```
  Per pi review-6 finding 1: V3 is multi-plugin and must
  resolve `${plugin}` per-plugin. The earlier round-6 wording
  used a single `PathContext` (which carried one
  `plugin_dir`) — that was wrong because two installed
  plugins live in distinct package dirs. V3 derives a
  per-plugin `PathContext` from
  `LockValidationContext.plugin_dirs.get(canonical)` when
  delegating to `trifecta::evaluate` / `carveout::compile_against`
  / per-plugin path-escape checks. A canonical id without an
  entry in `plugin_dirs` → `ValidationError::MissingPluginDir`.
  Pi review-5 expanded V3
  to mirror every manifest-side security rule on the lock
  side, since the lock is the runtime authority and a
  hand-edited lock can otherwise bypass install-time checks.
  - **Topic-id collision detection** across the lock entries
    (T2).
  - **Conflicting tool name detection**: any tool name
    appearing in `bindings.tools` of two distinct plugins is a
    hard error unless `[session].tool_owner.<name>` resolves
    the conflict to one specific plugin.
  - **`[session].tool_owner` target integrity** (pi review-5
    finding 2): every entry `tool_owner.<n> = "<canonical>"`
    must reference an installed plugin (parses + present in
    the lock); the referenced plugin's `bindings.tools` must
    contain `<n>`; redundant owner entries (no actual
    conflict for `<n>`) are rejected as
    `ValidationError::ToolOwnerRedundant` so dead state in
    the lock is loud.
  - **Provider activeness consistency**: `[session].provider_active`,
    if present, must reference an installed plugin whose
    `bindings.provider == true` and whose `provider_id` is set.
  - **Lock-side publish authority** (pi review-5 finding 1)
    — mirror of V1's `core.*` / `frontend.*` / `provider.<x>`
    rules, applied to the lock's `.grant.publishes` table:
    `core.*` / `frontend.*` rejected; `plugin.<id>.*` must
    match `topic_id::derive(canonical)` for the lock entry;
    `provider.<id>.*` requires `bindings.provider == true`
    and `bindings.provider_id == Some(id)`.
    `ValidationError::Lock{PublishOnReservedNamespace,
    PublishOnForeignTopicId, ProviderNamespaceMismatch}`.
  - **Lock-side `network.allow_hosts` requires proxy mode**
    (pi review-5 finding 3) — mirror of V1's rule, applied to
    every grant bundle's `network`. Non-empty `allow_hosts`
    with `mode = "deny"` or `"allow_all"` →
    `ValidationError::LockAllowHostsOutsideProxy`.
  - **Lock-side grant bundle key validation** (pi review-6
    finding 3): every key in `.grant.bundles` must be
    `default` or a tool name from `bindings.tools`; unknown
    keys → `ValidationError::LockUnknownBundleKey`. Without
    this rule, a hand-edited `[grant.bundles.typo]` would
    contribute to C2's spawn-time union.
  - **Lock-side capability path templates** (pi review-6
    finding 3): every `read_paths` / `read_dirs` /
    `write_paths` / `write_dirs` / `exec_paths` /
    `exec_dirs` entry in every `.grant.bundles.<n>` is
    re-parsed through `manifest::CapabilityPathTemplate`
    (M11). A bare relative path with no placeholder prefix
    → `ValidationError::LockCapabilityPathRelative`.
  - **Lock-side tool-name and sink-class grammar checks**
    (pi review-6 finding 3): `bindings.tools` values and
    `[session].tool_owner` keys re-validated against the
    M3 tool-name grammar; `bindings.tool_meta.<n>.sinks`
    values re-validated against the M3 sink-class grammar.
  - **`exec_paths` / `exec_dirs` refusal under
    `${project}`** (pi review-6 finding 4 — security RFC
    §6.9). After C3-style placeholder expansion + canonical
    resolution of any existing ancestor, if any
    `exec_paths` or `exec_dirs` entry resolves inside the
    `project_root`, V3 refuses with
    `ValidationError::ExecPathInsideProject`. No override
    flag in v1 (pi: this is a "footgun we don't want even
    the user accidentally enabling"). The check runs both
    on the manifest at install time (V1) and on the lock at
    runtime (V3 — symmetric mirror).
  - **Lock-side binding snapshot validation** (pi review-5
    finding 6):
    - `bindings.tool_meta.<n>.grant_match` (when set) is
      parsed through the same `SafePath` rule as the
      manifest-side field — no traversal in hand-edited
      locks.
    - `bindings.tool_meta` keys must all appear in
      `bindings.tools` (no orphan tool_meta entries).
    - `bindings.provider_id` must satisfy the provider-id
      grammar (single segment) and is `Some(_)` iff
      `bindings.provider == true`; mismatch →
      `ValidationError::ProviderIdInconsistent`.
    - `bindings.renderer_kinds` re-validated against the M7
      grammar (built-ins rejected; plugin kinds prefixed)
      so a hand-edited lock can't smuggle in a built-in
      override even if v1 dispatch is inert.
  - **Trifecta refusal** per security RFC §7.1.1 — V3 calls into
    `trifecta::evaluate` per plugin (Tr below); the typed error
    is `ValidationError::TrifectaRefused` and it carries the
    `(reads_untrusted, has_outbound, has_workspace_write)`
    triple for diagnostics.
  - **Carve-out enforcement** per §7.3 — V3 calls into
    `carveout::compile_against` per plugin against the union
    of all granted bundles; a
    carve-out failure surfaces as
    `ValidationError::CarveOutRefused` /
    `ValidationError::CarveOutTooLarge`.

### C — grant compiler (`rafaello_core::compile`)

The compiler emits a structured **plan**, not a pre-populated
builder (pi review-1 finding 1). m2's plugin supervisor consumes
the plan, starts an outpost proxy if needed, then applies the
plan onto a `lockin::SandboxBuilder` + `SandboxedCommand`.

- **C1.** Public output type:
  ```rust
  pub struct CompiledPlugin {
      pub canonical:        CanonicalId,
      pub topic_id:         String,
      pub entry_absolute:   PathBuf,
      pub filesystem:       FilesystemPlan, // read_paths/dirs, write_paths/dirs, exec_paths/dirs (all absolute)
      pub network:          NetworkPlan,    // Deny | AllowAll | Proxy { allow_hosts: Vec<String> }
      pub env:              EnvPlan,        // pass: Vec<String>, set: BTreeMap<String, String>
      pub limits:           LimitsPlan,
      pub subscribe_patterns: Vec<String>,
      pub publish_topics:     Vec<String>,
      pub auto_subscribes:    Vec<String>,  // self-subscribe: plugin.<topic-id>.tool_request
      pub tool_meta:        BTreeMap<String, ToolMeta>, // only entries this plugin owns post-tool_owner resolution; pi review-2 finding 4
      pub provider_id:      Option<String>,
      pub load:              LoadPolicy,        // pi review-6 finding 2: m2/m3's spawn path needs the load snapshot from the lock, not the live manifest
      pub flags:             CompiledFlags,
  }
  ```
  Conflicting tool names that the lock's
  `[session].tool_owner` resolves *away* from this plugin
  are **filtered out** of `tool_meta` so m2 cannot
  accidentally route them here. The corresponding routing
  table — owner per tool name — lives on `BrokerAcl`
  (G1 below) so m2 has a single authoritative source.
  `NetworkPlan::Proxy` does **not** include a port; m2 starts
  the outpost proxy and pairs the resulting port with the
  plan's `allow_hosts` when materialising the
  `SandboxBuilder`. `EnvPlan` is populated post-scrubbing
  (§Sc) and consumed by m2's call into `lockin_config::apply_env`
  *after* `command(...)` produces the `SandboxedCommand`.
- **C1.1.** **API contract** (pi review-6 medium 7):
  `compile_plugin` and `broker_acl::compile` **require** a
  prior successful `validate::lock(&Lock, &LockValidationContext)`
  on the same `Lock` value. Both functions document this
  precondition and return `CompileError::ValidationNotRun`
  if invariants V3 is supposed to enforce are violated
  (e.g. duplicate topic-id, conflicting tool name without
  resolution, foreign-namespace publish). They do **not**
  re-run V3 internally for performance and to keep one
  validation entry point. Production callers (m2's
  supervisor) call V3 once per lock-load and cache the
  result; m1's tests call V3 in a small per-test setup
  helper.
- **C2.** Public entry point:
  ```rust
  pub fn compile_plugin(
      lock:                &Lock,
      canonical:           &CanonicalId,
      ctx:                 &PathContext,
      recomputed_digests:  &RecomputedDigests,  // see D3
  ) -> Result<CompiledPlugin, CompileError>
  ```
  The compiler unions `default` ∪ every named bundle in
  `grant.bundles` into one spawn-time policy per
  `decisions.md` row 17 / `overview.md` §5.2 (pi review-4
  finding 1 dropped the round-3 `.active_bundles` selection
  knob — there is no per-call sandbox switch in v1). Tests
  that want to exercise scoped-bundle behaviour put per-tool
  authority into named bundles in the fixture lock; the
  compile output reflects the union.
- **C3.** **Placeholder substitution** is invoked once per
  string field, against a `PathContext` carrying:
  `project_root`, `home`, `plugin_dir` (the installed-package
  dir for *this* plugin — m2 picks the on-disk layout; m1
  receives the resolved absolute path), `cache_dir`,
  `state_dir`. Unknown placeholders → `CompileError::UnknownPlaceholder`.
  Post-expansion **path-escape check** (per M11): for every
  placeholder of `${project}` / `${plugin}`, the resolved
  absolute path must remain inside the substituted root.
  Resolver (pi review-5 finding 7 — non-existent leaf paths
  are legitimate for `write_dirs` like `${project}/target`):
  walk the post-expansion path component-by-component;
  canonicalise the longest existing ancestor (with symlink
  resolution + `SymlinkEscape` if the canonical target leaves
  the root); lexically join the non-existent suffix; final
  containment check on the resulting absolute path. Failures
  → `CompileError::PathEscape { field, after_expansion }`.
  This catches `read_dirs = ["${project}/../../etc"]`-style
  escapes the parser couldn't see while still allowing
  `write_dirs = ["${project}/target/non-existent-yet"]`.
- **C4.** **Plan emission order is not a sequence of builder
  calls** (pi review-1 finding 1) — the plan is a value, m2
  picks the application order. The compiler does, however,
  apply a deterministic **post-flatten ordering** to scalar
  arrays inside the plan (sorted by string value, dedup) so
  test fixtures can compare plans byte-equal without relying on
  insertion order.
- **C5.** **Per-plugin private state grant** per security RFC §7.5:
  the compiler unconditionally adds
  `${project}/.rafaello-plugin-data/<topic-id>/` (note:
  the **topic-id** form per pi review-2 finding 1 — not the
  raw canonical id; `<source>:<name>@<version>` is not a safe
  filename) to `filesystem.read_dirs` and
  `filesystem.write_dirs` regardless of grant. This grant is
  **not** counted toward `has_workspace_write` for trifecta
  purposes (Tr4 below).
- **C6.** **Resource limit defaults** per manifest RFC §8.2 #5:
  `max_cpu_time = 300`, `max_open_files = 1024` if the lock
  omits them. Never unbounded. Manifest field `max_cpu_time =
  0` (provider plugins per RFC §9.3) is honoured verbatim and
  *not* overridden by the default — `0` means no limit, and is
  recognised at the plan stage.
- **C7.** **Reserved env-var protection** per security RFC §5.5.1
  (pi review-1 finding 10; pi review-2 finding 5 unified the
  two paths to a single rule):
  - **Compiler stage (C7.1):** `compile_plugin` **rejects**
    any `env.pass` entry that literally names `RFL_BUS_FD` or
    `RFL_PLUGIN`, AND any `env.set` key that literally names
    them — both with `CompileError::ReservedEnvVarRequested`.
    Silent stripping was the round-2 wording for `env.set`;
    pi-2 rightly flagged that as a UX/security smell ("a
    malicious or broken manifest gets silently neutered").
    The compiler now treats both fields the same way: the
    manifest is rejected.
  - **Spawn stage (m2 work, not in m1 scope):** m2 injects the
    core-owned `RFL_BUS_FD` / `RFL_PLUGIN` values onto the
    `SandboxedCommand` *after* `lockin_config::apply_env`
    has applied the user `env` policy.
  m1's scrubber (Sc below) is a separate concern — it strips
  secret-pattern matches from `env.pass`. The env scrubber does
  **not** classify `RFL_BUS_FD` / `RFL_PLUGIN` as secrets;
  they cannot reach the scrubber in normal flow because C7.1
  rejects them upstream.

### D — digest computation (`rafaello_core::digest`)

- **D1.** `digest::content_digest(package_dir: &Path) ->
  Result<String, DigestError>` returns `sha256:<lowercase-hex>`.
  Algorithm: walk `package_dir` deterministically — **files
  only**, sorted by relative path normalised to `/`-separators
  (pi review-2 finding 11: previous wording said "files only"
  AND "empty directories contribute their relative path"; this
  revision picks files-only and drops empty-directory
  contribution, matching the git tree-hash convention). Hash
  each file's relative-path bytes (length-prefixed) followed
  by file-content bytes (length-prefixed); fold into a single
  sha256. **Symlinks** are followed when their target resolves
  inside `package_dir`, and refused when the target resolves
  outside (`DigestError::SymlinkEscape`). Directory-typed
  symlinks are followed; **cycle detection uses a
  recursion-stack** (the set of ancestor canonical paths in
  the current walk) rather than a global visited-set
  (pi review-6 finding 6: a global visited-set would silently
  skip distinct-logical-path / same-canonical-target pairs
  like `vendor_src -> src/`, which violates the
  "deterministic relative-path hashing" model). Cycles
  (canonical target equals an ancestor in the recursion
  stack) → `DigestError::SymlinkCycle { at: PathBuf }`. Two
  distinct in-package paths that point at the same canonical
  directory contribute under **both logical relative paths**
  — the digest is over package contents by relative path,
  not by inode. File permission bits,
  mtimes, and ownership are intentionally **excluded** from
  the digest so it is reproducible across hosts. The exact
  algorithm is pinned in m1's commit body and a doc comment on
  `content_digest`.
- **D2.** `digest::manifest_digest(canonical_bytes: &[u8]) ->
  String` — sha256 of `Manifest::canonical_bytes()` from M9.
- **D3.** Compiler input:
  ```rust
  pub struct RecomputedDigests {
      pub content:  String,   // "sha256:..." from content_digest
      pub manifest: String,   // "sha256:..." from manifest_digest
  }
  ```
  `compile::compile_plugin` (C2) takes a `&RecomputedDigests`
  and refuses with `CompileError::ContentDigestMismatch` /
  `CompileError::ManifestDigestMismatch` if either does not
  match the lock's stored values. The compiler does **not**
  recompute digests itself — that's the m2 install/update
  flow's responsibility; m1 keeps the policy and the
  computation cleanly separable so tests can construct
  matched/mismatched fixtures trivially.

### G — broker ACL extraction (`rafaello_core::broker_acl`)

The compiler's secondary output: a per-session table the m2
broker consumes to authorise publishes / subscribes.

- **G1.** `broker_acl::compile(lock: &Lock) -> Result<BrokerAcl,
  CompileError>` returning:
  - `plugins: BTreeMap<CanonicalId, PluginAcl>` where each
    `PluginAcl` carries `topic_id`, `publish_topics:
    Vec<String>`, `subscribe_patterns: Vec<String>`,
    `auto_subscribes: Vec<String>` (the compiler-inserted
    `plugin.<topic-id>.tool_request` self-subscribe per
    security RFC §5.4), and `provider_id: Option<String>`.
  - `tool_routes: BTreeMap<String, CanonicalId>` — pi review-2
    finding 4 — the **resolved tool-name → owning plugin**
    table. For tool names with a single declarer, the entry is
    that declarer; for tool names with conflicts, the entry is
    `[session].tool_owner.<name>` (V3 rejects unresolved
    conflicts before this code runs). m2's tool dispatcher
    looks up tools here, not in any individual plugin's
    `tool_meta`.
  - For frontends in m1: nothing (TUI is m3).
- **G2.** Topic-grammar revalidation at compile time: every
  publish topic and every subscribe pattern is re-checked
  against the topic / pattern grammars before being emitted
  into the ACL — catches lock corruption / hand-edits.
- **G3.** No live broker code lands in m1; the ACL is a typed
  value the m2 broker consumes.

### Tr — trifecta refusal (`rafaello_core::trifecta`)

Per security RFC §7.1. APIs name every context dependency (pi-1
finding 4).

- **Tr1.** `trifecta::evaluate(lock: &Lock, canonical:
  &CanonicalId, ctx: &PathContext) -> TrifectaState` returning
  the booleans `(reads_untrusted, has_outbound,
  has_workspace_write)` plus `refuse: bool`. The `ctx` is
  required because Tr2 must expand placeholders before deciding
  "outside `${project}`".
- **Tr2.** `reads_untrusted` per §7.1: any of (a)
  `network.mode != "deny"` (across the full bundle union),
  (b) `read_dirs` / `read_paths` (post-expansion) contains a
  path outside `${project}`, (c) `subscribes` matches
  `core.session.tool_result` or `core.session.assistant_message`.
- **Tr3.** `has_outbound`: `network.mode != "deny"` OR a
  one-hop direct check — for any *other* plugin in the lock
  whose subscribe patterns match this plugin's published
  topics, where that other plugin has `network.mode != "deny"`.
- **Tr4.** `has_workspace_write`: `write_dirs` non-empty
  (across the full bundle union) **excluding** the
  per-plugin private-state subtree — this is C5's automatic
  grant added by the compiler, not a `write_dirs` entry in the
  lock; trifecta evaluation runs against the lock entries
  (pre-C5 augmentation), so the exclusion is structural rather
  than a filter.
- **Tr5.** `refuse = reads_untrusted && has_outbound &&
  has_workspace_write && !flags.i_know_what_im_doing`. The
  refusal surfaces as `ValidationError::TrifectaRefused` from
  V3.

### K — carve-out decomposition (`rafaello_core::carveout`)

Per security RFC §7.3 / §7.3.1. Pi review-1 finding 5 forced a
single, consistent rule (read = decompose for project-class /
refuse for credential-class; write = always refuse; both classes
overridable by `allow_credential_paths`).

- **K1.** `carveout::CARVE_OUTS: &[CarveOut]` is the v1 set,
  classified into two classes:
  - **Project-class (decomposable on read, refused on write):**
    - `${project}/rafaello.lock`
    - `${project}/.rafaello/**`
  - **Credential-class (refused on both read and write):**
    - `${home}/.config/rafaello/**`
    - `${home}/.ssh/**`
    - `${home}/.gnupg/**`
    - `${home}/.aws/**`
    - `${home}/.config/gh/**`
    - `${home}/.netrc`
- **K2.** `carveout::compile_against(grant: &GrantBundle,
  canonical: &CanonicalId, ctx: &PathContext,
  allow_credential_paths: bool) -> Result<DecomposedGrant,
  CompileError>`. The rules (one rule per carve-out class +
  grant kind, no silent drops — pi review-2 finding 7):
  - **Reads of project-class carve-outs**: ancestor
    `read_dirs` entries are **decomposed** into immediate
    children of the ancestor minus the carve-out name, bounded
    at 256 synthesised entries (security RFC §7.3 rule 4). Over
    the cap → `CompileError::CarveOutTooLarge`.
  - **Reads of credential-class carve-outs**: ancestor
    `read_dirs` / `read_paths` entries are **refused** with
    `CompileError::CarveOutRefused` unless
    `allow_credential_paths` is set.
  - **Writes covering any carve-out (either class)**: ancestor
    `write_dirs` / `write_paths` entries are **refused** with
    `CompileError::CarveOutRefused` unless
    `allow_credential_paths` is set. Writes are never
    decomposed in v1 — pi-1 finding 5.
  - **Explicit leaf hits on a carve-out path** (e.g. literal
    `read_paths = ["${project}/rafaello.lock"]`): always
    `CompileError::CarveOutRefused` (no silent drop, no
    diagnostic-list field — pi review-2 finding 7 rejected the
    earlier "drop with diagnostic" wording as too surprising).
    `allow_credential_paths` overrides for both classes.
  - With `allow_credential_paths = true`, the broad grant is
    emitted verbatim (no decomposition, no refusal) and the
    flag is recorded in the compiled plan's `flags` so m2's
    `rfl status` can render the loud override (m2 work).
- **K3.** **Hidden-directory rule** (§7.3.1): the default
  workspace grant `${project}` (when present in the
  lock's `read_dirs`) is decomposed at compile time into the
  immediate non-hidden children of the project root. The
  per-plugin private state dir (C5) is added to `read_dirs` /
  `write_dirs` separately by the compiler — *not* by the
  carve-out module.
- **K4.** Decomposition is **snapshotted into the compiled
  plan**, not a live filter. Re-decomposition on lock change
  is the m2 install/update flow's job.

### Sc — env scrubber (`rafaello_core::scrubber`)

Per security RFC §7.4.

- **Sc1.** `scrubber::SECRET_PATTERNS: &[&str]` is the v1
  glob set: `*_TOKEN`, `*_SECRET`, `*_KEY`, `*_PASSWORD`,
  `AWS_*`, `GITHUB_TOKEN`, `OPENAI_*`, `ANTHROPIC_*`.
- **Sc2.** `scrubber::strip(env_pass: &[String], i_know_what_im_doing:
  bool) -> Vec<String>` returns the scrubbed list. With the
  override flag, returns `env_pass` unchanged.
- **Sc3.** Reserved env vars `RFL_BUS_FD` and `RFL_PLUGIN`
  are **removed by the compiler in C7.1 before the scrubber
  runs** (pi review-1 finding 10). Sc tests therefore cover
  the scrubber in isolation; the C7 tests cover the
  reserved-var protection. There is no test that lists
  reserved vars in `env_pass` and then asserts they "survive
  the scrubber" (the round-1 wording was wrong; this revision
  removes that test row from the matrix).

### Si — sink-default inference (`rafaello_core::sinks`)

Per security RFC §7.2.5 (pi review-1 finding 9 — was implicit in
round 1; this revision makes it explicit).

- **Si1.** `sinks::infer_defaults(effective: &GrantBundle,
  declared: &Option<Vec<String>>) -> Vec<String>` returns the
  snapshotted sink list when the manifest omits `sinks`. The
  `effective` parameter is the **per-tool effective grant**:
  for tool `<n>`, that's `default ∪ <n>` per `decisions.md`
  row 17 — pi review-3 finding 3 forced this commitment so
  inference cannot miss capabilities that arrive only via the
  tool-named bundle. Defaults per the security RFC table:
  - `network.mode != "deny"` → includes `"network"`,
  - `write_dirs` non-empty (excluding private state) →
    includes `"workspace_write"`,
  - both → both,
  - neither → `[]`.
  When `declared` is `Some(_)`, the declared list wins
  verbatim (no inference).
- **Si2.** The lock-level pass `validate::lock` (V3) runs the
  drift check for every `bindings.tool_meta.<n>` with
  `sinks_inferred = true` (regardless of whether the
  snapshotted `sinks` list is empty — pi review-3 finding 3
  fixed the round-3 contradiction): recompute
  `sinks::infer_defaults(effective_for_<n>, &None)`, compare
  byte-equal with the snapshotted `sinks`, and on mismatch
  return `ValidationError::SinkInferenceDrift { tool, expected,
  found }`. Locks with `sinks_inferred = false` are not drift-
  checked — the manifest's explicit declaration won at install
  time and stays authoritative.

### W — fittings `MethodNotFound` typed-method cutover (row 36)

Pi review-2 finding 3: row 36's "deferred to m1" framing
demands a definite landing decision. **Decision: in scope for
m1**, as a small Stream B cutover commit landing on
`rafaello-v0.1` alongside the `rafaello-core` work. Listed as a
distinct W section so it is unambiguous in the Out of Scope
list (it isn't there) and in `commits.md` (it gets its own
named commit). Does **not** depend on any `rafaello-core`
module; can land at any point in m1's commit sequence.

- **W1.** `fittings_core::error::FittingsError::MethodNotFound`
  gains a typed `method: Option<String>` field per
  `streams/b-fittings/rfc-fittings-errors.md` :71-79 / :202 /
  :239-244 and `decisions.md` row 36. The new shape:
  ```rust
  MethodNotFound { method: Option<String>, message: String, data: Option<Value> }
  ```
- **W2.** `fittings_wire::error_map` extracts/synthesises
  `data.method` from the typed field on encode and recovers
  the typed field from `data.method` on decode. Conflict
  semantics (pi review-5 medium 11):
  - **Encode** with `method = Some(name)`: synthesised
    `data.method = name` always wins. If caller-supplied
    `data` already had a `method` key, it is **overwritten**
    (typed-field precedence). The `data` value's other keys
    are preserved.
  - **Encode** with `method = None`: no synthesis. If
    caller-supplied `data` carries an opaque `method` key,
    it is preserved verbatim (round-trip back through
    decode produces `method: None` plus the `data` keys).
  - **Decode**: extract `data.method` if it parses as a
    string; otherwise `method = None`. The `data` field
    keeps the rest of the JSON object verbatim.
  Round-trip preserves the typed value.
- **W3.** Existing one-arg constructor
  `FittingsError::method_not_found(message)` keeps working and
  sets `method: None`, `data: None` (no churn for current call
  sites; m0 retrospective row 36's source-breaking concern is
  addressed by keeping the constructor signature).
- **W4.** New constructor
  `FittingsError::method_not_found_with_method(method,
  message)` for callers that have the method name to attach
  (the dispatcher's "no such method" path).
- **W5.** Test `fittings/tests/method_not_found_typed_method_round_trip.rs`:
  table-driven coverage of (a) `None` round-trip; (b) `Some(name)`
  → encode populates `data.method` → decode recovers
  `Some(name)`; (c) interaction with caller-supplied `data`
  (existing `data` keys preserved alongside the synthesised
  `method` key); (d) the existing one-arg constructor still
  builds `MethodNotFound { method: None, ... }`.

### E — typed errors (`rafaello_core::error`)

- **E1.** All m1 error types are `thiserror`-driven enums.
  Top-level `Error` (re-exported as `rafaello_core::Error`)
  unifies the module-local errors: `ManifestError`,
  `LockError`, `ValidationError`, `CompileError`,
  `DigestError`, `CarveOutError`, `TrifectaError`. Every
  variant carries enough structured context that m2's CLI
  layer can pretty-print without re-stringly-matching.

## Out of scope

- **Anything that runs.** No tokio reactor, no fittings client
  / server wiring, no `rfl` binary changes, no broker, no
  plugin spawn, no outpost proxy startup, no env application
  to `SandboxedCommand`.
- **`rfl install` / `rfl grant` / `rfl revoke` / `rfl update`
  CLI subcommands.** The lock has loaders + serialisers; the
  install/update flow that prompts for diffs and writes the
  lock is m2+. m1 fixtures construct locks programmatically.
- **Re-confirmation UX on digest mismatch.** The compiler
  flatly refuses; the diff-and-prompt flow is m2+.
- **Subprocess-renderer manifest dispatch.** Per
  `decisions.md` row 29, v1 ships only built-in renderers; m1
  rejects `[[renderers]]` entries that name a built-in kind
  (M7) and accepts plugin-prefixed kinds (`mermaid:diagram`)
  for forward-compat, but emits no dispatch wiring.
- **Helper plugins (`helper_for`, `RFL_HELPER_FD`).** Per
  `decisions.md` row 26, the field is rejected at parse time
  (M2); the v2 spec stays in security RFC §7.4.1 untouched.
- **External UDS-attached frontends.** Per `decisions.md`
  row 27.
- **Streaming entry patch ops.** Per `decisions.md` row 28.
- **Full openrpc.json validation.** m1 only checks the
  sibling file's presence (M10).
- **Stream F RFC body edits.** The §15.1 normative-delta
  items 1–4 + the security RFC's `requires_confirmation` →
  `always_confirm` rename + helper / external-attach drift
  are documented in `overview.md` and `decisions.md`; m1
  retrospective patches the RFCs per `plans/milestones/README.md`
  §"Stream RFC drift".
- (No item — `FittingsError::MethodNotFound` typed-method
  cutover is **in scope** for m1 per the W section above; pi
  review-2 finding 3 forced a definite call.)
- **Capsa backend.** Manifest RFC §10 stays paper-only.
- **Network proxy startup, port assignment, outpost wiring.**
  m1's `NetworkPlan::Proxy` records the `allow_hosts` list;
  m2's supervisor calls `outpost::NetworkPolicy::from_allowed_hosts`
  + starts the proxy + calls `SandboxBuilder::network_proxy(port)`.

## Demo bar

Per the milestones README m1 row, the demo bar is `cargo test
-p rafaello-core` integration coverage. The matrices below name
every required test file. Tests may be split or merged during
`commits.md` drafting as long as every named behaviour is
exercised at least once.

> **Note on fixture provenance.** Where the test description says
> "RFC §9.x-shaped manifest", the fixture is a **post-simplification
> rewrite** of the RFC example: `runtime` removed (row 30), `[rpc]`
> replaced by an `openrpc.json` sibling (row 31), `${secret:...}`
> in `env.set` removed (closed-set placeholders only — M8), the
> required `rafaello = ">=0.1, <0.2"` field added if the RFC
> example omitted it. The RFC body itself stays as the historical
> artefact (per `plans/README.md` §"Authoring conventions");
> patching the RFC to the post-row-30/31/32 schema is m1
> retrospective work. Pi review-1 finding 7 forced this clarification.

### Positive integration tests in `rafaello/crates/rafaello-core/tests/`

| Test file | Exercises |
|-----------|-----------|
| `manifest_parse_minimal.rs` | Smallest valid manifest (`schema`, `name`, `version`, `entry`, `rafaello`, empty `[provides]`, no capabilities) round-trips through `Manifest::parse` and `Manifest::canonical_bytes`. |
| `manifest_parse_tool_example.rs` | Post-simplification rewrite of RFC §9.1 (`rust-tools`-shaped) decodes; `provides.tools = ["format", "check"]` (single-segment names per pi-1 finding 3); scoped `[capabilities.format.filesystem]` (bundle key matches a tool name); `[load]` table; `[bus]` populate the typed structs. |
| `manifest_parse_provider_example.rs` | Post-simplification rewrite of RFC §9.3 (`anthropic`-shaped) decodes; `provides.provider = "anthropic"`; `eager` load string; `[capabilities.default.network]` with `proxy` mode + `allow_hosts`; `env.set` keys are literal strings only (no `${secret:...}`). |
| `manifest_parse_renderer_example.rs` | Post-simplification rewrite of RFC §9.2 decodes; `[[renderers]]` list of two prefixed non-built-in kinds (`mermaid:diagram`, `diff:code` — both Stream-E-prefixed per M7 / pi-4 finding 7); registration accepted. |
| `tool_meta_always_confirm_round_trip.rs` | A tool with `[provides.tool.<n>] always_confirm = true` round-trips manifest → lock → `CompiledPlugin.tool_meta.<n>.always_confirm == true` (M3 / L4 / C1 — pi-4 finding 6: load-bearing for m5's confirmation gate, must not be quietly dropped during projection). |
| `manifest_canonical_bytes_stable.rs` | `Manifest::canonical_bytes` is byte-stable across two parses of the same TOML re-emitted with key reordering and trivial whitespace differences. |
| `manifest_placeholder_expansion.rs` | All five placeholders expand against a hand-built `PathContext`; deeply-nested mixes (e.g. `${project}/sub/${plugin}/foo`) resolve correctly. |
| `manifest_validate_load_trigger_cross_refs.rs` | `[load]` table referencing only declared methods/topics/kinds passes `validate::manifest_standalone`. |
| `manifest_openrpc_sibling_present.rs` | `validate_with_package` succeeds against a fixture directory containing a non-empty `openrpc.json` next to the manifest. |
| `lock_parse_round_trip.rs` | Worked-example lock (mirroring security RFC §3.2 + L2/L3's expansions for `entry`, `grant.bundles`, `[session].tool_owner`) parses, serialises, parses again byte-equal. Includes `[session].provider_active`, `.flags`, `bindings.tool_meta`. |
| `lock_canonical_id_round_trip.rs` | `CanonicalId::parse("github:acme/grep@1.4.2").to_string() == input` over a small grammar matrix (different `source` shapes, semver pre-release / build metadata). |
| `topic_id_derivation.rs` | Deterministic for a fixed input: e.g. `derive("github:acme/grep@1.4.2") == "id_<known-prefix>"`; the test pins the expected first 16 chars of base32-no-pad-lowercase against a hand-computed fixture. |
| `topic_id_collision_detection.rs` | Pre-computed-prefix helper (`collisions_with_prefixes`) rejects two distinct canonical ids paired with identical prefixes (no test seam in `derive` itself; T3). |
| `compile_default_bundle.rs` | Worked lock entry with `default` bundle compiles; resulting `CompiledPlugin` carries the expected `FilesystemPlan`, `NetworkPlan`, `EnvPlan`, `LimitsPlan` (post-default-fill), `entry_absolute`, `topic_id`, etc. Asserts on the structured plan, not on builder calls. |
| `compile_scoped_bundle_union.rs` | Lock specifies a `default` bundle plus a `format` named bundle; resulting plan reflects the union of both (default's reads + format's writes) per `decisions.md` row 17 (single spawn-time policy, no per-call switch — pi review-4 finding 1); duplicate entries dedup; ordering deterministic per C4. |
| `compile_placeholder_resolves_to_absolute.rs` | All paths in the compiled plan are absolute; `${...}` is fully expanded. |
| `compile_private_state_grant.rs` | Compiled plan contains `${project}/.rafaello-plugin-data/<topic-id>/` (after expansion to absolute) in both `read_dirs` and `write_dirs` regardless of whether the lock requested it. (**Topic-id form**, not raw canonical id — C5 + pi review-3 finding 5.) |
| `compile_private_state_excluded_from_workspace_write.rs` | Trifecta evaluation against a lock whose only writes would be the private-state dir reports `has_workspace_write == false` (Tr4 — the structural exclusion). |
| `compile_resource_limit_defaults.rs` | Lock omits `limits`; compiled plan carries `max_cpu_time = 300`, `max_open_files = 1024`. A lock that explicitly sets `max_cpu_time = 0` (provider plugin shape) preserves the `0` verbatim. |
| `compile_network_proxy_plan.rs` | Lock specifies `network.mode = "proxy"`, `allow_hosts = ["api.example.com", "*.example.com"]`; compiled `NetworkPlan::Proxy { allow_hosts }` records the host list verbatim (m2 starts the outpost proxy and supplies the port; m1 only emits the plan). |
| `compile_env_set_passes_through.rs` | Lock's `env.set = { CARGO_TERM_COLOR = "always" }` reaches `EnvPlan.set` verbatim. (Reserved-key rejection is exercised by `compile_reserved_env_in_set.rs` in the negative matrix per C7's unified-rejection rule.) |
| `compile_digest_match.rs` | `RecomputedDigests` matches the lock's stored digests; compile succeeds; mismatched values are exercised in the negative matrix below. |
| `broker_acl_extraction.rs` | Two-plugin lock; `broker_acl::compile` emits per-plugin `publish_topics`, `subscribe_patterns`, the auto-inserted `plugin.<topic-id>.tool_request` self-subscribe, and the bound provider id (for the provider plugin). |
| `carveout_default_workspace_decomposition.rs` | `read_dirs = ["${project}"]` decomposes to immediate non-hidden children of a fixture project root (K3); the per-plugin private state dir is added separately by C5 (also asserted via the `compile_private_state_grant.rs` test). |
| `carveout_workspace_excludes_rafaello_dot_dirs.rs` | `read_dirs = ["${project}"]` against a project that contains `.rafaello/` decomposes around the carve-out (no entry covering `.rafaello/`). |
| `digest_content_deterministic.rs` | Two invocations of `content_digest` against the same fixture tree return the same value; constructing the tree in a different order doesn't change the digest. |
| `trifecta_two_plugins_one_hop.rs` | Plugin A has workspace_write + reads_untrusted but `network.mode = "deny"`; plugin B subscribes to A's published topic and has `network.mode = "proxy"`. A's `has_outbound` evaluates true via the one-hop check; combined with A's other booleans, the trifecta refusal fires. |
| `trifecta_iknowwhatimdoing_bypass.rs` | Same fixture; `flags.i_know_what_im_doing = true`; trifecta refusal suppressed; compile succeeds. |
| `sinks_infer_defaults.rs` | Tools whose manifest omits `sinks` (`sinks: None` per M3): a write-only grant infers `["workspace_write"]`; a network-only grant infers `["network"]`; both → both classes; neither → `[]` (Si1). A second case where the manifest declares `sinks: Some(vec![])` (explicit empty) is preserved verbatim — no inference applied. |
| `sinks_infer_from_named_bundle.rs` | A tool `format` whose `default` bundle has no network/write authority but whose `[capabilities.format.filesystem] write_dirs = ...` adds workspace_write authority: inference (Si1's `effective = default ∪ format`) yields `["workspace_write"]`. Pi review-3 finding 3 — covers the row 17 ∪ flatten path. |
| `manifest_load_event_pattern_match.rs` | `bus.subscribes = ["core.session.**"]`, `[load] event = ["core.session.started"]`: V1 accepts (the subscribe pattern matches the trigger topic — pi review-3 finding 9). A second case where `event = ["unrelated.x"]` with the same subscribes is rejected. |
| `sinks_inferred_flag_round_trips.rs` | A lock entry with `bindings.tool_meta.<n>.sinks_inferred = true` and a matching `sinks` snapshot round-trips through TOML serialise/parse byte-equal (L4). |
| `manifest_grant_match_present.rs` | `validate_with_package` succeeds when every `[provides.tool.<name>]` table that sets `grant_match` resolves to an existing regular file inside the package dir (M10). |
| `broker_acl_tool_owner_resolves_routing.rs` | Two-plugin lock both claiming `grep`; `[session].tool_owner.grep = "<plugin-A>"`. `broker_acl::compile`'s `tool_routes["grep"]` equals `<plugin-A>` and the losing plugin's `CompiledPlugin.tool_meta` does not contain `"grep"` (G1, C1 — pi review-2 finding 4). |
| `compile_network_proxy_allow_hosts_validates.rs` | Compile-time dry-run via `outpost::NetworkPolicy::from_allowed_hosts(...)` accepts the worked-example proxy `allow_hosts` list (`["api.example.com", "*.example.com"]`); m1 emits `NetworkPlan::Proxy { allow_hosts }` with the list verbatim (Risks §2). |
| `env_scrubber_strips_known_secrets.rs` | `env_pass = ["GITHUB_TOKEN", "OPENAI_API_KEY", "AWS_REGION", "PATH"]`; scrubbed list contains only `["PATH"]` (`GITHUB_TOKEN` matches literally; `OPENAI_API_KEY` matches `OPENAI_*`; `AWS_REGION` matches `AWS_*`). |
| `method_not_found_typed_method_round_trip.rs` | (W5 — lives under `fittings/tests/`, not `rafaello-core/tests/`.) Table-driven coverage of `MethodNotFound { method, message, data }`: `None` round-trip, `Some(name)` synthesised into `data.method` and recovered, interaction with caller-supplied `data` (existing keys preserved), one-arg constructor `method_not_found(msg)` builds `method: None`. |

### Negative integration tests in `rafaello/crates/rafaello-core/tests/`

| Test file | Asserts |
|-----------|---------|
| `manifest_unknown_field.rs` | A manifest with a top-level `purpose = "foo"` is rejected as `ManifestError::UnknownField`; same for an unknown nested key under `[provides]`. |
| `manifest_legacy_runtime_field.rs` | `runtime = "subprocess"` rejected as `ManifestError::ReservedField { field: "runtime" }` per `decisions.md` row 30. |
| `manifest_legacy_rpc_block.rs` | `[rpc] openrpc = "x"` rejected as `ManifestError::ReservedField { field: "rpc" }` per `decisions.md` row 31. |
| `manifest_helper_for_field.rs` | Top-level `helper_for = "..."` rejected as `ManifestError::ReservedField` per `decisions.md` row 26. |
| `manifest_publishes_core_topic.rs` | `bus.publishes = ["core.session.tool_result"]` rejected as `ValidationError::PublishOnReservedNamespace` (caught by V1 — canonical-id-independent). |
| `manifest_publishes_other_plugin_namespace.rs` | `bus.publishes = ["plugin.id_aaaaaaaaaaaaaaaa.foo"]` from a plugin whose own topic-id resolves to a different prefix is rejected by `validate::manifest_with_id` (V2 — needs the canonical id). |
| `manifest_publishes_frontend_topic.rs` | `bus.publishes = ["frontend.tui.user_message"]` from a non-frontend plugin rejected as `ValidationError::PublishOnFrontendNamespace`. |
| `manifest_provider_namespace_mismatch.rs` | A plugin declaring `provides.provider = "anthropic"` but listing `bus.publishes = ["provider.openai.tool_request"]` rejected as `ValidationError::ProviderNamespaceMismatch` (V2). |
| `manifest_publish_with_wildcard.rs` | `bus.publishes = ["plugin.id_xxxx.*"]` rejected as `ValidationError::PatternInPublishPosition`. |
| `manifest_subscribe_invalid_pattern.rs` | `bus.subscribes = ["core.**.tool_*"]` (in-segment `*`) rejected as `ValidationError::InvalidPatternSegment`. |
| `manifest_topic_segment_grammar.rs` | A topic / pattern segment violating `[a-z0-9_-]+` (uppercase, dot inside segment, slash) rejected as `ValidationError::IllegalTopicSegment`. |
| `manifest_dotted_tool_name.rs` | `provides.tools = ["rust.format"]` rejected as `ValidationError::IllegalToolName` (V1 — tool names are single segments per pi-1 finding 3). |
| `manifest_unknown_tool_table.rs` | `provides.tools = ["grep"]` but a `[provides.tool.gerp]` table (typo) → `ValidationError::UnknownToolTable`. (Note: missing tables for declared tools no longer error per pi-6 finding 5 — see `tool_table_omitted_uses_defaults.rs` in the positive matrix.) |
| `lock_unknown_bundle_key.rs` | Lock `[grant.bundles.typo]` for a key not in `default ∪ bindings.tools` → `ValidationError::LockUnknownBundleKey` (pi-6 finding 3). |
| `lock_capability_path_relative.rs` | Lock `read_dirs = ["relative/path"]` (no placeholder prefix, not absolute) → `ValidationError::LockCapabilityPathRelative` (pi-6 finding 3). |
| `lock_bindings_tools_invalid_grammar.rs` | Lock `bindings.tools = ["Rust-Tools"]` (uppercase) rejected per V3's tool-name grammar mirror (pi-6 finding 3). |
| `lock_tool_meta_invalid_sink.rs` | Lock `bindings.tool_meta.<n>.sinks = ["Network"]` (uppercase, fails sink-class grammar) rejected by V3 (pi-6 finding 3). |
| `manifest_exec_path_inside_project.rs` | Manifest `exec_paths = ["${project}/scripts/runner"]` rejected as `ValidationError::ExecPathInsideProject` per security RFC §6.9 (pi-6 finding 4). |
| `lock_exec_path_inside_project.rs` | Lock `exec_dirs = ["${project}/bin"]` rejected by V3 mirror (pi-6 finding 4). |
| `compile_without_validate_lock_errors.rs` | Calling `compile_plugin` against a lock with a topic-id collision when `validate::lock` was not called first → `CompileError::ValidationNotRun` (pi-6 medium 7 — API contract). |
| `manifest_unknown_bundle_key.rs` | `[capabilities.foo.filesystem]` for a bundle key not in `provides.tools` (and not `default`) → `ValidationError::UnknownBundleKey`. |
| `manifest_malformed_sinks.rs` | `[provides.tool.foo] sinks = [42]` (non-string), and `sinks = ["Network"]` (uppercase, fails the `[a-z0-9_]+` custom-class grammar): both rejected. |
| `manifest_reserved_renderer_kind.rs` | `[[renderers]] kind = "text"` rejected per M7 (built-ins reserved). |
| `manifest_load_trigger_unknown_command.rs` | `[load] command = ["does_not_exist"]` rejected as `ValidationError::LoadTriggerUnknownCommand`. |
| `manifest_missing_openrpc_sibling.rs` | `provides.tools = ["grep"]` but no `openrpc.json` next to the manifest → `ManifestError::MissingOpenRpc`. |
| `lock_unknown_field.rs` | A lock with an unknown field under `.bindings` is rejected (`deny_unknown_fields`). |
| `lock_canonical_id_invalid.rs` | `[plugin."github:acme/Grep@1.4"]` (uppercase in `name`, missing patch) rejected by `CanonicalId::parse`. |
| `lock_helper_field_rejected.rs` | A lock containing `bindings.helpers = [...]` or `bindings.helper_for = "..."` is rejected (deferred per row 26 — both directions of the helper relationship). |
| `lock_provider_active_unknown.rs` | `[session].provider_active` referencing a plugin not present in the lock → `ValidationError::ProviderActiveUnknown`. |
| `lock_provider_active_not_provider.rs` | `[session].provider_active` references an installed plugin whose `bindings.provider == false` → `ValidationError::ProviderActiveNotProvider`. |
| `lock_conflicting_tool_names.rs` | Two installed plugins both listing `"grep"` in `bindings.tools` without a `[session].tool_owner.grep` decision → `ValidationError::ConflictingToolName`. A second variant with `[session].tool_owner.grep = "<plugin-A>"` resolves the conflict; only the named plugin gets the routing. |
| `lock_missing_entry.rs` | Lock entry without `entry` field → `LockError::MissingEntry` (per L2 — required for the compiler's `entry_absolute`). |
| `lock_entry_traversal.rs` | Lock `.entry = "../evil"` rejected by `Lock::parse` (L2 + M11 `SafePath`); `.entry = "/abs/path"` rejected; `.entry = "ok//double-slash"` rejected. |
| `lock_entry_not_found.rs` | Lock `.entry = "bin/missing"` against a fixture `plugin_dir` that contains no such file → `CompileError::EntryNotFound` (L2 — compile-time check on the lock snapshot, not just install-time M10). |
| `lock_entry_escape_via_symlink.rs` | Lock `.entry = "bin/wat"` resolves through a symlink whose target is outside `plugin_dir` → `CompileError::EntryEscape` (L2). |
| `lock_entry_is_directory.rs` | Lock `.entry = "bin"` points at a directory rather than a regular file → `CompileError::EntryNotFile` (L2). |
| `manifest_invalid_name.rs` | Manifest `name = "Rust-Tools"` (uppercase), `name = "rust/tools"` (slash), `name = "rust.tools"` (dot), `name = ""` (empty): all rejected by V1 (M1's name grammar — pi review-3 finding 8). |
| `manifest_unprefixed_renderer_kind.rs` | `[[renderers]] kind = "code.diff"` (no `<vendor>:` prefix) rejected as `ValidationError::UnprefixedRendererKind` per M7 / pi-4 finding 7. |
| `manifest_allow_hosts_outside_proxy.rs` | `[capabilities.default.network] mode = "deny", allow_hosts = ["x.example"]` (or `mode = "allow_all"` with non-empty `allow_hosts`) rejected as `ValidationError::AllowHostsOutsideProxy` (pi-4 medium finding 8 — mirror of `lockin_config::resolve_network_plan`'s rule so a hand-edited manifest can't have a silently-ignored field). |
| `manifest_topic_too_few_segments.rs` | `bus.publishes = ["core"]` (single segment) rejected as `ValidationError::TopicTooFewSegments` per security RFC §5.1 (pi-5 medium 10). |
| `lock_publishes_core_topic.rs` | A lock with `.grant.publishes = ["core.session.tool_result"]` rejected by V3's lock-side publish ACL (pi-5 finding 1) — mirror of `manifest_publishes_core_topic.rs`. |
| `lock_publishes_frontend_topic.rs` | Lock `.grant.publishes = ["frontend.tui.user_message"]` rejected by V3 (pi-5 finding 1). |
| `lock_publishes_other_plugin_namespace.rs` | Lock `.grant.publishes = ["plugin.id_aaaa....foo"]` whose `<topic-id>` doesn't match the lock entry's derived topic-id rejected as `ValidationError::LockPublishOnForeignTopicId` (pi-5 finding 1). |
| `lock_provider_namespace_mismatch.rs` | Lock entry with `bindings.provider_id = "anthropic"` but `.grant.publishes = ["provider.openai.tool_request"]` → `ValidationError::LockProviderNamespaceMismatch` (pi-5 finding 1). |
| `lock_allow_hosts_outside_proxy.rs` | Lock grant bundle with `network.mode = "deny"` and `allow_hosts = ["x.example"]` → `ValidationError::LockAllowHostsOutsideProxy` (pi-5 finding 3). |
| `lock_tool_owner_unknown_plugin.rs` | `[session].tool_owner.grep = "github:nope/unknown@1.0.0"` referencing an uninstalled plugin → `ValidationError::ToolOwnerUnknownPlugin` (pi-5 finding 2). |
| `lock_tool_owner_plugin_does_not_declare_tool.rs` | `tool_owner.grep = "<plugin-A>"` but plugin A's `bindings.tools` does not contain `"grep"` → `ValidationError::ToolOwnerPluginDoesNotDeclareTool` (pi-5 finding 2). |
| `lock_tool_owner_redundant.rs` | `tool_owner.grep = "<plugin-A>"` when only plugin A claims `"grep"` (no actual conflict) → `ValidationError::ToolOwnerRedundant` (pi-5 finding 2 — rejecting dead state in the lock). |
| `lock_tool_meta_grant_match_traversal.rs` | Lock `bindings.tool_meta.<n>.grant_match = "../../escape.json"` rejected by V3's lock-side `SafePath` re-validation (pi-5 finding 6). |
| `lock_tool_meta_orphan.rs` | `bindings.tool_meta.foo` exists but `bindings.tools` doesn't contain `"foo"` → `ValidationError::OrphanToolMeta` (pi-5 finding 6). |
| `lock_provider_id_inconsistent.rs` | `bindings.provider = false` but `bindings.provider_id = Some("...")`, or `provider = true` with `provider_id = None` → `ValidationError::ProviderIdInconsistent` (pi-5 finding 6). |
| `lock_renderer_kind_unprefixed.rs` | Lock `bindings.renderer_kinds = ["code.diff"]` (unprefixed plugin kind) rejected per V3's M7 mirror (pi-5 finding 6). |
| `lock_renderer_kind_builtin.rs` | Lock `bindings.renderer_kinds = ["text"]` (built-in name) rejected per V3's M7 mirror (pi-5 finding 6). |
| `compile_capability_path_nonexistent_write_leaf.rs` | A grant `write_dirs = ["${project}/target/new"]` where the `target` directory exists but `new` does not — compiles successfully (pi-5 finding 7 resolver: existing-ancestor canonicalisation + lexical join + containment). |
| `compile_capability_path_symlink_ancestor_escape.rs` | A grant `read_dirs = ["${project}/symlinked"]` where `${project}/symlinked` is a symlink whose canonical target is outside `${project}` → `CompileError::SymlinkEscape` (pi-5 finding 7). |
| `validate_lock_multiplugin_context.rs` | `validate::lock` with a `LockValidationContext` whose `plugin_dirs` map carries distinct paths for two installed plugins resolves each plugin's `${plugin}` correctly; a `plugin_dirs` map missing one of the canonical ids → `ValidationError::MissingPluginDir` (pi-6 finding 1). |
| `lock_load_policy_round_trip.rs` | A lock with `bindings.load = { kind = ["mermaid:diagram"] }` parses, serialises, parses again byte-equal; `CompiledPlugin.load == LoadPolicy::Lazy { kind: vec!["mermaid:diagram"], ... }` (pi-6 finding 2 — L4 + C1 `LoadPolicy` snapshot). |
| `lock_load_policy_eager_string.rs` | A lock with `bindings.load = "eager"` round-trips; compiled `LoadPolicy::Eager`. |
| `tool_table_omitted_uses_defaults.rs` | A manifest with `provides.tools = ["grep"]` and **no** `[provides.tool.grep]` table parses, validates, and lock-snapshots `tool_meta.grep = ToolMeta { sinks: vec![], sinks_inferred: true (per Si1 inference), grant_match: None, always_confirm: false }` per overview §15.1 defaults (pi-6 finding 5 — round-6 alignment). |
| `digest_distinct_paths_same_target.rs` | A package containing `src/lib.rs` and `vendor_src/` (where `vendor_src -> src`): `content_digest` hashes the file contents under both `src/lib.rs` and `vendor_src/lib.rs` (recursion-stack cycle detection allows distinct logical paths sharing a canonical target — pi-6 finding 6). |
| `manifest_missing_openrpc_provider.rs` | Provider plugin manifest (no `provides.tools`, has `provides.provider = "anthropic"`) but no `openrpc.json` next to it → `ManifestError::MissingOpenRpc` (M10 + pi review-3 finding 4: row 31 requires the sibling for every plugin, not just tool plugins). |
| `digest_symlink_cycle.rs` | `package_dir` contains `loop -> .` (or `a -> b`, `b -> a`); `content_digest` returns `DigestError::SymlinkCycle` (D1 — pi review-3 finding 7). |
| `topic_id_collision_at_lock.rs` | Two distinct canonical ids whose pre-computed prefixes (passed via `collisions_with_prefixes`) match → `CollisionError`. |
| `digest_content_mismatch.rs` | Lock's `digest` field doesn't match the recomputed `content_digest`; `compile::compile_plugin` returns `CompileError::ContentDigestMismatch`. |
| `digest_manifest_mismatch.rs` | Lock's `manifest_digest` doesn't match recomputed value; `CompileError::ManifestDigestMismatch`. |
| `digest_symlink_escape.rs` | A symlink in the package directory pointing outside the package root → `DigestError::SymlinkEscape`. |
| `carveout_credential_path_refused_read.rs` | `read_dirs = ["${home}"]` covering `~/.ssh` with `allow_credential_paths = false` → `CompileError::CarveOutRefused` (K2 — credential class refuses on read). |
| `carveout_credential_path_refused_write.rs` | `write_dirs = ["${home}"]` covering `~/.ssh` with `allow_credential_paths = false` → `CompileError::CarveOutRefused`. |
| `carveout_project_write_refused.rs` | `write_dirs = ["${project}"]` (would cover `rafaello.lock` and `.rafaello/`) with `allow_credential_paths = false` → `CompileError::CarveOutRefused` (K2 — project-class also refuses on write; pi-1 finding 5 forced this consistency). |
| `carveout_credential_path_override.rs` | The two `_refused_*` variants above with `allow_credential_paths = true` compile; resulting plan records the broad grants verbatim and surfaces the loud-override flag. |
| `carveout_decomposition_blowup.rs` | A project root containing 300 immediate children plus `.rafaello/`; `read_dirs = ["${project}"]` decomposition exceeds the 256-entry cap → `CompileError::CarveOutTooLarge`. |
| `carveout_lockfile_path_explicit.rs` | An explicit `read_paths = ["${project}/rafaello.lock"]` or `read_paths = ["${home}/.netrc"]` (leaf hit on either project-class or credential-class carve-out) is **refused** with `CompileError::CarveOutRefused` unless `allow_credential_paths = true` (K2 fifth bullet — pi review-2 finding 7 dropped the earlier "silent drop with `dropped_carveouts`" wording). |
| `manifest_entry_traversal.rs` | `entry = "../evil"` rejected by `Manifest::parse` (M11 / `SafePath::parse`); `entry = "/abs/path"` rejected; `entry = "ok//double-slash"` rejected. |
| `manifest_entry_not_found.rs` | `entry = "bin/missing"` with no such file under `package_dir` → `ManifestError::EntryNotFound` from `validate_with_package` (M10). |
| `manifest_entry_escape_via_symlink.rs` | `entry = "bin/wat"` is a symlink whose target is outside `package_dir` after canonicalisation → `ManifestError::EntryEscape`. |
| `manifest_grant_match_traversal.rs` | `[provides.tool.foo] grant_match = "../schemas/x.json"` rejected by parse-time `SafePath` (M11). |
| `manifest_grant_match_missing.rs` | `grant_match = "schemas/missing.json"` with no such file → `ManifestError::GrantMatchNotFound` from `validate_with_package` (M10 / pi-2 finding 9). |
| `lock_canonical_id_path_traversal.rs` | `[plugin."../escape:foo@1.0.0"]` (`source` contains `..`); `[plugin."/abs:foo@1.0.0"]` (leading `/`); `[plugin."a//b:foo@1.0.0"]` (empty segment): all rejected by `CanonicalId::parse` (L8 — pi review-2 finding 1). |
| `compile_path_escape_after_expansion.rs` | Lock grants `read_dirs = ["${project}/../../etc"]` against a fixture project root; compile-time `${project}` expansion produces a path that escapes the substituted root → `CompileError::PathEscape` (C3). |
| `compile_invalid_allow_hosts.rs` | Lock with `network.mode = "proxy", allow_hosts = ["not a hostname"]` → `CompileError::InvalidAllowHosts` from the dry-run `outpost::NetworkPolicy::from_allowed_hosts` validation (Risks §2 / pi review-2 finding 6). |
| `compile_unknown_placeholder.rs` | A grant containing `${nope}` rejected as `CompileError::UnknownPlaceholder`. |
| `compile_reserved_env_in_pass.rs` | `env.pass = ["RFL_BUS_FD"]` → `CompileError::ReservedEnvVarRequested` (C7.1). |
| `compile_reserved_env_in_set.rs` | `env.set = { RFL_PLUGIN = "..." }` → `CompileError::ReservedEnvVarRequested` (C7.1). |
| `env_scrubber_strips_secret_globs.rs` | `env_pass = ["GITHUB_TOKEN", "OPENAI_API_KEY", "MY_PASSWORD", "AWS_PROFILE"]` are all stripped in the no-override path. |
| `env_scrubber_override.rs` | With `flags.i_know_what_im_doing = true`, the same `env_pass` survives the strip verbatim. |
| `sinks_inference_drift.rs` | A lock entry with `tool_meta.<n>.sinks_inferred = true` but a `sinks` snapshot that no longer matches the current grant inference → `ValidationError::SinkInferenceDrift` (Si2). |

### Manual validation in `manual-validation.md`

The driver runs and captures:

- `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green on Linux (CI rerun for macOS — m1 has
  no platform-specific code, so the macOS leg is delegated
  to CI per the m0 precedent). The repo has no root
  `Cargo.toml`, so `--manifest-path` is mandatory (pi
  review-4 finding 9).
- `cargo test --manifest-path fittings/Cargo.toml
  --workspace` green (pi review-5 finding 5 — the §W
  enum cutover is source-breaking for direct struct
  literals; the full fittings workspace suite must compile
  + pass to validate the blast radius, not just the new
  targeted regression test).
- `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` clean — the m1 surface is the
  future m2 consumer's API; doc warnings would surface drift
  between scope §C / §G / §V and the landed types.
- `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core --release` green (digest-module performance
  and carve-out worst-case behaviour).
- `nix develop --impure -L --command cargo test
  --manifest-path rafaello/Cargo.toml -p rafaello-core`
  green (per the m0 retrospective gotcha §4.6 — `--impure`
  is load-bearing for `nix develop` invocations).
- A `find rafaello/crates/rafaello-core/tests -type f -name '*.rs' | sort` listing capturing the integration test surface (m1 fixtures are inline / `tempfile`-based inside each test file; no separate `tests/fixtures/` directory in v1 — that pattern is reserved for if a test grows large enough to need an out-of-band tree).

## Risks

1. **Lockin's `command(...)` consumes the builder.** Resolved
   in C1: m1 emits a structured plan, not a pre-populated
   builder. m2 calls `command(entry)` (and applies env after).
   Tests assert on the plan value directly — no recording-ops
   shim, no upstream lockin patch. (Pi review-1 finding 1.)

2. **Outpost proxy startup is m2's job.** m1's `NetworkPlan::Proxy
   { allow_hosts: Vec<String> }` does not start the proxy; m2
   resolves the host list to a real `outpost::NetworkPolicy`,
   starts the proxy, and pairs the loopback port with the
   plan. The risk is that m1's `allow_hosts` is malformed in a
   way `outpost` later rejects. Mitigation: m1 has `outpost`
   as a **direct workspace dep** (S2) and calls
   `outpost::NetworkPolicy::from_allowed_hosts(...)` at compile
   time as a dry-run validation, returning
   `CompileError::InvalidAllowHosts` if the parse fails. The
   parsed `NetworkPolicy` value itself is discarded (m2
   recomputes it at spawn alongside the proxy startup). Pi
   review-3 finding 1 corrected the round-3 wording that
   claimed transitive access via `lockin-config`.

3. **Deterministic content digest across hosts.** The algorithm
   in §D1 commits to a specific normalisation. The test
   `digest_content_deterministic.rs` exercises a fixture
   constructed twice from different starting orders. If a CI
   host (e.g. macOS) produces a different hash, that's a real
   bug to fix in the walker, not a test allowance.

4. **Topic-id collision testing without a hash seam.** §T3
   exposes `topic_id::collisions_with_prefixes(pairs)` as a
   stable public API; production `collisions(plugins)`
   computes prefixes via `derive(...)` and delegates to it.
   Integration tests construct synthetic colliding-prefix
   fixtures and call the public helper directly. No
   `#[cfg(test)]` boundary, no `pub(crate)` re-export
   sleight-of-hand. Pi review-3 finding 6 caught the round-3
   wording that still said `pub(crate)`; this revision aligns
   Risks §4 with T3.

5. **Validation rule completeness vs. surface size.** The
   manifest validation surface is large (V1, V2, V3). The risk
   is that a real-world manifest the m2 driver brings in trips
   a rule m1 didn't think to write. Mitigation: every test in
   the negative matrix above is named after a real-world shape
   from security RFC §6 / manifest RFC §9; if the m2 driver
   brings in a manifest that trips an *unstated* rule, that's
   m1 retrospective territory (drift to add to the validator),
   not an m1 acceptance gap.

6. **`--allow-credential-paths` lock flag plumbing.** The
   security RFC's "loud override" surfaces in `rfl status`,
   which is m2+. m1 only rounds-trips the flag through the
   lock and respects it in the carve-out compiler. The
   override is set only by the m2+ install/grant flow, never
   by the runtime, so its silence in m1 has no security
   consequence.

7. **`MethodNotFound` typed-method-field cutover (row 36).**
   Resolved by pi review-2 finding 3: in scope, single small
   commit landing on `rafaello-v0.1`, captured under §W. No
   driver call needed at `commits.md` time.

8. **No external consumers yet.** `rafaello-core` is brand
   new. Per `decisions.md` row 33, it lives under
   `rafaello-v0.1`, so breakage risk to existing crates is
   bounded. The `rfl` binary's `main.rs` does not import
   `rafaello-core` in m1; the workspace only has to keep
   compiling.

## Internal split (driver guidance for `commits.md`)

Per `milestones/README.md`, m1 may split internally by module
group. Suggested grouping for `commits.md`; the driver picks
final granularity and surfaces an m1a/m1b split for owner
approval as soon as it becomes clear the groups cannot be kept
independently green.

1. **Crate skeleton + workspace-deps + manifest types and parser**
   (S1–S3, M1–M9): `rafaello-core` lib lands; manifest types
   compile; basic decoder tests including the reserved-field
   pre-scan. Self-contained; ~5–7 commits.
2. **Manifest validate-with-package + openrpc sibling** (M10):
   ~2 commits.
3. **Lock types + canonical id + bundle-aware grant + entry +
   tool_owner + round-trip** (L1–L9 — note L6 dropped per
   pi-4 finding 1):
   ~4–5 commits.
4. **Topic-id derivation + collision helper** (T1–T3):
   ~2 commits.
5. **Single-plugin validation** (V1, V2): ~3–4 commits.
6. **Digest module** (D1–D2 + the canonical-bytes wiring left
   from M9): ~2–3 commits.
7. **Sink default inference** (Si1): ~1–2 commits.
8. **Trifecta refusal** (Tr1–Tr5): ~2–3 commits.
9. **Carve-out decomposition + override + project-class /
   credential-class split** (K1–K4): ~3–4 commits.
10. **Env scrubber + reserved-env C7.1 protection**
    (Sc1–Sc3 + the env half of C7): ~2–3 commits.
11. **Cross-plugin lock validation** (V3 — wires up Tr +
    carveout + sink-drift + topic-id collision + tool-owner):
    ~2 commits.
12. **Compiler core: plan emission, placeholder substitution,
    bundle flatten, private state, limit defaults, network
    plan, env plan, digest gating** (C1–C7 minus the env half
    landed in step 10): ~5–7 commits.
13. **Broker ACL extraction** (G1–G3): ~2 commits.
14. **`MethodNotFound` typed-method-field fittings cutover**
    (W1–W5; row 36): one commit, ordering-independent — can
    land at any point in the m1 sequence (no `rafaello-core`
    dep). Default placement: late, after group 13, so the
    fittings change ships isolated from the new-crate work.
15. **`manual-validation.md`** (one commit at the end).

Realistic total: **~35–45 commits, sequential.** Pi review-2
finding 12 pushed for an upfront m1a/m1b split; the milestone
driver's `commits.md` instead encodes an **explicit go/no-go
checkpoint after group 7** (sink default inference / digests
landed): the driver stops, re-evaluates against the actual
landed sequence, and either continues with one milestone or
opens an m1a/m1b owner-ratification request. The natural split
boundary is exactly group 7 → group 8 (m1a: parsers + lock +
digests + topic-id + sink inference; m1b: validation +
carve-outs + trifecta + compiler + broker ACL + W). m1a has no
v1 consumer; m1b is m2's consumer surface. Default: ship one
milestone unless group 8+ cannot land green on group 1–7.

## Acceptance summary

m1 is done when:

- Every named test in the *Positive integration tests* and
  *Negative integration tests* matrices above is implemented
  and passes. Tests may split or merge during `commits.md`
  drafting as long as the named behaviours are all covered.
- `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` is green on Linux; CI rerun is the
  authoritative cross-platform signal for macOS.
- `cargo test --manifest-path fittings/Cargo.toml
  --workspace` is green (§W — full fittings workspace
  suite per pi-5 finding 5).
- `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` is warning-free.
- `manual-validation.md` records the items in the *Manual
  validation* section above.
- `retrospective.md` is written, with any drift surfaced
  during implementation landing in `overview.md` /
  `decisions.md` / stream RFCs as deltas. m1 retrospective
  specifically owns:
  - the Stream F drift items pinned in
    `milestones/README.md` §"Stream RFC drift" (the §15.1
    normative-delta items 1–4 plus the security RFC's
    `requires_confirmation` → `always_confirm` rename,
    helper / external-attach drift);
  - the **private-state path-key clarification** (pi
    review-4 finding 4): `overview.md` §5.5 /
    `decisions.md` row 16 / `glossary.md` "Per-plugin
    private state" all currently say `<plugin-id>` (which
    is ambiguous between canonical and topic-id forms).
    m1 retrospective patches all three to spell out
    "topic-id" as the path segment, matching what m1
    landed in C5.

## What changed from prior drafts

Round-6 pi review (`pi-review-6.md`) prompted these revisions:

- **`validate::lock` takes a multi-plugin context** (pi-6
  finding 1). New `LockValidationContext` with
  `plugin_dirs: BTreeMap<CanonicalId, PathBuf>`; V3 derives
  per-plugin `PathContext` from it. New positive test
  `validate_lock_multiplugin_context.rs`.
- **`bindings.load` snapshot in the lock + `CompiledPlugin.load`**
  (pi-6 finding 2). L4 adds `load: LoadPolicy`; C1 carries
  it. The spawn path (m2/m3) reads the snapshot, not the
  live manifest, per security RFC §3.2. New positive tests
  `lock_load_policy_round_trip.rs`,
  `lock_load_policy_eager_string.rs`.
- **More lock-side mirrors** (pi-6 finding 3). V3 adds:
  unknown grant bundle key check; capability path template
  re-validation on lock load; tool-name grammar check on
  `bindings.tools` and `[session].tool_owner` keys;
  sink-class grammar check on `bindings.tool_meta.<n>.sinks`.
  Four new negative tests.
- **`exec_paths` / `exec_dirs` inside `${project}` refused**
  per security RFC §6.9 (pi-6 finding 4). V1 + V3 mirror
  rules; no override. Two new negative tests:
  `manifest_exec_path_inside_project.rs`,
  `lock_exec_path_inside_project.rs`.
- **Tool-table presence aligned with overview defaults**
  (pi-6 finding 5). Missing `[provides.tool.<name>]` tables
  use the §15.1 defaults rather than failing validation.
  Round-6 wording reversed; new positive test
  `tool_table_omitted_uses_defaults.rs`; existing
  `manifest_unknown_tool_table.rs` description updated.
- **`content_digest` symlinked-directory semantics pinned**
  (pi-6 finding 6). Recursion-stack cycle detection (not
  global visited-set), so distinct logical paths sharing a
  canonical target both contribute to the hash. New
  positive test `digest_distinct_paths_same_target.rs`.
- **`compile_plugin` / `broker_acl::compile` precondition
  contract documented** (pi-6 medium 7). New C1.1 spelling
  out the V3-must-run-first invariant + the
  `CompileError::ValidationNotRun` failure mode. New
  negative test `compile_without_validate_lock_errors.rs`.

Round-5 pi review (`pi-review-5.md`) prompted these revisions:

- **Lock-side publish authority validation added** (pi-5
  finding 1). V3 mirrors V1/V2's namespace ACL on the lock's
  `.grant.publishes`. Four new negative tests:
  `lock_publishes_core_topic.rs`,
  `lock_publishes_frontend_topic.rs`,
  `lock_publishes_other_plugin_namespace.rs`,
  `lock_provider_namespace_mismatch.rs`.
- **`tool_owner` target integrity** (pi-5 finding 2). V3
  validates every `[session].tool_owner.<n>` entry: target
  installed, target's `bindings.tools` contains `<n>`, and
  no redundant entries (no actual conflict). Three new
  negative tests: `lock_tool_owner_unknown_plugin.rs`,
  `lock_tool_owner_plugin_does_not_declare_tool.rs`,
  `lock_tool_owner_redundant.rs`.
- **Lock-side `allow_hosts` mode rule** (pi-5 finding 3).
  V3 mirrors V1's rule on lock grant bundles. New negative
  test `lock_allow_hosts_outside_proxy.rs`.
- **`lockin::config::*` → `lockin_config::*`** (pi-5
  finding 4). The current crate is `lockin-config` (separate
  package), imported as `lockin_config`. Search/replace
  across scope.md.
- **W command broadened to fittings workspace suite** (pi-5
  finding 5). Goal / Manual validation / Acceptance summary
  now require `cargo test --manifest-path fittings/Cargo.toml
  --workspace`, since `MethodNotFound`'s typed-field cutover
  is source-breaking and the targeted regression test
  doesn't compile every consumer.
- **Lock-side binding snapshot validation expanded** (pi-5
  finding 6). V3 re-validates `bindings.tool_meta.<n>.grant_match`
  through `SafePath`, requires `tool_meta` keys to appear in
  `tools`, asserts `provider_id` consistency with `provider`,
  re-validates `renderer_kinds` against M7 grammar. Five new
  negative tests covering each rule.
- **Capability path resolver pinned for non-existent leaves**
  (pi-5 finding 7). C3 specifies the
  existing-ancestor-canonicalise-then-lexical-suffix rule.
  Two new tests: `compile_capability_path_nonexistent_write_leaf.rs`
  (positive), `compile_capability_path_symlink_ancestor_escape.rs`
  (negative).
- **`lockin` dep dropped from `rafaello-core`** (pi-5 medium
  8). m1's plan types are m1-owned; the public API never
  exposes `SandboxBuilder`. m2 imports `lockin` separately.
- **Inputs paths normalised to repo-relative** (pi-5 medium
  9). `/home/luiz/lab/lockin/...` → `lockin/...`.
- **Topic minimum-segment rule pinned to security RFC §5.1**
  (pi-5 medium 10). V1 adds `TopicTooFewSegments`. New
  negative test `manifest_topic_too_few_segments.rs`.
- **`MethodNotFound` data conflict semantics pinned** (pi-5
  medium 11). W2 specifies typed-field-precedence on encode
  and decode rules.

Round-4 pi review (`pi-review-4.md`) prompted these revisions:

- **`.active_bundles` removed; full bundle union per row 17**
  (pi-4 finding 1). The compiler unions every named bundle in
  `grant.bundles` with `default` into one spawn-time policy;
  there is no per-call sandbox switch in v1. L6 marked
  removed; C2 / V3 / Tr / K wording updated;
  `lock_active_bundle_unknown.rs` test removed;
  `compile_scoped_bundle_union.rs` rewritten to put per-tool
  authority into a named bundle and assert the union.
- **Two distinct path vocabularies** (pi-4 finding 2). M11
  splits into `SafePath` (relative-package: `entry`,
  `grant_match`) and `CapabilityPathTemplate`
  (placeholder-or-absolute: `read_paths` etc.). C3's
  post-expansion escape check stays on the capability side
  so `compile_path_escape_after_expansion.rs` is reachable.
  All `${PROJECT_ROOT}` / `${HOME}` references in the doc
  normalised to `${project}` / `${home}` per M8.
- **W cutover acceptance commands explicit** (pi-4 finding 3).
  Goal mentions both deliverables; Manual validation +
  Acceptance summary include the
  `cargo test --manifest-path fittings/Cargo.toml --test
  method_not_found_typed_method_round_trip` command. All
  `cargo test -p rafaello-core` invocations spell
  `--manifest-path rafaello/Cargo.toml` (pi-4 medium 9).
- **Private-state architectural drift flagged for m1
  retrospective** (pi-4 finding 4). m1 uses the topic-id
  form per C5; `overview.md` §5.5 / `decisions.md` row 16 /
  `glossary.md` "Per-plugin private state" still say
  `<plugin-id>` (ambiguous). m1 retrospective patches the
  three docs to say "topic-id" explicitly. Captured in the
  Acceptance summary's retrospective bullet below.
- **`milestones/README.md` m1 row patched** (pi-4 finding 5)
  to say "produces a structured `CompiledPlugin` plan
  consumed by m2", not "builder calls", and to include the
  fittings W cutover.
- **`always_confirm` round-trip test added** (pi-4 finding 6).
  New positive test `tool_meta_always_confirm_round_trip.rs`.
- **Renderer kind grammar pinned to Stream E §8 prefix rule**
  (pi-4 finding 7). M7 rejects unprefixed plugin kinds.
  Worked-example fixture updated `code.diff` → `diff:code`.
  New negative test `manifest_unprefixed_renderer_kind.rs`.
- **`allow_hosts` requires proxy mode** (pi-4 medium 8). V1
  rejects non-empty `allow_hosts` for `deny` / `allow_all`
  modes. New negative test
  `manifest_allow_hosts_outside_proxy.rs`.

Round-3 pi review (`pi-review-3.md`) prompted these revisions:

- **Workspace dep paths corrected and `NetworkPlan` ownership
  pinned** (pi-3 finding 1). S2 now uses
  `../lockin/crates/sandbox`, `../outpost/crates/outpost`
  (relative to `rafaello/Cargo.toml`); `lockin-config` dep
  removed (`lockin_config::NetworkPlan` is a different shape
  from m1's `NetworkPlan`); `outpost` is a direct workspace
  dep used only for the dry-run `from_allowed_hosts`
  validation. Risks §2 rewritten to match.
- **Lock-side `.entry` is escape-safe** (pi-3 finding 2). L2
  parses `.entry` through M11's `SafePath` rule; the compiler
  re-validates the snapshot at compile time
  (`EntryEscape`/`EntryNotFound`/`EntryNotFile`). New negative
  tests `lock_entry_traversal.rs`, `lock_entry_not_found.rs`,
  `lock_entry_escape_via_symlink.rs`, `lock_entry_is_directory.rs`.
- **Sink inference fixed** (pi-3 finding 3). Si1 takes the
  effective per-tool grant (`default ∪ <tool-name>` per row
  17), not just one bundle. Si2's drift check runs
  unconditionally for `sinks_inferred = true` entries
  (regardless of snapshot list emptiness). New positive test
  `sinks_infer_from_named_bundle.rs`.
- **`openrpc.json` sibling required for every plugin** (pi-3
  finding 4). M10's "if `provides.tools` non-empty" qualifier
  dropped to match `decisions.md` row 31's unqualified text.
  New negative test `manifest_missing_openrpc_provider.rs`.
- **`compile_private_state_grant.rs` test description fixed
  to `<topic-id>`** (pi-3 finding 5). Stale `<canonical-id>`
  wording removed.
- **Risks §4 rewritten to match T3** (pi-3 finding 6).
  `pub(crate)` / re-export sleight-of-hand replaced with
  "stable public API".
- **`content_digest` cycle handling specified** (pi-3 finding
  7). D1 adds canonical-target visited-set cycle detection;
  new `DigestError::SymlinkCycle`. New negative test
  `digest_symlink_cycle.rs`.
- **Manifest `name` grammar pinned** (pi-3 finding 8). M1
  validates `name` against the topic-segment grammar at parse
  time. New negative test `manifest_invalid_name.rs`.
- **`load.event` cross-validation uses subscribe-pattern
  matching** (pi-3 finding 9). V1 spelled out: at least one
  `bus.subscribes` pattern must match the trigger topic. New
  positive test `manifest_load_event_pattern_match.rs`.

Round-2 pi review (`pi-review-2.md`) prompted these revisions:

- **Path-escape safety pinned across canonical id, manifest
  paths, and post-expansion grants** (pi-2 finding 1). New M11
  defines `manifest::SafePath::parse` rejecting `..`, leading
  `/`, empty segments, control chars, and `\`. L8's
  `CanonicalId::source` grammar tightened to forbid `..`,
  empty segments, leading/trailing/double `/`. C5 switched
  the per-plugin private-state dir to use the **topic-id**
  form rather than the raw canonical id. C3 added a
  post-expansion path-escape check for `${project}` /
  `${plugin}` placeholders. M10 added `entry` /
  `grant_match` resolution + escape rules. New negative
  tests `manifest_entry_traversal.rs`,
  `manifest_entry_not_found.rs`,
  `manifest_entry_escape_via_symlink.rs`,
  `manifest_grant_match_traversal.rs`,
  `manifest_grant_match_missing.rs`,
  `lock_canonical_id_path_traversal.rs`,
  `compile_path_escape_after_expansion.rs`.
- **Sink-default state made consistent across M / L / C / Si**
  (pi-2 finding 2). Manifest's `[provides.tool.<n>] sinks`
  is `Option<Vec<String>>` (M3), with `None` ≠ `Some(vec![])`.
  Lock's `ToolMeta` gains `sinks_inferred: bool` (L4). New
  positive test `sinks_inferred_flag_round_trips.rs`.
- **`MethodNotFound` typed-method cutover decided to "in
  scope"** (pi-2 finding 3). New W section (W1–W5);
  out-of-scope bullet replaced with the in-scope pointer;
  Risks §7 updated; internal split group 14 updated.
- **`tool_owner` resolution surfaces in compiled outputs**
  (pi-2 finding 4). G1's `BrokerAcl` adds `tool_routes:
  BTreeMap<String, CanonicalId>`. C1's `tool_meta` filters
  out conflicting tools the lock resolved away from this
  plugin. New positive test
  `broker_acl_tool_owner_resolves_routing.rs`.
- **Reserved-env-var rule unified to "reject" in both
  `env.pass` and `env.set`** (pi-2 finding 5). C7.1 rewritten;
  the round-2 "strip" wording for `env.set` dropped. The
  positive `compile_env_set_passes_through.rs` row updated to
  drop the contradictory "stripped by C7.1 with a typed error"
  language.
- **`outpost` direct dep added to the workspace deps table**
  (pi-2 finding 6) so the dry-run `from_allowed_hosts`
  validation in Risks §2 actually compiles. New positive test
  `compile_network_proxy_allow_hosts_validates.rs`; new
  negative test `compile_invalid_allow_hosts.rs`.
- **Carve-out diagnostic field dropped; explicit-leaf hits
  refused** (pi-2 finding 7). K2 fifth bullet rewritten to
  refuse explicit leaf hits with `CarveOutRefused`; the
  earlier `dropped_carveouts` field is gone. The
  `carveout_lockfile_path_explicit.rs` row updated.
- **Topic-id helper visibility resolved as a public testable
  API** (pi-2 finding 8). T3's `collisions_with_prefixes` is
  now an intentional public function. The `pub(crate)` /
  re-export wording dropped from Risks §4.
- **`grant_match` sibling existence test added** (pi-2 finding
  9). M10 covers it; new positive test
  `manifest_grant_match_present.rs`.
- **Renderer manifest acceptance is explicitly inert in v1**
  (pi-2 finding 10). M7 wording extended to spell out that
  m3's renderer router ignores plugin renderer registrations
  and that row 29 governs.
- **`content_digest` algorithm pinned to files-only** (pi-2
  finding 11). D1 wording unified; symlink-to-directory
  behaviour spelled out.
- **m1a/m1b checkpoint encoded explicitly** (pi-2 finding 12).
  Internal-split text after the numbered list rewritten to
  describe the after-group-7 go/no-go gate.
- **Round-1 review file landed on `rafaello-v0.1`** (pi-2
  finding 13). `pi-review-1.md` was on `agents/m1/pi-scope-1`;
  cherry-picked to the integration branch alongside this
  revision so future readers can find it.

Round-1 pi review (`pi-review-1.md`) prompted these revisions:

- **Compile output is a structured plan, not a pre-populated
  `SandboxBuilder`** (pi finding 1). C1 redefines the public
  output as `CompiledPlugin { filesystem, network, env, limits,
  ... }`. m2 owns `command(...)`, env application, and outpost
  proxy startup. `compile_default_bundle.rs` and the other
  compile tests assert on the plan value directly.
- **Lock schema expanded** (pi finding 2):
  - L2 adds `entry: PathBuf` (compiler reads this for
    `entry_absolute`).
  - L3 reshapes `.grant` as `BTreeMap<BundleKey, GrantBundle>`
    so scoped bundles round-trip.
  - L6 (round-2 only) added `active_bundles: Vec<String>`
    to drive C2; **dropped in round 5** per pi review-4
    finding 1 — see the round-5 changelog.
  - L1 adds `[session].tool_owner: BTreeMap<String, String>`
    for V3's conflicting-tool-name resolution.
  - C2 takes `RecomputedDigests` (D3) explicitly; no
    out-of-band digest input.
- **Tool / method / bundle naming pinned** (pi finding 3).
  Tool names are single segments (`[a-z0-9_][a-z0-9_-]*`);
  bundle keys are `default` or a tool name. Worked-example
  fixtures use `format` not `rust.format`. New negative tests
  `manifest_dotted_tool_name.rs` and `manifest_unknown_bundle_key.rs`.
- **APIs name every context dependency** (pi finding 4).
  `trifecta::evaluate(lock, canonical, ctx)` takes
  `PathContext`; `validate::manifest_with_id(manifest,
  canonical)` is the explicit lock-bound validator;
  `validate::lock(lock, ctx)` documents the multi-plugin
  surface.
- **Carve-out rule pinned to a single policy** (pi finding 5).
  Project-class: read decomposes, write refuses. Credential-
  class: both refuse. Both classes overridable by
  `allow_credential_paths`. New negative tests
  `carveout_project_write_refused.rs` and the matched read /
  write split for credential-class.
- **Workspace dependency policy realistic** (pi finding 6).
  S2 introduces a `[workspace.dependencies]` table and lists
  every required crate (`semver`, `chrono`, `tempfile`,
  `lockin-config`, etc.). Earlier "no new deps" wording dropped.
- **Worked-example fixtures called out as post-simplification
  rewrites** (pi finding 7). Note added above the test matrix.
  Env scrubber positive row corrected: `AWS_REGION` is
  stripped (matches `AWS_*`); only `PATH` survives.
- **Reserved-field parsing strategy explicit** (pi finding 8).
  M2 specifies a TOML pre-scan ahead of serde so
  `runtime`/`[rpc]`/`helper_for` produce typed
  `ReservedField` errors with deferral hints rather than
  generic `UnknownField`.
- **Sink-default inference is in scope** (pi finding 9). New
  module `sinks` (Si1, Si2) inferring sink lists per security
  RFC §7.2.5 and detecting drift on hand-edited locks.
  `sinks_infer_defaults.rs` and `sinks_inference_drift.rs`
  added to the test matrix.
- **Reserved-env-var protection split into two stages** (pi
  finding 10). Compiler-stage protection (C7.1) refuses
  `RFL_BUS_FD` / `RFL_PLUGIN` in `env.pass` / `env.set`;
  scrubber (Sc) operates after that and never sees reserved
  vars. Round-1's "reserved vars survive the scrubber" test
  removed; replaced by `compile_reserved_env_in_pass.rs` and
  `compile_reserved_env_in_set.rs`.
- **Topic-id test seam removed** (pi finding 7's spirit,
  Risks §4). The `collisions_with_prefixes` helper is the
  stable internal surface; both production `collisions` and
  the integration tests call into it. No `#[cfg(test)]` /
  `feature = "test-seam"` boundary.
- **Internal split renumbered** to reflect the new module
  layout and the additional sinks / reserved-env / network-plan
  work; total estimate widened to 35–45 commits.
